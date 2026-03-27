//! Connection to the compositor — either a real unix socket or
//! an in-memory channel for testing.

use std::sync::mpsc;

use pane_proto::protocol::{ClientToComp, CompToClient};

/// A bidirectional connection to the compositor.
/// Abstracted over transport so the kit can be tested without
/// a running compositor.
pub struct Connection {
    pub sender: mpsc::Sender<ClientToComp>,
    pub receiver: mpsc::Receiver<CompToClient>,
}

/// Create a connected pair for testing: one side is the "client" (kit),
/// the other is the "compositor" (mock).
pub fn test_pair() -> (Connection, MockConnection) {
    let (client_tx, mock_rx) = mpsc::channel();
    let (mock_tx, client_rx) = mpsc::channel();

    let client = Connection {
        sender: client_tx,
        receiver: client_rx,
    };

    let mock = MockConnection {
        sender: mock_tx,
        receiver: mock_rx,
    };

    (client, mock)
}

/// The mock compositor's end of the connection.
pub struct MockConnection {
    pub sender: mpsc::Sender<CompToClient>,
    pub receiver: mpsc::Receiver<ClientToComp>,
}
