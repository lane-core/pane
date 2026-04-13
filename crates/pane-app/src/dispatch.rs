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
    pub on_reply: OnReply<H>,
    pub on_failed: OnFailed<H>,
}

/// An async dispatch entry: par-backed reply channel.
///
/// Unlike DispatchEntry<H>, this doesn't capture the handler type —
/// resolution goes through the par channel to a future on the
/// executor. Phase 4 of async migration.
pub(crate) struct AsyncDispatchEntry {
    pub sender: ReplySender,
}

/// The dynamic handler store for request/reply.
/// Internal to the looper — not in the handler API.
///
/// Pure entry storage and dispatch — handler-typed closure storage
/// that depends on `H`. Token allocation, outstanding-request
/// tracking, and failure cascade policy live in `ActiveSession`
/// (pane-session). `Dispatch<H>` stores entries keyed by
/// (PeerScope, Token) and fires callbacks on resolution.
///
/// Design heritage: N1+N2 boundary — Dispatch<H> is the
/// handler-typed layer (N2), ActiveSession is the protocol layer
/// (N1). The split ensures session-layer state is H-independent.
pub struct Dispatch<H> {
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
            entries: HashMap::new(),
            async_entries: HashMap::new(),
        }
    }

    /// Install a callback dispatch entry (E-Suspend analogy).
    /// The caller provides the token (allocated by ActiveSession).
    pub(crate) fn insert(&mut self, connection: PeerScope, token: Token, entry: DispatchEntry<H>) {
        self.entries.insert((connection, token), entry);
    }

    /// Install an async dispatch entry (Phase 4 E-Suspend).
    /// The caller provides the token (allocated by ActiveSession).
    /// Returns a ReplyFuture<T> to await on the executor. The
    /// ReplySender is stored internally and resolved when the
    /// wire reply arrives.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn insert_async<T: std::any::Any + Send>(
        &mut self,
        connection: PeerScope,
        token: Token,
    ) -> ReplyFuture<T> {
        let (sender, future) = reply_channel::<T>();
        self.async_entries
            .insert((connection, token), AsyncDispatchEntry { sender });
        future
    }

    /// Consume an entry and fire on_reply (E-React analogy).
    /// Checks callback entries first, then async entries.
    /// Returns None if the token doesn't exist (already consumed/cancelled).
    /// The caller is responsible for calling
    /// `ActiveSession::record_resolution()` after a successful fire.
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
            return Some((entry.on_reply)(handler, messenger, payload));
        }
        // Async entries — resolve through par channel
        if let Some(async_entry) = self.async_entries.remove(&(connection, token)) {
            async_entry.sender.resolve(payload);
            return Some(Flow::Continue);
        }
        None
    }

    /// Consume an entry and fire on_failed.
    /// Checks callback entries first, then async entries.
    /// The caller is responsible for calling
    /// `ActiveSession::record_resolution()` after a successful fire.
    pub(crate) fn fire_failed(
        &mut self,
        connection: PeerScope,
        token: Token,
        handler: &mut H,
        messenger: &crate::Messenger,
    ) -> Option<Flow> {
        if let Some(entry) = self.entries.remove(&(connection, token)) {
            return Some((entry.on_failed)(handler, messenger));
        }
        if let Some(async_entry) = self.async_entries.remove(&(connection, token)) {
            async_entry.sender.reject(ReplyError::Failed);
            return Some(Flow::Continue);
        }
        None
    }

    /// Cancel an entry without firing callbacks (S5).
    /// For async entries, sends Cancelled through the par channel.
    /// The caller is responsible for calling
    /// `ActiveSession::record_resolution()` after a successful cancel.
    pub(crate) fn cancel(&mut self, connection: PeerScope, token: Token) -> bool {
        if self.entries.remove(&(connection, token)).is_some() {
            return true;
        }
        if let Some(async_entry) = self.async_entries.remove(&(connection, token)) {
            async_entry.sender.reject(ReplyError::Cancelled);
            return true;
        }
        false
    }

    /// Fire on_failed for a specific set of tokens (N3 cascade
    /// resolution). Called after `ActiveSession::cascade_*` returns
    /// the tokens to fail. Iterates the token list, removes
    /// matching callback and async entries, fires on_failed / sends
    /// Disconnected. Flow returns are discarded — destruction is
    /// unconditional.
    ///
    /// Design heritage: replaces both fail_connection and fail_session.
    /// The cascade policy (which tokens to fail) moved to
    /// ActiveSession; Dispatch<H> handles the H-typed callback
    /// firing.
    pub(crate) fn fail_tokens(
        &mut self,
        connection: PeerScope,
        tokens: &[Token],
        handler: &mut H,
        messenger: &crate::Messenger,
    ) {
        for &token in tokens {
            if let Some(entry) = self.entries.remove(&(connection, token)) {
                let _ = (entry.on_failed)(handler, messenger);
            } else if let Some(async_entry) = self.async_entries.remove(&(connection, token)) {
                async_entry.sender.reject(ReplyError::Disconnected);
            }
        }
    }

    /// Drop all remaining entries without firing callbacks (S4 step 2).
    /// Async entries are dropped — ReplySender::Drop sends
    /// Disconnected to prevent orphaned futures.
    /// The caller is responsible for clearing the correlator on
    /// ActiveSession if needed.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.async_entries.clear();
    }

    /// Number of pending entries (callback + async).
    pub fn len(&self) -> usize {
        self.entries.len() + self.async_entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.async_entries.is_empty()
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
        let token = Token(0);

        dispatch.insert(
            conn,
            token,
            DispatchEntry {
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
        let token = Token(0);

        dispatch.insert(
            conn,
            token,
            DispatchEntry {
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
        let token = Token(0);

        dispatch.insert(
            conn,
            token,
            DispatchEntry::<TestHandler> {
                on_reply: Box::new(|_, _, _| panic!("should not fire")),
                on_failed: Box::new(|_, _| panic!("should not fire")),
            },
        );

        assert!(dispatch.cancel(conn, token));
        assert_eq!(dispatch.len(), 0);
    }

    #[test]
    fn fail_tokens_fires_on_failed_for_matching_entries() {
        let mut dispatch = Dispatch::new();
        let mut handler = TestHandler {
            replies: vec![],
            failures: vec![],
        };
        let messenger = Messenger::stub();

        let conn = PeerScope(1);
        let t0 = Token(0);
        let t1 = Token(1);
        let t2 = Token(2);

        dispatch.insert(
            conn,
            t0,
            DispatchEntry {
                on_reply: Box::new(|_, _, _| Flow::Continue),
                on_failed: Box::new(|h: &mut TestHandler, _| {
                    h.failures.push("t0".into());
                    Flow::Continue
                }),
            },
        );
        dispatch.insert(
            conn,
            t1,
            DispatchEntry {
                on_reply: Box::new(|_, _, _| Flow::Continue),
                on_failed: Box::new(|h: &mut TestHandler, _| {
                    h.failures.push("t1".into());
                    Flow::Continue
                }),
            },
        );
        dispatch.insert(
            conn,
            t2,
            DispatchEntry {
                on_reply: Box::new(|_, _, _| Flow::Continue),
                on_failed: Box::new(|h: &mut TestHandler, _| {
                    h.failures.push("t2".into());
                    Flow::Continue
                }),
            },
        );

        // Fail only t0 and t2 — t1 survives
        dispatch.fail_tokens(conn, &[t0, t2], &mut handler, &messenger);

        assert_eq!(handler.failures.len(), 2);
        assert!(handler.failures.contains(&"t0".to_string()));
        assert!(handler.failures.contains(&"t2".to_string()));
        assert_eq!(dispatch.len(), 1); // t1 remains
    }

    #[test]
    fn clear_drops_all_without_callbacks() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = PeerScope(1);

        dispatch.insert(
            conn,
            Token(0),
            DispatchEntry {
                on_reply: Box::new(|_, _, _| panic!("should not fire")),
                on_failed: Box::new(|_, _| panic!("should not fire")),
            },
        );
        dispatch.insert(
            conn,
            Token(1),
            DispatchEntry {
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
        let token = Token(0);

        dispatch.insert(
            conn,
            token,
            DispatchEntry {
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

        // fire_failed — not fire_reply, not fail_tokens
        let flow = dispatch.fire_failed(conn, token, &mut handler, &messenger);
        assert_eq!(flow, Some(Flow::Continue));
        assert_eq!(handler.failures, vec!["single_failed"]); // on_failed fired
        assert!(handler.replies.is_empty()); // on_reply did NOT fire
        assert_eq!(dispatch.len(), 0); // entry consumed
    }

    // Token monotonicity, counter arithmetic, and cap enforcement
    // are tested on RequestCorrelator and ActiveSession directly
    // in pane-session. Tests here focus on entry storage and
    // callback dispatch.

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
        let token = Token(0);

        let future = dispatch.insert_async::<String>(conn, token);
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
        let token = Token(0);

        let future = dispatch.insert_async::<u32>(conn, token);

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
        let token = Token(0);

        let future = dispatch.insert_async::<u32>(conn, token);
        assert!(dispatch.cancel(conn, token));
        assert_eq!(dispatch.len(), 0);

        let result = futures::executor::block_on(future.recv());
        assert_eq!(result.unwrap_err(), ReplyError::Cancelled);
    }

    #[test]
    fn async_fail_tokens_delivers_disconnected() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let mut handler = TestHandler {
            replies: vec![],
            failures: vec![],
        };
        let messenger = Messenger::stub();
        let conn = PeerScope(1);
        let t0 = Token(0);
        let t1 = Token(1);

        let f1 = dispatch.insert_async::<u32>(conn, t0);
        let f2 = dispatch.insert_async::<String>(conn, t1);
        assert_eq!(dispatch.len(), 2);

        dispatch.fail_tokens(conn, &[t0, t1], &mut handler, &messenger);
        assert_eq!(dispatch.len(), 0);

        let r1 = futures::executor::block_on(f1.recv());
        let r2 = futures::executor::block_on(f2.recv());
        assert_eq!(r1.unwrap_err(), ReplyError::Disconnected);
        assert_eq!(r2.unwrap_err(), ReplyError::Disconnected);
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
        let sync_token = Token(0);
        let async_token = Token(1);

        // One callback entry
        dispatch.insert(
            conn,
            sync_token,
            DispatchEntry {
                on_reply: Box::new(|h: &mut TestHandler, _, payload| {
                    let msg = payload.downcast::<String>().unwrap();
                    h.replies.push(*msg);
                    Flow::Continue
                }),
                on_failed: Box::new(|_, _| Flow::Continue),
            },
        );

        // One async entry
        let future = dispatch.insert_async::<String>(conn, async_token);

        assert_eq!(dispatch.len(), 2);

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

        let result = futures::executor::block_on(future.recv());
        assert_eq!(result.unwrap(), "async");
    }

    #[test]
    fn clear_drops_async_entries_sends_disconnected() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = PeerScope(1);

        let f1 = dispatch.insert_async::<u32>(conn, Token(0));
        let f2 = dispatch.insert_async::<u32>(conn, Token(1));
        assert_eq!(dispatch.len(), 2);

        // clear drops ReplySenders — Drop sends Disconnected
        dispatch.clear();
        assert_eq!(dispatch.len(), 0);

        let r1 = futures::executor::block_on(f1.recv());
        let r2 = futures::executor::block_on(f2.recv());
        assert_eq!(r1.unwrap_err(), ReplyError::Disconnected);
        assert_eq!(r2.unwrap_err(), ReplyError::Disconnected);
    }
}
