//! Request/reply dispatch state.
//!
//! Each send_request installs a one-shot callback entry. When
//! the reply arrives, the entry is consumed and the callback
//! fires. Keyed by (ConnectionId, Token).
//!
//! Static protocol dispatch (Handles<P> fn pointers) lives in
//! the service dispatch table on PaneBuilder/looper. Dispatch<H>
//! handles the dynamic request/reply correlation.

use std::collections::HashMap;
use pane_proto::Flow;

/// Connection identity. Stub — will be assigned by the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(pub u64);

/// Request token. Unique per Connection (AtomicU64).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token(pub u64);

/// A one-shot dispatch entry for a pending request/reply.
/// Type-erased at storage time; the callbacks capture the
/// concrete types via closure.
pub(crate) struct DispatchEntry<H> {
    pub on_reply: Box<dyn FnOnce(&mut H, &crate::Messenger, Box<dyn std::any::Any + Send>) -> Flow + Send>,
    pub on_failed: Box<dyn FnOnce(&mut H, &crate::Messenger) -> Flow + Send>,
}

/// The dynamic handler store for request/reply.
/// Internal to the looper — not in the handler API.
pub struct Dispatch<H> {
    entries: HashMap<(ConnectionId, Token), DispatchEntry<H>>,
    next_token: u64,
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
    pub(crate) fn insert(
        &mut self,
        connection: ConnectionId,
        entry: DispatchEntry<H>,
    ) -> Token {
        let token = Token(self.next_token);
        self.next_token += 1;
        self.entries.insert((connection, token), entry);
        token
    }

    /// Consume an entry and fire on_reply (E-React analogy).
    /// Returns None if the token doesn't exist (already consumed/cancelled).
    pub(crate) fn fire_reply(
        &mut self,
        connection: ConnectionId,
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
        connection: ConnectionId,
        token: Token,
        handler: &mut H,
        messenger: &crate::Messenger,
    ) -> Option<Flow> {
        let entry = self.entries.remove(&(connection, token))?;
        Some((entry.on_failed)(handler, messenger))
    }

    /// Cancel an entry without firing callbacks (S5).
    pub(crate) fn cancel(&mut self, connection: ConnectionId, token: Token) -> bool {
        self.entries.remove(&(connection, token)).is_some()
    }

    /// Fire on_failed for all entries keyed to a specific connection (S4).
    /// Called during destruction sequence step 1.
    pub(crate) fn fail_connection(
        &mut self,
        connection: ConnectionId,
        handler: &mut H,
        messenger: &crate::Messenger,
    ) {
        let keys: Vec<_> = self.entries.keys()
            .filter(|(c, _)| *c == connection)
            .copied()
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
        let mut handler = TestHandler { replies: vec![], failures: vec![] };
        let messenger = Messenger::new();
        let conn = ConnectionId(1);

        let token = dispatch.insert(conn, DispatchEntry {
            on_reply: Box::new(|h: &mut TestHandler, _, payload| {
                let msg = payload.downcast::<String>().unwrap();
                h.replies.push(*msg);
                Flow::Continue
            }),
            on_failed: Box::new(|h: &mut TestHandler, _| {
                h.failures.push("failed".into());
                Flow::Continue
            }),
        });

        assert_eq!(dispatch.len(), 1);

        let flow = dispatch.fire_reply(
            conn, token, &mut handler, &messenger,
            Box::new("hello".to_string()),
        );
        assert_eq!(flow, Some(Flow::Continue));
        assert_eq!(handler.replies, vec!["hello"]);
        assert_eq!(dispatch.len(), 0); // consumed
    }

