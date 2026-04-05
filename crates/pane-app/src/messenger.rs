//! Messenger: scoped pane handle for handler use.
//!
//! Wraps a Handle (pane identity) + ServiceRouter (which Connection
//! for which service). Cloneable, Send. Passed to handler methods
//! by the looper at dispatch time.
//!
//! Plan 9: like a fid — resolution happens once at open time;
//! the result is a direct binding, not a name.

use std::marker::PhantomData;

/// Scoped handle to a pane. The pane ID is baked in.
/// The handler receives this from the looper and uses it to
/// send messages, set content, manage timers, etc.
#[derive(Clone)]
pub struct Messenger {
    // TODO: Handle (pane identity) + ServiceRouter
    _private: PhantomData<()>,
}

impl Messenger {
    /// Stub — will be constructed by the looper.
    pub(crate) fn new() -> Self {
        Messenger { _private: PhantomData }
    }

    /// Set the pane's body content.
    pub fn set_content(&self, _data: &[u8]) {
        // TODO: send to server via Handle
    }

    /// Set the pulse timer interval. Returns a TimerToken
    /// whose Drop cancels the timer.
    pub fn set_pulse_rate(&self, _duration: std::time::Duration) -> TimerToken {
        // TODO: register timer with looper
        TimerToken { _private: PhantomData }
    }
}

/// Handle for a running timer. Drop cancels.
#[must_use = "dropping a TimerToken cancels the timer"]
pub struct TimerToken {
    _private: PhantomData<()>,
}

impl Drop for TimerToken {
    fn drop(&mut self) {
        // TODO: send CancelTimer to looper
    }
}

/// Marker trait for fire-and-forget messages via post_app_message.
/// Requires Clone (prevents smuggling obligation handles).
pub trait AppPayload: Clone + Send + 'static {}

// Blanket impl
impl<T: Clone + Send + 'static> AppPayload for T {}
