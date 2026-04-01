//! In-memory transport for testing.
//!
//! Creates a pair of connected channels that communicate via
//! std::sync::mpsc. Used for testing session type protocols
//! without sockets.

use std::sync::mpsc;

use crate::error::SessionError;
use crate::transport::Transport;
use crate::types::Chan;
use crate::dual::HasDual;

/// In-memory transport backed by mpsc channels.
pub struct MemoryTransport {
    tx: mpsc::Sender<Vec<u8>>,
    rx: mpsc::Receiver<Vec<u8>>,
}

impl MemoryTransport {
    /// Create a transport from raw channel endpoints.
    ///
    /// Prefer [`pair()`] for most uses. This constructor is for cases
    /// where you need to wrap the transport in another layer (e.g.,
    /// `ProxyTransport`) before creating the session channel.
    pub fn new(tx: mpsc::Sender<Vec<u8>>, rx: mpsc::Receiver<Vec<u8>>) -> Self {
        MemoryTransport { tx, rx }
    }
}

impl Transport for MemoryTransport {
    fn send_raw(&mut self, data: &[u8]) -> Result<(), SessionError> {
        self.tx
            .send(data.to_vec())
            .map_err(|_| SessionError::Disconnected)
    }

    fn recv_raw(&mut self) -> Result<Vec<u8>, SessionError> {
        self.rx.recv().map_err(|_| SessionError::Disconnected)
    }
}

/// Create a pair of connected in-memory session channels.
///
/// Returns `(client, server)` where client has session type `S`
/// and server has session type `Dual<S>`.
pub fn pair<S: HasDual>() -> (Chan<S, MemoryTransport>, Chan<S::Dual, MemoryTransport>) {
    let (tx1, rx1) = mpsc::channel();
    let (tx2, rx2) = mpsc::channel();

    let client_transport = MemoryTransport { tx: tx1, rx: rx2 };
    let server_transport = MemoryTransport { tx: tx2, rx: rx1 };

    (Chan::new(client_transport), Chan::new(server_transport))
}
