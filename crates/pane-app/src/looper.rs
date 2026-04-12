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

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;

use calloop::channel::{self as calloop_channel, Channel};
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, RegistrationToken};

use crate::exit_reason::ExitReason;
use crate::looper_core::{DispatchOutcome, LooperCore};
use crate::timer::{TimerControl, TimerId};
use crate::watchdog::{Heartbeat, Watchdog};
use pane_proto::control::{ControlMessage, TeardownReason};
use pane_proto::protocols::lifecycle::LifecycleMessage;
use pane_proto::Address;
use pane_session::bridge::LooperMessage;

/// Default watchdog timeout: 5 seconds. A handler callback that
/// hasn't returned in this time is presumed stuck (I2/I3).
const DEFAULT_WATCHDOG_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Calloop-backed event loop with batch-ordered dispatch.
///
/// Wraps LooperCore<H> (the dispatch engine) and adds calloop's
/// EventLoop for event source multiplexing. Uses a message
/// channel (from the bridge) and a timer control channel (from
/// Messenger/TimerToken) as calloop event sources.
///
/// A watchdog thread monitors the heartbeat and fires if the
/// looper stalls for longer than the watchdog timeout (I2/I3).
///
/// The looper thread's ThreadId is stored for I8 (send_and_wait)
/// enforcement in a later task.
pub struct Looper<H: pane_proto::Handler> {
    core: LooperCore<H>,
    thread_id: Option<thread::ThreadId>,
    timer_rx: Option<Channel<TimerControl>>,
    sync_rx: Option<Channel<crate::send_and_wait::SyncRequest>>,
    watchdog_timeout: std::time::Duration,
}

