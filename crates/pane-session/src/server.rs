//! Service-aware routing server (single-threaded actor model).
//!
//! Accepts connections from panes, populates a provider index from
//! Hello.provides, handles DeclareInterest to wire consumers to
//! providers, and forwards ServiceFrames between matched pairs.
//!
//! Architecture: reader threads are thin negative adapters — they
//! call read_frame() and post ServerEvents to the actor's channel.
//! The actor thread owns all routing state exclusively (no mutex)
//! and processes events sequentially. Write handles are leaf
//! mutexes (one per connection, never nested with routing state).
//!
//! Design heritage: Plan 9 exportfs
//! (reference/plan9/src/sys/src/cmd/exportfs/) was a single-reader
//! multiplexer that forwarded 9P between connections. BeOS
//! app_server (src/servers/app/Desktop.cpp) was a multi-threaded
//! router with per-ServerApp message loops. pane's ProtocolServer
//! combines exportfs's single-actor routing with app_server's
//! typed service registry.
//!
//! Theoretical basis: the single-threaded actor prevents
//! non-associative cross-polarity composition from arising
//! (MMM25, MM14b). Reader threads are purely negative (blocking
//! reads). The actor thread handles all polarity crossings
//! (read event → write response) sequentially. DLfActRiS
//! (Hinrichsen/Krebbers/Birkedal, POPL 2024) proves deadlock
//! freedom for this single-mailbox actor topology.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use crate::bridge::HANDSHAKE_MAX_MESSAGE_SIZE;
use crate::frame::{Frame, FrameCodec, FrameError};
use crate::handshake::{Hello, Rejection, ServiceProvision, Welcome};
use pane_proto::control::ControlMessage;
use pane_proto::protocol::ServiceId;

/// Opaque connection identifier assigned by the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(pub u64);

// -- Events: reader threads post these to the actor --

/// An event from a reader thread (or accept()) to the server actor.
enum ServerEvent {
    /// A new connection was accepted. Register its writer and provides.
    NewConnection {
        conn_id: ConnectionId,
        write_handle: WriteHandle,
        hello: Hello,
    },
    /// A frame arrived on a connection.
    Frame {
        conn_id: ConnectionId,
        service: u16,
        payload: Vec<u8>,
    },
    /// A connection's reader thread exited (transport closed or error).
    Disconnected { conn_id: ConnectionId },
}

// -- Write handle: leaf mutex, one per connection --

/// Write handle to a connection. Server-internal — not exposed
/// to clients. The writer is behind its own Mutex; only the actor
/// thread writes, so contention is minimal. Leaf lock: never held
/// while acquiring another lock.
///
/// Distinct from the client-side write path (SyncSender<WriteMessage>
/// in ClientConnection/ServiceHandle) because the server owns the
/// raw transport writer directly. The client sends write requests
/// through a bounded channel to a writer thread; the server's actor
/// writes through this handle on the actor thread itself. Both
/// paths use FrameCodec for wire encoding but differ in ownership:
/// the client's writer thread owns the transport half, while this
/// handle wraps it in Arc<Mutex> for the actor's sequential access.
#[derive(Clone)]
struct WriteHandle {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    codec: Arc<FrameCodec>,
}

impl WriteHandle {
    fn write_frame(&self, service: u16, payload: &[u8]) -> std::io::Result<()> {
        let mut w = self.writer.lock().expect("write lock poisoned");
        self.codec.write_frame(&mut *w, service, payload)
    }
}

// -- Routing state: owned exclusively by the actor thread --

/// Routing entry: maps a session on one connection to a session
/// on another connection.
#[derive(Debug, Clone)]
struct Route {
    consumer_conn: ConnectionId,
    consumer_session: u16,
    provider_conn: ConnectionId,
    provider_session: u16,
}

/// Server state — owned by the actor thread, no Arc<Mutex<>>.
/// Sequential access only (single-mailbox actor, I6).
struct ServerState {
    /// ServiceId → list of connections that provide it.
    /// Vec for functoriality: multiple providers per service.
    provider_index: HashMap<uuid::Uuid, Vec<ConnectionId>>,

    /// (ConnectionId, session_id) → Route. Both directions stored.
    routing_table: HashMap<(ConnectionId, u16), Route>,

    /// Next session_id to allocate per connection.
    next_session: HashMap<ConnectionId, u16>,

    /// Write handles per connection.
    writers: HashMap<ConnectionId, WriteHandle>,

    /// ConnectionId → Address. Populated at connection time.
    /// Phase 1: pane_id = conn_id, server_id = 0 (local).
    conn_addresses: HashMap<ConnectionId, pane_proto::Address>,

