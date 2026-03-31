//! Connection to the compositor — either a real unix socket or
//! an in-memory channel for testing.

use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::thread;

use pane_proto::protocol::{ClientToComp, CompToClient, ClientHello, ClientCaps, Accepted};
use pane_session::types::{Chan, Offer};
use pane_session::transport::Transport;

/// A bidirectional connection to the compositor.
/// Abstracted over transport so the kit can be tested without
/// a running compositor.
pub struct Connection {
    pub(crate) sender: mpsc::Sender<ClientToComp>,
    pub(crate) receiver: mpsc::Receiver<CompToClient>,
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
    pub(crate) sender: mpsc::Sender<CompToClient>,
    pub(crate) receiver: mpsc::Receiver<ClientToComp>,
}

/// Result of a successful client handshake.
#[allow(dead_code)]
pub struct HandshakeResult<T> {
    /// Capabilities accepted by the server.
    pub(crate) accepted: Accepted,
    /// The transport, reclaimed via finish() for active-phase reuse.
    pub(crate) transport: T,
}

/// Bridge a unix stream into a typed Connection.
///
/// Spawns two pump threads:
/// - Read pump: reads framed CompToClient from the stream, deserializes, sends to mpsc
/// - Write pump: reads ClientToComp from mpsc, serializes, writes framed to stream
///
/// The Connection's sender/receiver are the typed mpsc endpoints.
/// The pump threads exit when either the stream or the mpsc channels close.
pub fn from_unix_stream(stream: UnixStream) -> std::io::Result<Connection> {
    let read_stream = stream.try_clone()?;
    let shutdown_stream = stream.try_clone()?;
    let write_stream = stream;

    let (client_tx, write_rx) = mpsc::channel::<ClientToComp>();
    let (read_tx, client_rx) = mpsc::channel::<CompToClient>();

    // Read pump: stream → deserialize → mpsc
    thread::spawn(move || {
        use pane_session::framing::read_framed;
        loop {
            match read_framed(&mut &read_stream) {
                Ok(bytes) => {
                    match pane_proto::deserialize::<CompToClient>(&bytes) {
                        Ok(msg) => {
                            if read_tx.send(msg).is_err() { break; }
                        }
                        Err(_) => break,
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Write pump: mpsc → serialize → stream
    // When all senders drop (App closes), the for loop ends and we
    // shut down the socket to unblock the read pump.
    thread::spawn(move || {
        use pane_session::framing::write_framed;
        for msg in write_rx {
            match pane_proto::serialize(&msg) {
                Ok(bytes) => {
                    if write_framed(&mut &write_stream, &bytes).is_err() { break; }
                }
                Err(_) => break,
            }
        }
        // All senders dropped — connection closing. Shut down the socket
        // to unblock the read pump and the remote server.
        let _ = shutdown_stream.shutdown(std::net::Shutdown::Both);
    });

    Ok(Connection {
        sender: client_tx,
        receiver: client_rx,
    })
}

/// Run the client side of the session-typed handshake.
///
/// Sends ClientHello, receives ServerHello, sends ClientCaps,
/// and waits for the server's Accept/Reject decision.
///
/// On success, returns the accepted capabilities and the reclaimed
/// transport (via `finish()`). The caller can reuse the transport
/// for the active phase — e.g., `transport.into_stream()` for unix sockets.
pub fn run_client_handshake<T: Transport>(
    chan: Chan<pane_proto::protocol::ClientHandshake, T>,
    signature: &str,
) -> Result<HandshakeResult<T>, crate::error::Error> {
    use crate::error::{ConnectError, Error};

    let chan = chan.send(ClientHello {
        signature: signature.to_string(),
        version: 1,
        identity: None, // local connections; remote fills this in connect_remote
    }).map_err(|e| Error::Connect(ConnectError::Transport(e)))?;

    let (_server_hello, chan) = chan.recv()
        .map_err(|e| Error::Connect(ConnectError::Transport(e)))?;

    let chan = chan.send(ClientCaps { caps: vec![] })
        .map_err(|e| Error::Connect(ConnectError::Transport(e)))?;

    match chan.offer().map_err(|e| Error::Connect(ConnectError::Transport(e)))? {
        Offer::Left(chan) => {
            let (accepted, chan) = chan.recv()
                .map_err(|e| Error::Connect(ConnectError::Transport(e)))?;
            let transport = chan.finish();
            Ok(HandshakeResult { accepted, transport })
        }
        Offer::Right(chan) => {
            let (rejected, chan) = chan.recv()
                .map_err(|e| Error::Connect(ConnectError::Transport(e)))?;
            chan.close();
            Err(Error::Connect(ConnectError::Rejected(rejected.reason)))
        }
    }
}
