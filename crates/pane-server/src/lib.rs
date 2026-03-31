//! Compositor protocol server — socket listener, handshake, client message routing.
//!
//! This crate contains all compositor-side protocol logic that does NOT
//! depend on rendering (smithay, Vello, etc.). It builds on macOS,
//! enabling `cargo check -p pane-server` during development without
//! the Linux cross-build round trip.
//!
//! Lifecycle per client:
//! 1. UnixListener accepts a connection
//! 2. Handshake runs on a spawned thread (session-typed, blocking)
//! 3. Compositor registers the socket as a calloop SessionSource for
//!    event-driven message dispatch (no per-client threads)
//! 4. This crate stores per-client write streams and protocol state;
//!    the compositor (pane-comp) owns the calloop registration

use std::collections::HashMap;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use tracing::{info, warn, error};

use pane_proto::message::PaneId;
use pane_proto::protocol::{
    ClientToComp, CompToClient, PaneGeometry,
    ServerHandshake, ServerHello, Accepted, ConnectionTopology,
};
use pane_session::calloop::write_message;
use pane_session::transport::unix::UnixTransport;
use pane_session::types::Chan;

/// Per-client state tracked by the compositor.
/// Reading is handled by calloop SessionSource in pane-comp;
/// this struct holds only the write stream and owned pane list.
pub struct ClientSession {
    /// The stream for writing active-phase responses back to the client.
    pub stream: UnixStream,
    /// PaneIds owned by this client.
    pub panes: Vec<PaneId>,
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
    /// Connected clients, indexed by client ID.
    pub clients: HashMap<usize, ClientSession>,
    /// All panes across all clients.
    pub panes: HashMap<PaneId, PaneState>,
    next_client_id: usize,
    /// Instance identifier for federation.
    pub instance_id: String,
}

impl ProtocolServer {
    /// Create the listener socket at `$runtime_dir/pane/compositor.sock`.
    pub fn new(runtime_dir: &Path) -> std::io::Result<(Self, UnixListener)> {
        let pane_dir = runtime_dir.join("pane");
        std::fs::create_dir_all(&pane_dir)?;

        let sock_path = pane_dir.join("compositor.sock");
        let _ = std::fs::remove_file(&sock_path);

        let listener = UnixListener::bind(&sock_path)?;
        listener.set_nonblocking(true)?;

        info!("listening on {}", sock_path.display());

        Ok((
            ProtocolServer {
                sock_path,
                clients: HashMap::new(),
                panes: HashMap::new(),
                next_client_id: 0,
                instance_id: uuid::Uuid::new_v4().to_string(),
            },
            listener,
        ))
    }

    /// Create a protocol server without managing a listener socket.
    ///
    /// The caller is responsible for listener creation and cleanup.
    /// Use this when the server accepts connections from multiple
    /// transports (unix + TCP) or when socket lifecycle is managed
    /// externally (s6-fdholder).
    pub fn new_unmanaged() -> Self {
        ProtocolServer {
            sock_path: PathBuf::new(),
            clients: HashMap::new(),
            panes: HashMap::new(),
            next_client_id: 0,
            instance_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Allocate a new client ID.
    pub fn alloc_client_id(&mut self) -> usize {
        let id = self.next_client_id;
        self.next_client_id += 1;
        id
    }

    /// Register a client after handshake completes.
    /// Stores the write stream for sending responses. The read side
    /// is registered as a calloop SessionSource by the compositor.
    pub fn register_client(&mut self, client_id: usize, write_stream: UnixStream) {
        self.clients.insert(client_id, ClientSession {
            stream: write_stream,
            panes: Vec::new(),
        });

        info!("client {} registered", client_id);
    }

    /// Handle an active-phase message from a client.
    pub fn handle_message(
        &mut self,
        client_id: usize,
        msg: ClientToComp,
        geometry: PaneGeometry,
    ) {
        match msg {
            ClientToComp::CreatePane { pane: pane_id, tag } => {
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
                let _ = pane; // content rendering comes later
            }

            ClientToComp::CompletionResponse { .. } => {}

            ClientToComp::RequestResize { pane, width, height } => {
                info!("pane {:?} requested resize to {}x{}", pane, width, height);
                // Compositor decides whether to honor — tiling may constrain.
                // For now, acknowledge with the requested geometry.
                if let Some(client) = self.clients.get_mut(&client_id) {
                    let response = CompToClient::Resize {
                        pane,
                        geometry: PaneGeometry {
                            width, height,
                            cols: (width as u16) / 9,  // approximate cell size
                            rows: (height as u16) / 17,
                        },
                    };
                    Self::send_to_client(client, &response);
                }
            }

            ClientToComp::SetSizeLimits { pane, min_width, min_height, max_width, max_height } => {
                info!("pane {:?} size limits: {}x{} - {}x{}", pane,
                    min_width, min_height, max_width, max_height);
                // Store for layout engine (not yet implemented)
            }

            ClientToComp::SetHidden { pane, hidden } => {
                info!("pane {:?} hidden={}", pane, hidden);
                // Visibility control (not yet implemented)
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

/// Result of a successful server-side handshake.
pub struct ServerHandshakeResult<T> {
    /// The reclaimed transport, for active-phase reuse.
    pub transport: T,
    /// Application signature from ClientHello.
    pub signature: String,
    /// Peer identity (if declared — remote connections only).
    pub identity: Option<pane_proto::protocol::PeerIdentity>,
}

/// Run the server-side handshake on a new connection.
/// Blocking — call on a spawned thread.
/// Returns the reclaimed stream on success.
pub fn run_server_handshake(
    stream: UnixStream,
    instance_id: &str,
) -> Result<UnixStream, pane_session::SessionError> {
    let result = run_server_handshake_generic(
        UnixTransport::from_stream(stream),
        instance_id,
        ConnectionTopology::Local,
    )?;
    Ok(result.transport.into_stream())
}

/// Run the server-side handshake over any transport.
///
/// Generic over `T: Transport` — works with `UnixTransport`,
/// `TcpTransport`, or any other transport implementation.
/// The caller specifies the topology (Local for unix, Remote for TCP).
pub fn run_server_handshake_generic<T: pane_session::transport::Transport>(
    transport: T,
    instance_id: &str,
    topology: ConnectionTopology,
) -> Result<ServerHandshakeResult<T>, pane_session::SessionError> {
    let server: Chan<ServerHandshake, T> = Chan::new(transport);

    let (hello, server) = server.recv()?;
    info!("handshake: client '{}' v{}", hello.signature, hello.version);
    if let Some(ref id) = hello.identity {
        info!("handshake: remote identity {}@{} (uid {})", id.username, id.hostname, id.uid);
    }

    let signature = hello.signature.clone();
    let identity = hello.identity.clone();

    let server = server.send(ServerHello {
        compositor: "pane-comp".into(),
        version: 1,
        instance_id: instance_id.to_string(),
    })?;

    let (caps, server) = server.recv()?;
    info!("handshake: caps {:?}", caps.caps);

    let server = server.select_left()?;
    let server = server.send(Accepted {
        caps: caps.caps,
        topology,
    })?;

    let transport = server.finish();
    Ok(ServerHandshakeResult { transport, signature, identity })
}
