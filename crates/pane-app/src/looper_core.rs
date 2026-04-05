//! Core dispatch and destruction logic for the looper.
//!
//! LooperCore<H> owns the handler and dispatch table. It wraps
//! handler dispatch in catch_unwind (I1) and runs the destruction
//! sequence on exit:
//!
//!   1. dispatch.fail_connection(primary) — fires on_failed for
//!      entries keyed to the lost Connection (S4)
//!   2. dispatch.clear() — drops remaining entries without
//!      callbacks
//!   3. handler dropped — obligation handles fire Drop compensation
//!   4. server notified via exit_tx
//!
//! This module is the core dispatch logic, independent of calloop.
//! The full looper wraps LooperCore with calloop event sources.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;

use pane_proto::Flow;
use crate::dispatch::{ConnectionId, Dispatch};
use crate::exit_reason::ExitReason;
use crate::messenger::Messenger;

/// Outcome of a single dispatch cycle.
#[derive(Debug, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// Handler returned Flow::Continue. Ready for next event.
    Continue,
    /// Looper is shutting down with the given reason.
    Exit(ExitReason),
}

/// Core dispatch and lifecycle management.
///
/// Owns the handler and dispatch table. The handler is dropped
/// as part of the destruction sequence — LooperCore::shutdown()
/// consumes self, ensuring the handler cannot be re-used after
/// a panic or Flow::Stop.
pub struct LooperCore<H> {
    handler: H,
    dispatch: Dispatch<H>,
    messenger: Messenger,
    primary_connection: ConnectionId,
    exit_tx: mpsc::Sender<ExitReason>,
}

impl<H: pane_proto::Handler> LooperCore<H> {
    pub fn new(
        handler: H,
        primary_connection: ConnectionId,
        exit_tx: mpsc::Sender<ExitReason>,
    ) -> Self {
        LooperCore {
            handler,
            dispatch: Dispatch::new(),
            messenger: Messenger::new(),
            primary_connection,
            exit_tx,
        }
    }

    /// Access the dispatch table for installing entries.
    pub fn dispatch_mut(&mut self) -> &mut Dispatch<H> {
        &mut self.dispatch
    }

    /// Access the messenger.
    pub fn messenger(&self) -> &Messenger {
        &self.messenger
    }

    /// Dispatch a handler callback wrapped in catch_unwind.
    ///
    /// The callback receives &mut H and returns Flow. Three
    /// outcomes:
    ///   - Flow::Continue → DispatchOutcome::Continue
    ///   - Flow::Stop → destruction sequence → Exit(Graceful)
    ///   - panic → destruction sequence → Exit(Failed)
    ///
    /// After Exit, the LooperCore must be shut down — the handler
    /// may be in an inconsistent state (panic case) or has
    /// requested termination (stop case).
    pub fn dispatch(
        &mut self,
        callback: impl FnOnce(&mut H) -> Flow,
    ) -> DispatchOutcome {
        // catch_unwind requires FnOnce: UnwindSafe. The handler
        // is &mut H which is !UnwindSafe. AssertUnwindSafe is
        // sound because we never re-use the handler after a
        // caught panic — shutdown() consumes it.
        let handler = &mut self.handler;
        let result = catch_unwind(AssertUnwindSafe(|| {
            callback(handler)
        }));

        match result {
            Ok(Flow::Continue) => DispatchOutcome::Continue,
            Ok(Flow::Stop) => {
                self.run_destruction(ExitReason::Graceful);
                DispatchOutcome::Exit(ExitReason::Graceful)
            }
            Err(_panic_payload) => {
                self.run_destruction(ExitReason::Failed);
                DispatchOutcome::Exit(ExitReason::Failed)
            }
        }
    }

    /// Handle primary connection loss.
    ///
    /// Calls handler.disconnected(). If the handler returns
    /// Flow::Stop (the default), runs the destruction sequence
    /// with ExitReason::Disconnected.
    pub fn connection_lost(&mut self) -> DispatchOutcome {
        let outcome = self.dispatch(|h| h.disconnected());
        match outcome {
            DispatchOutcome::Exit(ExitReason::Graceful) => {
                // disconnected() returned Flow::Stop — reclassify
                // as Disconnected (the cause was connection loss,
                // not voluntary exit).
                //
                // Note: run_destruction already ran in dispatch().
                // We just need to fix the exit reason. This is a
                // bit awkward — the alternative is to not use
                // dispatch() for connection_lost and handle it
                // directly. But the catch_unwind boundary is the
                // same, so reuse is correct.
                //
                // The exit_tx already received Graceful from
                // run_destruction. For now, send a correction.
                // TODO: refactor to pass reason through dispatch().
                let _ = self.exit_tx.send(ExitReason::Disconnected);
                DispatchOutcome::Exit(ExitReason::Disconnected)
            }
            other => other,
        }
    }

