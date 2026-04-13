//! Par-backed reply futures for async request/reply resolution.
//!
//! Phase 4 of the async migration: executor-driven par Recv for
//! request/reply. Creates par exchange pairs where the Send side
//! lives in Dispatch and the Recv side is a future on the executor.
//!
//! When a wire reply arrives, Dispatch calls ReplySender::resolve()
//! which fires par's send1() -> wakes the executor -> future
//! completes with the typed reply payload.
//!
//! Waker chain:
//!   wire reply -> dispatch_reply -> ReplySender::resolve(payload)
//!     -> par::exchange::Send::send1(Ok(payload))
//!       -> futures::channel::oneshot::send()
//!         -> Waker::wake()
//!           -> calloop Ping::ping()
//!             -> executor polls future
//!               -> ReplyFuture::recv() completes
//!
//! Design heritage: Plan 9 devmnt mountmux correlated tags to
//! sleeping readers (devmnt.c:803). EAct's E-Suspend / E-React
//! ([FH] section 6): insert_async = E-Suspend, resolve = E-React.
//! par's Send/Recv duality encodes the obligation: every request
//! MUST receive exactly one resolution (reply, failure, or
//! disconnect).
//!
//! [N3] improvement: ReplySender's Drop guarantees resolution,
//! preventing orphaned futures. catch_unwind absorbs the edge case
//! where the future was already cancelled (executor destroyed
//! before dispatch resolution).

use pane_session::par::exchange::{Recv as ParRecv, Send as ParSend};
use pane_session::par::Session;
use std::any::Any;
use std::marker::PhantomData;

/// Why a reply future resolved without a successful payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyError {
    /// Provider sent a Failed message.
    Failed,
    /// Request was cancelled before reply arrived.
    Cancelled,
    /// Connection lost or dispatch cleared during shutdown.
    Disconnected,
}

/// Type-erased reply payload carried through the par channel.
type ReplyPayload = Result<Box<dyn Any + Send>, ReplyError>;

/// Send-side of a par-backed reply channel.
///
/// Stored in Dispatch's async_entries map. When a wire reply
/// arrives, Dispatch removes this entry and calls resolve().
/// On failure, reject(). Drop sends Disconnected to prevent
/// orphaned futures ([N3]).
///
/// The inner Option enables the Drop impl to take ownership
/// of the par Send for the final resolution without
/// double-consuming.
pub(crate) struct ReplySender {
    inner: Option<ParSend<ReplyPayload>>,
}

impl ReplySender {
    /// Resolve with a successful reply payload.
    /// Consumes self — the entry is removed from Dispatch first.
    pub(crate) fn resolve(mut self, payload: Box<dyn Any + Send>) {
        if let Some(sender) = self.inner.take() {
            sender.send1(Ok(payload));
        }
    }

    /// Reject with an error (Failed, Cancelled, Disconnected).
    /// Consumes self — the entry is removed from Dispatch first.
    pub(crate) fn reject(mut self, error: ReplyError) {
        if let Some(sender) = self.inner.take() {
            sender.send1(Err(error));
        }
    }
}

impl Drop for ReplySender {
    fn drop(&mut self) {
        if let Some(sender) = self.inner.take() {
            // The ReplyFuture (Recv side) may already be dropped
            // if the executor was destroyed before dispatch
            // resolution (e.g., looper shutdown sequence). par's
            // send1 panics with "receiver dropped" in that case.
            // catch_unwind absorbs it — the future is gone, nobody
            // to notify.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                sender.send1(Err(ReplyError::Disconnected));
            }));
        }
    }
}

/// Receive-side of a par-backed reply channel.
///
/// Wraps par's Recv with PhantomData<T> for type-safe downcasting.
/// The async recv() method awaits the par channel and downcasts
/// the type-erased payload to T.
///
/// Consumed by futures spawned on the calloop executor.
/// Runs on the looper thread (!Send executor, I6).
pub struct ReplyFuture<T> {
    recv: ParRecv<ReplyPayload>,
    _marker: PhantomData<T>,
}

impl<T: Any + Send> ReplyFuture<T> {
    /// Await the reply. Returns the typed payload on success,
    /// or ReplyError on failure/cancellation/disconnect.
    pub async fn recv(self) -> Result<T, ReplyError> {
        let result = self.recv.recv1().await;
        match result {
            Ok(boxed) => {
                let concrete = boxed
                    .downcast::<T>()
                    .expect("ReplyFuture type mismatch: dispatch sent wrong type");
                Ok(*concrete)
            }
            Err(e) => Err(e),
        }
    }
}

/// Create a par-backed reply channel pair.
///
/// Returns (sender, future). Sender goes into Dispatch's
/// async_entries map, future goes onto the calloop executor.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn reply_channel<T: Any + Send>() -> (ReplySender, ReplyFuture<T>) {
    // fork_sync creates the dual pair synchronously. The closure
    // receives the Send side; we stash it via a mutable slot.
    // Same pattern as Phase 3's Enqueue/Dequeue creation in
    // looper.rs:633-696.
    let mut send_slot: Option<ParSend<ReplyPayload>> = None;
    let recv: ParRecv<ReplyPayload> = ParRecv::fork_sync(|send| {
        send_slot = Some(send);
    });
    let send = send_slot.expect("fork_sync must deliver Send");

    let sender = ReplySender { inner: Some(send) };
    let future = ReplyFuture {
        recv,
        _marker: PhantomData,
    };
    (sender, future)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_channel_resolve_delivers_payload() {
        let (sender, future) = reply_channel::<String>();
        sender.resolve(Box::new("hello".to_string()));
        let result = futures::executor::block_on(future.recv());
        assert_eq!(result.unwrap(), "hello");
    }

    #[test]
    fn reply_channel_reject_delivers_error() {
        let (sender, future) = reply_channel::<String>();
        sender.reject(ReplyError::Failed);
        let result = futures::executor::block_on(future.recv());
        assert_eq!(result.unwrap_err(), ReplyError::Failed);
    }

    #[test]
    fn reply_channel_reject_cancelled() {
        let (sender, future) = reply_channel::<u64>();
        sender.reject(ReplyError::Cancelled);
        let result = futures::executor::block_on(future.recv());
        assert_eq!(result.unwrap_err(), ReplyError::Cancelled);
    }

    #[test]
    fn reply_sender_drop_sends_disconnected() {
        let (sender, future) = reply_channel::<u32>();
        drop(sender); // Drop without explicit resolve/reject
        let result = futures::executor::block_on(future.recv());
        assert_eq!(result.unwrap_err(), ReplyError::Disconnected);
    }

    #[test]
    fn reply_sender_drop_absorbs_panic_when_receiver_gone() {
        let (sender, future) = reply_channel::<u32>();
        drop(future); // Drop receiver first
        drop(sender); // Should not panic — catch_unwind absorbs
    }

    #[test]
    fn reply_channel_typed_downcast() {
        // Verify the type-erased round-trip preserves the value
        let (sender, future) = reply_channel::<Vec<u8>>();
        let payload = vec![1u8, 2, 3, 4];
        sender.resolve(Box::new(payload.clone()));
        let result = futures::executor::block_on(future.recv());
        assert_eq!(result.unwrap(), payload);
    }
}
