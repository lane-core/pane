//! The application entry point.

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}};
use std::thread;

use pane_proto::message::PaneId;
use pane_proto::protocol::{ClientToComp, CompToClient, CreatePaneTag};

use crate::connection::Connection;
use crate::error::{ConnectError, PaneError, Result};
use crate::looper_message::LooperMessage;
use crate::pane::Pane;
use crate::tag::Tag;

/// The application entry point. One per process.
///
/// App establishes the connection to the compositor and provides
/// the factory for creating panes. It does not run a message loop
/// itself — each pane gets its own loop via [`Pane::run`] or
/// [`Pane::run_with`].
///
/// You typically create one App at the start of main(), create your
/// panes, then call [`run`](App::run) to block until all panes
/// are closed.
///
/// # Threading
///
/// App itself is not `Send` — it lives on the thread that created
/// it. A background dispatcher thread routes compositor messages to
/// the correct pane channel.
///
/// # BeOS
///
/// Descends from `BApplication` (see also Haiku's
/// [BApplication documentation](reference/haiku-book/app/BApplication.dox)).
/// Key changes:
/// - Not a looper. Be's `BApplication` inherited `BLooper` and ran
///   the main message loop; pane's App is a connection and factory.
///   Per-pane loops replace the application-level loop.
/// - No `be_app` global. App is a held value, not a static.
/// - `run()` blocks until all panes close, rather than running a
///   message dispatch loop.
pub struct App {
    /// Sender to the compositor (active-phase messages).
    comp_tx: mpsc::Sender<ClientToComp>,
    /// Per-pane channels, keyed by PaneId. The dispatcher thread
    /// forwards CompToClient messages (wrapped as LooperMessage) to
    /// the right pane's channel.
    pane_channels: Arc<Mutex<HashMap<PaneId, mpsc::SyncSender<LooperMessage>>>>,
    /// Oneshot channels for pending create_pane responses.
    pending_creates: Arc<Mutex<VecDeque<mpsc::Sender<CompToClient>>>>,
    /// Application signature.
    signature: String,
    /// Live pane count.
    pane_count: Arc<AtomicUsize>,
    /// Signal for wait() — notified when pane_count reaches 0.
    done_signal: Arc<(Mutex<()>, std::sync::Condvar)>,
    /// Dispatcher thread handle.
    _dispatcher: thread::JoinHandle<()>,
}

impl App {
    /// Connect to the compositor via unix socket.
    ///
    /// Looks for the compositor socket at `$XDG_RUNTIME_DIR/pane/compositor.sock`.
    /// Runs the session-typed handshake, then enters the active phase.
    pub fn connect(signature: &str) -> std::result::Result<Self, ConnectError> {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .map_err(|_| ConnectError::NotRunning)?;
        let sock_path = std::path::PathBuf::from(runtime_dir).join("pane/compositor.sock");

        let stream = std::os::unix::net::UnixStream::connect(&sock_path)
            .map_err(|_| ConnectError::NotRunning)?;

        let transport = pane_session::transport::unix::UnixTransport::from_stream(stream);
        let chan = pane_session::types::Chan::new(transport);

        let hs = crate::connection::run_client_handshake(chan, signature)
            .map_err(|e| match e {
                crate::error::Error::Connect(ce) => ce,
                other => ConnectError::Transport(
                    pane_session::SessionError::Io(
                        std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", other)),
                    ),
                ),
            })?;

        // Handshake complete — reuse the same socket for active phase.
        // finish() reclaimed the transport; into_stream() gives the raw UnixStream.
        let active_stream = hs.transport.into_stream();
        let conn = crate::connection::from_unix_stream(active_stream)
            .map_err(|e| ConnectError::Transport(pane_session::SessionError::Io(e)))?;

        Self::connect_test(signature, conn)
    }

