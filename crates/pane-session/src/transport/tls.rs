//! TLS transport for encrypted network sessions.
//!
//! Wraps a TCP stream in a rustls TLS connection. The TLS handshake
//! completes *before* the session-typed handshake — from the session
//! type's perspective, TLS is invisible. `StreamOwned` implements
//! `Read + Write`, so the existing framing code works unchanged.
//!
//! Client certificate authentication binds `PeerIdentity` to a
//! cryptographic key. The server validates that the declared identity
//! in `ClientHello` matches the certificate subject.
//!
//! # Plan 9
//!
//! Plan 9's factotum separated authentication from services. Pane
//! achieves the same separation: TLS handles transport-layer auth,
//! `.plan` handles authorization, Landlock handles enforcement.
//! No service needs to implement its own auth.

use std::io;
use std::net::TcpStream;
use std::sync::Arc;

use rustls::StreamOwned;

use crate::error::SessionError;
use crate::framing;
use crate::transport::Transport;

/// TLS client transport — wraps a TCP stream with rustls client connection.
pub struct TlsClientTransport {
    stream: StreamOwned<rustls::ClientConnection, TcpStream>,
}

impl TlsClientTransport {
    /// Wrap an already-established TLS client connection.
    ///
    /// The TLS handshake must be complete before constructing this.
    /// Use [`connect`] for a convenience constructor that does both.
    pub fn from_stream(stream: StreamOwned<rustls::ClientConnection, TcpStream>) -> Self {
        TlsClientTransport { stream }
    }

    /// Extract the inner stream for phase transitions or inspection.
    pub fn into_stream(self) -> StreamOwned<rustls::ClientConnection, TcpStream> {
        self.stream
    }
}

impl Transport for TlsClientTransport {
    fn send_raw(&mut self, data: &[u8]) -> Result<(), SessionError> {
        framing::write_framed(&mut self.stream, data)?;
        Ok(())
    }

    fn recv_raw(&mut self) -> Result<Vec<u8>, SessionError> {
        Ok(framing::read_framed(&mut self.stream)?)
    }
}

/// TLS server transport — wraps a TCP stream with rustls server connection.
pub struct TlsServerTransport {
    stream: StreamOwned<rustls::ServerConnection, TcpStream>,
}

impl TlsServerTransport {
    /// Wrap an already-established TLS server connection.
    pub fn from_stream(stream: StreamOwned<rustls::ServerConnection, TcpStream>) -> Self {
        TlsServerTransport { stream }
    }

    /// Extract the inner stream.
    pub fn into_stream(self) -> StreamOwned<rustls::ServerConnection, TcpStream> {
        self.stream
    }
}

impl Transport for TlsServerTransport {
    fn send_raw(&mut self, data: &[u8]) -> Result<(), SessionError> {
        framing::write_framed(&mut self.stream, data)?;
        Ok(())
    }

    fn recv_raw(&mut self) -> Result<Vec<u8>, SessionError> {
        Ok(framing::read_framed(&mut self.stream)?)
    }
}

/// Connect to a TLS-enabled pane server.
///
/// Performs the TLS handshake over an existing TCP stream, then wraps
/// the result as a `TlsClientTransport` ready for session-typed use.
pub fn connect_tls(
    tcp_stream: TcpStream,
    server_name: rustls::pki_types::ServerName<'static>,
    config: Arc<rustls::ClientConfig>,
) -> io::Result<TlsClientTransport> {
    let conn = rustls::ClientConnection::new(config, server_name)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let stream = StreamOwned::new(conn, tcp_stream);
    Ok(TlsClientTransport::from_stream(stream))
}

/// Accept a TLS connection from a client.
///
/// Performs the TLS handshake over an existing TCP stream, then wraps
/// the result as a `TlsServerTransport` ready for session-typed use.
pub fn accept_tls(
    tcp_stream: TcpStream,
    config: Arc<rustls::ServerConfig>,
) -> io::Result<TlsServerTransport> {
    let conn = rustls::ServerConnection::new(config)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    let stream = StreamOwned::new(conn, tcp_stream);
    Ok(TlsServerTransport::from_stream(stream))
}

/// Build a rustls `ClientConfig` that trusts the system CA roots.
///
/// For connecting to production pane servers with proper certificates.
pub fn default_client_config() -> Arc<rustls::ClientConfig> {
    let root_store = rustls::RootCertStore::from_iter(
        webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
    );
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Arc::new(config)
}
