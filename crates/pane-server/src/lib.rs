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
//! 3. After handshake, a read pump thread feeds messages to an mpsc channel
//! 4. The compositor polls the channel and dispatches messages
//!
//! TODO: Replace read pump threads with calloop SessionSource for
//! proper event-driven handling. The current pump pattern works but
//! adds one thread per client. SessionSource would use calloop's
//! fd-readiness notification instead.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

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
    /// Connected clients, indexed by client ID.
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

    /// Register a client after handshake completes.
    /// Spawns a read pump thread that feeds messages to an mpsc channel.
    pub fn register_client(&mut self, client_id: usize, stream: UnixStream) {
        let write_stream = match stream.try_clone() {
            Ok(s) => s,
            Err(e) => {
                warn!("clone stream for client {}: {}", client_id, e);
                return;
            }
        };

        let (msg_tx, msg_rx) = mpsc::channel();
        let read_stream = stream;

        // Read pump: blocks on framed reads, deserializes, sends to channel.
        // Exits when the client disconnects or the channel closes.
        std::thread::spawn(move || {
            use pane_session::framing::read_framed;
            loop {
                match read_framed(&mut &read_stream) {
                    Ok(bytes) => {
                        match pane_proto::deserialize::<ClientToComp>(&bytes) {
                            Ok(msg) => {
                                if msg_tx.send(msg).is_err() { break; }
                            }
                            Err(_) => break,
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        self.clients.insert(client_id, ClientSession {
            stream: write_stream,
            panes: Vec::new(),
            msg_rx: Some(msg_rx),
        });

        info!("client {} registered", client_id);
    }

    /// Poll all clients for pending messages and disconnections.
    /// Returns (messages, disconnected_client_ids).
    pub fn poll_clients(&self) -> (Vec<(usize, ClientToComp)>, Vec<usize>) {
        let mut messages = Vec::new();
        let mut disconnected = Vec::new();

        for (&client_id, client) in &self.clients {
            if let Some(ref rx) = client.msg_rx {
                loop {
                    match rx.try_recv() {
                        Ok(msg) => messages.push((client_id, msg)),
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            disconnected.push(client_id);
                            break;
                        }
                    }
                }
            }
        }

        (messages, disconnected)
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

/// Run the server-side handshake on a new connection.
/// Blocking — call on a spawned thread.
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

    let server = server.select_left()?;
    let server = server.send(Accepted { caps: caps.caps })?;

    let transport = server.finish();
    Ok(transport.into_stream())
}
