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
//!
//! Design heritage: BeOS BLooper::task_looper()
//! (src/kits/app/Looper.cpp:1162) blocked via MessageFromPort()
//! on port_buffer_size_etc(fMsgPort) (Looper.cpp:1086), read one
//! message, then drained the port nonblocking via port_count +
//! zero-timeout loop (Looper.cpp:1187-1193) — batch read, serial
//! dispatch. Plan 9 devmnt gated callers as readers one at a time
//! per mount (devmnt.c mountio():803, "Gate readers onto the mount
//! point one at a time") and used mountmux to dispatch replies by
//! tag. pane's run() follows the same pattern: block on
//! mpsc::Receiver, dispatch sequentially, channel close =
//! disconnect.
//!
//! Theoretical basis: the active phase is a plain (non-dialogue)
//! duploid (MMM25, Mangel/Melliès/Munch-Maccagnoni 2025).
//! Sequential dispatch prevents non-associative cross-polarity
//! composition from arising — every dispatch is either (+,+) or
//! (-,-) or a single polarity crossing, never the (+,-,+) triple
//! where associativity fails (MM14b, Munch-Maccagnoni 2014b).
//! The writer monad Ψ(A) = (A, Vec<Effect>) on the positive
//! subcategory gives associative effect concatenation within
//! same-polarity batches. DLfActRiS (Hinrichsen/Krebbers/
//! Birkedal, POPL 2024) proves deadlock freedom for this
//! single-mailbox actor topology.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::mpsc;

use pane_proto::Flow;
use crate::dispatch::{PeerScope, Dispatch};
use crate::exit_reason::ExitReason;
use crate::messenger::Messenger;
use crate::service_dispatch::ServiceDispatch;

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
    service_dispatch: ServiceDispatch<H>,
    messenger: Messenger,
    primary_connection: PeerScope,
    exit_tx: mpsc::Sender<ExitReason>,
    exited: bool,
}

impl<H: pane_proto::Handler> LooperCore<H> {
    pub fn new(
        handler: H,
        primary_connection: PeerScope,
        exit_tx: mpsc::Sender<ExitReason>,
    ) -> Self {
        LooperCore {
            handler,
            dispatch: Dispatch::new(),
            service_dispatch: ServiceDispatch::new(),
            messenger: Messenger::new(),
            primary_connection,
            exit_tx,
            exited: false,
        }
    }