/// Events collected in one calloop iteration, sorted by dispatch phase.
///
/// Phases 4 (ctl writes) and 6 (post-batch effects/snapshot) are
/// not yet implemented — they will be added when ctl write handling
/// and snapshot publishing land.
struct Batch {
    /// Phase 0: Sync requests from non-looper threads. Processed
    /// before any reply/failed phases to ensure the dispatch entry
    /// and wire frame are emitted before reply dispatch.
    sync_requests: Vec<crate::send_and_wait::SyncRequest>,
    /// Phase 1: Reply — resolve pending obligations first.
    replies: Vec<(u16, u64, Vec<u8>)>,
    /// Phase 1: Failed — resolve pending obligations first.
    failed: Vec<(u16, u64)>,
    /// Phase 2: ServiceTeardown — fail remaining entries.
    teardowns: Vec<(u16, TeardownReason)>,
    /// Phase 3: PaneExited — death notifications.
    pane_exited: Vec<(Address, ExitReason)>,
    /// Phase 3: SubscriberConnected — provider-side interest acceptance.
    subscriber_connected: Vec<u16>,
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
            sync_requests: Vec::new(),
            replies: Vec::new(),
            failed: Vec::new(),
            teardowns: Vec::new(),
            pane_exited: Vec::new(),
            subscriber_connected: Vec::new(),
            lifecycle: Vec::new(),
            requests: Vec::new(),
            notifications: Vec::new(),
            channel_closed: false,
        }
    }

    fn is_empty(&self) -> bool {
        self.sync_requests.is_empty()
            && self.replies.is_empty()
            && self.failed.is_empty()
            && self.teardowns.is_empty()
            && self.pane_exited.is_empty()
            && self.subscriber_connected.is_empty()
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
            LooperMessage::Control(ControlMessage::InterestAccepted { session_id, .. }) => {
                // Provider-side: a subscriber connected to one of our
                // provided services. Consumer-side InterestAccepted is
                // consumed by PaneBuilder during setup — any that reach
                // the active dispatch loop are for the provider.
                self.subscriber_connected.push(session_id);
            }
            LooperMessage::Control(_) => {
                // Framework-internal control messages (DeclareInterest,
                // InterestDeclined, etc.) — handled during setup, not
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
    ///
    /// The timer_rx channel is registered as a calloop event
    /// source during run(). Pass None for tests that don't use
    /// timers — the Messenger::stub() constructor pairs with this.
    pub(crate) fn new(
        core: LooperCore<H>,
        timer_rx: Option<Channel<TimerControl>>,
        sync_rx: Option<Channel<crate::send_and_wait::SyncRequest>>,
    ) -> Self {
        Looper {
            core,
            thread_id: None,
            timer_rx,
            sync_rx,
            watchdog_timeout: DEFAULT_WATCHDOG_TIMEOUT,
        }
    }

    /// Override the watchdog timeout. Default is 5 seconds.
    /// Used by tests and applications with known long-running handlers.
    #[doc(hidden)]
    pub fn with_watchdog_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.watchdog_timeout = timeout;
        self
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

        // Publish the looper thread's ThreadId for I8 enforcement.
        // send_and_wait reads this via Messenger::looper_thread_id().
        let _ = self
            .core
            .messenger()
            .looper_thread_lock()
            .set(thread::current().id());

        // Spawn the watchdog thread. It monitors the heartbeat
        // and logs if the looper stalls (I2/I3 detection).
        let heartbeat = Heartbeat::new();
        let _watchdog = Watchdog::spawn(heartbeat.clone(), self.watchdog_timeout, |duration| {
            eprintln!(
                "pane: watchdog fired — handler callback stalled for {:.1}s (I2/I3 violation)",
                duration.as_secs_f64()
            );
        });

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

        // Register the timer control channel if present.
        // Timer commands (SetPulse/Cancel) arrive from Messenger
        // and TimerToken. SetPulse inserts a calloop Timer source
        // that fires Pulse events into the batch; Cancel removes
        // the timer source.
        if let Some(timer_rx) = self.timer_rx.take() {
            let timer_handle = handle.clone();
            if handle
                .insert_source(timer_rx, move |event, _, state: &mut LoopState| {
                    if let calloop_channel::Event::Msg(cmd) = event {
                        match cmd {
                            TimerControl::SetPulse { id, interval } => {
                                let timer = Timer::from_duration(interval);
                                let reg = timer_handle.insert_source(
                                    timer,
                                    move |_deadline, _metadata, state: &mut LoopState| {
                                        // Timer fired — inject Pulse into
                                        // the batch's lifecycle phase (3).
                                        state.batch.lifecycle.push(LifecycleMessage::Pulse);
                                        TimeoutAction::ToDuration(interval)
                                    },
                                );
                                if let Ok(reg_token) = reg {
                                    state.timers.insert(id, reg_token);
                                }
                            }
                            TimerControl::Cancel { id } => {
                                if let Some(reg_token) = state.timers.remove(&id) {
                                    timer_handle.remove(reg_token);
                                }
                            }
                        }
                    }
                })
                .is_err()
            {
                self.core.run_destruction_reason(ExitReason::InfraError);
                self.core.shutdown();
                return ExitReason::InfraError;
            }
        }

        // Register the sync request channel if present.
        // SyncRequests arrive from non-looper threads via
        // send_and_wait. Collected into the batch for Phase 0
        // processing (dispatch entry install + wire send).
        if let Some(sync_rx) = self.sync_rx.take() {
            if handle
                .insert_source(sync_rx, |event, _, state: &mut LoopState| {
                    if let calloop_channel::Event::Msg(req) = event {
                        state.batch.sync_requests.push(req);
                    }
                })
                .is_err()
            {
                self.core.run_destruction_reason(ExitReason::InfraError);
                self.core.shutdown();
                return ExitReason::InfraError;
            }
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
            timers: HashMap::new(),
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
            // Heartbeat: signal the watchdog that the looper is alive.
            // Placed at the top of the loop so the watchdog measures
            // time spent inside dispatch (handler callbacks + batch
            // processing), not time waiting for events.
            heartbeat.beat();

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
        // Phase 0: Sync requests — install dispatch entries and
        // send wire frames for blocked send_and_wait callers.
        // Must run before Reply/Failed so entries are installed
        // before any reply in a future batch could match them.
        // Does not touch the handler (no catch_unwind needed).
        let sync_reqs = std::mem::take(&mut batch.sync_requests);
        for req in sync_reqs {
            self.core.process_sync_request(req);
        }

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
        // torn-down sessions, then notify the handler (provider-side
        // subscriber_disconnected).
        let teardowns = std::mem::take(&mut batch.teardowns);
        for (session_id, reason) in teardowns {
            self.core.dispatch_teardown(session_id, reason.clone());
            let outcome = self
                .core
                .dispatch_subscriber_disconnected(session_id, reason);
            if let DispatchOutcome::Exit(reason) = outcome {
                batch.clear();
                return Some(reason);
            }
        }

        // Phase 3: PaneExited, then SubscriberConnected, then
        // Lifecycle — framework control.
        let pane_exited = std::mem::take(&mut batch.pane_exited);
        for (address, reason) in pane_exited {
            let outcome = self.core.dispatch_pane_exited(address, reason);
            if let DispatchOutcome::Exit(reason) = outcome {
                batch.clear();
                return Some(reason);
            }
        }
        let subscriber_connected = std::mem::take(&mut batch.subscriber_connected);
        for session_id in subscriber_connected {
            let outcome = self.core.dispatch_subscriber_connected(session_id);
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
        // Sync requests: callers are still blocked. Dropping the
        // SyncRequest drops reply_tx, which unblocks them with
        // RecvError (→ Disconnected).
        self.sync_requests.clear();
        self.replies.clear();
        self.failed.clear();
        self.teardowns.clear();
        self.pane_exited.clear();
        self.subscriber_connected.clear();
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
    /// Active timer registrations, keyed by TimerId.
    /// Used to remove calloop Timer sources on cancel.
    timers: HashMap<TimerId, RegistrationToken>,
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

        fn subscriber_connected(&mut self, session_id: u16) -> Flow {
            self.log
                .lock()
                .unwrap()
                .push(format!("subscriber_connected:{session_id}"));
            Flow::Continue
        }

        fn subscriber_disconnected(&mut self, session_id: u16, reason: TeardownReason) -> Flow {
            self.log
                .lock()
                .unwrap()
                .push(format!("subscriber_disconnected:{session_id}:{reason:?}"));
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
            crate::Messenger::stub(),
        );

        (Looper::new(core, None, None), msg_tx, msg_rx)
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
            crate::Messenger::stub(),
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

        let looper = Looper::new(core, None, None);

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
            crate::Messenger::stub(),
        );
        let looper = Looper::new(core, None, None);

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

    // ── Timer tests ─────────────────────────────────────────

    /// Test handler that counts pulse events and exits after
    /// a target count or on CloseRequested.
    struct PulseTestHandler {
        log: Arc<Mutex<Vec<String>>>,
        pulse_count: Arc<std::sync::atomic::AtomicUsize>,
        pulse_target: usize,
    }

    impl Handler for PulseTestHandler {
        fn ready(&mut self) -> Flow {
            self.log.lock().unwrap().push("ready".into());
            Flow::Continue
        }

        fn pulse(&mut self) -> Flow {
            let count = self
                .pulse_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                + 1;
            self.log.lock().unwrap().push(format!("pulse:{count}"));
            if count >= self.pulse_target {
                Flow::Stop
            } else {
                Flow::Continue
            }
        }

        fn close_requested(&mut self) -> Flow {
            self.log.lock().unwrap().push("close_requested".into());
            Flow::Stop
        }

        fn disconnected(&mut self) -> Flow {
            self.log.lock().unwrap().push("disconnected".into());
            Flow::Stop
        }
    }

    impl Drop for PulseTestHandler {
        fn drop(&mut self) {
            self.log.lock().unwrap().push("dropped".into());
        }
    }

    /// Setup a looper with a live timer channel. Returns the
    /// looper, msg sender, msg receiver, and timer control sender.
    fn setup_looper_with_timers(
        log: Arc<Mutex<Vec<String>>>,
        pulse_count: Arc<std::sync::atomic::AtomicUsize>,
        pulse_target: usize,
    ) -> (
        Looper<PulseTestHandler>,
        mpsc::Sender<LooperMessage>,
        mpsc::Receiver<LooperMessage>,
        calloop_channel::Sender<TimerControl>,
    ) {
        let (exit_tx, _exit_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let (write_tx, _write_rx) = std::sync::mpsc::sync_channel(16);

        let (timer_tx, timer_rx) = calloop_channel::channel::<TimerControl>();
        let (sync_tx, _sync_rx) = calloop_channel::channel::<crate::send_and_wait::SyncRequest>();
        let looper_thread = std::sync::Arc::new(std::sync::OnceLock::new());
        let messenger =
            crate::Messenger::new(timer_tx.clone(), sync_tx, write_tx.clone(), looper_thread);

        let handler = PulseTestHandler {
            log,
            pulse_count,
            pulse_target,
        };
        let core = LooperCore::with_service_dispatch(
            handler,
            PeerScope(1),
            write_tx,
            exit_tx,
            ServiceDispatch::new(),
            messenger,
        );

        (
            Looper::new(core, Some(timer_rx), None),
            msg_tx,
            msg_rx,
            timer_tx,
        )
    }

    // ── Claim: timer fires pulse ────────────────────────────

    #[test]
    fn timer_fires_pulse() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let pulse_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (looper, _msg_tx, msg_rx, timer_tx) =
            setup_looper_with_timers(log.clone(), pulse_count.clone(), 3);

        // Set a 10ms pulse timer before entering the loop.
        // The looper processes SetPulse as a calloop event.
        timer_tx
            .send(TimerControl::SetPulse {
                id: TimerId(99),
                interval: std::time::Duration::from_millis(10),
            })
            .unwrap();

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Graceful);

        let events = log.lock().unwrap().clone();
        // Handler should have received at least 3 pulses (target)
        // and then returned Flow::Stop.
        let pulse_events: Vec<_> = events.iter().filter(|e| e.starts_with("pulse:")).collect();
        assert!(
            pulse_events.len() >= 3,
            "Expected at least 3 pulse events, got: {events:?}"
        );
    }

    // ── Claim: timer_token drop cancels timer ───────────────

    #[test]
    fn timer_token_drop_cancels() {
        use crate::timer::TimerToken;

        let log = Arc::new(Mutex::new(Vec::new()));
        let pulse_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        // Set target very high — we don't expect to reach it
        let (looper, msg_tx, msg_rx, timer_tx) =
            setup_looper_with_timers(log.clone(), pulse_count.clone(), 1000);

        // Create a TimerToken, then immediately drop it.
        // The SetPulse and Cancel will both be in the channel
        // before the looper starts.
        let token = TimerToken::new(std::time::Duration::from_millis(10), timer_tx);
        drop(token);

        // Give the looper a short time to process, then close.
        let msg_tx_clone = msg_tx.clone();
        std::thread::spawn(move || {
            // Wait long enough that if the timer were still
            // active, it would have fired many times.
            std::thread::sleep(std::time::Duration::from_millis(100));
            msg_tx_clone
                .send(LooperMessage::Control(ControlMessage::Lifecycle(
                    LifecycleMessage::CloseRequested,
                )))
                .ok();
        });

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Graceful);

        let events = log.lock().unwrap().clone();
        let pulse_events: Vec<_> = events.iter().filter(|e| e.starts_with("pulse:")).collect();
        // Timer was cancelled before it could fire. Zero pulses
        // is the expected outcome, but we tolerate a small number
        // due to timer/channel ordering.
        assert!(
            pulse_events.len() <= 1,
            "Cancelled timer should fire at most once, got {}: {events:?}",
            pulse_events.len()
        );
    }

    // ── send_and_wait tests ─────────────────────────────────

    use crate::send_and_wait::SendAndWaitError;
    use crate::service_handle::ServiceHandle;
    use pane_proto::RequestProtocol;

    // A request protocol for send_and_wait tests.
    struct SawProto;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum SawMsg {
        Query(String),
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    enum SawReply {
        Answer(String),
    }

    impl Protocol for SawProto {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.looper.send_and_wait")
        }
        type Message = SawMsg;
    }

    impl RequestProtocol for SawProto {
        type Reply = SawReply;
    }

    /// Setup a looper with the sync channel wired up.
    /// Returns: looper, msg_tx (for injecting replies), msg_rx
    /// (for looper.run), messenger (for send_and_wait),
    /// ServiceHandle<SawProto>, write_rx (for reading outbound
    /// frames).
    fn setup_looper_for_saw() -> (
        Looper<BatchTestHandler>,
        mpsc::Sender<LooperMessage>,
        mpsc::Receiver<LooperMessage>,
        crate::Messenger,
        ServiceHandle<SawProto>,
        std::sync::mpsc::Receiver<(u16, Vec<u8>)>,
    ) {
        let (exit_tx, _exit_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let (write_tx, write_rx) = std::sync::mpsc::sync_channel(16);

        let (timer_tx, _timer_rx) = calloop_channel::channel::<TimerControl>();
        let (sync_tx, sync_rx) = calloop_channel::channel::<crate::send_and_wait::SyncRequest>();
        let looper_thread = std::sync::Arc::new(std::sync::OnceLock::new());
        let messenger = crate::Messenger::new(timer_tx, sync_tx, write_tx.clone(), looper_thread);

        let log = Arc::new(Mutex::new(Vec::new()));
        let handler = BatchTestHandler { log };

        let core = LooperCore::with_service_dispatch(
            handler,
            PeerScope(1),
            write_tx.clone(),
            exit_tx,
            ServiceDispatch::new(),
            messenger.clone(),
        );

        let looper = Looper::new(core, None, Some(sync_rx));
        let service_handle = ServiceHandle::<SawProto>::with_channel(5, write_tx);

        (looper, msg_tx, msg_rx, messenger, service_handle, write_rx)
    }

    // ── Claim: send_and_wait returns reply ──────────────────

    #[test]
    fn send_and_wait_returns_reply() {
        let (looper, msg_tx, msg_rx, messenger, service_handle, write_rx) = setup_looper_for_saw();

        // Run the looper on a background thread.
        let looper_thread = thread::spawn(move || looper.run(msg_rx));

        // Simulate the remote side: read the outbound request,
        // build a reply, and inject it back into the looper.
        let reply_thread = {
            let msg_tx = msg_tx.clone();
            thread::spawn(move || {
                let (session_id, payload) = write_rx.recv().expect("expected outbound request");
                let frame: ServiceFrame =
                    postcard::from_bytes(&payload).expect("deserialize outbound frame");
                match frame {
                    ServiceFrame::Request { token, .. } => {
                        // Build a reply with the echoed token.
                        let reply_bytes =
                            postcard::to_allocvec(&SawReply::Answer("hello back".into())).unwrap();
                        let reply_frame = ServiceFrame::Reply {
                            token,
                            payload: reply_bytes,
                        };
                        let reply_outer = postcard::to_allocvec(&reply_frame).unwrap();
                        msg_tx
                            .send(LooperMessage::Service {
                                session_id,
                                payload: reply_outer,
                            })
                            .unwrap();
                    }
                    other => panic!("expected Request, got {other:?}"),
                }
            })
        };

        // Call send_and_wait from this thread (non-looper).
        let result = service_handle.send_and_wait(
            &messenger,
            SawMsg::Query("hello".into()),
            std::time::Duration::from_secs(5),
        );

        assert_eq!(result.unwrap(), SawReply::Answer("hello back".into()));

        reply_thread.join().unwrap();

        // Shut down the looper cleanly.
        msg_tx
            .send(LooperMessage::Control(ControlMessage::Lifecycle(
                LifecycleMessage::CloseRequested,
            )))
            .unwrap();
        drop(msg_tx);

        let reason = looper_thread.join().unwrap();
        assert!(
            matches!(reason, ExitReason::Graceful | ExitReason::Disconnected),
            "expected Graceful or Disconnected, got {reason:?}"
        );
    }

    // ── Claim: send_and_wait panics from looper thread (I8) ─

    #[test]
    fn send_and_wait_panics_from_looper_thread() {
        // Create a messenger with the current thread's ID set
        // as the looper thread, simulating a call from the looper.
        let (timer_tx, _) = calloop_channel::channel::<TimerControl>();
        let (sync_tx, _) = calloop_channel::channel::<crate::send_and_wait::SyncRequest>();
        let looper_thread = std::sync::Arc::new(std::sync::OnceLock::new());
        looper_thread.set(thread::current().id()).unwrap();
        let (write_tx, _write_rx) = std::sync::mpsc::sync_channel(16);
        let messenger = crate::Messenger::new(timer_tx, sync_tx, write_tx.clone(), looper_thread);

        let handle = ServiceHandle::<SawProto>::with_channel(1, write_tx);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            handle.send_and_wait(
                &messenger,
                SawMsg::Query("should panic".into()),
                std::time::Duration::from_secs(1),
            )
        }));

        assert!(
            result.is_err(),
            "send_and_wait must panic from looper thread (I8)"
        );
    }

    // ── Claim: send_and_wait times out ──────────────────────

    #[test]
    fn send_and_wait_timeout() {
        let (looper, msg_tx, msg_rx, messenger, service_handle, _write_rx) = setup_looper_for_saw();

        // Run the looper — but no one reads from write_rx to reply.
        let looper_thread = thread::spawn(move || looper.run(msg_rx));

        let result = service_handle.send_and_wait(
            &messenger,
            SawMsg::Query("no reply".into()),
            std::time::Duration::from_millis(100),
        );

        assert_eq!(result.unwrap_err(), SendAndWaitError::Timeout);

        // Shut down.
        msg_tx
            .send(LooperMessage::Control(ControlMessage::Lifecycle(
                LifecycleMessage::CloseRequested,
            )))
            .unwrap();
        drop(msg_tx);
        looper_thread.join().unwrap();
    }

    // ── Claim: send_and_wait returns Failed ─────────────────

    #[test]
    fn send_and_wait_failed() {
        let (looper, msg_tx, msg_rx, messenger, service_handle, write_rx) = setup_looper_for_saw();

        let looper_thread = thread::spawn(move || looper.run(msg_rx));

        // Simulate the remote side sending Failed.
        let reply_thread = {
            let msg_tx = msg_tx.clone();
            thread::spawn(move || {
                let (session_id, payload) = write_rx.recv().expect("expected outbound request");
                let frame: ServiceFrame = postcard::from_bytes(&payload).expect("deserialize");
                match frame {
                    ServiceFrame::Request { token, .. } => {
                        let failed_frame = ServiceFrame::Failed { token };
                        let failed_outer = postcard::to_allocvec(&failed_frame).unwrap();
                        msg_tx
                            .send(LooperMessage::Service {
                                session_id,
                                payload: failed_outer,
                            })
                            .unwrap();
                    }
                    other => panic!("expected Request, got {other:?}"),
                }
            })
        };

        let result = service_handle.send_and_wait(
            &messenger,
            SawMsg::Query("will fail".into()),
            std::time::Duration::from_secs(5),
        );

        assert_eq!(result.unwrap_err(), SendAndWaitError::Failed);

        reply_thread.join().unwrap();

        msg_tx
            .send(LooperMessage::Control(ControlMessage::Lifecycle(
                LifecycleMessage::CloseRequested,
            )))
            .unwrap();
        drop(msg_tx);
        looper_thread.join().unwrap();
    }

    // ── Claim: send_and_wait unblocks on looper shutdown ──────

    #[test]
    fn send_and_wait_unblocks_on_shutdown() {
        let (looper, msg_tx, msg_rx, messenger, service_handle, _write_rx) = setup_looper_for_saw();

        let looper_thread = thread::spawn(move || looper.run(msg_rx));

        // Shut down the looper while the caller is blocked.
        // CloseRequested triggers the destruction sequence, which
        // calls fail_connection(primary) — this fires on_failed
        // for the sync dispatch entry, unblocking the caller with
        // Failed ("your request did not get a reply").
        let msg_tx_clone = msg_tx.clone();
        let shutdown_thread = thread::spawn(move || {
            // Small delay so send_and_wait has time to submit
            // the SyncRequest before the looper exits.
            thread::sleep(std::time::Duration::from_millis(50));
            msg_tx_clone
                .send(LooperMessage::Control(ControlMessage::Lifecycle(
                    LifecycleMessage::CloseRequested,
                )))
                .ok();
        });

        let result = service_handle.send_and_wait(
            &messenger,
            SawMsg::Query("shutting down".into()),
            std::time::Duration::from_secs(5),
        );

        // The destruction sequence fires on_failed for all entries
        // on the primary connection — the sync entry is on the
        // primary connection so reply routing works. The caller
        // receives Failed, meaning "request did not get a reply."
        assert_eq!(result.unwrap_err(), SendAndWaitError::Failed);

        shutdown_thread.join().unwrap();
        drop(msg_tx);
        looper_thread.join().unwrap();
    }

    // ── Claim: send_and_wait returns Disconnected if sync channel closed ─

    #[test]
    fn send_and_wait_disconnected_sync_channel_closed() {
        // If the sync channel is already closed when send_and_wait
        // tries to submit the SyncRequest, Disconnected is returned.
        let (timer_tx, _) = calloop_channel::channel::<TimerControl>();
        let (sync_tx, sync_rx) = calloop_channel::channel::<crate::send_and_wait::SyncRequest>();
        let looper_thread_lock = std::sync::Arc::new(std::sync::OnceLock::new());
        let (write_tx, _write_rx) = std::sync::mpsc::sync_channel(16);
        let messenger =
            crate::Messenger::new(timer_tx, sync_tx, write_tx.clone(), looper_thread_lock);

        // Drop the receiver to close the channel.
        drop(sync_rx);

        let (write_tx, _write_rx) = std::sync::mpsc::sync_channel(16);
        let handle = ServiceHandle::<SawProto>::with_channel(1, write_tx);

        let result = handle.send_and_wait(
            &messenger,
            SawMsg::Query("no looper".into()),
            std::time::Duration::from_secs(1),
        );

        assert_eq!(result.unwrap_err(), SendAndWaitError::Disconnected);
    }

    // ── Provider-side subscriber callbacks ───────────────────

    #[test]
    fn interest_accepted_triggers_subscriber_connected() {
        // InterestAccepted arriving during the active dispatch loop
        // (after setup) is for the provider. It must trigger
        // handler.subscriber_connected with the session_id.
        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log.clone());

        msg_tx
            .send(LooperMessage::Control(ControlMessage::InterestAccepted {
                service_uuid: uuid::Uuid::nil(),
                session_id: 42,
                version: 1,
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
            events.contains(&"subscriber_connected:42".to_string()),
            "InterestAccepted must trigger subscriber_connected. Got: {events:?}"
        );
    }

    #[test]
    fn service_teardown_triggers_subscriber_disconnected() {
        // ServiceTeardown must trigger subscriber_disconnected
        // with the session_id and TeardownReason.
        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log.clone());

        msg_tx
            .send(LooperMessage::Control(ControlMessage::ServiceTeardown {
                session_id: 7,
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
            events.contains(&"subscriber_disconnected:7:ServiceRevoked".to_string()),
            "ServiceTeardown must trigger subscriber_disconnected. Got: {events:?}"
        );
    }

    #[test]
    fn subscriber_connected_dispatches_in_phase_3() {
        // subscriber_connected (phase 3) must dispatch after
        // teardowns (phase 2) and before notifications (phase 5).
        // Use channel close for exit (not CloseRequested, which
        // exits in phase 3 before notifications in phase 5).
        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log.clone());

        let inner = tagged_payload(&TestMsg::Ping("after_connect".into()));
        let outer = wire_notification(inner);
        msg_tx
            .send(LooperMessage::Control(ControlMessage::InterestAccepted {
                service_uuid: uuid::Uuid::nil(),
                session_id: 10,
                version: 1,
            }))
            .unwrap();
        msg_tx
            .send(LooperMessage::Service {
                session_id: 3,
                payload: outer,
            })
            .unwrap();
        // Channel close terminates the loop after all phases.
        drop(msg_tx);

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Disconnected);

        let events = log.lock().unwrap().clone();
        let connect_pos = events
            .iter()
            .position(|e| e == "subscriber_connected:10")
            .expect("subscriber_connected must be logged");
        let notif_pos = events
            .iter()
            .position(|e| e == "notification:after_connect")
            .expect("notification must be logged");
        assert!(
            connect_pos < notif_pos,
            "subscriber_connected (phase 3) must dispatch before \
             notification (phase 5). Got: {events:?}"
        );
    }

    #[test]
    fn teardown_dispatches_subscriber_disconnected_before_subscriber_connected() {
        // In the same batch: teardown (phase 2) dispatches
        // subscriber_disconnected before InterestAccepted (phase 3)
        // dispatches subscriber_connected.
        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log.clone());

        msg_tx
            .send(LooperMessage::Control(ControlMessage::ServiceTeardown {
                session_id: 5,
                reason: TeardownReason::ConnectionLost,
            }))
            .unwrap();
        msg_tx
            .send(LooperMessage::Control(ControlMessage::InterestAccepted {
                service_uuid: uuid::Uuid::nil(),
                session_id: 6,
                version: 1,
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
        let disconnect_pos = events
            .iter()
            .position(|e| e == "subscriber_disconnected:5:ConnectionLost")
            .expect("subscriber_disconnected must be logged");
        let connect_pos = events
            .iter()
            .position(|e| e == "subscriber_connected:6")
            .expect("subscriber_connected must be logged");
        assert!(
            disconnect_pos < connect_pos,
            "subscriber_disconnected (phase 2) must dispatch before \
             subscriber_connected (phase 3). Got: {events:?}"
        );
    }

    #[test]
    fn subscriber_sender_push_via_messenger() {
        // Full loop: provider gets subscriber_connected, constructs
        // SubscriberSender via Messenger, pushes notification through
        // it. The notification bytes arrive on the write channel.
        let (write_tx, write_rx) = std::sync::mpsc::sync_channel(16);
        let messenger = {
            let (timer_tx, _) = calloop_channel::channel::<TimerControl>();
            let (sync_tx, _) = calloop_channel::channel::<crate::send_and_wait::SyncRequest>();
            let looper_thread = std::sync::Arc::new(std::sync::OnceLock::new());
            crate::Messenger::new(timer_tx, sync_tx, write_tx, looper_thread)
        };

        let sender: crate::SubscriberSender<TestProto> = messenger.subscriber_sender(42);
        sender.send_notification(TestMsg::Ping("pushed".into()));

        let (session_id, payload) = write_rx.recv().expect("expected frame");
        assert_eq!(session_id, 42);

        let frame: ServiceFrame = postcard::from_bytes(&payload).expect("ServiceFrame deser");
        match frame {
            ServiceFrame::Notification { payload } => {
                let expected_tag = TestProto::service_id().tag();
                assert_eq!(payload[0], expected_tag);
                let msg: TestMsg = postcard::from_bytes(&payload[1..]).expect("inner deser");
                assert!(matches!(msg, TestMsg::Ping(s) if s == "pushed"));
            }
            other => panic!("expected Notification, got {other:?}"),
        }
    }
}
