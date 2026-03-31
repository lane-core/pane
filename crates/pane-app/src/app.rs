//! The application entry point.

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}};
use std::thread;
use std::time::Duration;

use pane_proto::message::PaneId;
use pane_proto::protocol::{ClientToComp, CompToClient, CreatePaneTag};

use crate::connection::Connection;
use crate::error::{ConnectError, PaneError, Result};
use crate::looper_message::LooperMessage;
use crate::pane::Pane;
use crate::proxy::LooperSender;
use crate::tag::Tag;

/// Response from the dispatcher to create_pane_inner.
/// The dispatcher pre-registers the channel so there is no race
/// between PaneCreated dispatch and channel availability.
struct CreateResponse {
    msg: CompToClient,
    pane_rx: calloop::channel::Channel<LooperMessage>,
    pane_tx: LooperSender,
}

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
    /// Oneshot channels for pending create_pane responses, keyed by
    /// the client-proposed PaneId. UUID-keyed so PaneRefused can
    /// correlate back to the correct caller.
    pending_creates: Arc<Mutex<HashMap<PaneId, mpsc::Sender<CreateResponse>>>>,
    /// Live pane looper channels — shared with the dispatcher for
    /// message routing. Used by request_quit() to send QuitRequested
    /// to all panes.
    pane_channels: Arc<Mutex<HashMap<PaneId, LooperSender>>>,
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

        let hs = crate::connection::run_client_handshake(chan, signature, None)
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

    /// Connect to a remote pane server over TCP.
    ///
    /// Runs the session-typed handshake with identity forwarding,
    /// then enters the active phase. The connection is indistinguishable
    /// from a local connection at the kit API level — Pane, Messenger,
    /// Handler all work identically.
    ///
    /// # Plan 9
    ///
    /// This is the `import` equivalent — connecting to a remote pane
    /// server and using it as if it were local. The local machine has
    /// no architectural privilege; a remote server is just another
    /// server with higher latency.
    pub fn connect_remote(
        signature: &str,
        addr: impl std::net::ToSocketAddrs,
    ) -> std::result::Result<Self, ConnectError> {
        let stream = std::net::TcpStream::connect(addr)
            .map_err(|e| ConnectError::Transport(
                pane_session::SessionError::Io(e),
            ))?;

        let transport = pane_session::transport::tcp::TcpTransport::from_stream(stream);
        let chan = pane_session::types::Chan::new(transport);

        let identity = Some(crate::identity::local_identity());
        let hs = crate::connection::run_client_handshake(chan, signature, identity)
            .map_err(|e| match e {
                crate::error::Error::Connect(ce) => ce,
                other => ConnectError::Transport(
                    pane_session::SessionError::Io(
                        std::io::Error::new(std::io::ErrorKind::Other, format!("{:?}", other)),
                    ),
                ),
            })?;

        let active_stream = hs.transport.into_stream();
        let conn = crate::connection::from_tcp_stream(active_stream)
            .map_err(|e| ConnectError::Transport(pane_session::SessionError::Io(e)))?;

        Self::connect_test(signature, conn)
    }

    /// Connect using a test connection (for MockCompositor).
    #[doc(hidden)]
    pub fn connect_test(signature: &str, conn: Connection) -> std::result::Result<Self, ConnectError> {
        let comp_tx = conn.sender;
        let pane_channels: Arc<Mutex<HashMap<PaneId, LooperSender>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_creates: Arc<Mutex<HashMap<PaneId, mpsc::Sender<CreateResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Spawn the dispatcher thread — reads from compositor, routes to panes.
        // The dispatcher pre-registers per-pane channels when it sees PaneCreated,
        // eliminating the race between response delivery and channel availability.
        let channels = pane_channels.clone();
        let creates = pending_creates.clone();
        let receiver = conn.receiver;

        let dispatcher = thread::spawn(move || {
            loop {
                let msg = match receiver.recv() {
                    Ok(msg) => msg,
                    Err(_) => break, // connection closed
                };

                // PaneCreated / PaneRefused: correlate by PaneId, notify caller
                match &msg {
                    CompToClient::PaneCreated { pane, .. } => {
                        let mut pending = creates.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(oneshot) = pending.remove(pane) {
                            let (pane_tx, pane_rx) = calloop::channel::channel::<LooperMessage>();
                            channels.lock().unwrap_or_else(|e| e.into_inner())
                                .insert(*pane, pane_tx.clone());
                            let _ = oneshot.send(CreateResponse {
                                msg,
                                pane_rx,
                                pane_tx,
                            });
                            continue;
                        }
                    }
                    CompToClient::PaneRefused { pane, .. } => {
                        let mut pending = creates.lock().unwrap_or_else(|e| e.into_inner());
                        if let Some(oneshot) = pending.remove(pane) {
                            let (pane_tx, pane_rx) = calloop::channel::channel::<LooperMessage>();
                            let _ = oneshot.send(CreateResponse {
                                msg,
                                pane_rx,
                                pane_tx,
                            });
                            continue;
                        }
                    }
                    _ => {}
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
            pending_creates,
            pane_channels,
            signature: signature.to_string(),
            pane_count: Arc::new(AtomicUsize::new(0)),
            done_signal: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
            _dispatcher: dispatcher,
        })
    }

    /// Create a new pane with the given tag configuration.
    ///
    /// Returns a [`PaneCreateFuture`] that must be consumed via
    /// [`wait()`](PaneCreateFuture::wait) to obtain the [`Pane`].
    /// Dropping the future without calling `wait()` automatically
    /// cleans up the server-side pane (clunk-on-abandon).
    pub fn create_pane(&self, tag: Tag) -> Result<PaneCreateFuture> {
        let wire_tag = Some(tag.into_wire());
        self.create_pane_inner(wire_tag)
    }

    /// Create a component pane with no tag line.
    ///
    /// Component panes have no title or command surface — they are
    /// building blocks meant to be interacted with through their
    /// parent pane's tag or their own content area.
    pub fn create_component_pane(&self) -> Result<PaneCreateFuture> {
        self.create_pane_inner(None)
    }

    fn create_pane_inner(&self, wire_tag: Option<CreatePaneTag>) -> Result<PaneCreateFuture> {
        // Propose a UUID for the new pane — client-chosen, server confirms.
        let proposed_id = PaneId::new();

        let (create_tx, create_rx) = mpsc::channel();
        self.pending_creates.lock().unwrap_or_else(|e| e.into_inner())
            .insert(proposed_id, create_tx);

        // Send CreatePane to compositor
        self.comp_tx.send(ClientToComp::CreatePane { pane: proposed_id, tag: wire_tag })
            .map_err(|_| PaneError::Disconnected)?;

        Ok(PaneCreateFuture {
            response_rx: create_rx,
            proposed_id,
            comp_tx: self.comp_tx.clone(),
            pending_creates: self.pending_creates.clone(),
            pane_count: self.pane_count.clone(),
            done_signal: self.done_signal.clone(),
            consumed: false,
        })
    }

    /// Request all panes to quit.
    ///
    /// Sends `QuitRequested` to every live pane and collects responses.
    /// If all panes agree, they are closed via `Quit`. If any pane
    /// vetoes, no panes are closed.
    ///
    /// This is the all-or-nothing quit model from BeOS: a single veto
    /// aborts the entire quit, which is correct for save-all-or-cancel UX.
    ///
    /// Timeout defaults to 5 seconds per pane. Remote panes (TCP) may
    /// need the full timeout; local panes respond in microseconds.
    ///
    /// # BeOS
    ///
    /// `BApplication::QuitRequested` → `_QuitAllWindows`. Be unlocked
    /// the application during iteration and tolerated stale window
    /// pointers. pane uses channel-based coordination — no locks, no
    /// stale pointers, no iteration-order sensitivity.
    pub fn request_quit(&self) -> crate::exit::QuitResult {
        use crate::exit::QuitResult;

        let channels = self.pane_channels.lock().unwrap_or_else(|e| e.into_inner());
        if channels.is_empty() {
            return QuitResult::Approved;
        }

        // Send QuitRequested to all panes, collect response channels
        let mut responses: Vec<(PaneId, mpsc::Receiver<bool>)> = Vec::new();
        for (&pane_id, tx) in channels.iter() {
            let (response_tx, response_rx) = mpsc::channel();
            if tx.send(LooperMessage::QuitRequested { response_tx }).is_ok() {
                responses.push((pane_id, response_rx));
            }
            // If send fails (disconnected), treat as unreachable
        }
        drop(channels); // release lock before blocking on responses

        let mut vetoed = Vec::new();
        let mut unreachable = Vec::new();
        let timeout = Duration::from_secs(5);

        for (pane_id, rx) in responses {
            match rx.recv_timeout(timeout) {
                Ok(true) => {} // pane agreed
                Ok(false) => vetoed.push(pane_id),
                Err(_) => unreachable.push(pane_id),
            }
        }

        if !vetoed.is_empty() {
            return QuitResult::Vetoed(vetoed);
        }
        if !unreachable.is_empty() {
            return QuitResult::Unreachable(unreachable);
        }

        // All panes agreed — send Quit to close them
        let channels = self.pane_channels.lock().unwrap_or_else(|e| e.into_inner());
        for tx in channels.values() {
            let _ = tx.send(LooperMessage::Quit);
        }

        QuitResult::Approved
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


/// A handle to a pane that is being created on the server.
///
/// Returned by [`App::create_pane`] and [`App::create_component_pane`].
/// Must be consumed via [`wait()`](PaneCreateFuture::wait) to obtain
/// the [`Pane`]. If dropped without calling `wait()`, the server-side
/// pane is automatically cleaned up via `RequestClose` — the
/// clunk-on-abandon pattern from 9P.
///
/// # Session type
///
/// Degenerate `Recv<CreateResponse, End>` — receive one message,
/// transition to terminal. Follows the same affine pattern as
/// [`ReplyPort`](crate::ReplyPort): normal path consumes via a
/// method, Drop path compensates.
#[must_use = "dropping a PaneCreateFuture leaks a pane on the server — call .wait()"]
pub struct PaneCreateFuture {
    response_rx: mpsc::Receiver<CreateResponse>,
    proposed_id: PaneId,
    comp_tx: mpsc::Sender<ClientToComp>,
    pending_creates: Arc<Mutex<HashMap<PaneId, mpsc::Sender<CreateResponse>>>>,
    pane_count: Arc<AtomicUsize>,
    done_signal: Arc<(Mutex<()>, std::sync::Condvar)>,
    /// Set to true by wait()/wait_timeout() — tells Drop to skip
    /// the cancel-sender logic since the response was consumed.
    consumed: bool,
}

impl PaneCreateFuture {
    /// Wait for the compositor to create the pane (default 5s timeout).
    pub fn wait(self) -> Result<Pane> {
        self.wait_timeout(Duration::from_secs(5))
    }

    /// Wait for the compositor to create the pane with a custom timeout.
    pub fn wait_timeout(mut self, timeout: Duration) -> Result<Pane> {
        let response = self.response_rx.recv_timeout(timeout)
            .map_err(|_| PaneError::Timeout)?;

        let (pane_id, geometry) = match response.msg {
            CompToClient::PaneCreated { pane, geometry } => (pane, geometry),
            _ => return Err(PaneError::Refused.into()),
        };

        self.pane_count.fetch_add(1, Ordering::Relaxed);
        self.consumed = true;

        Ok(Pane::new(
            pane_id,
            geometry,
            response.pane_rx,
            self.comp_tx.clone(),
            response.pane_tx,
            self.pane_count.clone(),
            self.done_signal.clone(),
        ))
    }
}

impl Drop for PaneCreateFuture {
    fn drop(&mut self) {
        if self.consumed {
            return;
        }
        // The future was dropped without calling wait().
        // The server may have already created (or be about to create) the pane.
        // Replace our pending_creates entry with a cancel-sender that
        // auto-closes the orphan pane when PaneCreated arrives.
        let comp_tx = self.comp_tx.clone();
        let pane_id = self.proposed_id;
        let (cancel_tx, cancel_rx) = mpsc::channel();

        if let Ok(mut pending) = self.pending_creates.lock() {
            pending.insert(pane_id, cancel_tx);
        }

        // Spawn a lightweight cleanup thread. It blocks until the
        // dispatcher delivers the response (or the connection dies),
        // then sends RequestClose if the pane was created.
        thread::spawn(move || {
            if let Ok(response) = cancel_rx.recv_timeout(Duration::from_secs(10)) {
                if matches!(response.msg, CompToClient::PaneCreated { .. }) {
                    let _ = comp_tx.send(ClientToComp::RequestClose { pane: pane_id });
                }
            }
            // PaneRefused or timeout: nothing to clean up
        });
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
