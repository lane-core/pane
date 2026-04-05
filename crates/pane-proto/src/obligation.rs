//! Obligation handles: linear lens pattern in Rust.
//!
//! Every obligation-carrying type follows the pattern:
//!   - #[must_use] — compiler warns on unused
//!   - Move-only — no Clone
//!   - Single success method consumes self
//!   - Drop sends failure terminal
//!
//! Clarke et al. Definition 4.12: decompose state into
//! (focus, continuation). The continuation is used exactly
//! once — either the success path (.reply, .commit, .wait)
//! or the failure path (Drop).
//!
//! Obligation handles are NOT Message variants. They are
//! !Clone (move-only) and !Serialize (contain channel senders).
//! The protocol_handler macro generates a separate dispatch
//! path for obligation callbacks.

use std::sync::mpsc;

/// Failure signal sent when a ReplyPort is dropped without
/// calling .reply(). The requester's on_failed callback fires.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyFailed;

/// An obligation to reply to a request.
///
/// Linear lens structure: the request decomposes the handler's
/// interaction state into (request_data, reply_continuation).
/// The continuation is used exactly once:
///   - .reply(value) sends the value to the requester
///   - Drop sends ReplyFailed
///
/// Consuming .reply() makes double-reply a compile error.
/// Drop compensation makes forgotten replies a protocol-level
/// failure (the requester's on_failed fires) rather than a hang.
#[must_use = "dropping a ReplyPort sends ReplyFailed to the requester"]
pub struct ReplyPort<T: Send + 'static> {
    tx: Option<mpsc::Sender<Result<T, ReplyFailed>>>,
}

impl<T: Send + 'static> ReplyPort<T> {
    /// Create a ReplyPort and its receiving end.
    /// The receiver is held by the framework (Dispatch entry).
    pub fn channel() -> (ReplyPort<T>, mpsc::Receiver<Result<T, ReplyFailed>>) {
        let (tx, rx) = mpsc::channel();
        (ReplyPort { tx: Some(tx) }, rx)
    }

    /// Send a reply, consuming this obligation.
    /// The requester's on_reply callback fires with the value.
    pub fn reply(mut self, value: T) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Ok(value));
        }
    }
}

impl<T: Send + 'static> Drop for ReplyPort<T> {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Err(ReplyFailed));
        }
    }
}

/// Failure signal sent when a CompletionReplyPort is dropped
/// without calling .complete().
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionFailed;

/// An obligation to acknowledge completion.
/// Same linear lens pattern as ReplyPort, but the success
/// path carries no value — it's a pure acknowledgment.
#[must_use = "dropping a CompletionReplyPort sends CompletionFailed"]
pub struct CompletionReplyPort {
    tx: Option<mpsc::Sender<Result<(), CompletionFailed>>>,
}

impl CompletionReplyPort {
    pub fn channel() -> (CompletionReplyPort, mpsc::Receiver<Result<(), CompletionFailed>>) {
        let (tx, rx) = mpsc::channel();
        (CompletionReplyPort { tx: Some(tx) }, rx)
    }

    /// Acknowledge completion, consuming this obligation.
    pub fn complete(mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Ok(()));
        }
    }
}

impl Drop for CompletionReplyPort {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Err(CompletionFailed));
        }
    }
}

/// A handle to cancel a pending request.
///
/// Unlike other obligation handles, CancelHandle's Drop is a
/// no-op — the request completes normally. The obligation is
/// optional: .cancel() is voluntary abort.
///
/// Linear lens structure inverted: the "failure" path (doing
/// nothing) is the common case. The "success" path (.cancel)
/// is the active intervention.
pub struct CancelHandle {
    cancel_fn: Option<Box<dyn FnOnce() + Send>>,
}

impl CancelHandle {
    pub fn new(cancel_fn: impl FnOnce() + Send + 'static) -> Self {
        CancelHandle {
            cancel_fn: Some(Box::new(cancel_fn)),
        }
    }

    /// Cancel the pending request. Removes the Dispatch entry
    /// without firing callbacks (S5).
    pub fn cancel(mut self) {
        if let Some(f) = self.cancel_fn.take() {
            f();
        }
    }
}

