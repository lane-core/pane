//! TCP transport for network-transparent sessions.
//!
//! Length-prefixed postcard messages over TCP streams.
//! This is the network transport for pane — all cross-machine
//! session-typed communication uses this. For local IPC, use
//! [`unix::UnixTransport`](super::unix::UnixTransport).
//!
//! The TCP transport implements the same `Transport` trait as
//! the unix socket transport. From the session type's perspective,
//! the two are interchangeable — the protocol logic is identical.
//! TLS wrapping (when needed) happens below this layer.
//!
//! # Plan 9
//!
//! 9P ran over TCP, IL, and pipes — the protocol was transport-
//! independent. Pane follows the same principle: `Chan<S, T>` is
//! parameterized over transport, and adding TCP requires no
//! protocol changes.

use std::io;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};

use crate::dual::HasDual;
use crate::error::SessionError;
use crate::framing;
use crate::transport::Transport;
use crate::types::Chan;

/// TCP transport with length-prefixed framing.
pub struct TcpTransport {
    stream: TcpStream,
}

impl TcpTransport {
    /// Extract the underlying stream for phase transitions.
    /// After a session-typed handshake reaches `End`, call `chan.finish()`
    /// to get the transport, then `transport.into_stream()` to get the
    /// raw stream for calloop registration or active-phase messaging.
    pub fn into_stream(self) -> TcpStream {
        self.stream
    }

    /// Wrap an existing TCP stream as a session transport.
    pub fn from_stream(stream: TcpStream) -> Self {
        TcpTransport { stream }
    }
}

impl Transport for TcpTransport {
    fn send_raw(&mut self, data: &[u8]) -> Result<(), SessionError> {
        framing::write_framed(&mut self.stream, data)?;
        Ok(())
    }

    fn recv_raw(&mut self) -> Result<Vec<u8>, SessionError> {
        Ok(framing::read_framed(&mut self.stream)?)
    }
}

/// Accept a connection from a TCP listener and wrap it as a
/// session-typed channel with the given session type.
pub fn accept_session<S>(listener: &TcpListener) -> io::Result<Chan<S, TcpTransport>> {
    let (stream, _addr) = listener.accept()?;
    Ok(Chan::new(TcpTransport::from_stream(stream)))
}

/// Connect to a TCP address and wrap the connection as a
/// session-typed channel with the given session type.
pub fn connect_session<S>(addr: impl ToSocketAddrs) -> io::Result<Chan<S, TcpTransport>> {
    let stream = TcpStream::connect(addr)?;
    Ok(Chan::new(TcpTransport::from_stream(stream)))
}

/// Create a connected pair of session-typed TCP channels over localhost.
/// Returns `(client, server)` where client has session type `S`
/// and server has session type `Dual<S>`.
///
/// Binds a listener on `127.0.0.1:0` (OS-assigned port), connects,
/// and accepts. Useful for testing.
pub fn tcp_pair<S: HasDual>() -> io::Result<(
    Chan<S, TcpTransport>,
    Chan<S::Dual, TcpTransport>,
)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;

    let client_stream = TcpStream::connect(addr)?;
    let (server_stream, _) = listener.accept()?;

    Ok((
        Chan::new(TcpTransport::from_stream(client_stream)),
        Chan::new(TcpTransport::from_stream(server_stream)),
    ))
}
