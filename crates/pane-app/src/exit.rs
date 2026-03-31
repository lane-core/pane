use std::sync::{Arc, Mutex, mpsc};

use pane_proto::message::PaneId;

use crate::event::Message;
use crate::looper_message::LooperMessage;

/// Why the pane's event loop exited.
///
/// No BeOS equivalent — pane adds structured exit reasons for
/// crash monitoring via [`Messenger::monitor`](crate::Messenger::monitor).
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

/// Result of an app-level quit request.
///
/// Returned by [`App::request_quit`](crate::App::request_quit).
/// The caller decides how to proceed based on the result.
#[derive(Debug)]
pub enum QuitResult {
    /// All panes agreed to quit. They have been closed.
    Approved,
    /// At least one pane vetoed. No panes were closed.
    Vetoed(Vec<PaneId>),
    /// At least one pane was unreachable (timeout or disconnect).
    /// The caller can decide whether to force-quit these panes.
    Unreachable(Vec<PaneId>),
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
    ///
    /// Non-blocking: uses try_send to avoid stalling if a watcher's
    /// channel is full. Watchers with full or disconnected channels
    /// are pruned — exit notifications are advisory.
    pub(crate) fn broadcast(&self, pane: PaneId, reason: ExitReason) {
        let mut watchers = self.watchers.lock().unwrap_or_else(|e| e.into_inner());
        let msg = Message::PaneExited { pane, reason };
        watchers.retain(|tx| {
            tx.try_send(LooperMessage::Posted(msg.clone())).is_ok()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pane_id(n: u32) -> PaneId {
        PaneId::from_uuid(uuid::Uuid::from_u128(n as u128))
    }

    #[test]
    fn broadcast_nonblocking_under_backpressure() {
        let broadcaster = ExitBroadcaster::new();
        let mut healthy_rxs = Vec::new();
        let mut full_rxs = Vec::new();

        // 10 watchers with empty channels
        for _ in 0..10 {
            let (tx, rx) = mpsc::sync_channel::<LooperMessage>(256);
            broadcaster.add_watcher(tx);
            healthy_rxs.push(rx);
        }

        // 10 watchers with channels filled to capacity
        for _ in 0..10 {
            let (tx, rx) = mpsc::sync_channel::<LooperMessage>(4);
            // Fill to capacity
            for _ in 0..4 {
                tx.send(LooperMessage::Posted(Message::Activated)).unwrap();
            }
            broadcaster.add_watcher(tx);
            full_rxs.push(rx);
        }

        // Broadcast should complete immediately (not block on full channels)
        let id = pane_id(1);
        let start = std::time::Instant::now();
        broadcaster.broadcast(id, ExitReason::HandlerExit);
        let elapsed = start.elapsed();

        assert!(elapsed < std::time::Duration::from_millis(100),
            "broadcast should be non-blocking, took {:?}", elapsed);

        // Healthy watchers should receive the notification
        for (i, rx) in healthy_rxs.iter().enumerate() {
            let msg = rx.try_recv().expect(&format!("healthy watcher {i} should receive notification"));
            match msg {
                LooperMessage::Posted(Message::PaneExited { pane, reason }) => {
                    assert_eq!(pane, id);
                    assert_eq!(reason, ExitReason::HandlerExit);
                }
                other => panic!("expected PaneExited, got {:?}", other),
            }
        }

        // Full watchers should NOT have received it (their channels were full)
        for (i, rx) in full_rxs.iter().enumerate() {
            // Drain the 4 pre-filled messages
            for _ in 0..4 {
                let msg = rx.try_recv().unwrap();
                assert!(matches!(msg, LooperMessage::Posted(Message::Activated)),
                    "full watcher {i}: pre-filled message should be Activated");
            }
            // No PaneExited should follow
            assert!(rx.try_recv().is_err(),
                "full watcher {i} should not have received PaneExited");
        }

        // Second broadcast should only reach the 10 healthy watchers
        // (full watchers were pruned)
        broadcaster.broadcast(pane_id(2), ExitReason::Disconnected);
        let mut second_count = 0;
        for rx in &healthy_rxs {
            if rx.try_recv().is_ok() {
                second_count += 1;
            }
        }
        assert_eq!(second_count, 10,
            "second broadcast should reach all 10 healthy watchers, got {second_count}");
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
