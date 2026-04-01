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
use pane_proto::tag::{PaneTitle, CommandVocabulary};

use crate::error::{PaneError, Result};
use crate::event::Message;
use crate::exit::ExitBroadcaster;
use crate::looper_message::LooperMessage;

/// Type alias for the calloop channel sender used by loopers.
pub(crate) type LooperSender = calloop::channel::Sender<LooperMessage>;

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
///
/// # Plan 9
///
/// Transport-transparent like 9P file descriptors. A Messenger from
/// `App::connect` (local) and one from `App::connect_remote` (TCP)
/// expose identical operations — the transport boundary is in the
/// connection setup, not in the handle.
#[derive(Clone)]
pub struct Messenger {
    pub(crate) id: PaneId,
    pub(crate) sender: mpsc::Sender<ClientToComp>,
    pub(crate) looper_tx: Option<LooperSender>,
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
    pub fn with_looper(mut self, tx: LooperSender) -> Self {
        self.looper_tx = Some(tx);
        self
    }

    /// Check whether this messenger has an attached looper channel.
    ///
    /// Returns `false` if the messenger was never attached to a looper
    /// (e.g., constructed via `Messenger::new` without `with_looper`).
    /// A `true` return does not guarantee the next send will succeed —
    /// the looper may exit between the check and the send.
    ///
    /// # BeOS
    ///
    /// `BMessenger::IsValid`. Be's version checked whether the target
    /// looper's port existed, with the same caveat: validity could
    /// change between the check and the next operation.
    pub fn is_valid(&self) -> bool {
        self.looper_tx.is_some()
    }

    /// Post an event to this pane's own looper.
    ///
    /// This is the self-delivery mechanism: worker threads (network,
    /// computation) can post results back to the pane's event loop
    /// for sequential processing. The channel is unbounded — this
    /// call never blocks. Fails only if the looper has exited.
    ///
    /// # BeOS
    ///
    /// `BLooper::PostMessage`.
    pub fn send_message(&self, event: Message) -> Result<()> {
        let tx = self.looper_tx.as_ref().ok_or(PaneError::Disconnected)?;
        tx.send(LooperMessage::Posted(event))
            .map_err(|_| PaneError::Disconnected)?;
        Ok(())
    }

    /// Post an event to this pane's own looper, without blocking.
    ///
    /// Equivalent to [`send_message`](Messenger::send_message) — the
    /// looper channel is unbounded, so both methods have identical
    /// semantics. Retained for compatibility; prefer `send_message`.
    #[deprecated(since = "0.1.0", note = "use send_message; the looper channel is unbounded")]
    pub fn try_send_message(&self, event: Message) -> Result<()> {
        self.send_message(event)
    }

    /// Send a message to another pane's looper and block for a reply.
    ///
    /// The target's handler receives the message via
    /// [`Handler::request_received`](crate::Handler::request_received) with a [`ReplyPort`](crate::ReplyPort).
    /// The target calls `reply_port.reply(value)` to respond.
    ///
    /// **Do NOT call from a looper thread** — it blocks the event loop.
    /// Use from worker threads, CLI tools, or `App` methods only.
    /// The async reply path (via `Handler::reply_received`) is the
    /// deadlock-free alternative for looper-to-looper communication.
    ///
    /// # BeOS
    ///
    /// `BMessenger::SendMessage(BMessage*, BMessage* reply, timeout)` —
    /// the synchronous send-and-wait variant. Be's classic deadlock
    /// (mutual synchronous send between two loopers) is why pane makes
    /// the async path primary and restricts this to non-looper callers.
    pub fn send_and_wait(
        &self,
        target: &Messenger,
        msg: Message,
        timeout: std::time::Duration,
    ) -> std::result::Result<Box<dyn std::any::Any + Send>, PaneError> {
        if crate::looper::is_looper_thread() {
            return Err(PaneError::WouldDeadlock);
        }
        let target_tx = target.looper_tx.as_ref().ok_or(PaneError::Disconnected)?;
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel::<Message>(1);

        let token = next_timer_id(); // reuse the atomic counter for unique IDs
        let reply_port = crate::reply::ReplyPort::new(token, move |msg| {
            let _ = reply_tx.send(msg);
        });

        target_tx.send(LooperMessage::Request(msg, reply_port))
            .map_err(|_| PaneError::Disconnected)?;

        match reply_rx.recv_timeout(timeout) {
            Ok(Message::Reply { payload, .. }) => Ok(payload),
            Ok(Message::ReplyFailed { .. }) => Err(PaneError::Refused),
            Ok(_) => Err(PaneError::Refused), // unexpected message type
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(PaneError::Timeout),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(PaneError::Disconnected),
        }
    }

