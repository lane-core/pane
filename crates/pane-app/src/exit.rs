use std::sync::{Arc, Mutex};

use pane_proto::message::PaneId;

use crate::event::Message;
use crate::looper_message::LooperMessage;
use crate::proxy::LooperSender;

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
    watchers: Arc<Mutex<Vec<LooperSender>>>,
}

impl ExitBroadcaster {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Register a watcher's looper channel. Called by Messenger::monitor().
    pub(crate) fn add_watcher(&self, tx: LooperSender) {
        self.watchers.lock().unwrap_or_else(|e| e.into_inner()).push(tx);
    }

    /// Broadcast the exit to all watchers. Called when run() completes.
    ///
    /// Non-blocking: calloop channels are unbounded so send never blocks.
    /// Watchers with disconnected channels are pruned — exit
    /// notifications are advisory.
    pub(crate) fn broadcast(&self, pane: PaneId, reason: ExitReason) {
        let mut watchers = self.watchers.lock().unwrap_or_else(|e| e.into_inner());
        let msg = Message::PaneExited { pane, reason };
        watchers.retain(|tx| {
            tx.send(LooperMessage::Posted(msg.clone())).is_ok()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn pane_id(n: u32) -> PaneId {
        PaneId::from_uuid(uuid::Uuid::from_u128(n as u128))
    }

    /// Receive one message from a calloop channel (test helper).
    fn recv_one(
        channel: calloop::channel::Channel<LooperMessage>,
        timeout: Duration,
    ) -> Option<LooperMessage> {
        let mut event_loop: calloop::EventLoop<'_, Option<LooperMessage>> =
            calloop::EventLoop::try_new().unwrap();
        event_loop.handle().insert_source(channel, |event, _, result| {
            if let calloop::channel::Event::Msg(msg) = event {
                *result = Some(msg);
            }
        }).unwrap();
        let mut result = None;
        event_loop.dispatch(Some(timeout), &mut result).unwrap();
        result
    }

    #[test]
    fn broadcast_delivers_to_watchers() {
        let broadcaster = ExitBroadcaster::new();

        // 10 watchers with live channels
        let mut rxs = Vec::new();
        for _ in 0..10 {
            let (tx, rx) = calloop::channel::channel::<LooperMessage>();
            broadcaster.add_watcher(tx);
            rxs.push(rx);
        }

        let id = pane_id(1);
        let start = std::time::Instant::now();
        broadcaster.broadcast(id, ExitReason::HandlerExit);
        let elapsed = start.elapsed();

        assert!(elapsed < Duration::from_millis(100),
            "broadcast should be non-blocking, took {:?}", elapsed);

        // All watchers should receive the notification
        for (i, rx) in rxs.into_iter().enumerate() {
            let msg = recv_one(rx, Duration::from_secs(1))
                .unwrap_or_else(|| panic!("watcher {i} should receive notification"));
            match msg {
                LooperMessage::Posted(Message::PaneExited { pane, reason }) => {
                    assert_eq!(pane, id);
                    assert_eq!(reason, ExitReason::HandlerExit);
                }
                other => panic!("expected PaneExited, got {:?}", other),
            }
        }
    }

    #[test]
    fn broadcast_prunes_disconnected_watchers() {
        let broadcaster = ExitBroadcaster::new();

        // 5 healthy watchers (keep receivers alive)
        let mut healthy_rxs = Vec::new();
        for _ in 0..5 {
            let (tx, rx) = calloop::channel::channel::<LooperMessage>();
            healthy_rxs.push(rx); // keep receiver alive so channel stays open
            broadcaster.add_watcher(tx);
        }

        // 5 disconnected watchers (receiver already dropped)
        for _ in 0..5 {
            let (tx, rx) = calloop::channel::channel::<LooperMessage>();
            drop(rx); // disconnect
            broadcaster.add_watcher(tx);
        }

        // First broadcast: sends to all, but disconnected fail and get pruned
        broadcaster.broadcast(pane_id(1), ExitReason::HandlerExit);

        // Second broadcast: should only attempt the 5 healthy watchers
        broadcaster.broadcast(pane_id(2), ExitReason::Disconnected);

        // Verify the watcher list was pruned
        let watchers = broadcaster.watchers.lock().unwrap();
        assert_eq!(watchers.len(), 5,
            "disconnected watchers should have been pruned, got {}", watchers.len());

        drop(healthy_rxs); // keep alive until assertions complete
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