    /// Constructor with a pre-populated service dispatch table.
    /// Used by PaneBuilder::run_with after setup.
    pub fn with_service_dispatch(
        handler: H,
        primary_connection: PeerScope,
        exit_tx: mpsc::Sender<ExitReason>,
        service_dispatch: ServiceDispatch<H>,
    ) -> Self {
        LooperCore {
            handler,
            dispatch: Dispatch::new(),
            service_dispatch,
            messenger: Messenger::new(),
            primary_connection,
            exit_tx,
            exited: false,
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
        if self.exited {
            return DispatchOutcome::Exit(ExitReason::Failed);
        }

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
        self.exited = true;

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

    /// Dispatch a lifecycle message through the Handler's blanket
    /// Handles<Lifecycle> impl, wrapped in catch_unwind.
    pub fn dispatch_lifecycle(
        &mut self,
        msg: pane_proto::protocols::lifecycle::LifecycleMessage,
    ) -> DispatchOutcome {
        self.dispatch(|handler| {
            <H as pane_proto::Handles<pane_proto::protocols::lifecycle::Lifecycle>>::receive(
                handler, msg,
            )
        })
    }

    /// Dispatch a service frame. Parses ServiceFrame, routes by variant.
    ///
    /// Notification -> service dispatch table (static, typed).
    /// Request/Reply/Failed -> Dispatch<H> (dynamic, token-keyed).
    ///
    /// The looper parses ServiceFrame (per optics agent: token
    /// correlation is framework routing, not per-service concern).
    /// The per-service closure gets only the inner payload bytes.
    pub(crate) fn dispatch_service(&mut self, session_id: u8, payload: &[u8]) -> DispatchOutcome {
        let frame: pane_proto::ServiceFrame = match postcard::from_bytes(payload) {
            Ok(f) => f,
            Err(_) => return DispatchOutcome::Continue, // malformed — drop
        };

        match frame {
            pane_proto::ServiceFrame::Notification { payload: inner } => {
                let result = self.service_dispatch.dispatch_notification(
                    session_id, &mut self.handler, &self.messenger, &inner,
                );
                match result {
                    Some(flow) => self.flow_to_outcome(flow),
                    None => DispatchOutcome::Continue, // unknown session_id
                }
            }
            pane_proto::ServiceFrame::Request { token: _, payload: _ } => {
                // TODO: provider-side request handling
                DispatchOutcome::Continue
            }
            pane_proto::ServiceFrame::Reply { token: _, payload: _ } => {
                // TODO: consumer-side reply routing via Dispatch<H>
                DispatchOutcome::Continue
            }
            pane_proto::ServiceFrame::Failed { token: _ } => {
                // TODO: consumer-side failure routing via Dispatch<H>
                DispatchOutcome::Continue
            }
            _ => {
                // Future ServiceFrame variants (e.g. streaming).
                DispatchOutcome::Continue
            }
        }
    }

    /// Convert a Flow value into a DispatchOutcome, running
    /// the destruction sequence on Flow::Stop.
    fn flow_to_outcome(&mut self, flow: Flow) -> DispatchOutcome {
        match flow {
            Flow::Continue => DispatchOutcome::Continue,
            Flow::Stop => {
                self.run_destruction(ExitReason::Graceful);
                DispatchOutcome::Exit(ExitReason::Graceful)
            }
        }
    }

    /// Run the looper, reading LooperMessages from the channel.
    ///
    /// Sends Ready to the handler, then enters the main dispatch
    /// loop. Blocks until Flow::Stop, a handler panic, or the
    /// channel closes (reader thread exited / transport died).
    ///
    /// Consumes self — after run returns, the handler is dropped
    /// and the destruction sequence has completed.
    pub fn run(mut self, rx: mpsc::Receiver<pane_session::bridge::LooperMessage>) -> ExitReason {
        use pane_session::bridge::LooperMessage;
        use pane_proto::control::ControlMessage;

        // Main loop: read from channel, dispatch.
        // Ready arrives from the server as ControlMessage::Lifecycle(Ready)
        // — the looper does not inject it synthetically.
        let exit_reason = loop {
            match rx.recv() {
                Ok(LooperMessage::Control(ControlMessage::Lifecycle(msg))) => {
                    let outcome = self.dispatch_lifecycle(msg);
                    if let DispatchOutcome::Exit(reason) = outcome {
                        break reason;
                    }
                }
                Ok(LooperMessage::Control(_other)) => {
                    // Non-lifecycle ControlMessage — framework-internal.
                    // Service negotiation messages handled by PaneBuilder
                    // during setup.
                }
                Ok(LooperMessage::Service { session_id, payload }) => {
                    let outcome = self.dispatch_service(session_id, &payload);
                    if let DispatchOutcome::Exit(reason) = outcome {
                        break reason;
                    }
                }
                Err(_) => {
                    // Channel closed — reader thread exited (disconnect).
                    let outcome = self.connection_lost();
                    match outcome {
                        DispatchOutcome::Exit(reason) => break reason,
                        DispatchOutcome::Continue => {
                            // Handler chose to continue after disconnect,
                            // but the channel is closed — nothing to read.
                            // Force shutdown with Disconnected.
                            self.run_destruction(ExitReason::Disconnected);
                            break ExitReason::Disconnected;
                        }
                    }
                }
            }
        };

        let reason_copy = exit_reason.clone();
        self.shutdown();
        reason_copy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::{DispatchEntry, PeerScope};
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
        LooperCore::new(handler, PeerScope(1), exit_tx)
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
        // The exited flag prevents further dispatch into the
        // handler, which may be in an inconsistent state.
        let (log, exit_tx, _exit_rx) = setup();
        let mut core = make_core(log.clone(), exit_tx);

        let outcome = core.dispatch(|_h| panic!("crash"));
        assert_eq!(outcome, DispatchOutcome::Exit(ExitReason::Failed));

        // Second dispatch returns Exit immediately — the exited
        // guard prevents touching the handler.
        let outcome2 = core.dispatch(|_h| {
            panic!("should never run");
        });
        assert_eq!(outcome2, DispatchOutcome::Exit(ExitReason::Failed));
    }

    #[test]
    fn dispatch_after_exit_returns_exit_immediately() {
        // After Exit(Failed) from a panic, a second dispatch
        // must return Exit(Failed) without running the callback.
        let (log, exit_tx, _exit_rx) = setup();
        let mut core = make_core(log.clone(), exit_tx);

        // Trigger panic → Exit(Failed)
        let outcome = core.dispatch(|_h| panic!("crash"));
        assert_eq!(outcome, DispatchOutcome::Exit(ExitReason::Failed));

        // Clear the log so we can verify the callback doesn't run
        log.lock().unwrap().clear();

        // Second dispatch — callback must NOT execute
        let ran = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ran2 = ran.clone();
        let outcome2 = core.dispatch(|_h| {
            ran2.store(true, std::sync::atomic::Ordering::SeqCst);
            Flow::Continue
        });

        assert_eq!(outcome2, DispatchOutcome::Exit(ExitReason::Failed),
            "post-exit dispatch must return Exit immediately");
        assert!(!ran.load(std::sync::atomic::Ordering::SeqCst),
            "callback must not run after exit");
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
        let conn = PeerScope(1);
        core.dispatch_mut().insert(conn, DispatchEntry {
            on_reply: Box::new(|_h: &mut TestHandler, _, _| Flow::Continue),
            on_failed: Box::new(move |_h: &mut TestHandler, _| {
                log2.lock().unwrap().push("on_failed".into());
                Flow::Continue
            }),
        });

        // Install an entry on a different connection (should be
        // cleared in step 2, not failed in step 1)
        let other_conn = PeerScope(2);
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

    // ── Claim: E-Suspend → E-React end-to-end ──────────────

    #[test]
    fn e_suspend_e_react_end_to_end() {
        // The EAct core loop: E-Suspend installs a one-shot reply
        // entry → looper returns to idle → reply arrives → entry
        // consumed → on_reply fires on &mut H.
        //
        // The real looper splits the borrow between &mut Dispatch
        // and &mut H when calling fire_reply. Tests in this module
        // can access private fields, so we split the same way.
        let (log, exit_tx, _exit_rx) = setup();
        let mut core = make_core(log.clone(), exit_tx);
        let conn = PeerScope(1);

        // E-Suspend: install a dispatch entry whose on_reply
        // callback mutates the handler's log.
        let token = core.dispatch.insert(conn, DispatchEntry {
            on_reply: Box::new(|h: &mut TestHandler, _, payload| {
                let msg = payload.downcast::<String>().unwrap();
                h.log.lock().unwrap().push(format!("on_reply:{}", msg));
                Flow::Continue
            }),
            on_failed: Box::new(|h: &mut TestHandler, _| {
                h.log.lock().unwrap().push("on_failed_BUG".into());
                Flow::Continue
            }),
        });
        assert_eq!(core.dispatch.len(), 1, "E-Suspend: entry installed");

        // E-React: split the borrow to call fire_reply with both
        // &mut Dispatch and &mut H, mirroring the real looper.
        let messenger = core.messenger.clone();
        let flow = core.dispatch.fire_reply(
            conn, token,
            &mut core.handler,
            &messenger,
            Box::new("hello".to_string()),
        );

        // on_reply callback fired and modified handler state.
        assert_eq!(flow, Some(Flow::Continue));
        let events = log.lock().unwrap().clone();
        assert!(events.contains(&"on_reply:hello".to_string()),
            "E-React: on_reply must fire and modify handler state. Got: {:?}", events);

        // Entry consumed (one-shot).
        assert_eq!(core.dispatch.len(), 0, "entry consumed after fire_reply");

        // on_failed did NOT fire.
        assert!(!events.contains(&"on_failed_BUG".to_string()),
            "on_failed must not fire on successful reply. Got: {:?}", events);
    }

    // ── Claim: dispatching after destruction is not allowed ─

    #[test]
    fn multiple_flow_stop_does_not_double_destruct() {
        // After Exit, the exited flag prevents re-entering the
        // handler. A second dispatch returns Exit(Failed)
        // immediately — no double destruction, no handler access.
        let (log, exit_tx, _) = setup();
        let mut core = make_core(log.clone(), exit_tx);

        core.dispatch(|_h| Flow::Stop);
        // dispatch table is now empty (clear() ran)
        assert!(core.dispatch_mut().is_empty());

        // Second dispatch — guarded by exited flag
        let outcome = core.dispatch(|_h| Flow::Stop);
        assert_eq!(outcome, DispatchOutcome::Exit(ExitReason::Failed));
    }

    // ── Claim: fail_connection only affects keyed entries ───

    #[test]
    fn fail_connection_scoped_to_primary() {
        let (log, exit_tx, _) = setup();
        let log2 = log.clone();
        let mut core = make_core(log.clone(), exit_tx);

        // Entry on secondary connection — should survive fail_connection
        let secondary = PeerScope(99);
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

    // ── Service frame dispatch through run() ────────────────

    #[test]
    fn run_dispatches_service_notification() {
        use pane_session::bridge::LooperMessage;
        use pane_proto::ServiceFrame;
        use crate::service_dispatch::{ServiceDispatch, make_service_receiver};

        // Test protocol
        struct NotifyProto;
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
        enum NotifyMsg { Ping(String) }
        impl pane_proto::Protocol for NotifyProto {
            fn service_id() -> pane_proto::ServiceId {
                pane_proto::ServiceId::new("com.test.notify")
            }
            type Message = NotifyMsg;
        }

        struct NotifyHandler {
            log: Arc<Mutex<Vec<String>>>,
        }
        impl pane_proto::Handler for NotifyHandler {
            fn close_requested(&mut self) -> Flow { Flow::Stop }
        }
        impl pane_proto::Handles<NotifyProto> for NotifyHandler {
            fn receive(&mut self, msg: NotifyMsg) -> Flow {
                match msg {
                    NotifyMsg::Ping(s) => self.log.lock().unwrap().push(s),
                }
                Flow::Continue
            }
        }
        impl Drop for NotifyHandler {
            fn drop(&mut self) {
                self.log.lock().unwrap().push("dropped".into());
            }
        }

        let log = Arc::new(Mutex::new(Vec::new()));
        let (exit_tx, _exit_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();

        // Build service dispatch table
        let mut service_dispatch = ServiceDispatch::new();
        service_dispatch.register(3, make_service_receiver::<NotifyHandler, NotifyProto>());

        let handler = NotifyHandler { log: log.clone() };
        let core = LooperCore::with_service_dispatch(
            handler, PeerScope(1), exit_tx, service_dispatch,
        );

        // Send a service notification on session_id=3
        let inner = postcard::to_allocvec(&NotifyMsg::Ping("wired".into())).unwrap();
        let frame = ServiceFrame::Notification { payload: inner };
        let outer = postcard::to_allocvec(&frame).unwrap();
        msg_tx.send(LooperMessage::Service { session_id: 3, payload: outer }).unwrap();

        // Send CloseRequested to stop the loop
        msg_tx.send(LooperMessage::Control(
            pane_proto::control::ControlMessage::Lifecycle(
                pane_proto::protocols::lifecycle::LifecycleMessage::CloseRequested,
            ),
        )).unwrap();

        let reason = core.run(msg_rx);
        assert_eq!(reason, ExitReason::Graceful);

        let events = log.lock().unwrap().clone();
        assert!(events.contains(&"wired".to_string()),
            "handler must receive service notification. Got: {:?}", events);
    }

    // ── Vertical slice: IO → bridge → LooperCore ──────────────

    #[test]
    fn vertical_slice_transport_to_looper() {
        // End-to-end: MemoryTransport → FrameCodec → bridge reader
        // thread → mpsc → LooperCore::run() → Handler lifecycle.
        //
        // This is the proof that the vertical slice works: bytes on
        // a transport become lifecycle calls on a handler.
        use pane_session::transport::MemoryTransport;
        use pane_session::bridge::{connect_and_run, HANDSHAKE_MAX_MESSAGE_SIZE};
        use pane_session::frame::FrameCodec;
        use pane_session::handshake::{Hello, Welcome, Rejection};
        use pane_proto::control::ControlMessage;
        use pane_proto::protocols::lifecycle::LifecycleMessage;

        let log = Arc::new(Mutex::new(Vec::new()));
        let (exit_tx, exit_rx) = mpsc::channel();

        let (ct, mut st) = MemoryTransport::pair();

        // Server side: handshake + send lifecycle messages via FrameCodec.
        let server_handle = std::thread::spawn(move || {
            let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);

            // Read Hello
            let frame = codec.read_frame(&mut st).unwrap();
            let payload = match frame {
                pane_session::frame::Frame::Message { service: 0, payload } => payload,
                _ => panic!("expected Control frame"),
            };
            let _hello: Hello = postcard::from_bytes(&payload).unwrap();

            // Send Welcome
            let decision: Result<Welcome, Rejection> = Ok(Welcome {
                version: 1,
                instance_id: "vertical-slice-server".into(),
                max_message_size: 16 * 1024 * 1024,
                bindings: vec![],
            });
            let bytes = postcard::to_allocvec(&decision).unwrap();
            codec.write_frame(&mut st, 0, &bytes).unwrap();

            // Active phase: send Ready then CloseRequested
            let msg = ControlMessage::Lifecycle(LifecycleMessage::Ready);
            let bytes = postcard::to_allocvec(&msg).unwrap();
            codec.write_frame(&mut st, 0, &bytes).unwrap();

            let msg = ControlMessage::Lifecycle(LifecycleMessage::CloseRequested);
            let bytes = postcard::to_allocvec(&msg).unwrap();
            codec.write_frame(&mut st, 0, &bytes).unwrap();

            // Drop transport to close the connection.
            drop(st);
        });

        // Client side: connect, get reader channel, run looper.
        let client = connect_and_run(
            Hello {
                version: 1,
                max_message_size: 16 * 1024 * 1024,
                interests: vec![],
                provides: vec![],
            },
            ct,
        ).expect("client connect failed");

        assert_eq!(client.welcome.instance_id, "vertical-slice-server");

        // Wire the channel into LooperCore.
        let handler = TestHandler::new(log.clone());
        let core = LooperCore::new(handler, PeerScope(1), exit_tx);

        let exit_reason = core.run(client.rx);

        // Handler received Ready (from server) and CloseRequested
        // (from server). CloseRequested defaults to Flow::Stop,
        // so the looper exits gracefully.
        assert_eq!(exit_reason, ExitReason::Graceful);

        let events = log.lock().unwrap().clone();
        assert!(events.contains(&"ready".to_string()),
            "handler must receive Ready. Got: {:?}", events);
        // handler_dropped fires during shutdown
        assert!(events.contains(&"handler_dropped".to_string()),
            "handler must be dropped during shutdown. Got: {:?}", events);

        // Exit signal sent
        // Drain — may have Graceful from run_destruction + possibly others
        let mut reasons = vec![];
        while let Ok(r) = exit_rx.try_recv() {
            reasons.push(r);
        }
        assert!(reasons.contains(&ExitReason::Graceful),
            "exit signal must include Graceful. Got: {:?}", reasons);

        server_handle.join().unwrap();
    }

    #[test]
    fn vertical_slice_disconnect_during_run() {
        // Server drops transport without sending CloseRequested.
        // The reader thread sees EOF, closes the channel, and the
        // looper calls handler.disconnected() → Disconnected exit.
        use pane_session::transport::MemoryTransport;
        use pane_session::bridge::{connect_and_run, HANDSHAKE_MAX_MESSAGE_SIZE};
        use pane_session::frame::FrameCodec;
        use pane_session::handshake::{Hello, Welcome, Rejection};

        let log = Arc::new(Mutex::new(Vec::new()));
        let (exit_tx, exit_rx) = mpsc::channel();

        let (ct, mut st) = MemoryTransport::pair();

        // Server: handshake, then immediately drop.
        let server_handle = std::thread::spawn(move || {
            let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);

            let frame = codec.read_frame(&mut st).unwrap();
            let payload = match frame {
                pane_session::frame::Frame::Message { service: 0, payload } => payload,
                _ => panic!("expected Control frame"),
            };
            let _hello: Hello = postcard::from_bytes(&payload).unwrap();

            let decision: Result<Welcome, Rejection> = Ok(Welcome {
                version: 1,
                instance_id: "drop-server".into(),
                max_message_size: 16 * 1024 * 1024,
                bindings: vec![],
            });
            let bytes = postcard::to_allocvec(&decision).unwrap();
            codec.write_frame(&mut st, 0, &bytes).unwrap();

            // Send Ready, then drop — simulates a server that
            // crashes after the pane becomes active.
            use pane_proto::control::ControlMessage;
            use pane_proto::protocols::lifecycle::LifecycleMessage;
            let msg = ControlMessage::Lifecycle(LifecycleMessage::Ready);
            let bytes = postcard::to_allocvec(&msg).unwrap();
            codec.write_frame(&mut st, 0, &bytes).unwrap();

            drop(st);
        });

        let client = connect_and_run(
            Hello {
                version: 1,
                max_message_size: 16 * 1024 * 1024,
                interests: vec![],
                provides: vec![],
            },
            ct,
        ).expect("client connect failed");

        let handler = TestHandler::new(log.clone());
        let core = LooperCore::new(handler, PeerScope(1), exit_tx);

        let exit_reason = core.run(client.rx);

        assert_eq!(exit_reason, ExitReason::Disconnected);

        let events = log.lock().unwrap().clone();
        assert!(events.contains(&"ready".to_string()),
            "handler must receive Ready. Got: {:?}", events);
        assert!(events.contains(&"disconnected".to_string()),
            "handler must receive disconnected. Got: {:?}", events);

        server_handle.join().unwrap();

        let mut reasons = vec![];
        while let Ok(r) = exit_rx.try_recv() {
            reasons.push(r);
        }
        assert!(reasons.contains(&ExitReason::Disconnected),
            "exit signal must include Disconnected. Got: {:?}", reasons);
    }
}
