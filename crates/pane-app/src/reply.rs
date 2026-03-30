//! Reply mechanism for request-response messaging.
//!
//! `ReplyPort` is the session-type handle for the request-reply
//! sub-protocol. Holding one means "I owe a reply to this conversation."
//! Calling `.reply()` completes the sub-session. Dropping without
//! replying sends `ReplyFailed` to the requestor (same contract as
//! Be's `BMessage` destructor, which sent `B_NO_REPLY`).
//!
//! Two usage patterns:
//! - **Async (looper-to-looper):** The reply goes to the requestor's
//!   looper channel, delivered as `Message::Reply`.
//! - **Blocking (non-looper caller):** The reply goes to a one-shot
//!   channel that the caller blocks on. Used by scripting tools,
//!   worker threads, and `App` methods.
//!
//! Both use the same `ReplyPort` type — the destination is captured
//! in a closure, so the replier doesn't know or care how the reply
//! is delivered.

use crate::event::Message;

/// A one-use reply channel. Consumed by calling `reply()`.
///
/// If dropped without replying, sends `Message::ReplyFailed` to the
/// requestor automatically. This prevents the requestor from blocking
/// forever — the same contract as Be's `BMessage` destructor sending
/// `B_NO_REPLY` when a message requiring a reply was destroyed without
/// one.
///
/// # Session-type interpretation
///
/// ReplyPort represents the state "reply owed" in the sub-session
/// `Send<Request, Recv<Reply, End>>`. Calling `.reply()` transitions
/// to End. Drop without reply transitions to a failure terminal.
/// Rust's ownership system enforces that exactly one of these happens.
#[must_use = "dropping a ReplyPort without replying sends ReplyFailed to the requestor"]
pub struct ReplyPort {
    token: u64,
    reply_fn: Option<Box<dyn FnOnce(Message) + Send>>,
}

impl ReplyPort {
    /// Create a ReplyPort that sends replies via the given closure.
    pub(crate) fn new(token: u64, reply_fn: impl FnOnce(Message) + Send + 'static) -> Self {
        Self {
            token,
            reply_fn: Some(Box::new(reply_fn)),
        }
    }

    /// The token identifying this conversation.
    pub fn token(&self) -> u64 {
        self.token
    }

    /// Send a reply, completing the sub-session.
    ///
    /// The payload is delivered to the requestor as `Message::Reply`.
    /// This consumes the ReplyPort — you can't reply twice.
    pub fn reply(mut self, payload: impl Send + 'static) {
        if let Some(f) = self.reply_fn.take() {
            f(Message::Reply {
                token: self.token,
                payload: Box::new(payload),
            });
        }
    }
}

impl Drop for ReplyPort {
    fn drop(&mut self) {
        // If reply_fn is still Some, reply() was never called.
        if let Some(f) = self.reply_fn.take() {
            f(Message::ReplyFailed { token: self.token });
        }
    }
}

impl std::fmt::Debug for ReplyPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReplyPort")
            .field("token", &self.token)
            .field("replied", &self.reply_fn.is_none())
            .finish()
    }
}