    /// Send an async request to another pane's looper. The reply
    /// arrives as `Message::Reply` in your handler's `reply_received`.
    ///
    /// Returns a token to match against when the reply arrives.
    /// If the target drops the request without replying, your handler
    /// receives `Message::ReplyFailed` with the same token.
    pub fn send_request(
        &self,
        target: &Messenger,
        msg: Message,
    ) -> Result<u64> {
        let target_tx = target.looper_tx.as_ref().ok_or(PaneError::Disconnected)?;
        let self_tx = self.looper_tx.as_ref().ok_or(PaneError::Disconnected)?.clone();

        let token = next_timer_id();
        let reply_port = crate::reply::ReplyPort::new(token, move |msg| {
            let _ = self_tx.send(LooperMessage::Posted(msg));
        });

        target_tx.send(LooperMessage::Request(msg, reply_port))
            .map_err(|_| PaneError::Disconnected)?;

        Ok(token)
    }

    /// Post an application-defined message to this pane's looper.
    ///
    /// Use this from worker threads to deliver async results back to
    /// the event loop. The value is delivered to [`Handler::app_message`](crate::Handler::app_message).
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
        self.send_message(Message::AppMessage(Box::new(msg)))
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

    // set_completions removed — completions are now replied via
    // CompletionReplyPort, which the handler receives directly in
    // the completion_request hook. See completions.rs.

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
        *token = Some(self.send_periodic_fn(|| Message::Pulse, rate)?);
        Ok(())
    }

    /// Post an event to this pane's looper after a delay.
    ///
    /// The event is consumed on delivery (no Clone needed). The looper
    /// integrates one-shots into its calloop dispatch timeout.
    ///
    /// # BeOS
    ///
    /// `BMessageRunner` with count=1.
    pub fn send_delayed(&self, event: Message, delay: std::time::Duration) -> Result<()> {
        let tx = self.looper_tx.as_ref().ok_or(PaneError::Disconnected)?;
        tx.send(LooperMessage::AddOneShot {
            event,
            fire_at: std::time::Instant::now() + delay,
        }).map_err(|_| PaneError::Disconnected)?;
        Ok(())
    }

    /// Post an event to this pane's looper repeatedly at the given interval.
    ///
    /// Convenience wrapper that clones the event each fire. The event must
    /// be a clonable variant (system events like Pulse, Activated, etc.).
    /// For non-clonable events, use [`send_periodic_fn`](Messenger::send_periodic_fn).
    ///
    /// # Panics
    ///
    /// Panics at fire time if the event is `Message::AppMessage` or `Message::Reply`
    /// (these are consumed once and cannot be cloned).
    pub fn send_periodic(&self, event: Message, interval: std::time::Duration) -> Result<TimerToken>
    {
        self.send_periodic_fn(move || event.clone(), interval)
    }

    /// Post events to this pane's looper repeatedly using a factory closure.
    ///
    /// The closure is called each time the timer fires, producing a fresh
    /// event. This avoids cloning and works with any event type.
    ///
    /// # BeOS
    ///
    /// `BMessageRunner` with count=`B_INFINITE_TIMEOUT`. Be's BMessageRunner
    /// was a system service in the registrar; pane integrates timers directly
    /// into each looper's calloop event loop.
    pub fn send_periodic_fn(
        &self,
        make_event: impl Fn() -> Message + Send + 'static,
        interval: std::time::Duration,
    ) -> Result<TimerToken> {
        let tx = self.looper_tx.as_ref().ok_or(PaneError::Disconnected)?;
        let id = next_timer_id();
        let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let token = TimerToken { id, cancelled: cancelled.clone(), sender: Some(tx.clone()) };

        tx.send(LooperMessage::AddTimer {
            id,
            make_event: Box::new(make_event),
            interval,
            cancelled,
        }).map_err(|_| PaneError::Disconnected)?;

        Ok(token)
    }

    /// Add a filter to this pane's event loop at runtime.
    ///
    /// The filter takes effect before the next event batch — not
    /// mid-batch. Returns a [`FilterToken`](crate::FilterToken) for later removal via
    /// [`remove_filter`](Messenger::remove_filter).
    ///
    /// # BeOS
    ///
    /// `BLooper::AddCommonFilter` / `BHandler::AddFilter`. Be's version
    /// required the looper lock and mutated the filter list directly,
    /// which had a latent self-modification-during-iteration bug.
    /// pane defers mutation via the looper's message channel, making it
    /// safe from any thread.
    pub fn add_filter(&self, filter: impl crate::filter::MessageFilter) -> Result<crate::filter::FilterToken> {
        let tx = self.looper_tx.as_ref().ok_or(PaneError::Disconnected)?;
        let id = next_filter_id();
        tx.send(LooperMessage::AddFilter {
            id,
            filter: Box::new(filter),
        }).map_err(|_| PaneError::Disconnected)?;
        Ok(crate::filter::FilterToken { id })
    }

    /// Remove a runtime filter by its token.
    ///
    /// No-op if the filter was already removed. The removal takes
    /// effect before the next event batch.
    ///
    /// # BeOS
    ///
    /// `BLooper::RemoveCommonFilter` / `BHandler::RemoveFilter`.
    pub fn remove_filter(&self, token: &crate::filter::FilterToken) -> Result<()> {
        let tx = self.looper_tx.as_ref().ok_or(PaneError::Disconnected)?;
        tx.send(LooperMessage::RemoveFilter { id: token.id })
            .map_err(|_| PaneError::Disconnected)?;
        Ok(())
    }

    fn send(&self, msg: ClientToComp) -> Result<()> {
        self.sender.send(msg).map_err(|_| PaneError::Disconnected)?;
        Ok(())
    }
}