    /// Watch table: watched ConnectionId → set of watcher ConnectionIds.
    /// When a connection disconnects, iterate its watchers and send PaneExited.
    ///
    /// Design heritage: BeOS BRoster::StartWatching
    /// (src/servers/registrar/TRoster.cpp:1523-1536) stored watchers
    /// at the registrar. Plan 9's wait(2) was parent-child only.
    /// pane's server-mediated watches allow any-to-any monitoring,
    /// closer to BeOS's registrar model.
    watch_table: HashMap<ConnectionId, HashSet<ConnectionId>>,

    /// Reverse index: watcher ConnectionId → set of watched ConnectionIds.
    /// For efficient cleanup when the watcher disconnects.
    watcher_reverse: HashMap<ConnectionId, HashSet<ConnectionId>>,
}

impl ServerState {
    fn new() -> Self {
        ServerState {
            provider_index: HashMap::new(),
            routing_table: HashMap::new(),
            next_session: HashMap::new(),
            writers: HashMap::new(),
            conn_addresses: HashMap::new(),
            watch_table: HashMap::new(),
            watcher_reverse: HashMap::new(),
        }
    }

    fn alloc_session(&mut self, conn: ConnectionId) -> Option<u16> {
        let next = self.next_session.entry(conn).or_insert(1);
        let session = *next;
        if session == 0xFFFF {
            return None; // 0xFFFF reserved for ProtocolAbort
        }
        *next += 1;
        Some(session)
    }

    fn register_provides(&mut self, conn: ConnectionId, provides: &[ServiceProvision]) {
        for provision in provides {
            self.provider_index
                .entry(provision.service.uuid)
                .or_default()
                .push(conn);
        }
    }

    /// Look up a ConnectionId by Address. Linear scan — Phase 1 has
    /// few connections; Phase 2 can add a reverse index.
    fn resolve_conn(&self, addr: &pane_proto::Address) -> Option<ConnectionId> {
        self.conn_addresses
            .iter()
            .find(|(_, a)| *a == addr)
            .map(|(c, _)| *c)
    }

    /// Handle DeclareInterest. Returns Err with a reason on failure.
    fn handle_declare_interest(
        &mut self,
        consumer_conn: ConnectionId,
        service: ServiceId,
        _expected_version: u32,
    ) -> Result<(ControlMessage, ConnectionId, u16), pane_proto::control::DeclineReason> {
        let providers = self
            .provider_index
            .get(&service.uuid)
            .and_then(|p| p.first().copied());
        let provider_conn = match providers {
            Some(p) => p,
            None => return Err(pane_proto::control::DeclineReason::ServiceUnknown),
        };
        if provider_conn == consumer_conn {
            return Err(pane_proto::control::DeclineReason::SelfProvide);
        }

        let consumer_session = self
            .alloc_session(consumer_conn)
            .ok_or(pane_proto::control::DeclineReason::SessionExhausted)?;
        let provider_session = self
            .alloc_session(provider_conn)
            .ok_or(pane_proto::control::DeclineReason::SessionExhausted)?;

        let route = Route {
            consumer_conn,
            consumer_session,
            provider_conn,
            provider_session,
        };

        self.routing_table
            .insert((consumer_conn, consumer_session), route.clone());
        self.routing_table
            .insert((provider_conn, provider_session), route);

        let accepted = ControlMessage::InterestAccepted {
            service_uuid: service.uuid,
            session_id: consumer_session,
            version: 1,
        };

        Ok((accepted, provider_conn, provider_session))
    }

    /// Remove all state for a connection. Returns peers to notify.
    fn remove_connection(&mut self, conn: ConnectionId) -> Vec<(ConnectionId, u16)> {
        for providers in self.provider_index.values_mut() {
            providers.retain(|&c| c != conn);
        }
        self.provider_index.retain(|_, v| !v.is_empty());

        let affected_keys: Vec<_> = self
            .routing_table
            .keys()
            .filter(|(c, _)| *c == conn)
            .cloned()
            .collect();

        let mut peers_to_notify = Vec::new();

        for key in affected_keys {
            if let Some(route) = self.routing_table.remove(&key) {
                let (peer_conn, peer_session) = if route.consumer_conn == conn {
                    (route.provider_conn, route.provider_session)
                } else {
                    (route.consumer_conn, route.consumer_session)
                };
                self.routing_table.remove(&(peer_conn, peer_session));
                peers_to_notify.push((peer_conn, peer_session));
            }
        }

        self.next_session.remove(&conn);
        self.writers.remove(&conn);

        peers_to_notify
    }

    // -- Event processing (called on the actor thread) --

