//! The application entry point.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}};
use std::thread;

use pane_proto::message::PaneId;
use pane_proto::protocol::{ClientToComp, CompToClient, CreatePaneTag};

use crate::connection::Connection;
use crate::error::{ConnectError, PaneError, Result};
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
    /// forwards CompToClient messages to the right pane's channel.
    pane_channels: Arc<Mutex<HashMap<u64, mpsc::Sender<CompToClient>>>>,
    /// Oneshot channels for pending create_pane responses.
    pending_creates: Arc<Mutex<Vec<mpsc::Sender<CompToClient>>>>,
    /// Application signature.
    signature: String,
    /// Live pane count.
    pane_count: Arc<AtomicUsize>,
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
        let pane_channels: Arc<Mutex<HashMap<u64, mpsc::Sender<CompToClient>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_creates: Arc<Mutex<Vec<mpsc::Sender<CompToClient>>>> =
            Arc::new(Mutex::new(Vec::new()));

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
                    if let Some(oneshot) = pending.pop() {
                        let _ = oneshot.send(msg);
                        continue;
                    }
                }

                // Route to the correct pane's channel
                let pane_id = extract_pane_id(&msg);
                if let Some(id) = pane_id {
                    let channels = channels.lock().unwrap();
                    if let Some(tx) = channels.get(&id) {
                        let _ = tx.send(msg);
                    }
                }
            }
        });

        Ok(App {
            comp_tx,
            pane_channels,
            pending_creates,
            signature: signature.to_string(),
            pane_count: Arc::new(AtomicUsize::new(0)),
            _dispatcher: dispatcher,
        })
    }

    /// Create a new pane with the given tag configuration.
    ///
    /// Blocks until the compositor responds with PaneCreated or
    /// times out after 5 seconds.
    pub fn create_pane(&self, tag: impl Into<Option<Tag>>) -> Result<Pane> {
        let tag = tag.into();
        let wire_tag = tag.map(|t| t.into_wire());

        // Set up oneshot for the PaneCreated response
        let (create_tx, create_rx) = mpsc::channel();
        self.pending_creates.lock().unwrap().push(create_tx);

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

        // Create the per-pane channel and register it
        let (pane_tx, pane_rx) = mpsc::channel();
        self.pane_channels.lock().unwrap().insert(pane_id.get() as u64, pane_tx);
        self.pane_count.fetch_add(1, Ordering::Relaxed);

        Ok(Pane::new(
            pane_id,
            geometry,
            pane_rx,
            self.comp_tx.clone(),
            self.pane_count.clone(),
        ))
    }

    /// Wait until all panes are closed.
    pub fn wait(&self) {
        while self.pane_count.load(Ordering::Relaxed) > 0 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}

/// Extract the PaneId from a CompToClient message.
fn extract_pane_id(msg: &CompToClient) -> Option<u64> {
    match msg {
        CompToClient::PaneCreated { pane, .. } => Some(pane.get() as u64),
        CompToClient::Resize { pane, .. } => Some(pane.get() as u64),
        CompToClient::Focus { pane } => Some(pane.get() as u64),
        CompToClient::Blur { pane } => Some(pane.get() as u64),
        CompToClient::Key { pane, .. } => Some(pane.get() as u64),
        CompToClient::Mouse { pane, .. } => Some(pane.get() as u64),
        CompToClient::Close { pane } => Some(pane.get() as u64),
        CompToClient::CloseAck { pane } => Some(pane.get() as u64),
        CompToClient::CommandActivated { pane } => Some(pane.get() as u64),
        CompToClient::CommandDismissed { pane } => Some(pane.get() as u64),
        CompToClient::CommandExecuted { pane, .. } => Some(pane.get() as u64),
        CompToClient::CompletionRequest { pane, .. } => Some(pane.get() as u64),
    }
}