impl Drop for CancelHandle {
    fn drop(&mut self) {
        // Intentional no-op. The request completes normally.
        // cancel_fn is dropped without calling it.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ReplyPort ──────────────────────────────────────────

    #[test]
    fn reply_port_sends_value_on_reply() {
        let (port, rx) = ReplyPort::channel();
        port.reply(42u32);
        assert_eq!(rx.recv().unwrap(), Ok(42));
    }

    #[test]
    fn reply_port_sends_failed_on_drop() {
        let (port, rx) = ReplyPort::<u32>::channel();
        drop(port);
        assert_eq!(rx.recv().unwrap(), Err(ReplyFailed));
    }

    #[test]
    fn reply_port_drop_in_handler_default() {
        // The Handler trait's default request_received does:
        //   drop(reply); Flow::Continue
        // This must send ReplyFailed, not hang.
        let (port, rx) = ReplyPort::<String>::channel();

        // Simulate the default handler behavior
        fn handle_request(reply: ReplyPort<String>) -> crate::Flow {
            drop(reply);
            crate::Flow::Continue
        }

        let flow = handle_request(port);
        assert_eq!(flow, crate::Flow::Continue);
        assert_eq!(rx.recv().unwrap(), Err(ReplyFailed));
    }

    #[test]
    fn reply_port_is_move_only() {
        // ReplyPort contains Option<mpsc::Sender<_>> which is !Clone.
        // This is a compile-time property — the test documents it.
        // Uncommenting the following would fail to compile:
        //   let (port, _rx) = ReplyPort::<u32>::channel();
        //   let port2 = port.clone();  // ERROR: no method named `clone`
    }

    #[test]
    fn reply_port_reply_consumes_self() {
        // After .reply(), the port is consumed. Using it again
        // is a compile error. This is the linear lens property:
        // the continuation is used exactly once.
        // Uncommenting the following would fail to compile:
        //   let (port, _rx) = ReplyPort::<u32>::channel();
        //   port.reply(1);
        //   port.reply(2);  // ERROR: use of moved value
        let (port, rx) = ReplyPort::channel();
        port.reply(1u32);
        assert_eq!(rx.recv().unwrap(), Ok(1));
    }

    #[test]
    fn reply_port_receiver_closed_is_silent() {
        // If the receiver is dropped (requester gave up), .reply()
        // silently succeeds (let _ = tx.send(...)). No panic.
        let (port, rx) = ReplyPort::channel();
        drop(rx);
        port.reply(42u32); // does not panic
    }

    #[test]
    fn reply_port_drop_receiver_closed_is_silent() {
        // If the receiver is dropped, Drop's failure send also
        // silently succeeds. No panic on cleanup.
        let (port, rx) = ReplyPort::<u32>::channel();
        drop(rx);
        drop(port); // does not panic
    }

    // ── CompletionReplyPort ────────────────────────────────

    #[test]
    fn completion_port_sends_ok_on_complete() {
        let (port, rx) = CompletionReplyPort::channel();
        port.complete();
        assert_eq!(rx.recv().unwrap(), Ok(()));
    }

    #[test]
    fn completion_port_sends_failed_on_drop() {
        let (port, rx) = CompletionReplyPort::channel();
        drop(port);
        assert_eq!(rx.recv().unwrap(), Err(CompletionFailed));
    }

    // ── CancelHandle ───────────────────────────────────────

    #[test]
    fn cancel_handle_fires_on_cancel() {
        let (tx, rx) = mpsc::channel();
        let handle = CancelHandle::new(move || {
            tx.send("cancelled").unwrap();
        });
        handle.cancel();
        assert_eq!(rx.recv().unwrap(), "cancelled");
    }

    #[test]
    fn cancel_handle_drop_is_noop() {
        let (tx, rx) = mpsc::channel();
        let handle = CancelHandle::new(move || {
            tx.send("cancelled").unwrap();
        });
        drop(handle);
        // cancel_fn was NOT called — receiver gets nothing
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn cancel_handle_cancel_consumes_self() {
        // After .cancel(), the handle is consumed.
        // Uncommenting would fail to compile:
        //   let handle = CancelHandle::new(|| {});
        //   handle.cancel();
        //   handle.cancel();  // ERROR: use of moved value
        let handle = CancelHandle::new(|| {});
        handle.cancel();
    }

    // ── Cross-cutting: panic safety (I1) ───────────────────

    #[test]
    fn reply_port_drop_fires_during_panic_unwind() {
        // I1: panic = unwind means Drop fires during stack unwinding.
        // ReplyPort's Drop sends ReplyFailed even when the handler
        // panics. This is the backstop — the requester gets a failure
        // signal rather than hanging forever.
        let (port, rx) = ReplyPort::<u32>::channel();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _port = port; // moved into the closure
            panic!("handler crashed");
        }));

        assert!(result.is_err()); // panic was caught
        assert_eq!(rx.recv().unwrap(), Err(ReplyFailed)); // Drop fired
    }

    #[test]
    fn reply_port_reply_before_panic_sends_value() {
        // If the handler calls .reply() before panicking, the reply
        // is sent. The subsequent panic doesn't cause a double-send
        // because .reply() consumed the port via Option::take().
        let (port, rx) = ReplyPort::channel();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            port.reply(42u32);
            panic!("handler crashed after reply");
        }));

        assert!(result.is_err());
        assert_eq!(rx.recv().unwrap(), Ok(42)); // reply arrived
        // No second message — Drop found tx already taken
        assert!(rx.try_recv().is_err());
    }
}