    /// Connect using a test connection (for MockCompositor).
    #[doc(hidden)]
    pub fn connect_test(signature: &str, conn: Connection) -> std::result::Result<Self, ConnectError> {
        let comp_tx = conn.sender;
        let pane_channels: Arc<Mutex<HashMap<PaneId, mpsc::SyncSender<LooperMessage>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_creates: Arc<Mutex<VecDeque<mpsc::Sender<CompToClient>>>> =
            Arc::new(Mutex::new(VecDeque::new()));

        // Spawn the dispatcher thread — reads from compositor, routes to panes
        let channels = pane_channels.clone();
        let creates = pending_creates.clone();
        let receiver = conn.receiver;

        let dispatcher = thread::spawn(move || {
            loop {
                let msg = match receiver.recv() {
                    Ok(msg) => msg,
                    Err(_) => break, // connection closed
                };

                // Check if this is a PaneCreated response to a pending create
                if let CompToClient::PaneCreated { .. } = &msg {
                    let mut pending = creates.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(oneshot) = pending.pop_front() {
                        let _ = oneshot.send(msg);
                        continue;
                    }
                }

                // Route to the correct pane's channel (wrap as LooperMessage)
                let id = msg.pane_id();
                let channels = channels.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(tx) = channels.get(&id) {
                    let _ = tx.send(LooperMessage::FromComp(msg));
                }
            }
        });

        Ok(App {
            comp_tx,
            pane_channels,
            pending_creates,
            signature: signature.to_string(),
            pane_count: Arc::new(AtomicUsize::new(0)),
            done_signal: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
            _dispatcher: dispatcher,
        })
    }

    /// Create a new pane with the given tag configuration.
    ///
    /// Blocks until the compositor responds with PaneCreated or
    /// times out after 5 seconds.
    pub fn create_pane(&self, tag: Tag) -> Result<Pane> {
        let wire_tag = Some(tag.into_wire());
        self.create_pane_inner(wire_tag)
    }

    /// Create a component pane with no tag line.
    ///
    /// Component panes have no title or command surface — they are
    /// building blocks meant to be interacted with through their
    /// parent pane's tag or their own content area.
    pub fn create_component_pane(&self) -> Result<Pane> {
        self.create_pane_inner(None)
    }

    fn create_pane_inner(&self, wire_tag: Option<CreatePaneTag>) -> Result<Pane> {

        // Set up oneshot for the PaneCreated response
        let (create_tx, create_rx) = mpsc::channel();
        self.pending_creates.lock().unwrap_or_else(|e| e.into_inner()).push_back(create_tx);

        // Send CreatePane to compositor
        self.comp_tx.send(ClientToComp::CreatePane { tag: wire_tag })
            .map_err(|_| PaneError::Disconnected)?;

        // Wait for PaneCreated (with timeout)
        let response = create_rx.recv_timeout(std::time::Duration::from_secs(5))
            .map_err(|_| PaneError::Timeout)?;

        let (pane_id, geometry) = match response {
            CompToClient::PaneCreated { pane, geometry } => (pane, geometry),
            _ => return Err(PaneError::Refused.into()),
        };

        // Register the per-pane channel IMMEDIATELY after receiving PaneCreated.
        // There is a small race window between the dispatcher forwarding PaneCreated
        // and this registration — events arriving in that window are dropped.
        // The proper fix (Stage 3) is to have the dispatcher pre-register the channel
        // or buffer events for unknown pane IDs. For now, the window is microseconds
        // and only affects events sent by the compositor in the same batch as PaneCreated.
        // Bounded channel: backpressure prevents unbounded memory growth
        // from a chatty compositor or rapid self-delivery. 256 is generous —
        // the looper drains and coalesces on each wakeup, so the queue
        // should rarely exceed single digits under normal operation.
        let (pane_tx, pane_rx) = mpsc::sync_channel::<LooperMessage>(256);
        self.pane_channels.lock().unwrap_or_else(|e| e.into_inner()).insert(pane_id, pane_tx.clone());
        self.pane_count.fetch_add(1, Ordering::Relaxed);

        Ok(Pane::new(
            pane_id,
            geometry,
            pane_rx,
            self.comp_tx.clone(),
            pane_tx,
            self.pane_count.clone(),
            self.done_signal.clone(),
        ))
    }

    /// Wait until all panes are closed.
    pub fn run(&self) {
        let (lock, cvar) = &*self.done_signal;
        let guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        let _guard = cvar.wait_while(guard, |_| {
            self.pane_count.load(Ordering::Relaxed) > 0
        }).unwrap();
    }
}


impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("signature", &self.signature)
            .field("pane_count", &self.pane_count.load(Ordering::Relaxed))
            .finish()
    }
}
