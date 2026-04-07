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

use pane_proto::Flow;
use std::collections::HashMap;

/// Scopes dispatch entries by peer. Distinct from the server's
/// ConnectionId — this is looper-internal, the server never sees it.
/// Named PeerScope (not ConnectionId) to avoid confusion with
/// pane-session's server-side ConnectionId.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerScope(pub u64);

/// Request token. Unique per Dispatch instance (monotonic counter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token(pub u64);

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

/// The dynamic handler store for request/reply.
/// Internal to the looper — not in the handler API.
pub struct Dispatch<H> {
    entries: HashMap<(PeerScope, Token), DispatchEntry<H>>,
    next_token: u64,
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
            next_token: 0,
        }
    }

    /// Install a dispatch entry (E-Suspend analogy).
    /// Returns the Token for cancellation.
    pub(crate) fn insert(&mut self, connection: PeerScope, entry: DispatchEntry<H>) -> Token {
        let token = Token(self.next_token);
        self.next_token += 1;
        self.entries.insert((connection, token), entry);
        token
    }

    /// Consume an entry and fire on_reply (E-React analogy).
    /// Returns None if the token doesn't exist (already consumed/cancelled).
    pub(crate) fn fire_reply(
        &mut self,
        connection: PeerScope,
        token: Token,
        handler: &mut H,
        messenger: &crate::Messenger,
        payload: Box<dyn std::any::Any + Send>,
    ) -> Option<Flow> {
        let entry = self.entries.remove(&(connection, token))?;
        Some((entry.on_reply)(handler, messenger, payload))
    }

    /// Consume an entry and fire on_failed.
    pub(crate) fn fire_failed(
        &mut self,
        connection: PeerScope,
        token: Token,
        handler: &mut H,
        messenger: &crate::Messenger,
    ) -> Option<Flow> {
        let entry = self.entries.remove(&(connection, token))?;
        Some((entry.on_failed)(handler, messenger))
    }

    /// Cancel an entry without firing callbacks (S5).
    /// Used when Cancel message wiring lands (PLAN.md).
    #[allow(dead_code)]
    pub(crate) fn cancel(&mut self, connection: PeerScope, token: Token) -> bool {
        self.entries.remove(&(connection, token)).is_some()
    }

    /// Fire on_failed for all entries keyed to a specific connection (S4).
    /// Called during destruction sequence step 1.
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
                let _ = (entry.on_failed)(handler, messenger);
            }
        }
    }

    /// Fire on_failed for all entries on a specific session (S1 fix).
    /// Called when a service teardown is received — the consumer's
    /// outstanding requests for that session will never get replies.
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
                let _ = (entry.on_failed)(handler, messenger);
            }
        }
    }

    /// Drop all remaining entries without firing callbacks (S4 step 2).
    /// Called during destruction sequence after fail_connection.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of pending entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
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

    #[test]
    fn tokens_are_unique_per_connection() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = PeerScope(1);

        let t1 = dispatch.insert(
            conn,
            DispatchEntry {
                session_id: 0,
                on_reply: Box::new(|_, _, _| Flow::Continue),
                on_failed: Box::new(|_, _| Flow::Continue),
            },
        );
        let t2 = dispatch.insert(
            conn,
            DispatchEntry {
                session_id: 0,
                on_reply: Box::new(|_, _, _| Flow::Continue),
                on_failed: Box::new(|_, _| Flow::Continue),
            },
        );

        assert_ne!(t1, t2); // S1: token uniqueness
    }
}
