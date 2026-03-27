//! Unix domain socket transport.
//!
//! Length-prefixed postcard messages over unix stream sockets.
//! This is the production transport for pane — all inter-process
//! session-typed communication uses this.

use std::io;
use std::os::unix::net::{UnixListener, UnixStream};

use crate::error::SessionError;
use crate::framing;
use crate::dual::HasDual;
use crate::types::Chan;
use crate::transport::Transport;

/// Re-exported from framing module for backward compatibility.
pub const MAX_MESSAGE_SIZE: usize = framing::MAX_MESSAGE_SIZE;

/// Unix domain socket transport with length-prefixed framing.
pub struct UnixTransport {
    stream: UnixStream,
}

impl UnixTransport {
    /// Extract the underlying stream for phase transitions.
    /// After a session-typed handshake reaches `End`, call `chan.finish()`
    /// to get the transport, then `transport.into_stream()` to get the
    /// raw stream for calloop registration or active-phase messaging.
    pub fn into_stream(self) -> UnixStream {
        self.stream
    }

    /// Wrap an existing unix stream as a session transport.
    pub fn from_stream(stream: UnixStream) -> Self {
        UnixTransport { stream }
    }
}

impl Transport for UnixTransport {
    fn send_raw(&mut self, data: &[u8]) -> Result<(), SessionError> {
        framing::write_framed(&mut self.stream, data)?;
        Ok(())
    }

    fn recv_raw(&mut self) -> Result<Vec<u8>, SessionError> {
        Ok(framing::read_framed(&mut self.stream)?)
    }
}

/// Create a connected pair of session-typed unix socket channels.
/// Returns `(client, server)` where client has session type `S`
/// and server has session type `Dual<S>`.
///
/// Uses `UnixStream::pair()` — both ends in the same process.
/// Useful for testing and for in-process sub-session creation.
pub fn unix_pair<S: HasDual>() -> io::Result<(
    Chan<S, UnixTransport>,
    Chan<S::Dual, UnixTransport>,
)> {
    let (a, b) = UnixStream::pair()?;
    Ok((
        Chan::new(UnixTransport::from_stream(a)),
        Chan::new(UnixTransport::from_stream(b)),
    ))
}

/// Accept a connection from a unix listener and wrap it as a
/// session-typed channel with the given session type.
pub fn accept_session<S>(listener: &UnixListener) -> io::Result<Chan<S, UnixTransport>> {
    let (stream, _addr) = listener.accept()?;
    Ok(Chan::new(UnixTransport::from_stream(stream)))
}

/// Connect to a unix socket and wrap the connection as a
/// session-typed channel with the given session type.
pub fn connect_session<S>(path: impl AsRef<std::path::Path>) -> io::Result<Chan<S, UnixTransport>> {
    let stream = UnixStream::connect(path)?;
    Ok(Chan::new(UnixTransport::from_stream(stream)))
}