/// Generate a unique timer ID.
fn next_timer_id() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Generate a unique filter ID.
fn next_filter_id() -> crate::filter::FilterId {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Token for cancelling a periodic timer.
///
/// Created by `Messenger::send_periodic`. Dropping the token cancels
/// the timer (affine: every timer lifecycle terminates). Call
/// `cancel()` explicitly if you need cancellation before drop.
///
/// # Cancel mechanism
///
/// Dual-path: the `AtomicBool` is checked by the timer callback on
/// next fire (immediate, no channel latency) AND `CancelTimer` eagerly
/// removes the calloop source (no ghost source in the TimerWheel).
/// Both are idempotent.
///
/// # BeOS
///
/// `BMessageRunner` was non-copyable and cancel-on-destruct. This
/// matches that model.
#[derive(Debug)]
pub struct TimerToken {
    /// Unique timer ID (matches the `id` in `LooperMessage::AddTimer`).
    pub(crate) id: u64,
    cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Channel for sending `CancelTimer` to eagerly remove the source.
    sender: Option<LooperSender>,
}

impl TimerToken {
    /// Cancel the periodic timer. The looper removes the source on
    /// the next iteration.
    pub fn cancel(&self) {
        self.cancelled.store(true, std::sync::atomic::Ordering::Release);
        if let Some(ref tx) = self.sender {
            let _ = tx.send(LooperMessage::CancelTimer { id: self.id });
        }
    }
}

impl Drop for TimerToken {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl std::fmt::Debug for Messenger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Messenger")
            .field("id", &self.id)
            .finish()
    }
}
