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
/// Connects to the compositor, registers with the roster, and provides
/// the factory for creating panes. Each pane gets its own thread.
pub struct App {
    /// Sender to the compositor (active-phase messages).
    comp_tx: mpsc::Sender<ClientToComp>,
    /// Per-pane channels, keyed by PaneId. The dispatcher thread
    /// forwards CompToClient messages (wrapped as LooperMessage) to
    /// the right pane's channel.
    pane_channels: Arc<Mutex<HashMap<PaneId, mpsc::Sender<LooperMessage>>>>,
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
    /// Connect to the compositor.
    /// Currently returns NotRunning — no compositor exists yet (Phase 4).
    pub fn connect(_signature: &str) -> std::result::Result<Self, ConnectError> {
        Err(ConnectError::NotRunning)
    }

    /// Connect using a test connection (for MockCompositor).
    pub fn connect_test(signature: &str, conn: Connection) -> std::result::Result<Self, ConnectError> {
        let comp_tx = conn.sender;
        let pane_channels: Arc<Mutex<HashMap<PaneId, mpsc::Sender<LooperMessage>>>> =
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
                    let mut pending = creates.lock().unwrap();
                    if let Some(oneshot) = pending.pop_front() {
                        let _ = oneshot.send(msg);
                        continue;
                    }
                }

                // Route to the correct pane's channel (wrap as LooperMessage)
                let id = msg.pane_id();
                let channels = channels.lock().unwrap();
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
        self.pending_creates.lock().unwrap().push_back(create_tx);

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
        let (pane_tx, pane_rx) = mpsc::channel::<LooperMessage>();
        self.pane_channels.lock().unwrap().insert(pane_id, pane_tx.clone());
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
    pub fn wait(&self) {
        let (lock, cvar) = &*self.done_signal;
        let guard = lock.lock().unwrap();
        let _guard = cvar.wait_while(guard, |_| {
            self.pane_count.load(Ordering::Relaxed) > 0
        }).unwrap();
    }
}

