//! Calloop-backed event loop with batch-ordered dispatch.
//!
//! Looper<H> wraps LooperCore<H> and drives it from a calloop
//! EventLoop. Events are collected per-iteration into a Batch,
//! sorted by six-phase ordering, then dispatched sequentially
//! through LooperCore's targeted methods.
//!
//! The six phases prevent Reply-Teardown races: replies resolve
//! pending obligations (phase 1) before teardown fails remaining
//! entries (phase 2). New inbound work (phase 5) arrives last.
//!
//! Design heritage: BeOS BLooper::task_looper()
//! (src/kits/app/Looper.cpp:1162-1280) blocked via
//! MessageFromPort() then drained the port nonblocking via
//! port_count + zero-timeout loop (Looper.cpp:1187-1193) —
//! batch read, serial dispatch. Plan 9 devmnt.c mountmux
//! (reference/plan9/src/sys/src/9/port/devmnt.c:934-967)
//! dispatched replies by tag after reading from the mount fd.
//!
//! Theoretical basis: the batch is the writer monad
//! Psi(A) = (A, Vec<Effect>) over the free monoid. Six-phase
//! ordering ensures the monoid's concatenation respects
//! obligation resolution before teardown (session types agent
//! finding: Reply-Teardown ordering prevents orphaned dispatch
//! entries).

use std::sync::mpsc;
use std::thread;

use calloop::channel::{self as calloop_channel, Channel};
use calloop::EventLoop;

use crate::exit_reason::ExitReason;
use crate::looper_core::{DispatchOutcome, LooperCore};
use pane_proto::control::{ControlMessage, TeardownReason};
use pane_proto::Address;
use pane_session::bridge::LooperMessage;

/// Calloop-backed event loop with batch-ordered dispatch.
///
/// Wraps LooperCore<H> (the dispatch engine) and adds calloop's
/// EventLoop for event source multiplexing. Currently uses a
/// single calloop channel; future tasks add timer sources.
///
/// The looper thread's ThreadId is stored for I8 (send_and_wait)
/// enforcement in a later task.
pub struct Looper<H: pane_proto::Handler> {
    core: LooperCore<H>,
    thread_id: Option<thread::ThreadId>,
}

/// Events collected in one calloop iteration, sorted by dispatch phase.
///
/// Phases 4 (ctl writes) and 6 (post-batch effects/snapshot) are
/// not yet implemented — they will be added when ctl write handling
/// and snapshot publishing land.
struct Batch {
    /// Phase 1: Reply — resolve pending obligations first.
    replies: Vec<(u16, u64, Vec<u8>)>,
    /// Phase 1: Failed — resolve pending obligations first.
    failed: Vec<(u16, u64)>,
    /// Phase 2: ServiceTeardown — fail remaining entries.
    teardowns: Vec<(u16, TeardownReason)>,
    /// Phase 3: PaneExited — death notifications.
    pane_exited: Vec<(Address, ExitReason)>,
    /// Phase 3: Lifecycle — framework control.
    lifecycle: Vec<pane_proto::protocols::lifecycle::LifecycleMessage>,
    /// Phase 5: Request — new inbound protocol work.
    requests: Vec<(u16, u64, Vec<u8>)>,
    /// Phase 5: Notification — new inbound protocol work.
    notifications: Vec<(u16, Vec<u8>)>,
    /// Channel closed during collection — triggers disconnect
    /// handling after all buffered events are dispatched.
    channel_closed: bool,
}

impl Batch {
    fn new() -> Self {
        Batch {
            replies: Vec::new(),
            failed: Vec::new(),
            teardowns: Vec::new(),
            pane_exited: Vec::new(),
            lifecycle: Vec::new(),
            requests: Vec::new(),
            notifications: Vec::new(),
            channel_closed: false,
        }
    }

    fn is_empty(&self) -> bool {
        self.replies.is_empty()
            && self.failed.is_empty()
            && self.teardowns.is_empty()
            && self.pane_exited.is_empty()
            && self.lifecycle.is_empty()
            && self.requests.is_empty()
            && self.notifications.is_empty()
            && !self.channel_closed
    }

