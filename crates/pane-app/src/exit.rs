use std::sync::{Arc, Mutex, mpsc};

use pane_proto::message::PaneId;

use crate::event::Message;
use crate::looper_message::LooperMessage;

/// Why the pane's event loop exited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    /// Handler returned Ok(false) voluntarily (e.g., user pressed Escape).
    /// The kit should send RequestClose to the compositor.
    HandlerExit,
    /// Handler returned Ok(false) in response to Message::CloseRequested.
    /// The compositor already knows — don't send RequestClose.
    CompositorClose,
    /// The connection to the compositor was lost.
    /// Can't send anything — the channel is dead.
    Disconnected,
}

impl ExitReason {
    /// Should the kit send RequestClose to the compositor?
    pub fn should_request_close(&self) -> bool {
        matches!(self, ExitReason::HandlerExit)
    }
}

/// Broadcasts a pane's exit to all monitoring loopers.
///
/// When pane A monitors pane B (via Messenger::monitor()), A registers
/// its looper_tx here. When B exits, the broadcaster sends
/// Message::PaneExited to all registered watchers.
///
/// Limitation: this is actor-level notification only — it says WHO died,
/// not WHICH pending interaction failed. When inter-pane request-response
/// exists, conversation-level failure callbacks are needed on top of this.
/// See serena memory `pane/eact_analysis_gaps` Gap 3.
#[derive(Clone, Default)]
pub(crate) struct ExitBroadcaster {
    watchers: Arc<Mutex<Vec<mpsc::SyncSender<LooperMessage>>>>,
}

impl ExitBroadcaster {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Register a watcher's looper channel. Called by Messenger::monitor().
    pub(crate) fn add_watcher(&self, tx: mpsc::SyncSender<LooperMessage>) {
        self.watchers.lock().unwrap_or_else(|e| e.into_inner()).push(tx);
    }

    /// Broadcast the exit to all watchers. Called when run() completes.
    pub(crate) fn broadcast(&self, pane: PaneId, reason: ExitReason) {
        let watchers = self.watchers.lock().unwrap_or_else(|e| e.into_inner());
        let msg = Message::PaneExited { pane, reason };
        for tx in watchers.iter() {
            let _ = tx.send(LooperMessage::Posted(msg.clone()));
        }
    }
}

impl std::fmt::Debug for ExitBroadcaster {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.watchers.lock().unwrap_or_else(|e| e.into_inner()).len();
        f.debug_struct("ExitBroadcaster")
            .field("watcher_count", &count)
            .finish()
    }
}
