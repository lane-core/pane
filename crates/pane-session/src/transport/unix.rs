//! Unix domain socket transport.
//!
//! Length-prefixed postcard messages over unix stream sockets.
//! This is the production transport for pane — all inter-process
//! session-typed communication uses this.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use crate::error::SessionError;
use crate::transport::Transport;

/// Maximum message size: 16 MB. Prevents malicious or corrupt length
/// prefixes from causing unbounded allocation. Pane protocol messages
/// are typically under 64 KB; anything approaching 16 MB is either
/// a bug or an attack.
pub const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;

/// Unix domain socket transport with length-prefixed framing.
pub struct UnixTransport {
    stream: UnixStream,
}

impl UnixTransport {
    /// Wrap an existing unix stream as a session transport.
    pub fn from_stream(stream: UnixStream) -> Self {
        UnixTransport { stream }
    }
}

impl Transport for UnixTransport {
    fn send_raw(&mut self, data: &[u8]) -> Result<(), SessionError> {
        let len = (data.len() as u32).to_le_bytes();
        self.stream.write_all(&len)?;
        self.stream.write_all(data)?;
        self.stream.flush()?;
        Ok(())
    }

    fn recv_raw(&mut self) -> Result<Vec<u8>, SessionError> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf)?;
        let len = u32::from_le_bytes(len_buf) as usize;

        if len > MAX_MESSAGE_SIZE {
            return Err(SessionError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("message size {} exceeds maximum {}", len, MAX_MESSAGE_SIZE),
            )));
        }

        let mut buf = vec![0u8; len];
        self.stream.read_exact(&mut buf)?;
        Ok(buf)
    }
}