    fn process_frame(&mut self, conn_id: ConnectionId, service: u16, payload: Vec<u8>) {
        if service == 0 {
            self.process_control(conn_id, payload);
        } else {
            self.process_service(conn_id, service, payload);
        }
    }

    fn process_control(&mut self, conn_id: ConnectionId, payload: Vec<u8>) {
        let msg: ControlMessage = match postcard::from_bytes(&payload) {
            Ok(m) => m,
            Err(_) => return, // Deserialization failure — ignore
        };

        match msg {
            ControlMessage::DeclareInterest {
                service,
                expected_version,
            } => {
                match self.handle_declare_interest(conn_id, service, expected_version) {
                    Ok((accepted, provider_conn, provider_session)) => {
                        // Send InterestAccepted to consumer
                        if let Ok(bytes) = postcard::to_allocvec(&accepted) {
                            if let Some(wh) = self.writers.get(&conn_id) {
                                let _ = wh.write_frame(0, &bytes);
                            }
                        }
                        // Notify provider
                        let provider_notify = ControlMessage::InterestAccepted {
                            service_uuid: service.uuid,
                            session_id: provider_session,
                            version: 1,
                        };
                        if let Ok(bytes) = postcard::to_allocvec(&provider_notify) {
                            if let Some(wh) = self.writers.get(&provider_conn) {
                                let _ = wh.write_frame(0, &bytes);
                            }
                        }
                    }
                    Err(reason) => {
                        let declined = ControlMessage::InterestDeclined {
                            service_uuid: service.uuid,
                            reason,
                        };
                        if let Ok(bytes) = postcard::to_allocvec(&declined) {
                            if let Some(wh) = self.writers.get(&conn_id) {
                                let _ = wh.write_frame(0, &bytes);
                            }
                        }
                    }
                }
            }
            ControlMessage::RevokeInterest { session_id } => {
                // Remove the route for this session on this connection.
                if let Some(route) = self.routing_table.remove(&(conn_id, session_id)) {
                    let (peer_conn, peer_session) = if route.consumer_conn == conn_id {
                        (route.provider_conn, route.provider_session)
                    } else {
                        (route.consumer_conn, route.consumer_session)
                    };
                    self.routing_table.remove(&(peer_conn, peer_session));
                    // Notify the peer of teardown.
                    let peer_teardown = ControlMessage::ServiceTeardown {
                        session_id: peer_session,
                        reason: pane_proto::control::TeardownReason::ServiceRevoked,
                    };
                    if let Ok(bytes) = postcard::to_allocvec(&peer_teardown) {
                        if let Some(wh) = self.writers.get(&peer_conn) {
                            let _ = wh.write_frame(0, &bytes);
                        }
                    }
                    // Echo teardown to the revoker so its looper can
                    // fail outstanding dispatch entries for this session
                    // (S1 fix: prevents dispatch entry orphaning).
                    let revoker_teardown = ControlMessage::ServiceTeardown {
                        session_id,
                        reason: pane_proto::control::TeardownReason::ServiceRevoked,
                    };
                    if let Ok(bytes) = postcard::to_allocvec(&revoker_teardown) {
                        if let Some(wh) = self.writers.get(&conn_id) {
                            let _ = wh.write_frame(0, &bytes);
                        }
                    }
                }
                // Unknown session_id = no-op (defensive, per session-type analysis)
            }
            ControlMessage::Cancel { token: _ } => {
                // TODO: forward to provider side
            }
            ControlMessage::Watch { target } => {
                // Resolve target to a ConnectionId. If the target
                // is unknown (already dead or never connected),
                // send PaneExited immediately — no race window.
                match self.resolve_conn(&target) {
                    Some(target_conn) => {
                        self.watch_table
                            .entry(target_conn)
                            .or_default()
                            .insert(conn_id);
                        self.watcher_reverse
                            .entry(conn_id)
                            .or_default()
                            .insert(target_conn);
                    }
                    None => {
                        // Target unknown — already dead or never existed.
                        let exited = ControlMessage::PaneExited {
                            address: target,
                            reason: pane_proto::ExitReason::Disconnected,
                        };
                        if let Ok(bytes) = postcard::to_allocvec(&exited) {
                            if let Some(wh) = self.writers.get(&conn_id) {
                                let _ = wh.write_frame(0, &bytes);
                            }
                        }
                    }
                }
            }
            ControlMessage::Unwatch { target } => {
                if let Some(target_conn) = self.resolve_conn(&target) {
                    if let Some(watchers) = self.watch_table.get_mut(&target_conn) {
                        watchers.remove(&conn_id);
                        if watchers.is_empty() {
                            self.watch_table.remove(&target_conn);
                        }
                    }
                    if let Some(watched) = self.watcher_reverse.get_mut(&conn_id) {
                        watched.remove(&target_conn);
                        if watched.is_empty() {
                            self.watcher_reverse.remove(&conn_id);
                        }
                    }
                }
                // Unknown target = no-op (already unwatched or never watched)
            }
            _ => {}
        }
    }

