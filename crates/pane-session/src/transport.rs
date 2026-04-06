//! Bidirectional byte transport for session channels.
//!
//! Two-phase connection model:
//!   Phase 1 (connect): returns Result. Common failures (server not
//!     running, connection refused) are caught before par is involved.
//!   Phase 2 (handshake + active): panics on broken connection.
//!     A verified transport that dies mid-session is exceptional.

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
/// After connection is verified (Phase 1), send_raw/recv_raw panic
/// on broken connection. A transport that was alive and then broke
/// mid-protocol is an exceptional condition — the session is aborted.
pub trait Transport: Sized + Send {
    /// Send raw bytes. Panics on broken connection.
    fn send_raw(&mut self, data: &[u8]);

    /// Receive raw bytes. Panics on broken connection.
    fn recv_raw(&mut self) -> Vec<u8>;
}

/// In-memory transport for testing.
pub struct MemoryTransport {
    tx: std::sync::mpsc::Sender<Vec<u8>>,
    rx: std::sync::mpsc::Receiver<Vec<u8>>,
}

impl MemoryTransport {
    /// Create a pair of connected in-memory transports.
    pub fn pair() -> (Self, Self) {
        let (tx1, rx1) = std::sync::mpsc::channel();
        let (tx2, rx2) = std::sync::mpsc::channel();
        (
            MemoryTransport { tx: tx1, rx: rx2 },
            MemoryTransport { tx: tx2, rx: rx1 },
        )
    }
}

impl Transport for MemoryTransport {
    fn send_raw(&mut self, data: &[u8]) {
        self.tx.send(data.to_vec()).expect("peer disconnected");
    }

    fn recv_raw(&mut self) -> Vec<u8> {
        self.rx.recv().expect("peer disconnected")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_transport_roundtrip() {
        let (mut a, mut b) = MemoryTransport::pair();
        a.send_raw(b"hello");
        assert_eq!(b.recv_raw(), b"hello");
        b.send_raw(b"world");
        assert_eq!(a.recv_raw(), b"world");
    }

    #[test]
    #[should_panic(expected = "peer disconnected")]
    fn send_after_peer_drop_panics() {
        let (mut a, b) = MemoryTransport::pair();
        drop(b);
        a.send_raw(b"hello");
    }

    #[test]
    #[should_panic(expected = "peer disconnected")]
    fn recv_after_peer_drop_panics() {
        let (a, mut b) = MemoryTransport::pair();
        drop(a);
        b.recv_raw();
    }
}