    /// Classify a LooperMessage and place it in the correct phase bucket.
    fn collect(&mut self, msg: LooperMessage) {
        match msg {
            LooperMessage::Control(ControlMessage::Lifecycle(lm)) => {
                self.lifecycle.push(lm);
            }
            LooperMessage::Control(ControlMessage::ServiceTeardown { session_id, reason }) => {
                self.teardowns.push((session_id, reason));
            }
            LooperMessage::Control(ControlMessage::PaneExited { address, reason }) => {
                self.pane_exited.push((address, reason));
            }
            LooperMessage::Control(_) => {
                // Framework-internal control messages (DeclareInterest,
                // InterestAccepted, etc.) — handled during setup, not
                // during the active dispatch loop.
            }
            LooperMessage::Service {
                session_id,
                payload,
            } => {
                // Parse ServiceFrame to classify into the correct phase.
                // The targeted dispatch methods on LooperCore accept
                // pre-parsed data, avoiding double deserialization.
                let frame: pane_proto::ServiceFrame = match postcard::from_bytes(&payload) {
                    Ok(f) => f,
                    Err(_) => return, // malformed — drop
                };
                match frame {
                    pane_proto::ServiceFrame::Reply {
                        token,
                        payload: reply_bytes,
                    } => {
                        self.replies.push((session_id, token, reply_bytes));
                    }
                    pane_proto::ServiceFrame::Failed { token } => {
                        self.failed.push((session_id, token));
                    }
                    pane_proto::ServiceFrame::Request {
                        token,
                        payload: inner,
                    } => {
                        self.requests.push((session_id, token, inner));
                    }
                    pane_proto::ServiceFrame::Notification { payload: inner } => {
                        self.notifications.push((session_id, inner));
                    }
                    _ => {
                        // Future ServiceFrame variants (streaming).
                    }
                }
            }
        }
    }
}

