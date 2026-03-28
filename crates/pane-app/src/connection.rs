//! Connection to the compositor — either a real unix socket or
//! an in-memory channel for testing.

use std::sync::mpsc;

use pane_proto::protocol::{ClientToComp, CompToClient, ClientHello, ClientCaps, Accepted};
use pane_session::types::{Chan, Offer};
use pane_session::transport::Transport;

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

/// Result of a successful client handshake.
pub struct HandshakeResult {
    /// Capabilities accepted by the server.
    pub accepted: Accepted,
}

/// Run the client side of the session-typed handshake.
///
/// Sends ClientHello, receives ServerHello, sends ClientCaps,
/// and waits for the server's Accept/Reject decision.
pub fn run_client_handshake<T: Transport>(
    chan: Chan<pane_proto::protocol::ClientHandshake, T>,
    signature: &str,
) -> Result<HandshakeResult, crate::error::Error> {
    use crate::error::{ConnectError, Error};

    let chan = chan.send(ClientHello {
        signature: signature.to_string(),
        version: 1,
    }).map_err(|e| Error::Connect(ConnectError::Transport(e)))?;

    let (_server_hello, chan) = chan.recv()
        .map_err(|e| Error::Connect(ConnectError::Transport(e)))?;

    let chan = chan.send(ClientCaps { caps: vec![] })
        .map_err(|e| Error::Connect(ConnectError::Transport(e)))?;

    match chan.offer().map_err(|e| Error::Connect(ConnectError::Transport(e)))? {
        Offer::Left(chan) => {
            let (accepted, chan) = chan.recv()
                .map_err(|e| Error::Connect(ConnectError::Transport(e)))?;
            chan.close();
            Ok(HandshakeResult { accepted })
        }
        Offer::Right(chan) => {
            let (rejected, chan) = chan.recv()
                .map_err(|e| Error::Connect(ConnectError::Transport(e)))?;
            chan.close();
            Err(Error::Connect(ConnectError::Rejected(rejected.reason)))
        }
    }
}
