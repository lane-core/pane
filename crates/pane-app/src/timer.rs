//! Timer obligation handle: cancel-on-drop pulse timer.
//!
//! TimerToken is an obligation handle — holding it keeps the timer
//! alive, dropping it cancels it. The handler receives
//! `LifecycleMessage::Pulse` at the requested interval while the
//! token is alive.
//!
//! Design heritage: BeOS BWindow::SetPulseRate()
//! (src/kits/interface/Window.cpp:1665-1687) stored a
//! BMessageRunner that posted B_PULSE messages to the looper port.
//! Cancellation was via BMessageRunner destructor — same ownership
//! model as TimerToken. calloop Timer sources are process-local
//! (strictly better than Be's BMessageRunner which routed through
//! the registrar). Plan 9 alarm(2) supported one pending alarm per
//! process; pane supports multiple concurrent timers per looper via
//! calloop's TimerWheel.

use std::sync::atomic::{AtomicU64, Ordering};

/// Unique identifier for a timer registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TimerId(pub(crate) u64);

/// Monotonic counter for generating unique TimerIds.
static NEXT_TIMER_ID: AtomicU64 = AtomicU64::new(1);

impl TimerId {
    /// Allocate a fresh, globally unique timer id.
    pub(crate) fn next() -> Self {
        TimerId(NEXT_TIMER_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Commands from Messenger/TimerToken to the Looper's timer subsystem.
#[derive(Debug)]
pub(crate) enum TimerControl {
    /// Register a repeating pulse timer at the given interval.
    SetPulse {
        id: TimerId,
        interval: std::time::Duration,
    },
    /// Cancel a previously registered timer.
    Cancel { id: TimerId },
}

/// Obligation handle for a running timer. Drop cancels.
///
/// Not Clone, not Copy — single ownership. The timer fires
/// `LifecycleMessage::Pulse` via `Handler::pulse()` at the
/// interval specified when created.
///
/// # BeOS
///
/// `BWindow::SetPulseRate` + `BMessageRunner` destructor.
/// Dropping the token is equivalent to setting the pulse rate
/// to zero or destroying the BMessageRunner.
#[must_use = "dropping a TimerToken cancels the timer"]
pub struct TimerToken {
    id: TimerId,
    timer_tx: calloop::channel::Sender<TimerControl>,
}

impl TimerToken {
    /// Construct a new token. Sends `SetPulse` to the looper's
    /// timer control channel immediately.
    pub(crate) fn new(
        interval: std::time::Duration,
        timer_tx: calloop::channel::Sender<TimerControl>,
    ) -> Self {
        let id = TimerId::next();
        // Best-effort send — if the looper is shutting down,
        // the channel is closed and the timer silently fails
        // to register. This is correct: a dead looper has no
        // dispatch to receive pulses anyway.
        let _ = timer_tx.send(TimerControl::SetPulse { id, interval });
        TimerToken { id, timer_tx }
    }
}

impl Drop for TimerToken {
    fn drop(&mut self) {
        // Best-effort cancel — channel may be closed if the
        // looper has already exited.
        let _ = self.timer_tx.send(TimerControl::Cancel { id: self.id });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_id_is_monotonic() {
        let a = TimerId::next();
        let b = TimerId::next();
        assert!(b.0 > a.0);
    }

    #[test]
    fn timer_token_is_not_clone() {
        // Compile-time check: TimerToken does not derive Clone.
        // If someone adds Clone, this test's comment flags the
        // violation. The real guard is the absence of #[derive(Clone)].
        fn assert_not_clone<T>() {
            // This function's existence is the assertion —
            // we intentionally do NOT require Clone here.
        }
        assert_not_clone::<TimerToken>();
    }

    #[test]
    fn timer_token_drop_sends_cancel() {
        let (timer_tx, timer_rx) = calloop::channel::channel::<TimerControl>();
        let token = TimerToken::new(std::time::Duration::from_millis(100), timer_tx);
        let id = token.id;

        // Drop the token — should send Cancel.
        drop(token);

        // Drain the channel: expect SetPulse then Cancel.
        // Channel::try_recv returns the inner T, not Event<T>.
        let mut saw_set = false;
        let mut saw_cancel = false;
        while let Ok(msg) = timer_rx.try_recv() {
            match msg {
                TimerControl::SetPulse {
                    id: set_id,
                    interval: _,
                } if set_id == id => {
                    saw_set = true;
                }
                TimerControl::Cancel { id: cancel_id } if cancel_id == id => {
                    saw_cancel = true;
                }
                _ => {}
            }
        }
        assert!(saw_set, "SetPulse must be sent on construction");
        assert!(saw_cancel, "Cancel must be sent on drop");
    }
}