    #[test]
    fn fire_reply_consumes_entry() {
        let mut dispatch = Dispatch::new();
        let mut handler = TestHandler { replies: vec![], failures: vec![] };
        let messenger = Messenger::new();
        let conn = ConnectionId(1);

        let token = dispatch.insert(conn, DispatchEntry {
            on_reply: Box::new(|_, _, _| Flow::Continue),
            on_failed: Box::new(|_, _| Flow::Continue),
        });

        // First fire succeeds
        assert!(dispatch.fire_reply(conn, token, &mut handler, &messenger, Box::new(())).is_some());
        // Second fire returns None — entry consumed
        assert!(dispatch.fire_reply(conn, token, &mut handler, &messenger, Box::new(())).is_none());
    }

    #[test]
    fn cancel_removes_without_callback() {
        let mut dispatch = Dispatch::new();
        let conn = ConnectionId(1);

        let token = dispatch.insert(conn, DispatchEntry::<TestHandler> {
            on_reply: Box::new(|_, _, _| panic!("should not fire")),
            on_failed: Box::new(|_, _| panic!("should not fire")),
        });

        assert!(dispatch.cancel(conn, token));
        assert_eq!(dispatch.len(), 0);
    }

    #[test]
    fn fail_connection_fires_on_failed_for_matching_entries() {
        let mut dispatch = Dispatch::new();
        let mut handler = TestHandler { replies: vec![], failures: vec![] };
        let messenger = Messenger::new();

        let conn1 = ConnectionId(1);
        let conn2 = ConnectionId(2);

        dispatch.insert(conn1, DispatchEntry {
            on_reply: Box::new(|_, _, _| Flow::Continue),
            on_failed: Box::new(|h: &mut TestHandler, _| {
                h.failures.push("conn1".into());
                Flow::Continue
            }),
        });
        dispatch.insert(conn2, DispatchEntry {
            on_reply: Box::new(|_, _, _| Flow::Continue),
            on_failed: Box::new(|h: &mut TestHandler, _| {
                h.failures.push("conn2".into());
                Flow::Continue
            }),
        });

        // Fail connection 1 only
        dispatch.fail_connection(conn1, &mut handler, &messenger);

        assert_eq!(handler.failures, vec!["conn1"]);
        assert_eq!(dispatch.len(), 1); // conn2 entry remains
    }

    #[test]
    fn clear_drops_all_without_callbacks() {
        let mut dispatch = Dispatch::<TestHandler>::new();
        let conn = ConnectionId(1);

        dispatch.insert(conn, DispatchEntry {
            on_reply: Box::new(|_, _, _| panic!("should not fire")),
            on_failed: Box::new(|_, _| panic!("should not fire")),
        });
        dispatch.insert(conn, DispatchEntry {
            on_reply: Box::new(|_, _, _| panic!("should not fire")),
            on_failed: Box::new(|_, _| panic!("should not fire")),
        });

        dispatch.clear();
        assert_eq!(dispatch.len(), 0);
    }

    #[test]
    fn fire_failed_delivers_failure_to_handler() {
        let mut dispatch = Dispatch::new();
        let mut handler = TestHandler { replies: vec![], failures: vec![] };
        let messenger = Messenger::new();
        let conn = ConnectionId(1);

        let token = dispatch.insert(conn, DispatchEntry {
            on_reply: Box::new(|h: &mut TestHandler, _, payload| {
                let msg = payload.downcast::<String>().unwrap();
                h.replies.push(*msg);
                Flow::Continue
            }),
            on_failed: Box::new(|h: &mut TestHandler, _| {
                h.failures.push("single_failed".into());
                Flow::Continue
            }),
        });

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
        let conn = ConnectionId(1);

        let t1 = dispatch.insert(conn, DispatchEntry {
            on_reply: Box::new(|_, _, _| Flow::Continue),
            on_failed: Box::new(|_, _| Flow::Continue),
        });
        let t2 = dispatch.insert(conn, DispatchEntry {
            on_reply: Box::new(|_, _, _| Flow::Continue),
            on_failed: Box::new(|_, _| Flow::Continue),
        });

        assert_ne!(t1, t2); // S1: token uniqueness
    }
}
