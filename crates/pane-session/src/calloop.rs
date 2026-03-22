//! Calloop integration for session-typed channels.
//!
//! Provides `SessionSource` — a calloop `EventSource` that fires when
//! a unix socket has a complete length-prefixed message ready to read.

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

use calloop::{EventSource, Poll, PostAction, Readiness, Token, TokenFactory};
use calloop::generic::Generic;
use calloop::Interest;

/// A calloop event source for a unix socket carrying length-prefixed messages.
///
/// Fires with `SessionEvent::Message(bytes)` when a complete message arrives,
/// or `SessionEvent::Disconnected` when the peer closes the connection.
pub struct SessionSource {
    readiness: Generic<UnixStream>,
    /// Cloned stream for reading (the Generic owns one copy for fd polling,
    /// we keep another for actual I/O).
    reader: UnixStream,
}

/// Events produced by a SessionSource.
#[derive(Debug)]
pub enum SessionEvent {
    /// A complete message was received (raw bytes, postcard-encoded).
    Message(Vec<u8>),
    /// The peer disconnected.
    Disconnected,
}

impl SessionSource {
    /// Create a session source from a connected unix stream.
    pub fn new(stream: UnixStream) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        let reader = stream.try_clone()?;
        let readiness = Generic::new(stream, Interest::READ, calloop::Mode::Level);
        Ok(SessionSource { readiness, reader })
    }

    /// Get a reference to the underlying stream for sending responses.
    /// The caller is responsible for length-prefixed framing.
    pub fn stream(&self) -> &UnixStream {
        &self.reader
    }
}

/// Send a length-prefixed postcard message on a unix stream.
pub fn write_message(stream: &UnixStream, data: &[u8]) -> io::Result<()> {
    // UnixStream implements Write via &UnixStream too
    let mut s = stream;
    let len = (data.len() as u32).to_le_bytes();
    s.write_all(&len)?;
    s.write_all(data)?;
    s.flush()
}

impl EventSource for SessionSource {
    type Event = SessionEvent;
    type Metadata = ();
    type Ret = io::Result<PostAction>;
    type Error = io::Error;

    fn process_events<C>(
        &mut self,
        readiness: Readiness,
        token: Token,
        mut callback: C,
    ) -> Result<PostAction, Self::Error>
    where
        C: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        self.readiness.process_events(readiness, token, |_, _| {
            // Temporarily blocking for the read
            self.reader.set_nonblocking(false)?;
            let result = read_length_prefixed(&mut self.reader);
            self.reader.set_nonblocking(true)?;

            match result {
                Ok(bytes) => callback(SessionEvent::Message(bytes), &mut ()),
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof
                    || e.kind() == io::ErrorKind::ConnectionReset
                    || e.kind() == io::ErrorKind::BrokenPipe => {
                    let _ = callback(SessionEvent::Disconnected, &mut ());
                    Ok(PostAction::Remove)
                }
                Err(e) => Err(e),
            }
        })
    }

    fn register(&mut self, poll: &mut Poll, token_factory: &mut TokenFactory) -> calloop::Result<()> {
        self.readiness.register(poll, token_factory)
    }

    fn reregister(&mut self, poll: &mut Poll, token_factory: &mut TokenFactory) -> calloop::Result<()> {
        self.readiness.reregister(poll, token_factory)
    }

    fn unregister(&mut self, poll: &mut Poll) -> calloop::Result<()> {
        self.readiness.unregister(poll)
    }
}

fn read_length_prefixed(stream: &mut UnixStream) -> io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf)?;
    Ok(buf)
}