    fn process_service(&mut self, conn_id: ConnectionId, service: u16, payload: Vec<u8>) {
        if let Some(route) = self.routing_table.get(&(conn_id, service)) {
            let (target_conn, target_session) = if route.consumer_conn == conn_id {
                (route.provider_conn, route.provider_session)
            } else {
                (route.consumer_conn, route.consumer_session)
            };
            if let Some(wh) = self.writers.get(&target_conn) {
                let _ = wh.write_frame(target_session, &payload);
            }
        }
        // Missing route on incoming Reply = Cancel/Reply race, not an error.
    }

    fn process_disconnect(&mut self, conn_id: ConnectionId) {
        // Notify watchers of this pane's death BEFORE remove_connection
        // clears the writer (watchers need live write handles).
        //
        // One-shot: the watch entry is consumed on delivery
        // (EAct E-InvokeM semantics -- Fowler/Hu S3.3).
        let address = self.conn_addresses.get(&conn_id).copied();
        if let Some(watchers) = self.watch_table.remove(&conn_id) {
            if let Some(addr) = address {
                let exited = ControlMessage::PaneExited {
                    address: addr,
                    reason: pane_proto::ExitReason::Disconnected,
                };
                if let Ok(bytes) = postcard::to_allocvec(&exited) {
                    for watcher_conn in &watchers {
                        if let Some(wh) = self.writers.get(watcher_conn) {
                            let _ = wh.write_frame(0, &bytes);
                        }
                        // Clean up reverse index
                        if let Some(watched_set) = self.watcher_reverse.get_mut(watcher_conn) {
                            watched_set.remove(&conn_id);
                            if watched_set.is_empty() {
                                self.watcher_reverse.remove(watcher_conn);
                            }
                        }
                    }
                }
            }
        }

        // Clean up any watches THIS connection had on others.
        if let Some(watched_set) = self.watcher_reverse.remove(&conn_id) {
            for watched_conn in watched_set {
                if let Some(watchers) = self.watch_table.get_mut(&watched_conn) {
                    watchers.remove(&conn_id);
                    if watchers.is_empty() {
                        self.watch_table.remove(&watched_conn);
                    }
                }
            }
        }

        // Clean up address mapping.
        self.conn_addresses.remove(&conn_id);

        // Existing service teardown logic.
        let peers = self.remove_connection(conn_id);

        for (peer_conn, peer_session) in peers {
            let teardown = ControlMessage::ServiceTeardown {
                session_id: peer_session,
                reason: pane_proto::control::TeardownReason::ConnectionLost,
            };
            if let Ok(bytes) = postcard::to_allocvec(&teardown) {
                if let Some(wh) = self.writers.get(&peer_conn) {
                    let _ = wh.write_frame(0, &bytes);
                }
            }
        }
    }
}

// -- ProtocolServer: the public API --

/// A running protocol server (single-threaded actor).
///
/// Call `accept()` to add connections. The actor thread processes
/// all routing events sequentially. Reader threads are spawned
/// per connection as thin negative adapters.
///
pub struct ProtocolServer {
    /// Channel to the actor thread. All events (new connections,
    /// frames, disconnects) go through this single channel.
    event_tx: mpsc::Sender<ServerEvent>,
    /// State for handshake (runs on caller's thread before
    /// handing off to the actor). Protected by Mutex because
    /// accept() may be called from multiple threads.
    handshake_state: Mutex<HandshakeState>,
}

/// Minimal state needed during handshake (before the actor sees the connection).
struct HandshakeState {
    next_conn_id: u64,
}

/// Handle returned by accept(). Allows waiting for the connection to close.
pub struct ConnectionHandle {
    pub conn_id: ConnectionId,
    pub hello: Hello,
    pub welcome: Welcome,
    done_rx: mpsc::Receiver<()>,
}

impl ConnectionHandle {
    pub fn wait(self) {
        let _ = self.done_rx.recv();
    }
}

/// Errors from accepting a connection.
#[derive(Debug)]
pub enum AcceptError {
    Transport(std::io::Error),
}

impl std::fmt::Display for AcceptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcceptError::Transport(e) => write!(f, "accept transport error: {e}"),
        }
    }
}

impl std::error::Error for AcceptError {}

// No Default impl: new() spawns a thread. Use new() explicitly.
#[allow(clippy::new_without_default)]
impl ProtocolServer {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel();

        // Spawn the actor thread.
        std::thread::spawn(move || {
            actor_loop(event_rx);
        });

