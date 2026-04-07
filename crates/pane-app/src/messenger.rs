//! Messenger: inbound self-reference handle for handler use.
//!
//! Represents "what I am" — the pane's own identity and framework
//! capabilities (address, content, timers). Cloneable, passed to
//! handler methods by the looper at dispatch time.
//!
//! Distinct from ServiceHandle<P>, which represents "who I'm
//! talking to" — an outbound connection to a remote service.
//! Messenger is inbound (self-reference), ServiceHandle is
//! outbound (remote reference). They are not unifiable because
//! they face opposite directions and carry different state:
//! Messenger carries self-address and framework APIs; ServiceHandle
//! carries a session_id, write channel, and protocol type parameter.
//!
//! Plan 9: like a fid — resolution happens once at open time;
//! the result is a direct binding, not a name.

use std::sync::{Arc, OnceLock};
use std::thread::ThreadId;

use pane_proto::Address;

use crate::send_and_wait::SyncRequest;
use crate::timer::{TimerControl, TimerToken};

/// Scoped handle to a pane. The pane ID is baked in.
/// The handler receives this from the looper and uses it to
/// send messages, set content, manage timers, etc.
///
/// Clone + Send: the handler can stash it, pass it to spawned
/// work, or hand it to framework callbacks. The calloop channel
/// sender inside is Clone + Send when T: Send.
#[derive(Clone)]
pub struct Messenger {
    // TODO: Handle (pane identity) + ServiceRouter
    self_address: Address,
    timer_tx: calloop::channel::Sender<TimerControl>,
    /// Channel for submitting synchronous requests to the looper.
    /// The looper installs dispatch entries and sends wire frames
    /// on behalf of the blocked caller.
    sync_tx: calloop::channel::Sender<SyncRequest>,
    /// The looper thread's ThreadId, set during Looper::run().
    /// Used by send_and_wait for I8 enforcement: callers on the
    /// looper thread must not block (deadlock).
    looper_thread: Arc<OnceLock<ThreadId>>,
}

impl Messenger {
    /// Construct a Messenger with real timer and sync channels.
    /// Called by the builder during setup.
    pub(crate) fn new(
        timer_tx: calloop::channel::Sender<TimerControl>,
        sync_tx: calloop::channel::Sender<SyncRequest>,
        looper_thread: Arc<OnceLock<ThreadId>>,
    ) -> Self {
        Messenger {
            self_address: Address::local(0),
            timer_tx,
            sync_tx,
            looper_thread,
        }
    }

    /// Test-only constructor with dummy channels.
    /// Timer and sync sends silently fail — correct for tests
    /// that don't exercise those paths.
    #[doc(hidden)]
    pub fn stub() -> Self {
        let (timer_tx, _) = calloop::channel::channel::<TimerControl>();
        let (sync_tx, _) = calloop::channel::channel::<SyncRequest>();
        Self::new(timer_tx, sync_tx, Arc::new(OnceLock::new()))
    }

    /// This pane's address. Extractable, sendable to others
    /// as "here's how to reach me."
    pub fn address(&self) -> Address {
        self.self_address
    }

    /// Set the pane's body content.
    pub fn set_content(&self, _data: &[u8]) {
        // TODO: send to server via Handle
    }

    /// Request death notification for a target pane.
    /// The server sends PaneExited when the target exits.
    /// Cancel with [`unwatch`](Self::unwatch). Server cleans
    /// up if this pane exits while watching.
    ///
    /// # BeOS
    ///
    /// `BRoster::StartWatching`
    /// (src/servers/registrar/TRoster.cpp:1523-1536).
    pub fn watch(&self, _target: Address) {
        // TODO: send ControlMessage::Watch { target } on control channel
    }

    /// Cancel a prior watch registration.
    pub fn unwatch(&self, _target: Address) {
        // TODO: send ControlMessage::Unwatch { target } on control channel
    }

    /// Set the pulse timer interval. Returns a TimerToken
    /// whose Drop cancels the timer.
    ///
    /// The timer fires `LifecycleMessage::Pulse` at the given
    /// interval, dispatched through `Handler::pulse()`. Dropping
    /// the returned token cancels the timer.
    ///
    /// Calling this multiple times creates independent timers.
    /// Each token must be held (or dropped) independently.
    ///
    /// # BeOS
    ///
    /// `BWindow::SetPulseRate(bigtime_t)`
    /// (src/kits/interface/Window.cpp:1665-1687).
    /// Be's version replaced the existing pulse; pane returns
    /// independent tokens because obligation handles compose
    /// better than mutable global state.
    pub fn set_pulse_rate(&self, duration: std::time::Duration) -> TimerToken {
        TimerToken::new(duration, self.timer_tx.clone())
    }

    /// Submit a synchronous request to the looper for processing.
    /// Called by ServiceHandle::send_and_wait.
    pub(crate) fn send_sync_request(&self, req: SyncRequest) -> Result<(), SyncRequest> {
        self.sync_tx.send(req).map_err(|e| e.0)
    }

    /// The looper thread's ThreadId, if the looper has started.
    /// Returns None before Looper::run() sets it.
    pub(crate) fn looper_thread_id(&self) -> Option<ThreadId> {
        self.looper_thread.get().copied()
    }

    /// Handle to the looper thread OnceLock. The Looper sets this
    /// during run() before entering the event loop.
    pub(crate) fn looper_thread_lock(&self) -> &Arc<OnceLock<ThreadId>> {
        &self.looper_thread
    }
}

/// Marker trait for fire-and-forget messages via post_app_message.
/// Requires Clone (prevents smuggling obligation handles).
pub trait AppPayload: Clone + Send + 'static {}

// Blanket impl
impl<T: Clone + Send + 'static> AppPayload for T {}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_messenger() -> Messenger {
        Messenger::stub()
    }

    #[test]
    fn messenger_address_returns_address() {
        let m = test_messenger();
        let addr = m.address();
        // Stub address is local(0)
        assert!(addr.is_local());
        assert_eq!(addr.pane_id, 0);
    }

    #[test]
    fn messenger_address_is_copy() {
        let m = test_messenger();
        let a = m.address();
        let b = a; // Copy
        let c = a; // still usable
        assert_eq!(b, c);
    }

    #[test]
    fn messenger_clone_preserves_address() {
        let m = test_messenger();
        let m2 = m.clone();
        assert_eq!(m.address(), m2.address());
    }

    #[test]
    fn messenger_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Messenger>();
    }

    #[test]
    fn messenger_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<Messenger>();
    }

    #[test]
    fn set_pulse_rate_returns_timer_token() {
        let m = test_messenger();
        let _token = m.set_pulse_rate(std::time::Duration::from_millis(100));
        // Token exists, will cancel on drop
    }
}