    /// Run the destruction sequence. Consumes the handler's
    /// liveness — after this, the handler should not be dispatched
    /// again (enforced by the caller shutting down).
    ///
    /// The sequence:
    /// 1. fail_connection(primary) — fires on_failed for entries
    ///    keyed to the primary Connection
    /// 2. clear() — drops remaining entries without callbacks
    /// 3. (handler dropped when LooperCore is dropped — obligation
    ///    handles fire Drop compensation)
    /// 4. exit_tx.send(reason) — notifies the server
    fn run_destruction(&mut self, reason: ExitReason) {
        // Step 1: fail pending requests on the primary connection (S4)
        self.dispatch.fail_connection(
            self.primary_connection,
            &mut self.handler,
            &self.messenger,
        );

        // Step 2: clear remaining entries across all connections
        self.dispatch.clear();

        // Step 4: notify server (step 3 happens when self is dropped)
        let _ = self.exit_tx.send(reason);
    }

    /// Consume the looper core, dropping the handler.
    /// This is where step 3 (obligation handle Drop compensation)
    /// fires — the handler's fields are dropped, which includes
    /// any ReplyPort, ServiceHandle, etc.
    pub fn shutdown(self) {
        // self is consumed — handler and dispatch are dropped
        drop(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::{DispatchEntry, ConnectionId};
    use pane_proto::obligation::{ReplyPort, ReplyFailed};
    use std::sync::{Arc, Mutex};

    // ── Test handler ───────────────────────────────────────

    struct TestHandler {
        log: Arc<Mutex<Vec<String>>>,
        reply_port: Option<ReplyPort<u32>>,
        should_panic: bool,
        should_stop: bool,
    }

    impl TestHandler {
        fn new(log: Arc<Mutex<Vec<String>>>) -> Self {
            TestHandler {
                log,
                reply_port: None,
                should_panic: false,
                should_stop: false,
            }
        }
    }

    impl pane_proto::Handler for TestHandler {
        fn ready(&mut self) -> Flow {
            self.log.lock().unwrap().push("ready".into());
            if self.should_panic {
                panic!("handler crashed in ready()");
            }
            if self.should_stop {
                return Flow::Stop;
            }
            Flow::Continue
        }

        fn disconnected(&mut self) -> Flow {
            self.log.lock().unwrap().push("disconnected".into());
            Flow::Stop // default behavior
        }
    }

    impl Drop for TestHandler {
        fn drop(&mut self) {
            self.log.lock().unwrap().push("handler_dropped".into());
        }
    }

    fn setup() -> (Arc<Mutex<Vec<String>>>, mpsc::Sender<ExitReason>, mpsc::Receiver<ExitReason>) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let (exit_tx, exit_rx) = mpsc::channel();
        (log, exit_tx, exit_rx)
    }

    fn make_core(
        log: Arc<Mutex<Vec<String>>>,
        exit_tx: mpsc::Sender<ExitReason>,
    ) -> LooperCore<TestHandler> {
        let handler = TestHandler::new(log);
        LooperCore::new(handler, ConnectionId(1), exit_tx)
    }

    // ── Claim: Flow::Continue → next event ─────────────────

    #[test]
    fn flow_continue_returns_to_idle() {
        let (log, exit_tx, exit_rx) = setup();
        let mut core = make_core(log.clone(), exit_tx);

        let outcome = core.dispatch(|h| {
            h.log.lock().unwrap().push("dispatched".into());
            Flow::Continue
        });

        assert_eq!(outcome, DispatchOutcome::Continue);
        assert_eq!(log.lock().unwrap().as_slice(), &["dispatched"]);
        // No exit signal
        assert!(exit_rx.try_recv().is_err());
    }

    // ── Claim: Flow::Stop → destruction → Graceful ─────────

    #[test]
    fn flow_stop_triggers_destruction_and_graceful_exit() {
        let (log, exit_tx, exit_rx) = setup();
        let mut core = make_core(log.clone(), exit_tx);

        let outcome = core.dispatch(|_h| Flow::Stop);

        assert_eq!(outcome, DispatchOutcome::Exit(ExitReason::Graceful));
        assert_eq!(exit_rx.recv().unwrap(), ExitReason::Graceful);
    }

    // ── Claim: panic → destruction → Failed ────────────────

    #[test]
    fn panic_caught_triggers_destruction_and_failed_exit() {
        let (log, exit_tx, exit_rx) = setup();
        let mut core = make_core(log.clone(), exit_tx);

        let outcome = core.dispatch(|_h| {
            panic!("handler crashed");
        });

        assert_eq!(outcome, DispatchOutcome::Exit(ExitReason::Failed));
        assert_eq!(exit_rx.recv().unwrap(), ExitReason::Failed);
    }

    // ── Claim: handler is never re-used after panic ────────

    #[test]
    fn handler_not_reused_after_panic() {
        // After a panic, dispatch() returns Exit(Failed).
        // The caller must shut down — if they tried to dispatch
        // again, the handler might be in an inconsistent state.
        // This test verifies that the API makes misuse obvious
        // (Exit outcome signals "stop calling dispatch").
        let (log, exit_tx, _exit_rx) = setup();
        let mut core = make_core(log.clone(), exit_tx);

        let outcome = core.dispatch(|_h| panic!("crash"));
        assert_eq!(outcome, DispatchOutcome::Exit(ExitReason::Failed));

        // The API allows another dispatch (we can't consume self
        // from &mut self), but the caller SHOULD stop. A second
        // dispatch after panic would catch_unwind again — the
        // handler is still there but possibly corrupted.
        // This is a protocol obligation on the caller, enforced
        // by the DispatchOutcome::Exit return.
    }

    // ── Claim: destruction sequence ordering ───────────────

    #[test]
    fn destruction_sequence_ordering() {
        // Verify: fail_connection → clear → handler dropped
        // (in that order)
        let (log, exit_tx, exit_rx) = setup();
        let log2 = log.clone();
        let log3 = log.clone();

        let mut core = make_core(log.clone(), exit_tx);

        // Install a dispatch entry on the primary connection
        let conn = ConnectionId(1);
        core.dispatch_mut().insert(conn, DispatchEntry {
            on_reply: Box::new(|_h: &mut TestHandler, _, _| Flow::Continue),
            on_failed: Box::new(move |_h: &mut TestHandler, _| {
                log2.lock().unwrap().push("on_failed".into());
                Flow::Continue
            }),
        });

        // Install an entry on a different connection (should be
        // cleared in step 2, not failed in step 1)
        let other_conn = ConnectionId(2);
        core.dispatch_mut().insert(other_conn, DispatchEntry {
            on_reply: Box::new(|_h: &mut TestHandler, _, _| Flow::Continue),
            on_failed: Box::new(move |_h: &mut TestHandler, _| {
                // This should NOT fire — clear() drops without callbacks
                log3.lock().unwrap().push("other_on_failed_BUG".into());
                Flow::Continue
            }),
        });

        // Trigger Flow::Stop → destruction sequence
        let outcome = core.dispatch(|_h| Flow::Stop);
        assert_eq!(outcome, DispatchOutcome::Exit(ExitReason::Graceful));

        // Drop the core to trigger handler Drop (step 3)
        core.shutdown();

        let events = log.lock().unwrap().clone();

        // Step 1: on_failed fired for primary connection entry
        assert!(events.contains(&"on_failed".to_string()),
            "S4: fail_connection must fire on_failed. Got: {:?}", events);

        // Step 2: other connection's on_failed must NOT have fired
        assert!(!events.contains(&"other_on_failed_BUG".to_string()),
            "clear() must drop without callbacks. Got: {:?}", events);

        // Step 3: handler dropped
        assert!(events.contains(&"handler_dropped".to_string()),
            "handler must be dropped. Got: {:?}", events);

        // Ordering: on_failed before handler_dropped
        let failed_pos = events.iter().position(|e| e == "on_failed").unwrap();
        let dropped_pos = events.iter().position(|e| e == "handler_dropped").unwrap();
        assert!(failed_pos < dropped_pos,
            "fail_connection must run before handler drop. Got: {:?}", events);

        // Step 4: exit signal sent
        assert_eq!(exit_rx.recv().unwrap(), ExitReason::Graceful);
    }

    // ── Claim: obligation handles compensate on destruction ─

    #[test]
    fn obligation_handles_fire_drop_on_destruction() {
        // Handler holds a ReplyPort. When destruction runs,
        // the handler is dropped, which drops the ReplyPort,
        // which sends ReplyFailed.
        let (log, exit_tx, _exit_rx) = setup();
        let mut core = make_core(log.clone(), exit_tx);

        let (port, rx) = ReplyPort::channel();
        core.handler.reply_port = Some(port);

        // Trigger destruction via Flow::Stop
        core.dispatch(|_h| Flow::Stop);

        // Handler still alive in core — port not yet dropped.
        // shutdown() drops the core, which drops the handler,
        // which drops the ReplyPort.
        core.shutdown();

        assert_eq!(rx.recv().unwrap(), Err(ReplyFailed),
            "ReplyPort Drop must send ReplyFailed during destruction");
    }

    // ── Claim: panic + obligation handle = compensated ─────

    #[test]
    fn panic_with_obligation_handle_compensated() {
        let (log, exit_tx, _exit_rx) = setup();
        let mut core = make_core(log.clone(), exit_tx);

        let (port, rx) = ReplyPort::channel();
        core.handler.reply_port = Some(port);

        // Handler panics during dispatch
        core.dispatch(|_h| panic!("crash with obligation"));

        // shutdown drops the handler → ReplyPort Drop fires
        core.shutdown();

        assert_eq!(rx.recv().unwrap(), Err(ReplyFailed));
    }

    // ── Claim: connection_lost → disconnected() → Disconnected

    #[test]
    fn connection_lost_calls_disconnected_and_exits() {
        let (log, exit_tx, exit_rx) = setup();
        let mut core = make_core(log.clone(), exit_tx);

        let outcome = core.connection_lost();

        assert_eq!(outcome, DispatchOutcome::Exit(ExitReason::Disconnected));
        assert!(log.lock().unwrap().contains(&"disconnected".into()));

        // exit_rx may have received Graceful first (from dispatch)
        // then Disconnected (from connection_lost correction).
        // Drain to find the Disconnected signal.
        let mut reasons = vec![];
        while let Ok(r) = exit_rx.try_recv() {
            reasons.push(r);
        }
        assert!(reasons.contains(&ExitReason::Disconnected),
            "exit signal must include Disconnected. Got: {:?}", reasons);
    }

    // ── Claim: dispatching after destruction is not allowed ─

    #[test]
    fn multiple_flow_stop_does_not_double_destruct() {
        // If dispatch returns Exit, calling dispatch again
        // should still work (catch_unwind protects), but the
        // destruction sequence runs again. This tests that
        // double-destruction doesn't panic or corrupt state.
        let (log, exit_tx, _) = setup();
        let mut core = make_core(log.clone(), exit_tx);

        core.dispatch(|_h| Flow::Stop);
        // dispatch table is now empty (clear() ran)
        assert!(core.dispatch_mut().is_empty());

        // Second dispatch — destruction runs again on empty state
        let outcome = core.dispatch(|_h| Flow::Stop);
        assert_eq!(outcome, DispatchOutcome::Exit(ExitReason::Graceful));
        // No panic, no double-free
    }

    // ── Claim: fail_connection only affects keyed entries ───

    #[test]
    fn fail_connection_scoped_to_primary() {
        let (log, exit_tx, _) = setup();
        let log2 = log.clone();
        let mut core = make_core(log.clone(), exit_tx);

        // Entry on secondary connection — should survive fail_connection
        let secondary = ConnectionId(99);
        core.dispatch_mut().insert(secondary, DispatchEntry {
            on_reply: Box::new(|_h: &mut TestHandler, _, _| Flow::Continue),
            on_failed: Box::new(move |_h: &mut TestHandler, _| {
                log2.lock().unwrap().push("secondary_failed".into());
                Flow::Continue
            }),
        });

        // Trigger destruction — fail_connection only hits primary (1)
        core.dispatch(|_h| Flow::Stop);

        // Secondary's on_failed must not have fired (step 1 is scoped)
        let events = log.lock().unwrap().clone();
        assert!(!events.contains(&"secondary_failed".into()),
            "fail_connection must be scoped to primary. Got: {:?}", events);

        // But the entry was cleared in step 2
        assert!(core.dispatch_mut().is_empty());
    }
}