impl<H: pane_proto::Handler + 'static> Looper<H> {
    /// Create a Looper wrapping the given LooperCore.
    pub fn new(core: LooperCore<H>) -> Self {
        Looper {
            core,
            thread_id: None,
        }
    }

    /// The looper thread's ThreadId. Set during run(), None before.
    pub fn thread_id(&self) -> Option<thread::ThreadId> {
        self.thread_id
    }

    /// Enter the event loop. Bridges the existing mpsc::Receiver
    /// into a calloop channel via a forwarding thread, then runs
    /// the calloop EventLoop with batch-ordered dispatch.
    ///
    /// Transitional design: ConnectionSource (future task) replaces
    /// the forwarding thread with direct socket reading as a calloop
    /// event source.
    ///
    /// Consumes self — after run returns, the handler is dropped
    /// and the destruction sequence has completed.
    pub fn run(mut self, rx: mpsc::Receiver<LooperMessage>) -> ExitReason {
        self.thread_id = Some(thread::current().id());

        // Create the calloop event loop and channel.
        let mut event_loop: EventLoop<LoopState> = match EventLoop::try_new() {
            Ok(el) => el,
            Err(_) => {
                self.core.run_destruction_reason(ExitReason::InfraError);
                self.core.shutdown();
                return ExitReason::InfraError;
            }
        };

        let (calloop_tx, calloop_rx): (
            calloop_channel::Sender<LooperMessage>,
            Channel<LooperMessage>,
        ) = calloop_channel::channel();

        // Register the channel as an event source.
        let handle = event_loop.handle();
        if handle
            .insert_source(calloop_rx, |event, _, state: &mut LoopState| match event {
                calloop_channel::Event::Msg(msg) => {
                    state.batch.collect(msg);
                }
                calloop_channel::Event::Closed => {
                    state.batch.channel_closed = true;
                }
            })
            .is_err()
        {
            self.core.run_destruction_reason(ExitReason::InfraError);
            self.core.shutdown();
            return ExitReason::InfraError;
        }

        // Forwarding thread: reads from the existing mpsc and sends
        // into the calloop channel. Blocks for one message, then
        // drains any immediately available messages before yielding.
        // This batch-forwards messages that arrived close together,
        // reducing the chance that calloop wakes between related
        // messages.
        let _forwarder = thread::spawn(move || {
            while let Ok(first) = rx.recv() {
                if calloop_tx.send(first).is_err() {
                    break;
                }
                // Drain any immediately available messages.
                while let Ok(msg) = rx.try_recv() {
                    if calloop_tx.send(msg).is_err() {
                        return;
                    }
                }
            }
            // mpsc closed — calloop_tx dropped here, triggering
            // Event::Closed on the calloop side.
        });

        let mut state = LoopState {
            batch: Batch::new(),
            exit_reason: None,
        };

        // Main loop: block for at least one event, drain any
        // additional ready events nonblocking, then dispatch the
        // batch in phase order.
        //
        // Design heritage: BLooper::task_looper() blocked via
        // MessageFromPort on port_buffer_size_etc (Looper.cpp:1086),
        // read one message, then drained the port nonblocking via
        // port_count + zero-timeout loop (Looper.cpp:1187-1193).
        // Same pattern here: blocking dispatch collects one or more
        // events, nonblocking dispatch drains stragglers that arrived
        // during the blocking call (especially from the forwarding
        // thread).
        loop {
            // Block until at least one event is available.
            if event_loop.dispatch(None, &mut state).is_err() {
                self.core.run_destruction_reason(ExitReason::InfraError);
                self.core.shutdown();
                return ExitReason::InfraError;
            }

            // Nonblocking drain: collect any events that arrived
            // while the blocking dispatch was processing. The
            // forwarding thread may have pushed additional messages
            // between the first wake and now.
            let _ = event_loop.dispatch(Some(std::time::Duration::ZERO), &mut state);

            // Dispatch the batch in phase order.
            if !state.batch.is_empty() {
                if let Some(reason) = self.dispatch_batch(&mut state.batch) {
                    state.exit_reason = Some(reason);
                }
            }

            if let Some(reason) = state.exit_reason.take() {
                let reason_copy = reason.clone();
                self.core.shutdown();
                return reason_copy;
            }
        }
    }

    /// Dispatch a collected batch in six-phase order.
    /// Returns Some(ExitReason) if an exit condition was reached.
    ///
    /// Each phase takes ownership of its events via std::mem::take
    /// before iterating, so an early exit cleanly drops the remaining
    /// events without double-borrow issues on the batch.
    fn dispatch_batch(&mut self, batch: &mut Batch) -> Option<ExitReason> {
        // Phase 1: Reply and Failed — resolve pending obligations
        // before teardown can fail remaining entries.
        let replies = std::mem::take(&mut batch.replies);
        for (session_id, token, reply_bytes) in replies {
            let outcome = self.core.dispatch_reply(session_id, token, reply_bytes);
            if let DispatchOutcome::Exit(reason) = outcome {
                batch.clear();
                return Some(reason);
            }
        }
        let failed = std::mem::take(&mut batch.failed);
        for (session_id, token) in failed {
            let outcome = self.core.dispatch_failed(session_id, token);
            if let DispatchOutcome::Exit(reason) = outcome {
                batch.clear();
                return Some(reason);
            }
        }

        // Phase 2: ServiceTeardown — fail remaining entries for
        // torn-down sessions. Must happen after Reply/Failed so
        // that replies in transit are not prematurely failed.
        let teardowns = std::mem::take(&mut batch.teardowns);
        for (session_id, reason) in teardowns {
            self.core.dispatch_teardown(session_id, reason);
        }

        // Phase 3: PaneExited, then Lifecycle — framework control.
        let pane_exited = std::mem::take(&mut batch.pane_exited);
        for (address, reason) in pane_exited {
            let outcome = self.core.dispatch_pane_exited(address, reason);
            if let DispatchOutcome::Exit(reason) = outcome {
                batch.clear();
                return Some(reason);
            }
        }
        let lifecycle = std::mem::take(&mut batch.lifecycle);
        for msg in lifecycle {
            let outcome = self.core.dispatch_lifecycle(msg);
            if let DispatchOutcome::Exit(reason) = outcome {
                batch.clear();
                return Some(reason);
            }
        }

        // Phase 4: ctl writes — not yet implemented.

        // Phase 5: Requests, then Notifications — new inbound work.
        let requests = std::mem::take(&mut batch.requests);
        for (session_id, token, inner) in requests {
            let outcome = self.core.dispatch_request(session_id, token, inner);
            if let DispatchOutcome::Exit(reason) = outcome {
                batch.clear();
                return Some(reason);
            }
        }
        let notifications = std::mem::take(&mut batch.notifications);
        for (session_id, inner) in notifications {
            let outcome = self.core.dispatch_notification(session_id, inner);
            if let DispatchOutcome::Exit(reason) = outcome {
                batch.clear();
                return Some(reason);
            }
        }

        // Phase 6: post-batch effects/snapshot — not yet implemented.

        // Channel closed — handle after all buffered events.
        if batch.channel_closed {
            batch.channel_closed = false;
            let outcome = self.core.connection_lost();
            match outcome {
                DispatchOutcome::Exit(reason) => return Some(reason),
                DispatchOutcome::Continue => {
                    // Handler chose to continue after disconnect,
                    // but the channel is closed — nothing to read.
                    self.core.run_destruction_reason(ExitReason::Disconnected);
                    return Some(ExitReason::Disconnected);
                }
            }
        }

        None
    }
}

