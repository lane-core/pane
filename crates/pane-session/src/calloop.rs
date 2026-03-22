//! Calloop integration for session-typed channels.
//!
//! Provides `SessionSource` — a calloop `EventSource` that fires when
//! a unix socket has a complete length-prefixed message ready to read.

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

use calloop::{EventSource, Poll, PostAction, Readiness, Token, TokenFactory};
use calloop::generic::Generic;
use calloop::Interest;

use crate::transport::unix::MAX_MESSAGE_SIZE;

/// Events produced by a SessionSource.
#[derive(Debug)]
pub enum SessionEvent {
    /// A complete message was received (raw bytes, postcard-encoded).
    Message(Vec<u8>),
    /// The peer disconnected.
    Disconnected,
}

/// A calloop event source for a unix socket carrying length-prefixed messages.
///
/// Accumulates partial reads in an internal buffer. The socket stays
/// non-blocking at all times — no toggling that could affect other fds
/// sharing the same file description.
pub struct SessionSource {
    /// For fd readiness notification only.
    source: Generic<UnixStream>,
    /// Separate clone for actual I/O — avoids NoIoDrop mutability issues.
    reader: UnixStream,
    /// Accumulation buffer for partial reads.
    buf: Vec<u8>,
    /// State machine: reading_len → reading_body.
    state: ReadState,
}

#[derive(Clone, Copy)]
enum ReadState {
    /// Waiting for the 4-byte length prefix.
    ReadingLen,
    /// Have the length, waiting for the body.
    ReadingBody(usize),
}

impl SessionSource {
    /// Create a session source from a connected unix stream.
    pub fn new(stream: UnixStream) -> io::Result<Self> {
        stream.set_nonblocking(true)?;
        let reader = stream.try_clone()?;
        let source = Generic::new(stream, Interest::READ, calloop::Mode::Level);
        Ok(SessionSource {
            source,
            reader,
            buf: Vec::new(),
            state: ReadState::ReadingLen,
        })
    }

    /// Get a reference to the underlying stream for sending responses.
    pub fn stream(&self) -> &UnixStream {
        &self.reader
    }
}

/// Send a length-prefixed message on a unix stream.
pub fn write_message(stream: &UnixStream, data: &[u8]) -> io::Result<()> {
    let mut s = stream;
    let len = (data.len() as u32).to_le_bytes();
    s.write_all(&len)?;
    s.write_all(data)?;
    s.flush()
}

/// Try to read available bytes into the buffer, returning how many were read.
/// Returns Ok(0) on EOF (peer disconnected).
fn try_fill(stream: &UnixStream, buf: &mut Vec<u8>, need: usize) -> io::Result<bool> {
    let have = buf.len();
    if have >= need {
        return Ok(true); // already have enough
    }
    let remaining = need - have;
    let mut tmp = vec![0u8; remaining];
    let stream_ref: &UnixStream = stream;
    match Read::read(&mut &*stream_ref, &mut tmp) {
        Ok(0) => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "peer disconnected")),
        Ok(n) => {
            buf.extend_from_slice(&tmp[..n]);
            Ok(buf.len() >= need)
        }
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(false),
        Err(e) => Err(e),
    }
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
        // We read from our own reader, not from calloop's wrapped stream.
        // Calloop's Generic is used only for fd readiness notification.
        self.source.process_events(readiness, token, |_, _| {
            loop {
                match self.state {
                    ReadState::ReadingLen => {
                        match try_fill(&self.reader, &mut self.buf, 4) {
                            Ok(true) => {
                                let len = u32::from_le_bytes([
                                    self.buf[0], self.buf[1], self.buf[2], self.buf[3],
                                ]) as usize;
                                if len > MAX_MESSAGE_SIZE {
                                    return Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        format!("message size {} exceeds max {}", len, MAX_MESSAGE_SIZE),
                                    ));
                                }
                                self.buf.clear();
                                self.state = ReadState::ReadingBody(len);
                            }
                            Ok(false) => return Ok(PostAction::Continue),
                            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                                let _ = callback(SessionEvent::Disconnected, &mut ());
                                return Ok(PostAction::Remove);
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    ReadState::ReadingBody(len) => {
                        match try_fill(&self.reader, &mut self.buf, len) {
                            Ok(true) => {
                                let msg = std::mem::take(&mut self.buf);
                                self.state = ReadState::ReadingLen;
                                match callback(SessionEvent::Message(msg), &mut ())? {
                                    PostAction::Remove => return Ok(PostAction::Remove),
                                    PostAction::Disable => return Ok(PostAction::Disable),
                                    _ => continue,
                                }
                            }
                            Ok(false) => return Ok(PostAction::Continue),
                            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                                let _ = callback(SessionEvent::Disconnected, &mut ());
                                return Ok(PostAction::Remove);
                            }
                            Err(e) => return Err(e),
                        }
                    }
                }
            }
        })
    }

    fn register(&mut self, poll: &mut Poll, token_factory: &mut TokenFactory) -> calloop::Result<()> {
        self.source.register(poll, token_factory)
    }

    fn reregister(&mut self, poll: &mut Poll, token_factory: &mut TokenFactory) -> calloop::Result<()> {
        self.source.reregister(poll, token_factory)
    }

    fn unregister(&mut self, poll: &mut Poll) -> calloop::Result<()> {
        self.source.unregister(poll)
    }
}
