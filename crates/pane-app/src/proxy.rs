//! Messenger — a lightweight, cloneable handle for sending messages
//! to the compositor and posting events to the pane's own looper.
//!
//! This is the BMessenger pattern: the handle doesn't own the pane,
//! it just knows how to send messages on its behalf. Pass it to
//! spawned threads for async work (network fetches, computation).
//!
//! Two directions:
//! - `set_title()`, `set_vocabulary()`, etc. → compositor
//! - `send_message()` → pane's own looper (BLooper::PostMessage)

use std::sync::{Arc, Mutex, mpsc};

use pane_proto::message::PaneId;
use pane_proto::protocol::ClientToComp;
use pane_proto::tag::{PaneTitle, CommandVocabulary, Completion};

use crate::error::{PaneError, Result};
use crate::event::Message;
use crate::exit::ExitBroadcaster;
use crate::looper_message::LooperMessage;

/// A cloneable handle for sending messages to the compositor and
/// posting events to a pane's own looper.
///
/// Messenger is the thread-boundary crossing point. Your handler
/// receives a `&Messenger` on every event; clone it and hand it
/// to spawned threads for async work (network fetches, file I/O,
/// computation). Those threads post results back via
/// [`send_message`](Messenger::send_message), and the pane's
/// event loop picks them up on its next iteration.
///
/// # Threading
///
/// `Clone + Send`. All clones target the same pane. Sending is safe
/// from any thread — the underlying channel handles synchronization.
///
/// # BeOS
///
/// `BMessenger` (see also Haiku's
/// [BMessenger documentation](reference/haiku-book/app/BMessenger.dox)).
/// Key changes:
/// - Combines outbound (to compositor) and self-delivery (to own
///   looper) in one handle
/// - Timer methods ([`send_delayed`](Messenger::send_delayed),
///   [`send_periodic`](Messenger::send_periodic)) are on Messenger
///   rather than a separate `BMessageRunner` class
#[derive(Clone)]
pub struct Messenger {
    pub(crate) id: PaneId,
    pub(crate) sender: mpsc::Sender<ClientToComp>,
    pub(crate) looper_tx: Option<mpsc::SyncSender<LooperMessage>>,
    pub(crate) broadcaster: ExitBroadcaster,
    /// Shared pulse timer token — all clones of a Messenger reference
    /// the same token so cancellation works regardless of which clone
    /// calls set_pulse_rate.
    pub(crate) pulse_token: Arc<Mutex<Option<TimerToken>>>,
}

impl Messenger {
    /// Create a Messenger from an ID and compositor sender.
    /// The looper channel is set internally by Pane — not needed
    /// for test construction where only compositor sends matter.
    pub fn new(id: PaneId, sender: mpsc::Sender<ClientToComp>) -> Self {
        Messenger {
            id,
            sender,
            looper_tx: None,
            broadcaster: ExitBroadcaster::new(),
            pulse_token: Arc::new(Mutex::new(None)),
        }
    }

    /// Attach the looper's self-delivery channel. Internal.
    #[doc(hidden)]
    pub fn with_looper(mut self, tx: mpsc::SyncSender<LooperMessage>) -> Self {
        self.looper_tx = Some(tx);
        self
    }

    /// Post an event to this pane's own looper.
    ///
    /// This is the self-delivery mechanism: worker threads (network,
    /// computation) can post results back to the pane's event loop
    /// for sequential processing. The BLooper::PostMessage equivalent.
    pub fn send_message(&self, event: Message) -> Result<()> {
        let tx = self.looper_tx.as_ref().ok_or(PaneError::Disconnected)?;
        tx.send(LooperMessage::Posted(event))
            .map_err(|_| PaneError::Disconnected)?;
        Ok(())
    }