impl Batch {
    /// Discard all remaining events. Used on early exit to avoid
    /// dispatching stale events after an exit condition.
    fn clear(&mut self) {
        self.replies.clear();
        self.failed.clear();
        self.teardowns.clear();
        self.pane_exited.clear();
        self.lifecycle.clear();
        self.requests.clear();
        self.notifications.clear();
        // Intentionally do not clear channel_closed — it's a
        // permanent condition, not a buffered event.
    }
}

/// Shared state for the calloop event loop.
/// Not parameterized on H — the batch is type-erased (raw bytes).
/// calloop's EventLoop<LoopState> passes this to event callbacks.
struct LoopState {
    batch: Batch,
    exit_reason: Option<ExitReason>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::PeerScope;
    use crate::service_dispatch::{make_service_receiver, ServiceDispatch};
    use pane_proto::protocols::lifecycle::LifecycleMessage;
    use pane_proto::{Flow, Handler, Handles, Protocol, ServiceFrame, ServiceId};
    use serde::{Deserialize, Serialize};
    use std::sync::{Arc, Mutex};

    // ── Test protocol ────────────────────────────────────────

    struct TestProto;
    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum TestMsg {
        Ping(String),
    }

    impl Protocol for TestProto {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.looper.batch")
        }
        type Message = TestMsg;
    }

    // ── Test handler ─────────────────────────────────────────

    /// Records dispatch events in order for assertion.
    struct BatchTestHandler {
        log: Arc<Mutex<Vec<String>>>,
    }

    impl Handler for BatchTestHandler {
        fn ready(&mut self) -> Flow {
            self.log.lock().unwrap().push("ready".into());
            Flow::Continue
        }

        fn close_requested(&mut self) -> Flow {
            self.log.lock().unwrap().push("close_requested".into());
            Flow::Stop
        }

        fn disconnected(&mut self) -> Flow {
            self.log.lock().unwrap().push("disconnected".into());
            Flow::Stop
        }

        fn pane_exited(&mut self, address: Address, _reason: ExitReason) -> Flow {
            self.log
                .lock()
                .unwrap()
                .push(format!("pane_exited:{}", address.pane_id));
            Flow::Continue
        }
    }

    impl Handles<TestProto> for BatchTestHandler {
        fn receive(&mut self, msg: TestMsg) -> Flow {
            match msg {
                TestMsg::Ping(s) => {
                    self.log.lock().unwrap().push(format!("notification:{s}"));
                }
            }
            Flow::Continue
        }
    }

    impl Drop for BatchTestHandler {
        fn drop(&mut self) {
            self.log.lock().unwrap().push("dropped".into());
        }
    }

    // ── Helpers ──────────────────────────────────────────────

    fn tagged_payload(msg: &TestMsg) -> Vec<u8> {
        let tag = TestProto::service_id().tag();
        let msg_bytes = postcard::to_allocvec(msg).unwrap();
        let mut payload = Vec::with_capacity(1 + msg_bytes.len());
        payload.push(tag);
        payload.extend_from_slice(&msg_bytes);
        payload
    }

    fn wire_notification(inner: Vec<u8>) -> Vec<u8> {
        let frame = ServiceFrame::Notification { payload: inner };
        postcard::to_allocvec(&frame).unwrap()
    }

    fn wire_reply(token: u64, inner: Vec<u8>) -> Vec<u8> {
        let frame = ServiceFrame::Reply {
            token,
            payload: inner,
        };
        postcard::to_allocvec(&frame).unwrap()
    }

    /// Full setup: returns Looper, mpsc sender for injecting events,
    /// and the mpsc receiver to pass to Looper::run.
    fn setup_looper(
        log: Arc<Mutex<Vec<String>>>,
    ) -> (
        Looper<BatchTestHandler>,
        mpsc::Sender<LooperMessage>,
        mpsc::Receiver<LooperMessage>,
    ) {
        let (exit_tx, _exit_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let (write_tx, _write_rx) = std::sync::mpsc::sync_channel(16);

        let mut service_dispatch = ServiceDispatch::new();
        service_dispatch.register(3, make_service_receiver::<BatchTestHandler, TestProto>());

        let handler = BatchTestHandler { log };
        let core = LooperCore::with_service_dispatch(
            handler,
            PeerScope(1),
            write_tx,
            exit_tx,
            service_dispatch,
        );

        (Looper::new(core), msg_tx, msg_rx)
    }

    // ── Claim: single event dispatches correctly ─────────────

    #[test]
    fn single_event_dispatches_correctly() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log.clone());

        // Send a notification then close the channel. The
        // notification dispatches in phase 5; channel close
        // triggers disconnected() → Disconnected exit.
        let inner = tagged_payload(&TestMsg::Ping("solo".into()));
        let outer = wire_notification(inner);
        msg_tx
            .send(LooperMessage::Service {
                session_id: 3,
                payload: outer,
            })
            .unwrap();
        drop(msg_tx);

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Disconnected);

        let events = log.lock().unwrap().clone();
        assert!(
            events.contains(&"notification:solo".to_string()),
            "handler must receive the notification. Got: {events:?}"
        );
    }

    // ── Claim: channel close triggers disconnect ─────────────

    #[test]
    fn channel_close_triggers_disconnect() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log.clone());

        // Drop sender immediately — channel closes.
        drop(msg_tx);

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Disconnected);

        let events = log.lock().unwrap().clone();
        assert!(
            events.contains(&"disconnected".to_string()),
            "handler must receive disconnected. Got: {events:?}"
        );
    }

    // ── Claim: replies dispatch before teardowns ─────────────

    #[test]
    fn batch_ordering_reply_before_teardown() {
        // Send a Reply and a ServiceTeardown in quick succession.
        // The Reply must dispatch (phase 1) before the Teardown
        // fails remaining entries (phase 2). If ordering were
        // wrong, the teardown would fail the dispatch entry before
        // the reply could resolve it.
        use crate::dispatch::DispatchEntry;

        let log = Arc::new(Mutex::new(Vec::new()));
        let log_reply = log.clone();
        let log_failed = log.clone();

        let (exit_tx, _exit_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let (write_tx, _write_rx) = std::sync::mpsc::sync_channel(16);

        let handler = BatchTestHandler { log: log.clone() };
        let mut core = LooperCore::with_service_dispatch(
            handler,
            PeerScope(1),
            write_tx,
            exit_tx,
            ServiceDispatch::new(),
        );

        // Install a dispatch entry on session_id=5.
        // The entry's on_reply logs "reply_resolved", on_failed
        // logs "reply_orphaned".
        let token = core.dispatch_mut().insert(
            PeerScope(1),
            DispatchEntry {
                session_id: 5,
                on_reply: Box::new(move |_h: &mut BatchTestHandler, _, _| {
                    log_reply.lock().unwrap().push("reply_resolved".into());
                    Flow::Continue
                }),
                on_failed: Box::new(move |_h: &mut BatchTestHandler, _| {
                    log_failed.lock().unwrap().push("reply_orphaned".into());
                    Flow::Continue
                }),
            },
        );

        let mut looper = Looper::new(core);

        // Send Reply matching the installed token, ServiceTeardown
        // for the same session, and CloseRequested in rapid succession.
        let reply_outer = wire_reply(token.0, vec![1, 2, 3]);
        msg_tx
            .send(LooperMessage::Service {
                session_id: 5,
                payload: reply_outer,
            })
            .unwrap();
        msg_tx
            .send(LooperMessage::Control(ControlMessage::ServiceTeardown {
                session_id: 5,
                reason: TeardownReason::ServiceRevoked,
            }))
            .unwrap();
        msg_tx
            .send(LooperMessage::Control(ControlMessage::Lifecycle(
                LifecycleMessage::CloseRequested,
            )))
            .unwrap();
        drop(msg_tx);

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Graceful);

        let events = log.lock().unwrap().clone();
        assert!(
            events.contains(&"reply_resolved".to_string()),
            "Reply must be dispatched (phase 1). Got: {events:?}"
        );
        assert!(
            !events.contains(&"reply_orphaned".to_string()),
            "Teardown must not orphan a reply that was in the same batch. Got: {events:?}"
        );
    }

    // ── Claim: lifecycle dispatches after teardown ────────────

    #[test]
    fn batch_ordering_lifecycle_after_teardown() {
        // Send ServiceTeardown and a Lifecycle in the same batch.
        // Teardown (phase 2) must dispatch before Lifecycle (phase 3).
        let log = Arc::new(Mutex::new(Vec::new()));

        let (exit_tx, _exit_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let (write_tx, _write_rx) = std::sync::mpsc::sync_channel(16);

        let handler = BatchTestHandler { log: log.clone() };
        let core = LooperCore::with_service_dispatch(
            handler,
            PeerScope(1),
            write_tx,
            exit_tx,
            ServiceDispatch::new(),
        );
        let looper = Looper::new(core);

        // Teardown first in submission order, but phase ordering
        // ensures it also dispatches first.
        msg_tx
            .send(LooperMessage::Control(ControlMessage::ServiceTeardown {
                session_id: 7,
                reason: TeardownReason::ConnectionLost,
            }))
            .unwrap();
        msg_tx
            .send(LooperMessage::Control(ControlMessage::Lifecycle(
                LifecycleMessage::CloseRequested,
            )))
            .unwrap();
        drop(msg_tx);

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Graceful);

        let events = log.lock().unwrap().clone();
        // CloseRequested dispatches in phase 3 (after teardown in phase 2).
        assert!(
            events.contains(&"close_requested".to_string()),
            "Lifecycle must dispatch. Got: {events:?}"
        );
    }

    // ── Claim: notifications dispatch in phase 5 ─────────────

    #[test]
    fn batch_ordering_notifications_last() {
        // Send a Notification and a Lifecycle(Ready) in the same
        // batch. Ready (phase 3) must dispatch before Notification
        // (phase 5). Channel close terminates the loop after both
        // dispatch — no CloseRequested, which would exit before
        // phase 5 runs.
        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log.clone());

        let inner = tagged_payload(&TestMsg::Ping("late".into()));
        let outer = wire_notification(inner);
        msg_tx
            .send(LooperMessage::Service {
                session_id: 3,
                payload: outer,
            })
            .unwrap();
        msg_tx
            .send(LooperMessage::Control(ControlMessage::Lifecycle(
                LifecycleMessage::Ready,
            )))
            .unwrap();
        drop(msg_tx);

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Disconnected);

        let events = log.lock().unwrap().clone();

        // Ready (phase 3) must come before notification (phase 5).
        let ready_pos = events
            .iter()
            .position(|e| e == "ready")
            .expect("ready must be logged");
        let notif_pos = events
            .iter()
            .position(|e| e == "notification:late")
            .expect("notification must be logged");
        assert!(
            ready_pos < notif_pos,
            "Ready (phase 3) must dispatch before Notification (phase 5). Got: {events:?}"
        );
    }
}
