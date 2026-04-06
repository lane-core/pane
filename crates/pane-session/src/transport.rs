//! Bidirectional byte transport for session channels.
//!
//! Two-phase connection model:
//!   Phase 1 (connect): returns Result. Common failures (server not
//!     running, connection refused) are caught before par is involved.
//!   Phase 2 (handshake + active): uses FrameCodec over Read + Write.
//!     A verified transport that dies mid-session is exceptional.
//!
//! Design heritage: Plan 9 assumed a reliable byte stream (TCP, pipe)
//! for all 9P communication. BeOS used kernel ports (message-oriented,
//! reliable, in-order). pane uses Read + Write (byte stream), matching
//! Plan 9. MemoryTransport bridges the gap for testing — it's an
//! in-process byte stream over mpsc channels.

use crate::handshake::Rejection;

/// Connection failure — either transport-level or protocol-level.
///
/// Transport errors surface in Phase 1 (server not reachable).
/// Rejections surface in Phase 2 (server explicitly declined
/// the handshake after receiving Hello).
#[derive(Debug)]
pub enum ConnectError {
    /// Transport unreachable (network, socket, etc.).
    Transport(std::io::Error),
    /// Server explicitly rejected the handshake.
    Rejected(Rejection),
}

impl std::fmt::Display for ConnectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectError::Transport(e) => write!(f, "transport error: {e}"),
            ConnectError::Rejected(r) => write!(f, "connection rejected: {:?}", r.reason),
        }
    }
}

impl std::error::Error for ConnectError {}

/// A bidirectional byte transport.
///
/// Any `Read + Write + Send + 'static` type is a Transport.
/// UnixStream, TcpStream, and test adapters all qualify.
pub trait Transport: std::io::Read + std::io::Write + Send + 'static {}

/// Blanket impl: anything that is Read + Write + Send + 'static is a Transport.
impl<T: std::io::Read + std::io::Write + Send + 'static> Transport for T {}

/// In-memory byte transport for testing.
///
/// Write side pushes bytes through an mpsc channel. Read side
/// pulls bytes from the channel into an internal buffer. Writes
/// on one side are readable on the other; bytes arrive in order;
/// EOF when the peer is dropped.
pub struct MemoryTransport {
    tx: std::sync::mpsc::Sender<Vec<u8>>,
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
    /// Buffered bytes received but not yet consumed by read().
    read_buf: Vec<u8>,
    /// Current read position within read_buf.
    read_pos: usize,
}

impl MemoryTransport {
    /// Create a pair of connected in-memory transports.
    pub fn pair() -> (Self, Self) {
        let (tx1, rx1) = std::sync::mpsc::channel();
        let (tx2, rx2) = std::sync::mpsc::channel();
        (
            MemoryTransport { tx: tx1, rx: rx2, read_buf: Vec::new(), read_pos: 0 },
            MemoryTransport { tx: tx2, rx: rx1, read_buf: Vec::new(), read_pos: 0 },
        )
    }
}

impl std::io::Read for MemoryTransport {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // If internal buffer is exhausted, block for the next chunk.
        if self.read_pos >= self.read_buf.len() {
            match self.rx.recv() {
                Ok(data) => {
                    self.read_buf = data;
                    self.read_pos = 0;
                }
                // Peer dropped — signal EOF.
                Err(_) => return Ok(0),
            }
        }

        let available = &self.read_buf[self.read_pos..];
        let n = std::cmp::min(buf.len(), available.len());
        buf[..n].copy_from_slice(&available[..n]);
        self.read_pos += n;
        Ok(n)
    }
}

impl std::io::Write for MemoryTransport {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.tx.send(buf.to_vec()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "peer disconnected")
        })?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        // No buffering — writes go through immediately.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn memory_transport_roundtrip() {
        let (mut a, mut b) = MemoryTransport::pair();
        a.write_all(b"hello").unwrap();
        let mut buf = [0u8; 5];
        b.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"hello");

        b.write_all(b"world").unwrap();
        let mut buf = [0u8; 5];
        a.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"world");
    }

    #[test]
    fn write_after_peer_drop_is_broken_pipe() {
        let (mut a, b) = MemoryTransport::pair();
        drop(b);
        let err = a.write_all(b"hello").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn read_after_peer_drop_is_eof() {
        let (a, mut b) = MemoryTransport::pair();
        drop(a);
        let mut buf = [0u8; 1];
        let n = b.read(&mut buf).unwrap();
        assert_eq!(n, 0); // EOF
    }

    #[test]
    fn partial_reads_work() {
        let (mut a, mut b) = MemoryTransport::pair();
        a.write_all(b"hello world").unwrap();

        // Read in two chunks
        let mut buf = [0u8; 5];
        b.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"hello");

        let mut buf = [0u8; 6];
        b.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b" world");
    }

    #[test]
    fn multiple_writes_coalesce_into_reads() {
        let (mut a, mut b) = MemoryTransport::pair();
        a.write_all(b"ab").unwrap();
        a.write_all(b"cd").unwrap();

        let mut buf = [0u8; 4];
        b.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"abcd");
    }
}