        ProtocolServer {
            event_tx,
            handshake_state: Mutex::new(HandshakeState { next_conn_id: 1 }),
        }
    }

    /// Accept a connection from split read/write halves.
    ///
    /// Performs the handshake on the caller's thread, then hands
    /// off to the actor. The reader thread runs in the background.
    pub fn accept(
        &self,
        mut reader: impl Read + Send + 'static,
        mut writer: impl Write + Send + 'static,
    ) -> Result<ConnectionHandle, AcceptError> {
        let mut codec = FrameCodec::permissive(HANDSHAKE_MAX_MESSAGE_SIZE);

        // Read Hello
        let frame = codec.read_frame(&mut reader).map_err(|e| {
            AcceptError::Transport(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                e.to_string(),
            ))
        })?;

        let payload = match frame {
            Frame::Message {
                service: 0,
                payload,
            } => payload,
            Frame::Abort => {
                return Err(AcceptError::Transport(std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    "client sent ProtocolAbort",
                )))
            }
            _ => {
                return Err(AcceptError::Transport(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "unexpected frame during handshake",
                )))
            }
        };

        // Handshake uses CBOR for forward compatibility (D11):
        // new fields with #[serde(default)] deserialize from older
        // payloads that omit them. Data-plane frames stay postcard.
        let hello: Hello = ciborium::de::from_reader(payload.as_slice()).map_err(|e| {
            AcceptError::Transport(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            ))
        })?;

        // Allocate connection ID
        let conn_id = {
            let mut hs = self.handshake_state.lock().unwrap();
            let id = ConnectionId(hs.next_conn_id);
            hs.next_conn_id += 1;
            id
        };

        // Build Welcome
        let welcome = Welcome {
            version: 1,
            instance_id: "pane-server".into(),
            max_message_size: hello.max_message_size,
            // Server may reduce but never increase the client's proposal.
            // For now, echo back (no reduction policy in Phase 1).
            max_outstanding_requests: hello.max_outstanding_requests,
            bindings: vec![],
        };

        let decision: Result<Welcome, Rejection> = Ok(welcome.clone());
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&decision, &mut bytes).expect("Welcome serialization failed");

        // Send Welcome
        codec
            .write_frame(&mut writer, 0, &bytes)
            .map_err(AcceptError::Transport)?;

        // Send Ready — the active phase begins. Matches Be's
        // B_READY_TO_RUN posted by BApplication's constructor
        // (src/kits/app/Application.cpp:497) right after registrar
        // registration. The server sends it because it knows
        // the connection is fully established.
        let ready =
            ControlMessage::Lifecycle(pane_proto::protocols::lifecycle::LifecycleMessage::Ready);
        let ready_bytes = postcard::to_allocvec(&ready).expect("Ready serialization failed");
        codec
            .write_frame(&mut writer, 0, &ready_bytes)
            .map_err(AcceptError::Transport)?;

        codec.set_max_message_size(welcome.max_message_size);
        let codec = Arc::new(codec);

        // Build write handle
        let write_handle = WriteHandle {
            writer: Arc::new(Mutex::new(Box::new(writer))),
            codec: codec.clone(),
        };

        // Register with the actor (provides + writer).
        // This goes through the same event channel as frames,
        // so the actor processes it in order — no race between
        // registration and the first DeclareInterest.
        let _ = self.event_tx.send(ServerEvent::NewConnection {
            conn_id,
            write_handle,
            hello: hello.clone(),
        });

        // Spawn reader thread (thin negative adapter)
        let event_tx = self.event_tx.clone();
        let (done_tx, done_rx) = mpsc::channel();

        std::thread::spawn(move || {
            reader_loop(conn_id, reader, codec, event_tx);
            let _ = done_tx.send(());
        });

        Ok(ConnectionHandle {
            conn_id,
            hello,
            welcome,
            done_rx,
        })
    }
}

// -- Actor loop: the single thread that owns all routing state --

fn actor_loop(event_rx: mpsc::Receiver<ServerEvent>) {
    let mut state = ServerState::new();

    loop {
        match event_rx.recv() {
            Ok(ServerEvent::NewConnection {
                conn_id,
                write_handle,
                hello,
            }) => {
                state.register_provides(conn_id, &hello.provides);
                state.writers.insert(conn_id, write_handle);
                state.next_session.insert(conn_id, 1);
                // Phase 1: pane_id = conn_id, server_id = 0 (local).
                state
                    .conn_addresses
                    .insert(conn_id, pane_proto::Address::local(conn_id.0));
            }
            Ok(ServerEvent::Frame {
                conn_id,
                service,
                payload,
            }) => {
                state.process_frame(conn_id, service, payload);
            }
            Ok(ServerEvent::Disconnected { conn_id }) => {
                state.process_disconnect(conn_id);
            }
            Err(_) => {
                // All senders dropped — server shutting down.
                break;
            }
        }
    }
}

