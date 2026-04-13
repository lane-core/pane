//! Request/reply dispatch state.
//!
//! Each send_request installs a one-shot callback entry. When
//! the reply arrives, the entry is consumed and the callback
//! fires. Keyed by (PeerScope, Token).
//!
//! Static protocol dispatch (Handles<P> fn pointers) lives in
//! the service dispatch table on PaneBuilder/looper. Dispatch<H>
//! handles the dynamic request/reply correlation.
//!
//! Design heritage: Plan 9's devmnt tracked outstanding request
//! tags and matched replies via mountmux
//! (reference/plan9/src/sys/src/9/port/devmnt.c, mountmux()).
//! devmnt.c:803 gates readers onto the mount point one at a time.
//!
//! Theoretical basis: EAct's handler store σ maps (session, role)
//! to handler closures (Fowler/Hu, "Speak Now: Safe Actor
//! Programming with Multiparty Session Types," §3.3 Operational
//! Semantics). pane reinterprets this as token→continuation:
//! insert is E-Suspend (register callback), fire_reply is E-React
//! (consume and invoke).

use crate::reply_future::{reply_channel, ReplyError, ReplyFuture, ReplySender};
use pane_proto::Flow;
use pane_session::RequestCorrelator;
use std::collections::HashMap;

// Re-export so existing `use crate::dispatch::{PeerScope, Token}` paths work.
pub use pane_session::{PeerScope, Token};

/// A one-shot dispatch entry for a pending request/reply.
/// Type-erased at storage time; the callbacks capture the
/// concrete types via closure.
/// Reply callback: handler + messenger + type-erased payload → Flow.
pub(crate) type OnReply<H> =
    Box<dyn FnOnce(&mut H, &crate::Messenger, Box<dyn std::any::Any + Send>) -> Flow + Send>;

/// Failure callback: handler + messenger → Flow.
pub(crate) type OnFailed<H> = Box<dyn FnOnce(&mut H, &crate::Messenger) -> Flow + Send>;

pub(crate) struct DispatchEntry<H> {
    pub session_id: u16,
    pub on_reply: OnReply<H>,
    pub on_failed: OnFailed<H>,
}

/// An async dispatch entry: par-backed reply channel.
///
/// Unlike DispatchEntry<H>, this doesn't capture the handler type —
/// resolution goes through the par channel to a future on the
/// executor. Phase 4 of async migration.
pub(crate) struct AsyncDispatchEntry {
    pub session_id: u16,
    pub sender: ReplySender,
}

/// The dynamic handler store for request/reply.
/// Internal to the looper — not in the handler API.
///
/// Token allocation and outstanding-request tracking are delegated
/// to `RequestCorrelator` (pane-session). `Dispatch<H>` adds the
/// handler-typed closure storage that depends on `H`.
pub struct Dispatch<H> {
    correlator: RequestCorrelator,
    entries: HashMap<(PeerScope, Token), DispatchEntry<H>>,
    /// Par-backed async entries (Phase 4). Resolution goes through
    /// the par channel to a future on the calloop executor, not
    /// through handler callbacks.
    async_entries: HashMap<(PeerScope, Token), AsyncDispatchEntry>,
}

impl<H> Default for Dispatch<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H> Dispatch<H> {
    pub fn new() -> Self {
        Dispatch {
            correlator: RequestCorrelator::new(),
            entries: HashMap::new(),
            async_entries: HashMap::new(),
        }
    }

    /// Install a callback dispatch entry (E-Suspend analogy).
    /// Returns the Token for cancellation. Increments the
    /// outstanding request counter — every entry represents a
    /// pending request awaiting reply resolution.
    pub(crate) fn insert(&mut self, connection: PeerScope, entry: DispatchEntry<H>) -> Token {
        let token = self.correlator.allocate_token();
        self.entries.insert((connection, token), entry);
        token
    }

