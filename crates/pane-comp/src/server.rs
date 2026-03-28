//! Protocol server — accepts client connections over unix sockets.
//!
//! Lifecycle per client:
//! 1. UnixListener accepts a connection
//! 2. Handshake runs on a spawned thread (session-typed, blocking)
//! 3. After handshake, the stream is registered with calloop as a
//!    SessionSource for non-blocking active-phase message processing
//! 4. Active-phase messages are handled in calloop callbacks

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;

use tracing::{info, warn, error};

use pane_proto::message::PaneId;
use pane_proto::protocol::{
    ClientToComp, CompToClient, PaneGeometry,
    ServerHandshake, ServerHello, Accepted,
};
use pane_session::calloop::write_message;
use pane_session::transport::unix::UnixTransport;
use pane_session::types::Chan;

/// Per-client state tracked by the compositor.
pub struct ClientSession {
    /// The stream for writing active-phase responses back to the client.
    pub stream: UnixStream,
    /// PaneIds owned by this client.
    pub panes: Vec<PaneId>,
    /// Incoming messages from the read pump thread.
    pub msg_rx: Option<mpsc::Receiver<ClientToComp>>,
}

/// Per-pane state tracked by the compositor.
pub struct PaneState {
    /// Which client owns this pane.
    pub client_id: usize,
    /// Current title (from SetTitle).
    pub title: String,
}

/// The protocol server manages the listener socket and client state.
pub struct ProtocolServer {
    sock_path: PathBuf,
    next_pane_id: u32,
    /// Connected clients, indexed by calloop registration token.
    pub clients: HashMap<usize, ClientSession>,
    /// All panes across all clients.
    pub panes: HashMap<PaneId, PaneState>,
    next_client_id: usize,
}

impl ProtocolServer {
    /// Create the listener socket at `$runtime_dir/pane/compositor.sock`.
    pub fn new(runtime_dir: &Path) -> std::io::Result<(Self, UnixListener)> {
        let pane_dir = runtime_dir.join("pane");
        std::fs::create_dir_all(&pane_dir)?;

        let sock_path = pane_dir.join("compositor.sock");

        // Remove stale socket if it exists
        let _ = std::fs::remove_file(&sock_path);

        let listener = UnixListener::bind(&sock_path)?;
        listener.set_nonblocking(true)?;

        info!("listening on {}", sock_path.display());

        Ok((
            ProtocolServer {
                sock_path,
                next_pane_id: 1,
                clients: HashMap::new(),
                panes: HashMap::new(),
                next_client_id: 0,
            },
            listener,
        ))
    }

    /// Allocate a new PaneId.
    pub fn alloc_pane_id(&mut self) -> PaneId {
        let id = PaneId::new(NonZeroU32::new(self.next_pane_id).unwrap());
        self.next_pane_id += 1;
        id
    }

    /// Allocate a new client ID.
    pub fn alloc_client_id(&mut self) -> usize {
        let id = self.next_client_id;
        self.next_client_id += 1;
        id
    }

    /// Handle an active-phase message from a client.
    pub fn handle_message(
        &mut self,
        client_id: usize,
        msg: ClientToComp,
        geometry: PaneGeometry,
    ) {
        match msg {
            ClientToComp::CreatePane { tag } => {
                let pane_id = self.alloc_pane_id();
                let title = tag.as_ref()
                    .map(|t| t.title.text.clone())
                    .unwrap_or_default();

                info!("client {} creating pane {:?} '{}'", client_id, pane_id, title);

                self.panes.insert(pane_id, PaneState {
                    client_id,
                    title,
                });

                if let Some(client) = self.clients.get_mut(&client_id) {
                    client.panes.push(pane_id);
                    let response = CompToClient::PaneCreated { pane: pane_id, geometry };
                    Self::send_to_client(client, &response);
                }
            }

            ClientToComp::RequestClose { pane } => {
                info!("client {} requesting close for pane {:?}", client_id, pane);
                self.panes.remove(&pane);
                if let Some(client) = self.clients.get_mut(&client_id) {
                    client.panes.retain(|p| *p != pane);
                    let response = CompToClient::CloseAck { pane };
                    Self::send_to_client(client, &response);
                }
            }

            ClientToComp::SetTitle { pane, title } => {
                if let Some(state) = self.panes.get_mut(&pane) {
                    state.title = title.text.clone();
                    info!("pane {:?} title: '{}'", pane, state.title);
                }
            }

            ClientToComp::SetVocabulary { pane, .. } => {
                info!("pane {:?} vocabulary updated", pane);
            }

            ClientToComp::SetContent { pane, .. } => {
                // Content rendering comes in Stage 3
                let _ = pane;
            }

            ClientToComp::CompletionResponse { .. } => {
                // Completion handling comes later
            }
        }
    }

    /// Remove a disconnected client and clean up its panes.
    pub fn remove_client(&mut self, client_id: usize) {
        if let Some(client) = self.clients.remove(&client_id) {
            for pane_id in &client.panes {
                self.panes.remove(pane_id);
            }
            info!("client {} disconnected ({} panes removed)",
                client_id, client.panes.len());
        }
    }

    fn send_to_client(client: &ClientSession, msg: &CompToClient) {
        let bytes = match pane_proto::serialize(msg) {
            Ok(b) => b,
            Err(e) => { error!("serialize error: {}", e); return; }
        };
        if let Err(e) = write_message(&client.stream, &bytes) {
            warn!("write to client failed: {}", e);
        }
    }

    /// Clean up the socket file on shutdown.
    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(&self.sock_path);
    }
}

impl Drop for ProtocolServer {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Run the server-side handshake on a new connection.
/// This is blocking and should be called on a spawned thread.
/// Returns the reclaimed stream on success.
pub fn run_server_handshake(stream: UnixStream) -> Result<UnixStream, pane_session::SessionError> {
    let transport = UnixTransport::from_stream(stream);
    let server: Chan<ServerHandshake, _> = Chan::new(transport);

    let (hello, server) = server.recv()?;
    info!("handshake: client '{}' v{}", hello.signature, hello.version);

    let server = server.send(ServerHello {
        compositor: "pane-comp".into(),
        version: 1,
    })?;

    let (caps, server) = server.recv()?;
    info!("handshake: caps {:?}", caps.caps);

    // Accept all clients for now
    let server = server.select_left()?;
    let server = server.send(Accepted { caps: caps.caps })?;

    let transport = server.finish();
    Ok(transport.into_stream())
}

// Listener registration and handshake thread spawning are done in main.rs.
// The calloop event loop accepts connections, spawns handshake threads,
// and polls for completed handshakes in the frame timer via CompState methods.
