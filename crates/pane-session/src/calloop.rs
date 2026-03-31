//! Calloop integration for session-typed channels.
//!
//! Provides `SessionSource` — a calloop `EventSource` that fires when
//! a stream has a complete length-prefixed message ready to read.
//! Parameterized over stream type: works with `UnixStream`, `TcpStream`,
//! or any type implementing `AsFd + Read`.

use std::io::{self, Read, Write};
use std::os::unix::io::AsFd;
use std::os::unix::net::UnixStream;

use calloop::{EventSource, Poll, PostAction, Readiness, Token, TokenFactory};
use calloop::generic::Generic;
use calloop::Interest;

use crate::framing::MAX_MESSAGE_SIZE;

/// Events produced by a SessionSource.
#[derive(Debug)]
pub enum SessionEvent {
    /// A complete message was received (raw bytes, postcard-encoded).
    Message(Vec<u8>),
    /// The peer disconnected.
    Disconnected,
}

/// A calloop event source for a stream carrying length-prefixed messages.
///
/// Accumulates partial reads in an internal buffer. The stream stays
/// non-blocking at all times — no toggling that could affect other fds
/// sharing the same file description.
///
/// Parameterized over stream type `S`: works with `UnixStream`,
/// `TcpStream`, or any `AsFd + Read + 'static`.
pub struct SessionSource<S: AsFd + Read + 'static> {
    /// For fd readiness notification only.
    source: Generic<S>,
    /// Separate stream for actual I/O — avoids NoIoDrop mutability issues.
    reader: S,
    /// Accumulation buffer for partial reads.
    buf: Vec<u8>,
    /// State machine: reading_len → reading_body.
    state: ReadState,
}

/// Type alias for backward compatibility — most calloop code uses unix streams.
pub type UnixSessionSource = SessionSource<UnixStream>;

#[derive(Clone, Copy)]
enum ReadState {
    /// Waiting for the 4-byte length prefix.
    ReadingLen,
    /// Have the length, waiting for the body.
    ReadingBody(usize),
}

impl SessionSource<UnixStream> {
    /// Create a session source from a connected unix stream.
    ///
    /// Convenience constructor that clones the stream internally.
    /// For other stream types, use [`SessionSource::from_streams`].
    pub fn new(stream: UnixStream) -> io::Result<Self> {
        let reader = stream.try_clone()?;
        Self::from_streams(stream, reader)
    }
}

impl<S: AsFd + Read + 'static> SessionSource<S> {
    /// Create a session source from two streams: one for calloop fd
    /// registration, one for reading.
    ///
    /// The caller is responsible for providing two handles to the same
    /// underlying connection (e.g., via `try_clone()` for unix/TCP streams).
    /// The `poll_stream` is registered with calloop for readiness
    /// notification; the `read_stream` is used for actual I/O.
    pub fn from_streams(poll_stream: S, read_stream: S) -> io::Result<Self> {
        let source = Generic::new(poll_stream, Interest::READ, calloop::Mode::Level);
        Ok(SessionSource {
            source,
            reader: read_stream,
            buf: Vec::new(),
            state: ReadState::ReadingLen,
        })
    }

    /// Get a reference to the reader stream.
    pub fn stream(&self) -> &S {
        &self.reader
    }
}

/// Send a length-prefixed message on a stream.
///
/// Works with `&UnixStream`, `&TcpStream`, or any type where
/// `&T` implements `Write` (the standard library convention for
/// shared-reference I/O on sockets).
pub fn write_message<T>(stream: &T, data: &[u8]) -> io::Result<()>
where
    for<'a> &'a T: Write,
{
    crate::framing::write_framed(&mut &*stream, data)
}

/// Try to read available bytes into the buffer, returning how many were read.
/// Returns Ok(0) on EOF (peer disconnected).
fn try_fill<S: Read>(stream: &mut S, buf: &mut Vec<u8>, need: usize) -> io::Result<bool> {
    let have = buf.len();
    if have >= need {
        return Ok(true); // already have enough
    }
    let remaining = need - have;
    let mut tmp = vec![0u8; remaining];
    match stream.read(&mut tmp) {
        Ok(0) => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "peer disconnected")),
        Ok(n) => {
            buf.extend_from_slice(&tmp[..n]);
            Ok(buf.len() >= need)
        }
        Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(false),
        Err(e) => Err(e),
    }
}

impl<S: AsFd + Read + 'static> EventSource for SessionSource<S> {
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
                        match try_fill(&mut self.reader, &mut self.buf, 4) {
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
                        match try_fill(&mut self.reader, &mut self.buf, len) {
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
