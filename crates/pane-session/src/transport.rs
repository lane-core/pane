//! Transport: bidirectional byte transport for session channels.
//!
//! Implementations handle framing, connection management, and
//! serialization. Chan<S> uses Transport for the physical delivery.

use std::io;

/// A bidirectional byte transport.
///
/// Session operations (send, recv, select, offer) are built on
/// send_raw/recv_raw. The session layer handles serialization
/// (postcard); the transport handles delivery.
pub trait Transport: Sized + Send {
    /// Send raw bytes (already serialized).
    /// Panics on broken connection (par's CLL model — a session
    /// either completes or is annihilated).
    fn send_raw(&mut self, data: &[u8]);

    /// Receive raw bytes (to be deserialized).
    /// Panics on broken connection.
    fn recv_raw(&mut self) -> Vec<u8>;
}

/// In-memory transport for testing. Uses paired Vec buffers.
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
}
