//! PaneHandle — a lightweight, cloneable handle for sending messages
//! to the compositor and posting events to the pane's own looper.
//!
//! This is the BMessenger pattern: the handle doesn't own the pane,
//! it just knows how to send messages on its behalf. Pass it to
//! spawned threads for async work (network fetches, computation).
//!
//! Two directions:
//! - `set_title()`, `set_vocabulary()`, etc. → compositor
//! - `post_message()` → pane's own looper (BLooper::PostMessage)

use std::sync::mpsc;

use pane_proto::message::PaneId;
use pane_proto::protocol::ClientToComp;
use pane_proto::tag::{PaneTitle, CommandVocabulary, Completion};

use crate::error::{PaneError, Result};
use crate::event::PaneMessage;
use crate::looper_message::LooperMessage;

/// A cloneable handle for sending messages to the compositor on
/// behalf of a specific pane, and for posting events to the pane's
/// own looper. The BMessenger equivalent.
///
/// Use this inside event handlers and spawned threads to update
/// pane state without owning the Pane itself.
#[derive(Clone)]
pub struct PaneHandle {
    pub(crate) id: PaneId,
    pub(crate) sender: mpsc::Sender<ClientToComp>,
    pub(crate) looper_tx: Option<mpsc::Sender<LooperMessage>>,
}

impl PaneHandle {
    /// Create a PaneHandle from an ID and compositor sender.
    /// The looper channel is set internally by Pane — not needed
    /// for test construction where only compositor sends matter.
    pub fn new(id: PaneId, sender: mpsc::Sender<ClientToComp>) -> Self {
        PaneHandle { id, sender, looper_tx: None }
    }

    /// Attach the looper's self-delivery channel. Internal.
    pub(crate) fn with_looper(mut self, tx: mpsc::Sender<LooperMessage>) -> Self {
        self.looper_tx = Some(tx);
        self
    }

    /// Post an event to this pane's own looper.
    ///
    /// This is the self-delivery mechanism: worker threads (network,
    /// computation) can post results back to the pane's event loop
    /// for sequential processing. The BLooper::PostMessage equivalent.
    pub fn post_message(&self, event: PaneMessage) -> Result<()> {
        let tx = self.looper_tx.as_ref().ok_or(PaneError::Disconnected)?;
        tx.send(LooperMessage::Posted(event))
            .map_err(|_| PaneError::Disconnected)?;
        Ok(())
    }

    /// The pane's compositor-assigned ID.
    pub fn id(&self) -> PaneId {
        self.id
    }

    /// Update the pane's title.
    pub fn set_title(&self, title: PaneTitle) -> Result<()> {
        self.send(ClientToComp::SetTitle { pane: self.id, title })
    }

    /// Update the pane's command vocabulary.
    pub fn set_vocabulary(&self, vocabulary: CommandVocabulary) -> Result<()> {
        self.send(ClientToComp::SetVocabulary { pane: self.id, vocabulary })
    }

    /// Update the pane's body content.
    pub fn set_content(&self, content: &[u8]) -> Result<()> {
        self.send(ClientToComp::SetContent {
            pane: self.id,
            content: content.to_vec(),
        })
    }

    /// Respond to a completion request.
    pub fn set_completions(&self, token: u64, completions: Vec<Completion>) -> Result<()> {
        self.send(ClientToComp::CompletionResponse {
            pane: self.id,
            token,
            completions,
        })
    }

    /// Post an event to this pane's looper after a delay.
    ///
    /// Spawns a thread that sleeps then delivers via self-delivery.
    /// The BMessageRunner single-shot equivalent.
    pub fn post_delayed(&self, event: PaneMessage, delay: std::time::Duration) -> Result<()> {
        let tx = self.looper_tx.as_ref().ok_or(PaneError::Disconnected)?.clone();
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            let _ = tx.send(LooperMessage::Posted(event));
        });
        Ok(())
    }

    /// Post an event to this pane's looper repeatedly at the given interval.
    ///
    /// Returns a TimerToken that can cancel the timer. The first delivery
    /// happens after one interval. The BMessageRunner periodic equivalent.
    pub fn post_periodic(&self, event: PaneMessage, interval: std::time::Duration) -> Result<TimerToken>
    where
        PaneMessage: Clone,
    {
        let tx = self.looper_tx.as_ref().ok_or(PaneError::Disconnected)?.clone();
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let token = TimerToken { cancelled: cancelled.clone() };

        std::thread::spawn(move || {
            loop {
                std::thread::sleep(interval);
                if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                if tx.send(LooperMessage::Posted(event.clone())).is_err() {
                    break;
                }
            }
        });

        Ok(token)
    }

    fn send(&self, msg: ClientToComp) -> Result<()> {
        self.sender.send(msg).map_err(|_| PaneError::Disconnected)?;
        Ok(())
    }
}

/// Token for cancelling a periodic timer.
///
/// Created by `PaneHandle::post_periodic`. Drop does not cancel —
/// call `cancel()` explicitly.
#[derive(Debug, Clone)]
pub struct TimerToken {
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl TimerToken {
    /// Cancel the periodic timer. The next scheduled delivery will not fire.
    pub fn cancel(&self) {
        self.cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl std::fmt::Debug for PaneHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneHandle")
            .field("id", &self.id)
            .finish()
    }
}