    /// Install an async dispatch entry (Phase 4 E-Suspend).
    /// Returns the Token and a ReplyFuture<T> for the caller to
    /// await on the executor. The ReplySender is stored internally
    /// and resolved when the wire reply arrives.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn insert_async<T: std::any::Any + Send>(
        &mut self,
        connection: PeerScope,
        session_id: u16,
    ) -> (Token, ReplyFuture<T>) {
        let token = self.correlator.allocate_token();
        let (sender, future) = reply_channel::<T>();
        self.async_entries.insert(
            (connection, token),
            AsyncDispatchEntry { session_id, sender },
        );
        (token, future)
    }

    /// Consume an entry and fire on_reply (E-React analogy).
    /// Checks callback entries first, then async entries.
    /// Returns None if the token doesn't exist (already consumed/cancelled).
    /// Decrements the outstanding request counter on removal.
    pub(crate) fn fire_reply(
        &mut self,
        connection: PeerScope,
        token: Token,
        handler: &mut H,
        messenger: &crate::Messenger,
        payload: Box<dyn std::any::Any + Send>,
    ) -> Option<Flow> {
        // Callback entries (hot path)
        if let Some(entry) = self.entries.remove(&(connection, token)) {
            self.correlator.record_resolution();
            return Some((entry.on_reply)(handler, messenger, payload));
        }
        // Async entries — resolve through par channel
        if let Some(async_entry) = self.async_entries.remove(&(connection, token)) {
            self.correlator.record_resolution();
            async_entry.sender.resolve(payload);
            return Some(Flow::Continue);
        }
        None
    }

    /// Consume an entry and fire on_failed.
    /// Checks callback entries first, then async entries.
    /// Decrements the outstanding request counter on removal.
    pub(crate) fn fire_failed(
        &mut self,
        connection: PeerScope,
        token: Token,
        handler: &mut H,
        messenger: &crate::Messenger,
    ) -> Option<Flow> {
        if let Some(entry) = self.entries.remove(&(connection, token)) {
            self.correlator.record_resolution();
            return Some((entry.on_failed)(handler, messenger));
        }
        if let Some(async_entry) = self.async_entries.remove(&(connection, token)) {
            self.correlator.record_resolution();
            async_entry.sender.reject(ReplyError::Failed);
            return Some(Flow::Continue);
        }
        None
    }

    /// Cancel an entry without firing callbacks (S5).
    /// For async entries, sends Cancelled through the par channel.
    /// Decrements the outstanding request counter on removal.
    pub(crate) fn cancel(&mut self, connection: PeerScope, token: Token) -> bool {
        if self.entries.remove(&(connection, token)).is_some() {
            self.correlator.record_resolution();
            return true;
        }
        if let Some(async_entry) = self.async_entries.remove(&(connection, token)) {
            self.correlator.record_resolution();
            async_entry.sender.reject(ReplyError::Cancelled);
            return true;
        }
        false
    }

    /// Fire on_failed for all entries keyed to a specific connection (S4).
    /// Called during destruction sequence step 1.
    /// Async entries receive Disconnected through par channel.
    /// Decrements the outstanding request counter for each removed entry.
    pub(crate) fn fail_connection(
        &mut self,
        connection: PeerScope,
        handler: &mut H,
        messenger: &crate::Messenger,
    ) {
        let keys: Vec<_> = self
            .entries
            .keys()
            .filter(|(c, _)| *c == connection)
            .copied()
            .collect();
        for key in keys {
            if let Some(entry) = self.entries.remove(&key) {
                self.correlator.record_resolution();
                let _ = (entry.on_failed)(handler, messenger);
            }
        }
        let async_keys: Vec<_> = self
            .async_entries
            .keys()
            .filter(|(c, _)| *c == connection)
            .copied()
            .collect();
        for key in async_keys {
            if let Some(async_entry) = self.async_entries.remove(&key) {
                self.correlator.record_resolution();
                async_entry.sender.reject(ReplyError::Disconnected);
            }
        }
    }

    /// Fire on_failed for all entries on a specific session (S1 fix).
    /// Called when a service teardown is received — the consumer's
    /// outstanding requests for that session will never get replies.
    /// Async entries receive Disconnected through par channel.
    /// Decrements the outstanding request counter for each removed entry.
    pub(crate) fn fail_session(
        &mut self,
        session_id: u16,
        handler: &mut H,
        messenger: &crate::Messenger,
    ) {
        let keys: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, e)| e.session_id == session_id)
            .map(|(k, _)| *k)
            .collect();
        for key in keys {
            if let Some(entry) = self.entries.remove(&key) {
                self.correlator.record_resolution();
                let _ = (entry.on_failed)(handler, messenger);
            }
        }
        let async_keys: Vec<_> = self
            .async_entries
            .iter()
            .filter(|(_, e)| e.session_id == session_id)
            .map(|(k, _)| *k)
            .collect();
        for key in async_keys {
            if let Some(async_entry) = self.async_entries.remove(&key) {
                self.correlator.record_resolution();
                async_entry.sender.reject(ReplyError::Disconnected);
            }
        }
    }

    /// Drop all remaining entries without firing callbacks (S4 step 2).
    /// Async entries are dropped — ReplySender::Drop sends
    /// Disconnected to prevent orphaned futures.
    /// Resets the outstanding request counter to zero.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.async_entries.clear();
        self.correlator.clear();
    }

    /// Number of pending entries (callback + async).
    pub fn len(&self) -> usize {
        self.entries.len() + self.async_entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.async_entries.is_empty()
    }

    /// Set the negotiated request cap from Welcome.max_outstanding_requests (D9).
    /// 0 = unlimited. Called once after handshake completes.
    /// Not yet wired from handshake — will be called when
    /// ConnectionSource lands the Welcome cap value.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn set_request_cap(&mut self, cap: u16) {
        self.correlator.set_cap(cap);
    }

    /// Current count of outstanding requests (callback + async).
    pub fn outstanding_requests(&self) -> u64 {
        self.correlator.outstanding_requests()
    }

    /// The negotiated cap. 0 = unlimited.
    pub fn request_cap(&self) -> u16 {
        self.correlator.request_cap()
    }

    /// Whether sending another request would exceed the cap.
    /// Returns false when cap is 0 (unlimited).
    pub(crate) fn would_exceed_cap(&self) -> bool {
        self.correlator.would_exceed_cap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Messenger;

    struct TestHandler {
        replies: Vec<String>,
        failures: Vec<String>,
    }

    #[test]
    fn insert_and_fire_reply() {
        let mut dispatch = Dispatch::new();
        let mut handler = TestHandler {
            replies: vec![],
            failures: vec![],
        };
        let messenger = Messenger::stub();
        let conn = PeerScope(1);

        let token = dispatch.insert(
            conn,
            DispatchEntry {
                session_id: 0,
                on_reply: Box::new(|h: &mut TestHandler, _, payload| {
                    let msg = payload.downcast::<String>().unwrap();
                    h.replies.push(*msg);
                    Flow::Continue
                }),
                on_failed: Box::new(|h: &mut TestHandler, _| {
                    h.failures.push("failed".into());
                    Flow::Continue
                }),
            },
        );

        assert_eq!(dispatch.len(), 1);

        let flow = dispatch.fire_reply(
            conn,
            token,
            &mut handler,
            &messenger,
            Box::new("hello".to_string()),
        );
        assert_eq!(flow, Some(Flow::Continue));
        assert_eq!(handler.replies, vec!["hello"]);
        assert_eq!(dispatch.len(), 0); // consumed
    }

    #[test]
    fn fire_reply_consumes_entry() {
        let mut dispatch = Dispatch::new();
        let mut handler = TestHandler {
            replies: vec![],
            failures: vec![],
        };
        let messenger = Messenger::stub();
        let conn = PeerScope(1);

        let token = dispatch.insert(
            conn,
            DispatchEntry {
                session_id: 0,
                on_reply: Box::new(|_, _, _| Flow::Continue),
                on_failed: Box::new(|_, _| Flow::Continue),
            },
        );

        // First fire succeeds
        assert!(dispatch
            .fire_reply(conn, token, &mut handler, &messenger, Box::new(()))
            .is_some());
        // Second fire returns None — entry consumed
        assert!(dispatch
            .fire_reply(conn, token, &mut handler, &messenger, Box::new(()))
            .is_none());
    }

    #[test]
    fn cancel_removes_without_callback() {
        let mut dispatch = Dispatch::new();
        let conn = PeerScope(1);

        let token = dispatch.insert(
            conn,
            DispatchEntry::<TestHandler> {
                session_id: 0,
                on_reply: Box::new(|_, _, _| panic!("should not fire")),
                on_failed: Box::new(|_, _| panic!("should not fire")),
            },
        );

        assert!(dispatch.cancel(conn, token));
        assert_eq!(dispatch.len(), 0);
    }

    #[test]
    fn fail_connection_fires_on_failed_for_matching_entries() {
        let mut dispatch = Dispatch::new();
        let mut handler = TestHandler {
            replies: vec![],
            failures: vec![],
        };
        let messenger = Messenger::stub();

        let conn1 = PeerScope(1);
        let conn2 = PeerScope(2);

        dispatch.insert(
            conn1,
            DispatchEntry {
                session_id: 0,
                on_reply: Box::new(|_, _, _| Flow::Continue),
                on_failed: Box::new(|h: &mut TestHandler, _| {
                    h.failures.push("conn1".into());
                    Flow::Continue
                }),
            },
        );
        dispatch.insert(
            conn2,
            DispatchEntry {
                session_id: 0,
                on_reply: Box::new(|_, _, _| Flow::Continue),
                on_failed: Box::new(|h: &mut TestHandler, _| {
                    h.failures.push("conn2".into());
                    Flow::Continue
                }),
            },
        );

        // Fail connection 1 only
        dispatch.fail_connection(conn1, &mut handler, &messenger);

        assert_eq!(handler.failures, vec!["conn1"]);
        assert_eq!(dispatch.len(), 1); // conn2 entry remains
    }

    #[test]
    fn clear_drops_all_without_callbacks() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = PeerScope(1);

        dispatch.insert(
            conn,
            DispatchEntry {
                session_id: 0,
                on_reply: Box::new(|_, _, _| panic!("should not fire")),
                on_failed: Box::new(|_, _| panic!("should not fire")),
            },
        );
        dispatch.insert(
            conn,
            DispatchEntry {
                session_id: 0,
                on_reply: Box::new(|_, _, _| panic!("should not fire")),
                on_failed: Box::new(|_, _| panic!("should not fire")),
            },
        );

        dispatch.clear();
        assert_eq!(dispatch.len(), 0);
    }

    #[test]
    fn fire_failed_delivers_failure_to_handler() {
        let mut dispatch = Dispatch::new();
        let mut handler = TestHandler {
            replies: vec![],
            failures: vec![],
        };
        let messenger = Messenger::stub();
        let conn = PeerScope(1);

        let token = dispatch.insert(
            conn,
            DispatchEntry {
                session_id: 0,
                on_reply: Box::new(|h: &mut TestHandler, _, payload| {
                    let msg = payload.downcast::<String>().unwrap();
                    h.replies.push(*msg);
                    Flow::Continue
                }),
                on_failed: Box::new(|h: &mut TestHandler, _| {
                    h.failures.push("single_failed".into());
                    Flow::Continue
                }),
            },
        );

        assert_eq!(dispatch.len(), 1);

        // fire_failed — not fire_reply, not fail_connection
        let flow = dispatch.fire_failed(conn, token, &mut handler, &messenger);
        assert_eq!(flow, Some(Flow::Continue));
        assert_eq!(handler.failures, vec!["single_failed"]); // on_failed fired
        assert!(handler.replies.is_empty()); // on_reply did NOT fire
        assert_eq!(dispatch.len(), 0); // entry consumed
    }

    // Token monotonicity, counter arithmetic, and cap enforcement
    // are tested on RequestCorrelator directly in pane-session's
    // correlator.rs. Tests here focus on the integration between
    // the correlator and the handler-typed entry map.

    // -- Async dispatch entry tests (Phase 4) --

    #[test]
    fn async_insert_and_fire_reply_resolves_future() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let mut handler = TestHandler {
            replies: vec![],
            failures: vec![],
        };
        let messenger = Messenger::stub();
        let conn = PeerScope(1);

        let (token, future) = dispatch.insert_async::<String>(conn, 5);
        assert_eq!(dispatch.len(), 1);

        let flow = dispatch.fire_reply(
            conn,
            token,
            &mut handler,
            &messenger,
            Box::new("async hello".to_string()),
        );
        assert_eq!(flow, Some(Flow::Continue));
        assert_eq!(dispatch.len(), 0);

        // Handler callbacks were NOT invoked — resolution goes through par channel
        assert!(handler.replies.is_empty());

        let result = futures::executor::block_on(future.recv());
        assert_eq!(result.unwrap(), "async hello");
    }

    #[test]
    fn async_fire_failed_delivers_failed_error() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let mut handler = TestHandler {
            replies: vec![],
            failures: vec![],
        };
        let messenger = Messenger::stub();
        let conn = PeerScope(1);

        let (token, future) = dispatch.insert_async::<u32>(conn, 5);

        let flow = dispatch.fire_failed(conn, token, &mut handler, &messenger);
        assert_eq!(flow, Some(Flow::Continue));
        assert_eq!(dispatch.len(), 0);

        // Handler callbacks were NOT invoked
        assert!(handler.failures.is_empty());

        let result = futures::executor::block_on(future.recv());
        assert_eq!(result.unwrap_err(), ReplyError::Failed);
    }

    #[test]
    fn async_cancel_delivers_cancelled() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = PeerScope(1);

        let (token, future) = dispatch.insert_async::<u32>(conn, 5);
        assert!(dispatch.cancel(conn, token));
        assert_eq!(dispatch.len(), 0);

        let result = futures::executor::block_on(future.recv());
        assert_eq!(result.unwrap_err(), ReplyError::Cancelled);
    }

    #[test]
    fn async_fail_connection_delivers_disconnected() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let mut handler = TestHandler {
            replies: vec![],
            failures: vec![],
        };
        let messenger = Messenger::stub();
        let conn = PeerScope(1);

        let (_t1, f1) = dispatch.insert_async::<u32>(conn, 5);
        let (_t2, f2) = dispatch.insert_async::<String>(conn, 6);
        assert_eq!(dispatch.len(), 2);

        dispatch.fail_connection(conn, &mut handler, &messenger);
        assert_eq!(dispatch.len(), 0);

        let r1 = futures::executor::block_on(f1.recv());
        let r2 = futures::executor::block_on(f2.recv());
        assert_eq!(r1.unwrap_err(), ReplyError::Disconnected);
        assert_eq!(r2.unwrap_err(), ReplyError::Disconnected);
    }

    #[test]
    fn async_fail_session_delivers_disconnected() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let mut handler = TestHandler {
            replies: vec![],
            failures: vec![],
        };
        let messenger = Messenger::stub();
        let conn = PeerScope(1);

        // Two entries on session 5, one on session 6
        let (_t1, f1) = dispatch.insert_async::<u32>(conn, 5);
        let (_t2, f2) = dispatch.insert_async::<u32>(conn, 5);
        let (_t3, f3) = dispatch.insert_async::<u32>(conn, 6);
        assert_eq!(dispatch.len(), 3);

        dispatch.fail_session(5, &mut handler, &messenger);
        assert_eq!(dispatch.len(), 1); // session 6 entry survives

        let r1 = futures::executor::block_on(f1.recv());
        let r2 = futures::executor::block_on(f2.recv());
        assert_eq!(r1.unwrap_err(), ReplyError::Disconnected);
        assert_eq!(r2.unwrap_err(), ReplyError::Disconnected);

        // Session 6 entry still live — clean it up
        dispatch.clear();
        let r3 = futures::executor::block_on(f3.recv());
        assert_eq!(r3.unwrap_err(), ReplyError::Disconnected);
    }

    #[test]
    fn mixed_sync_and_async_entries_coexist() {
        let mut dispatch = Dispatch::new();
        let mut handler = TestHandler {
            replies: vec![],
            failures: vec![],
        };
        let messenger = Messenger::stub();
        let conn = PeerScope(1);

        // One callback entry
        let sync_token = dispatch.insert(
            conn,
            DispatchEntry {
                session_id: 0,
                on_reply: Box::new(|h: &mut TestHandler, _, payload| {
                    let msg = payload.downcast::<String>().unwrap();
                    h.replies.push(*msg);
                    Flow::Continue
                }),
                on_failed: Box::new(|_, _| Flow::Continue),
            },
        );

        // One async entry
        let (async_token, future) = dispatch.insert_async::<String>(conn, 0);

        assert_eq!(dispatch.len(), 2);
        assert_eq!(dispatch.outstanding_requests(), 2);

        // Resolve the sync entry
        dispatch.fire_reply(
            conn,
            sync_token,
            &mut handler,
            &messenger,
            Box::new("sync".to_string()),
        );
        assert_eq!(handler.replies, vec!["sync"]);
        assert_eq!(dispatch.len(), 1);

        // Resolve the async entry
        dispatch.fire_reply(
            conn,
            async_token,
            &mut handler,
            &messenger,
            Box::new("async".to_string()),
        );
        assert_eq!(dispatch.len(), 0);
        assert_eq!(dispatch.outstanding_requests(), 0);

        let result = futures::executor::block_on(future.recv());
        assert_eq!(result.unwrap(), "async");
    }

    #[test]
    fn clear_drops_async_entries_sends_disconnected() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = PeerScope(1);

        let (_t1, f1) = dispatch.insert_async::<u32>(conn, 5);
        let (_t2, f2) = dispatch.insert_async::<u32>(conn, 5);
        assert_eq!(dispatch.len(), 2);

        // clear drops ReplySenders — Drop sends Disconnected
        dispatch.clear();
        assert_eq!(dispatch.len(), 0);
        assert_eq!(dispatch.outstanding_requests(), 0);

        let r1 = futures::executor::block_on(f1.recv());
        let r2 = futures::executor::block_on(f2.recv());
        assert_eq!(r1.unwrap_err(), ReplyError::Disconnected);
        assert_eq!(r2.unwrap_err(), ReplyError::Disconnected);
    }

    #[test]
    fn outstanding_requests_counts_both_entry_types() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = PeerScope(1);

        dispatch.insert(
            conn,
            DispatchEntry {
                session_id: 0,
                on_reply: Box::new(|_: &mut TestHandler, _, _| Flow::Continue),
                on_failed: Box::new(|_: &mut TestHandler, _| Flow::Continue),
            },
        );
        let (_token, _future) = dispatch.insert_async::<u32>(conn, 0);

        // Both entry types contribute to outstanding count
        assert_eq!(dispatch.outstanding_requests(), 2);
        assert_eq!(dispatch.len(), 2);

        dispatch.clear();
        assert_eq!(dispatch.outstanding_requests(), 0);
    }
}