    /// Post an application-defined message to this pane's looper.
    ///
    /// Use this from worker threads to deliver async results back to
    /// the event loop. The value is delivered to [`Handler::message_received`].
    ///
    /// ```ignore
    /// let proxy = proxy.clone();
    /// std::thread::spawn(move || {
    ///     let result = expensive_computation();
    ///     proxy.post_app_message(result).ok();
    /// });
    /// ```
    ///
    /// # BeOS
    ///
    /// `BMessenger::SendMessage` with an app-defined `what` code.
    pub fn post_app_message<T: Send + 'static>(&self, msg: T) -> Result<()> {
        self.send_message(Message::App(Box::new(msg)))
    }

    /// Monitor this pane: when it exits, deliver `Message::PaneExited`
    /// to the given watcher's looper. Erlang-style crash propagation.
    ///
    /// No BeOS equivalent — pane adds crash monitoring that BeOS lacked.
    ///
    /// ```ignore
    /// // In pane A's handler, monitoring pane B:
    /// b_handle.monitor(a_proxy);
    /// // When B exits, A receives Message::PaneExited { pane: b_id, reason }
    /// ```
    pub fn monitor(&self, watcher: &Messenger) {
        if let Some(ref tx) = watcher.looper_tx {
            self.broadcaster.add_watcher(tx.clone());
        }
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

    /// Request the compositor to resize this pane.
    /// The compositor decides whether to honor it (tiling may constrain).
    pub fn resize_to(&self, width: u32, height: u32) -> Result<()> {
        self.send(ClientToComp::RequestResize { pane: self.id, width, height })
    }

    /// Declare size limits for this pane.
    /// The compositor uses these during layout — it won't shrink
    /// below min or grow beyond max.
    pub fn set_size_limits(
        &self,
        min: (u32, u32),
        max: (u32, u32),
    ) -> Result<()> {
        self.send(ClientToComp::SetSizeLimits {
            pane: self.id,
            min_width: min.0, min_height: min.1,
            max_width: max.0, max_height: max.1,
        })
    }

    /// Request the pane be hidden or shown.
    ///
    /// # BeOS
    ///
    /// `BWindow::Hide` / `BWindow::Show`. Be used cumulative (refcounted)
    /// hide/show — two `Hide()` calls required two `Show()` calls. pane
    /// uses boolean (idempotent) because the compositor owns visibility
    /// through the layout/tag system. Client-side refcounting can be
    /// layered on top if needed.
    pub fn set_hidden(&self, hidden: bool) -> Result<()> {
        self.send(ClientToComp::SetHidden { pane: self.id, hidden })
    }

    /// Set the pulse rate. Pulse messages are delivered at this interval.
    /// Pass `Duration::ZERO` to disable. BWindow::SetPulseRate equivalent.
    ///
    /// Cancels any previous pulse rate. Only one pulse timer per pane.
    pub fn set_pulse_rate(&self, rate: std::time::Duration) -> Result<()> {
        let mut token = self.pulse_token.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(prev) = token.take() {
            prev.cancel();
        }
        if rate.is_zero() {
            return Ok(());
        }
        *token = Some(self.send_periodic(Message::Pulse, rate)?);
        Ok(())
    }

    /// Post an event to this pane's looper after a delay.
    ///
    /// Spawns a thread that sleeps then delivers via self-delivery.
    /// The BMessageRunner single-shot equivalent.
    pub fn send_delayed(&self, event: Message, delay: std::time::Duration) -> Result<()> {
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
    pub fn send_periodic(&self, event: Message, interval: std::time::Duration) -> Result<TimerToken>
    {
        let tx = self.looper_tx.as_ref().ok_or(PaneError::Disconnected)?.clone();
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let token = TimerToken { cancelled: cancelled.clone() };

        std::thread::spawn(move || {
            loop {
                std::thread::sleep(interval);
                if cancelled.load(std::sync::atomic::Ordering::Acquire) {
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
/// Created by `Messenger::send_periodic`. Drop does not cancel —
/// call `cancel()` explicitly.
#[derive(Debug, Clone)]
pub struct TimerToken {
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl TimerToken {
    /// Cancel the periodic timer. The next scheduled delivery will not fire.
    pub fn cancel(&self) {
        self.cancelled.store(true, std::sync::atomic::Ordering::Release);
    }
}

impl std::fmt::Debug for Messenger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Messenger")
            .field("id", &self.id)
            .finish()
    }
}