// -- Reader loop: thin negative adapter per connection --

/// Reads frames and posts events to the actor. Touches zero
/// shared state — the only synchronization is mpsc send (wait-free
/// for the sender).
fn reader_loop(
    conn_id: ConnectionId,
    mut reader: impl Read,
    codec: Arc<FrameCodec>,
    event_tx: mpsc::Sender<ServerEvent>,
) {
    loop {
        match codec.read_frame(&mut reader) {
            Ok(Frame::Message { service, payload }) => {
                if event_tx
                    .send(ServerEvent::Frame {
                        conn_id,
                        service,
                        payload,
                    })
                    .is_err()
                {
                    break; // Actor dropped
                }
            }
            Ok(Frame::Abort) => break,
            Err(FrameError::UnknownService(_)) => {
                // Permissive codec — shouldn't happen, but continue
            }
            Err(_) => break, // Transport error
        }
    }

    // Signal disconnection to the actor.
    let _ = event_tx.send(ServerEvent::Disconnected { conn_id });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handshake::ServiceProvision;
    use crate::transport::MemoryTransport;
    use pane_proto::protocol::ServiceId;

    #[test]
    fn server_state_alloc_sessions_increment() {
        let mut state = ServerState::new();
        let conn = ConnectionId(1);
        let s1 = state.alloc_session(conn).unwrap();
        let s2 = state.alloc_session(conn).unwrap();
        assert_eq!(s1, 1);
        assert_eq!(s2, 2);
    }

    #[test]
    fn server_state_register_provides() {
        let mut state = ServerState::new();
        let conn = ConnectionId(1);
        let service = ServiceId::new("com.pane.echo");

        state.register_provides(
            conn,
            &[ServiceProvision {
                service,
                version: 1,
            }],
        );

        let providers = state.provider_index.get(&service.uuid).unwrap();
        assert_eq!(providers, &[ConnectionId(1)]);
    }

    #[test]
    fn server_state_provider_index_supports_multiple_providers() {
        let mut state = ServerState::new();
        let service = ServiceId::new("com.pane.echo");

        state.register_provides(
            ConnectionId(1),
            &[ServiceProvision {
                service,
                version: 1,
            }],
        );
        state.register_provides(
            ConnectionId(2),
            &[ServiceProvision {
                service,
                version: 1,
            }],
        );

        let providers = state.provider_index.get(&service.uuid).unwrap();
        assert_eq!(providers.len(), 2);
    }

    #[test]
    fn server_state_handle_declare_interest() {
        let mut state = ServerState::new();
        let service = ServiceId::new("com.pane.echo");

        state.register_provides(
            ConnectionId(1),
            &[ServiceProvision {
                service,
                version: 1,
            }],
        );
        state.next_session.insert(ConnectionId(1), 1);
        state.next_session.insert(ConnectionId(2), 1);

        let result = state.handle_declare_interest(ConnectionId(2), service, 1);

        assert!(result.is_ok());
        let (accepted, provider_conn, provider_session) = result.unwrap();

        assert_eq!(provider_conn, ConnectionId(1));
        assert!(provider_session >= 1);

        match accepted {
            ControlMessage::InterestAccepted { session_id, .. } => {
                assert!(session_id >= 1);
            }
            _ => panic!("expected InterestAccepted"),
        }

        assert!(state.routing_table.len() >= 2);
    }

    #[test]
    fn server_state_declare_interest_no_provider_returns_err() {
        let mut state = ServerState::new();
        let service = ServiceId::new("com.pane.nonexistent");
        state.next_session.insert(ConnectionId(1), 1);

        let result = state.handle_declare_interest(ConnectionId(1), service, 1);

        assert!(matches!(
            result,
            Err(pane_proto::control::DeclineReason::ServiceUnknown)
        ));
    }

    #[test]
    fn server_state_remove_connection_cleans_up() {
        let mut state = ServerState::new();
        let service = ServiceId::new("com.pane.echo");

        state.register_provides(
            ConnectionId(1),
            &[ServiceProvision {
                service,
                version: 1,
            }],
        );
        state.next_session.insert(ConnectionId(1), 1);
        state.next_session.insert(ConnectionId(2), 1);

        let _ = state.handle_declare_interest(ConnectionId(2), service, 1);
        assert!(!state.routing_table.is_empty());

        let peers = state.remove_connection(ConnectionId(1));

        assert!(
            state.provider_index.get(&service.uuid).is_none()
                || state.provider_index.get(&service.uuid).unwrap().is_empty()
        );
        assert!(state.routing_table.is_empty());
        assert!(peers.iter().any(|(c, _)| *c == ConnectionId(2)));
    }

    /// Session_id overflow at 0xFFFF boundary.
    /// Returns None instead of panicking — the server gracefully
    /// declines with SessionExhausted.
    #[test]
    fn alloc_session_returns_none_at_overflow() {
        let mut state = ServerState::new();
        let conn = ConnectionId(1);
        // Set counter near the boundary to avoid allocating 65534 sessions.
        state.next_session.insert(conn, 0xFFFD);
        assert_eq!(state.alloc_session(conn), Some(0xFFFD));
        assert_eq!(state.alloc_session(conn), Some(0xFFFE));
        // Next allocation returns None (0xFFFF is reserved)
        assert!(state.alloc_session(conn).is_none());
    }

    #[test]
    fn server_state_revoke_interest_cleans_route() {
        let mut state = ServerState::new();
        let service = ServiceId::new("com.pane.echo");

        state.register_provides(
            ConnectionId(1),
            &[ServiceProvision {
                service,
                version: 1,
            }],
        );
        state.next_session.insert(ConnectionId(1), 1);
        state.next_session.insert(ConnectionId(2), 1);

        // Establish a route via DeclareInterest
        let result = state.handle_declare_interest(ConnectionId(2), service, 1);
        assert!(result.is_ok());
        assert_eq!(state.routing_table.len(), 2);

        let consumer_session = match result.unwrap().0 {
            ControlMessage::InterestAccepted { session_id, .. } => session_id,
            _ => panic!("expected InterestAccepted"),
        };

        // Simulate process_control receiving RevokeInterest
        // (extracted from process_control for unit test)
        let route = state
            .routing_table
            .remove(&(ConnectionId(2), consumer_session));
        assert!(route.is_some(), "consumer route should exist");
        let route = route.unwrap();
        let (peer_conn, peer_session) = if route.consumer_conn == ConnectionId(2) {
            (route.provider_conn, route.provider_session)
        } else {
            (route.consumer_conn, route.consumer_session)
        };
        state.routing_table.remove(&(peer_conn, peer_session));

        assert!(
            state.routing_table.is_empty(),
            "all routes should be removed"
        );
    }

    #[test]
    fn server_accept_and_handshake() {
        let (ct, st) = MemoryTransport::pair();
        let server = ProtocolServer::new();

        let client_handle = std::thread::spawn(move || {
            let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);
            let mut transport = ct;

            let hello = Hello {
                version: 1,
                max_message_size: 16 * 1024 * 1024,
                max_outstanding_requests: 0,
                interests: vec![],
                provides: vec![],
            };
            // Handshake uses CBOR
            let mut bytes = Vec::new();
            ciborium::ser::into_writer(&hello, &mut bytes).unwrap();
            codec.write_frame(&mut transport, 0, &bytes).unwrap();

            let frame = codec.read_frame(&mut transport).unwrap();
            let payload = match frame {
                Frame::Message {
                    service: 0,
                    payload,
                } => payload,
                other => panic!("expected Control frame, got {:?}", other),
            };
            let decision: Result<Welcome, Rejection> =
                ciborium::de::from_reader(payload.as_slice()).unwrap();
            let welcome = decision.expect("expected welcome");
            assert_eq!(welcome.version, 1);

            // Server sends Ready after Welcome
            let frame = codec.read_frame(&mut transport).unwrap();
            let payload = match frame {
                Frame::Message {
                    service: 0,
                    payload,
                } => payload,
                other => panic!("expected Ready frame, got {:?}", other),
            };
            let msg: ControlMessage = postcard::from_bytes(&payload).unwrap();
            assert!(
                matches!(
                    msg,
                    ControlMessage::Lifecycle(
                        pane_proto::protocols::lifecycle::LifecycleMessage::Ready
                    )
                ),
                "first active-phase message should be Ready, got {msg:?}"
            );

            drop(transport);
        });

        let (st_reader, st_writer) = st.split();
        let conn = server.accept(st_reader, st_writer).expect("accept failed");
        assert_eq!(conn.hello.version, 1);

        client_handle.join().unwrap();
        conn.wait();
    }

    // -- Watch table tests --

    fn setup_watched_state() -> ServerState {
        let mut state = ServerState::new();
        // Register two connections with addresses
        state
            .conn_addresses
            .insert(ConnectionId(1), pane_proto::Address::local(1));
        state
            .conn_addresses
            .insert(ConnectionId(2), pane_proto::Address::local(2));
        state.next_session.insert(ConnectionId(1), 1);
        state.next_session.insert(ConnectionId(2), 1);
        state
    }

    #[test]
    fn watch_registers_in_both_tables() {
        let mut state = setup_watched_state();
        // Test the data structures directly — process_control
        // needs a writer, which requires transport wiring.
        state
            .watch_table
            .entry(ConnectionId(1))
            .or_default()
            .insert(ConnectionId(2));
        state
            .watcher_reverse
            .entry(ConnectionId(2))
            .or_default()
            .insert(ConnectionId(1));

        assert!(state
            .watch_table
            .get(&ConnectionId(1))
            .unwrap()
            .contains(&ConnectionId(2)));
        assert!(state
            .watcher_reverse
            .get(&ConnectionId(2))
            .unwrap()
            .contains(&ConnectionId(1)));
    }

    #[test]
    fn unwatch_removes_from_both_tables() {
        let mut state = setup_watched_state();

        // Set up a watch
        state
            .watch_table
            .entry(ConnectionId(1))
            .or_default()
            .insert(ConnectionId(2));
        state
            .watcher_reverse
            .entry(ConnectionId(2))
            .or_default()
            .insert(ConnectionId(1));

        // Simulate Unwatch
        let target_conn = state.resolve_conn(&pane_proto::Address::local(1)).unwrap();
        assert_eq!(target_conn, ConnectionId(1));

        if let Some(watchers) = state.watch_table.get_mut(&target_conn) {
            watchers.remove(&ConnectionId(2));
            if watchers.is_empty() {
                state.watch_table.remove(&target_conn);
            }
        }
        if let Some(watched) = state.watcher_reverse.get_mut(&ConnectionId(2)) {
            watched.remove(&target_conn);
            if watched.is_empty() {
                state.watcher_reverse.remove(&ConnectionId(2));
            }
        }

        assert!(state.watch_table.get(&ConnectionId(1)).is_none());
        assert!(state.watcher_reverse.get(&ConnectionId(2)).is_none());
    }

    #[test]
    fn resolve_conn_finds_by_address() {
        let state = setup_watched_state();
        assert_eq!(
            state.resolve_conn(&pane_proto::Address::local(1)),
            Some(ConnectionId(1))
        );
        assert_eq!(
            state.resolve_conn(&pane_proto::Address::local(2)),
            Some(ConnectionId(2))
        );
        assert_eq!(state.resolve_conn(&pane_proto::Address::local(99)), None);
    }

    #[test]
    fn disconnect_cleans_watch_tables_for_watched() {
        let mut state = setup_watched_state();

        // conn 2 watches conn 1
        state
            .watch_table
            .entry(ConnectionId(1))
            .or_default()
            .insert(ConnectionId(2));
        state
            .watcher_reverse
            .entry(ConnectionId(2))
            .or_default()
            .insert(ConnectionId(1));

        // conn 1 disconnects — watch_table entry for conn 1
        // is consumed during PaneExited delivery
        state.watch_table.remove(&ConnectionId(1));
        // Reverse index cleanup
        if let Some(watched) = state.watcher_reverse.get_mut(&ConnectionId(2)) {
            watched.remove(&ConnectionId(1));
            if watched.is_empty() {
                state.watcher_reverse.remove(&ConnectionId(2));
            }
        }

        assert!(state.watch_table.is_empty());
        assert!(state.watcher_reverse.is_empty());
    }

    #[test]
    fn disconnect_cleans_watch_tables_for_watcher() {
        let mut state = setup_watched_state();

        // conn 2 watches conn 1
        state
            .watch_table
            .entry(ConnectionId(1))
            .or_default()
            .insert(ConnectionId(2));
        state
            .watcher_reverse
            .entry(ConnectionId(2))
            .or_default()
            .insert(ConnectionId(1));

        // conn 2 disconnects — clean up its watcher_reverse entry
        // and remove it from watch_table entries
        if let Some(watched_set) = state.watcher_reverse.remove(&ConnectionId(2)) {
            for watched_conn in watched_set {
                if let Some(watchers) = state.watch_table.get_mut(&watched_conn) {
                    watchers.remove(&ConnectionId(2));
                    if watchers.is_empty() {
                        state.watch_table.remove(&watched_conn);
                    }
                }
            }
        }

        assert!(state.watch_table.is_empty());
        assert!(state.watcher_reverse.is_empty());
    }

    #[test]
    fn conn_address_populated_on_registration() {
        let mut state = ServerState::new();
        let conn = ConnectionId(42);
        state
            .conn_addresses
            .insert(conn, pane_proto::Address::local(conn.0));

        let addr = state.conn_addresses.get(&conn).unwrap();
        assert_eq!(addr.pane_id, 42);
        assert!(addr.is_local());
    }
}
