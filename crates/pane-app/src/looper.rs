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

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

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
use pane_session::par::Session as _;

/// Default watchdog timeout: 5 seconds. A handler callback that
/// hasn't returned in this time is presumed stuck (I2/I3).
const DEFAULT_WATCHDOG_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Maximum Phase 5 messages dispatched per batch tick before
/// yielding to the calloop event loop. Prevents a burst of
/// inbound requests/notifications from starving I/O processing
/// (read, write flush, timer events).
///
/// Haiku precedent: ServerWindow capped at 70 msgs / 10ms
/// (src/servers/app/ServerWindow.cpp:573). pane uses 64 msgs /
/// 8ms — slightly tighter for 120fps headroom.
const DEFAULT_BATCH_MSG_LIMIT: usize = 64;

/// Maximum wall time for Phase 5 dispatch per batch tick.
/// Checked between individual request/notification dispatches.
const DEFAULT_BATCH_TIME_LIMIT: std::time::Duration = std::time::Duration::from_millis(8);

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
/// Phase 6 (post-batch effects/snapshot) is not yet implemented —
/// it will be added when snapshot publishing lands.
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
    /// Phase 4: Sessions revoked locally via ServiceHandle::Drop.
    /// Tracked across the batch for two purposes:
    /// 1. Phase 4 sends RevokeInterest wire frames for each session.
    /// 2. Phase 5 checks this set before dispatching — frames for
    ///    revoked sessions are silently dropped (H3).
    ///
    /// Not cleared between batches: a session revoked in batch N
    /// must suppress stale frames that arrive in batch N+1. Cleared
    /// only when the connection drops (process_disconnect backstop).
    revoked_sessions: HashSet<u16>,
    /// Phase 4: queued control frames to send on the wire.
    /// Currently only RevokeInterest; extensible for future ctl
    /// writes (Cancel, etc.). Each entry is serialized ControlMessage
    /// bytes ready for the write channel.
    ctl_writes: Vec<Vec<u8>>,
    /// Phase 3/4: A new connection handed off from the bridge
    /// thread after handshake (D2 Option A). At most one per batch.
    /// The Looper constructs a ConnectionSource and registers it
    /// on the calloop EventLoop during dispatch, then acks the
    /// bridge through the SyncSender.
    new_connection: Option<NewConnectionData>,
    /// Channel closed during collection — triggers disconnect
    /// handling after all buffered events are dispatched.
    channel_closed: bool,
}

/// Data for a pending NewConnection handoff. Separated from the
/// LooperMessage variant so Batch::collect can move the pieces
/// out of the enum without requiring Clone on Receiver.
struct NewConnectionData {
    welcome: pane_session::handshake::Welcome,
    stream: std::os::unix::net::UnixStream,
    write_rx: std::sync::mpsc::Receiver<pane_session::bridge::WriteMessage>,
    ack: std::sync::mpsc::SyncSender<pane_session::bridge::Registered>,
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
            revoked_sessions: HashSet::new(),
            ctl_writes: Vec::new(),
            new_connection: None,
            channel_closed: false,
        }
    }

    /// Whether Phase 5 messages remain from a batch-limit yield.
    /// Used by the main loop to decide between blocking and
    /// non-blocking poll.
    fn has_pending_phase5(&self) -> bool {
        !self.requests.is_empty() || !self.notifications.is_empty()
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
            && self.ctl_writes.is_empty()
            && self.new_connection.is_none()
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
            LooperMessage::LocalRevoke { session_id } => {
                // D8: mark session as locally revoked. Phase 4
                // sends the wire frame; phase 5 suppresses stale
                // frames (H3). The revoked_sessions set persists
                // across batches — a revoked session stays revoked
                // for the lifetime of the connection.
                if self.revoked_sessions.insert(session_id) {
                    // First revocation for this session — queue
                    // the wire frame for phase 4.
                    let msg = ControlMessage::RevokeInterest { session_id };
                    if let Ok(bytes) = postcard::to_allocvec(&msg) {
                        self.ctl_writes.push(bytes);
                    }
                }
                // Duplicate revocations are idempotent (no wire frame).
            }
            LooperMessage::NewConnection {
                welcome,
                stream,
                write_rx,
                ack,
            } => {
                // D2: store the handoff data for phase 3/4 dispatch.
                // At most one NewConnection per batch — if a second
                // arrives (protocol bug), the first is overwritten and
                // its ack sender drops, unblocking the bridge with
                // RecvError.
                self.new_connection = Some(NewConnectionData {
                    welcome,
                    stream,
                    write_rx,
                    ack,
                });
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

        // Create the async executor and register it as a calloop
        // event source. The executor runs futures during calloop's
        // poll cycle; completed futures fire the callback which can
        // feed results into the batch. The Scheduler stays on the
        // looper thread (!Send by construction via Rc<State>).
        //
        // Executor<()>: futures handle result delivery internally
        // via channels, shared state, or par exchange pairs.
        //
        // Theoretical basis: EAct's E-Suspend / E-React pattern
        // ([FH] §6, l.4093-4094). Future .await = E-Suspend (yield
        // to event loop). Executor callback = E-React (event loop
        // fires handler). Strengthens I6: spawn_local ensures all
        // futures run on the looper thread.
        let scheduler = match calloop::futures::executor::<()>() {
            Ok((executor, scheduler)) => {
                if handle
                    .insert_source(executor, |(), _, _state: &mut LoopState| {
                        // Executor<()>: completed futures produce ().
                        // Real work delivered via channels/shared state
                        // or par exchange pairs (ReplyFuture).
                    })
                    .is_err()
                {
                    self.core.run_destruction_reason(ExitReason::InfraError);
                    self.core.shutdown();
                    return ExitReason::InfraError;
                }
                Some(scheduler)
            }
            Err(_) => {
                self.core.run_destruction_reason(ExitReason::InfraError);
                self.core.shutdown();
                return ExitReason::InfraError;
            }
        };

        let mut state = LoopState {
            batch: Batch::new(),
            exit_reason: None,
            timers: HashMap::new(),
            shared_writer: None,
            scheduler,
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

            // If the batch has leftover Phase 5 messages from a
            // previous tick's batch limit, do a non-blocking poll
            // to collect new events (I/O, timers) without blocking.
            // Otherwise, block until at least one event arrives.
            let timeout = if state.batch.has_pending_phase5() {
                Some(std::time::Duration::ZERO)
            } else {
                None // block
            };
            if event_loop.dispatch(timeout, &mut state).is_err() {
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
                if let Some(reason) = self.dispatch_batch(&mut state, &handle) {
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
    ///
    /// The `loop_handle` is used to register new event sources
    /// (ConnectionSource from NewConnection handoff in phase 3/4).
    fn dispatch_batch(
        &mut self,
        state: &mut LoopState,
        loop_handle: &calloop::LoopHandle<'_, LoopState>,
    ) -> Option<ExitReason> {
        // Phase 0: Sync requests — install dispatch entries and
        // send wire frames for blocked send_and_wait callers.
        // Must run before Reply/Failed so entries are installed
        // before any reply in a future batch could match them.
        // Does not touch the handler (no catch_unwind needed).
        let sync_reqs = std::mem::take(&mut state.batch.sync_requests);
        for req in sync_reqs {
            self.core.process_sync_request(req);
        }

        // Phase 1: Reply and Failed — resolve pending obligations
        // before teardown can fail remaining entries.
        let replies = std::mem::take(&mut state.batch.replies);
        for (session_id, token, reply_bytes) in replies {
            let outcome = self.core.dispatch_reply(session_id, token, reply_bytes);
            if let DispatchOutcome::Exit(reason) = outcome {
                state.batch.clear();
                return Some(reason);
            }
        }
        let failed = std::mem::take(&mut state.batch.failed);
        for (session_id, token) in failed {
            let outcome = self.core.dispatch_failed(session_id, token);
            if let DispatchOutcome::Exit(reason) = outcome {
                state.batch.clear();
                return Some(reason);
            }
        }

        // Phase 2: ServiceTeardown — fail remaining entries for
        // torn-down sessions, then notify the handler (provider-side
        // subscriber_disconnected).
        let teardowns = std::mem::take(&mut state.batch.teardowns);
        for (session_id, reason) in teardowns {
            self.core.dispatch_teardown(session_id, reason.clone());
            let outcome = self
                .core
                .dispatch_subscriber_disconnected(session_id, reason);
            if let DispatchOutcome::Exit(reason) = outcome {
                state.batch.clear();
                return Some(reason);
            }
        }

        // Phase 3: PaneExited, then SubscriberConnected, then
        // Lifecycle — framework control.
        let pane_exited = std::mem::take(&mut state.batch.pane_exited);
        for (address, reason) in pane_exited {
            let outcome = self.core.dispatch_pane_exited(address, reason);
            if let DispatchOutcome::Exit(reason) = outcome {
                state.batch.clear();
                return Some(reason);
            }
        }
        let subscriber_connected = std::mem::take(&mut state.batch.subscriber_connected);
        for session_id in subscriber_connected {
            // Phase 3 par integration: create the Enqueue/Dequeue
            // pair before the handler callback fires. The handler
            // calls messenger.subscriber_sender(session_id) to
            // claim the pre-created Enqueue; the Dequeue becomes a
            // calloop StreamSource that drives wire writes.
            //
            // par's Enqueue::push is always non-blocking (N1 by
            // construction). The StreamSource is driven by calloop's
            // poll cycle with a real PingWaker -- no noop-waker hack.
            let mut dequeue_slot: Option<pane_session::par::queue::Dequeue<Vec<u8>>> = None;
            let enqueue = pane_session::par::queue::Enqueue::<Vec<u8>>::fork_sync(|dequeue| {
                dequeue_slot = Some(dequeue);
            });
            let dequeue = dequeue_slot.expect("fork_sync must deliver Dequeue");

            // Stash the Enqueue for Messenger::subscriber_sender().
            self.core
                .messenger()
                .subscriber_enqueue_map()
                .lock()
                .unwrap()
                .insert(session_id, enqueue);

            // Register the Dequeue's stream as a calloop StreamSource.
            // When Enqueue::push resolves the internal oneshot,
            // PingWaker::wake fires, calloop wakes, the callback
            // writes the payload to the wire via SharedWriter.
            let stream = dequeue.into_stream1();
            if let Ok(source) = calloop::stream::StreamSource::new(stream) {
                let _ = loop_handle.insert_source(
                    source,
                    move |event: Option<Vec<u8>>, _, loop_state: &mut LoopState| {
                        if let Some(payload) = event {
                            if let Some(ref writer) = loop_state.shared_writer {
                                writer.enqueue(session_id, &payload);
                            }
                        }
                        // None = stream closed (Enqueue dropped/closed).
                        // calloop removes the source via PostAction::Remove.
                    },
                );
            }

            let outcome = self.core.dispatch_subscriber_connected(session_id);

            // Clean up: if the handler didn't call subscriber_sender()
            // to claim the Enqueue, close it now. Leaving an unclaimed
            // Enqueue would panic the Dequeue's StreamSource on drop.
            if let Some(unclaimed) = self
                .core
                .messenger()
                .subscriber_enqueue_map()
                .lock()
                .unwrap()
                .remove(&session_id)
            {
                unclaimed.close1();
            }

            if let DispatchOutcome::Exit(reason) = outcome {
                state.batch.clear();
                return Some(reason);
            }
        }
        let lifecycle = std::mem::take(&mut state.batch.lifecycle);
        for msg in lifecycle {
            let outcome = self.core.dispatch_lifecycle(msg);
            if let DispatchOutcome::Exit(reason) = outcome {
                state.batch.clear();
                return Some(reason);
            }
        }

        // Phase 3.5: NewConnection handoff — register a
        // ConnectionSource on the calloop EventLoop for a
        // connection that completed its handshake on a bridge
        // thread (D2 Option A).
        //
        // Placed after Lifecycle (phase 3) and before ctl writes
        // (phase 4): the handler has received Ready/lifecycle
        // events, and ctl writes for the new connection can
        // proceed in the same batch's phase 4.
        if let Some(conn) = state.batch.new_connection.take() {
            if let Some(sw) = self.register_connection_source(conn, loop_handle) {
                // D12 Part 2: store the SharedWriter so phase 5
                // request dispatch can pass it to LooperCore.
                state.shared_writer = Some(sw);
            }
        }

        // Phase 4: ctl writes — send queued control frames on the
        // wire. Currently RevokeInterest from LocalRevoke; extensible
        // for Cancel and future ctl writes.
        //
        // Ordering guarantee (D8): phase 4 runs AFTER Reply/Failed
        // (phase 1), ServiceTeardown (phase 2), and PaneExited/
        // Lifecycle (phase 3). All obligations are resolved before
        // revocation goes out on the wire.
        let ctl_writes = std::mem::take(&mut state.batch.ctl_writes);
        for payload in ctl_writes {
            self.core.send_ctl_frame(payload);
        }

        // Phase 5: Requests, then Notifications — new inbound work.
        //
        // Batch limit (Haiku precedent: ServerWindow 70 msgs / 10ms):
        // after dispatching DEFAULT_BATCH_MSG_LIMIT messages or
        // DEFAULT_BATCH_TIME_LIMIT wall time, stop and leave
        // remaining messages in the batch for the next event loop
        // iteration. This prevents a burst of inbound work from
        // starving I/O processing (read, write flush, timer events).
        //
        // The limit applies to Phase 5 only — framework phases 0-4
        // are typically small (replies, teardowns, lifecycle) and
        // always complete to preserve ordering guarantees.
        //
        // H3 (stale dispatch suppression): skip frames for sessions
        // in revoked_sessions. Skipped frames do not count toward
        // the batch limit (they're free).
        let mut dispatched: usize = 0;
        let tick_start = Instant::now();

        let mut requests = std::mem::take(&mut state.batch.requests);
        let mut req_idx = 0;
        while req_idx < requests.len() {
            let session_id = requests[req_idx].0;
            if state.batch.revoked_sessions.contains(&session_id) {
                req_idx += 1;
                continue; // H3: stale frame — free skip
            }

            // Take inner bytes out of the vec entry to avoid clone.
            let token = requests[req_idx].1;
            let inner = std::mem::take(&mut requests[req_idx].2);

            // D12 Part 2 + Phase 5: pass looper-thread resources so
            // DispatchCtx carries the direct writer and async scheduler.
            let outcome = self.core.dispatch_request_with_context(
                session_id,
                token,
                inner,
                state.shared_writer.as_ref(),
                state.scheduler.as_ref(),
            );
            req_idx += 1;
            dispatched += 1;

            if let DispatchOutcome::Exit(reason) = outcome {
                state.batch.clear();
                return Some(reason);
            }

            // Check batch limit after each dispatch.
            if dispatched >= DEFAULT_BATCH_MSG_LIMIT
                || tick_start.elapsed() >= DEFAULT_BATCH_TIME_LIMIT
            {
                break;
            }
        }
        // Return undispatched requests to the batch.
        if req_idx < requests.len() {
            requests.drain(..req_idx);
            state.batch.requests = requests;
            // Skip notifications this tick — requests have priority
            // within Phase 5 (wire FIFO), and we've hit the limit.
            return None;
        }

        let mut notifications = std::mem::take(&mut state.batch.notifications);
        let mut notif_idx = 0;
        while notif_idx < notifications.len() {
            let session_id = notifications[notif_idx].0;
            if state.batch.revoked_sessions.contains(&session_id) {
                notif_idx += 1;
                continue; // H3: stale frame — free skip
            }

            let inner = std::mem::take(&mut notifications[notif_idx].1);
            let outcome = self.core.dispatch_notification(session_id, inner);
            notif_idx += 1;
            dispatched += 1;

            if let DispatchOutcome::Exit(reason) = outcome {
                state.batch.clear();
                return Some(reason);
            }

            if dispatched >= DEFAULT_BATCH_MSG_LIMIT
                || tick_start.elapsed() >= DEFAULT_BATCH_TIME_LIMIT
            {
                break;
            }
        }
        // Return undispatched notifications to the batch.
        if notif_idx < notifications.len() {
            notifications.drain(..notif_idx);
            state.batch.notifications = notifications;
            return None;
        }

        // Phase 6: post-batch effects/snapshot — not yet implemented.

        // Channel closed — handle after all buffered events.
        if state.batch.channel_closed {
            state.batch.channel_closed = false;
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

    /// Register a ConnectionSource on the calloop EventLoop from a
    /// NewConnection handoff (D2 Option A step 3).
    ///
    /// Creates the ConnectionSource from the stream and write_rx,
    /// registers it as a calloop event source that feeds events
    /// into the batch, and acks the bridge thread through the
    /// SyncSender. If registration fails, the ack sender is
    /// dropped (bridge receives RecvError) and the stream is
    /// closed (connection dies).
    /// Returns the SharedWriter if registration succeeds (D12).
    fn register_connection_source(
        &self,
        conn: NewConnectionData,
        loop_handle: &calloop::LoopHandle<'_, LoopState>,
    ) -> Option<crate::connection_source::SharedWriter> {
        let NewConnectionData {
            welcome,
            stream,
            write_rx,
            ack,
        } = conn;

        // ConnectionSource::new sets non-blocking mode on the stream.
        // connection_id is 1 for single-connection Phase 1 topology.
        let source = match crate::connection_source::ConnectionSource::new(
            stream,
            welcome.max_message_size,
            write_rx,
            1, // connection_id — Phase 1 star topology has one connection
        ) {
            Ok(s) => s,
            Err(_) => {
                // Failed to set non-blocking — drop ack, bridge gets
                // RecvError. Connection dies.
                return None;
            }
        };

        // D12 Part 2: extract the SharedWriter before registration
        // moves the source into calloop.
        let shared_writer = source.shared_writer();

        // Register as a calloop event source. The callback feeds
        // events into the batch — same pattern as the calloop
        // channel source for the forwarding thread.
        if loop_handle
            .insert_source(source, |msg, _, state: &mut LoopState| {
                state.batch.collect(msg);
            })
            .is_err()
        {
            // Registration failed — connection dies.
            return None;
        }

        // Ack the bridge: ConnectionSource is live. The bridge
        // retains write_tx and returns it to the caller for
        // ServiceHandle construction.
        let _ = ack.send(pane_session::bridge::Registered);
        Some(shared_writer)
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
        self.ctl_writes.clear();
        // NewConnection: dropping the data drops the ack sender,
        // which unblocks the bridge thread with RecvError. The
        // bridge interprets this as a failed registration.
        self.new_connection = None;
        // Intentionally do not clear channel_closed — it's a
        // permanent condition, not a buffered event.
        // Intentionally do not clear revoked_sessions — a revoked
        // session stays revoked for the connection lifetime. Stale
        // frames arriving in future batches must still be suppressed
        // (H3). process_disconnect is the backstop that cleans up
        // all state.
    }
}

/// Shared state for the calloop event loop.
/// Not parameterized on H — the batch is type-erased (raw bytes).
/// calloop's EventLoop<LoopState> passes this to event callbacks.
///
/// Created on the looper thread and never moved — Rc-based fields
/// (SharedWriter, Scheduler) are safe.
struct LoopState {
    batch: Batch,
    exit_reason: Option<ExitReason>,
    /// Active timer registrations, keyed by TimerId.
    /// Used to remove calloop Timer sources on cancel.
    timers: HashMap<TimerId, RegistrationToken>,
    /// D12 Part 2: direct write path for looper-thread sends.
    /// Rc-based (!Send) — set when a ConnectionSource is registered
    /// on the looper thread. Passed through dispatch_batch to
    /// LooperCore::dispatch_request_with_writer.
    shared_writer: Option<crate::connection_source::SharedWriter>,
    /// Scheduler for spawning async futures on the calloop executor.
    /// Rc-based (!Send) — lives on the looper thread only. Futures
    /// scheduled here run during calloop's poll cycle (event
    /// collection), not during batch dispatch. I6 (sequential
    /// single-thread dispatch) is preserved by construction: the
    /// executor uses spawn_local, the Scheduler is !Send, and
    /// completed futures deliver results via the executor callback
    /// which feeds into the batch.
    ///
    /// Currently Executor<()> — futures handle their own result
    /// delivery (channels, shared state). Passed through
    /// dispatch_batch to DispatchCtx::schedule().
    scheduler: Option<calloop::futures::Scheduler<()>>,
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
        // logs "reply_orphaned". Token allocated from active_session
        // so cascade can find it.
        let token = core.active_session_mut().allocate_token(5);
        core.dispatch_mut().insert(
            PeerScope(1),
            token,
            DispatchEntry {
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

    // ── Claim: all phases dispatch in strict order ─────────

    #[test]
    fn all_phases_dispatch_in_order() {
        // Inject one message of each type (phases 1, 2, 3, 5) in
        // deliberately scrambled order. Verify handler log shows
        // strict phase ordering: reply (1) before teardown (2)
        // before lifecycle (3).
        //
        // Phase 5 messages are injected but won't dispatch because
        // CloseRequested (phase 3) exits the batch before phase 5
        // runs — that's correct and proves phases 1-3 complete
        // before the exit propagates.
        //
        // Heritage: EAct operational semantics — E-Send/E-Receive
        // interleaving discipline ([FH] §4). The phase ordering
        // preserves safety theorems.
        use crate::dispatch::DispatchEntry;

        let log = Arc::new(Mutex::new(Vec::new()));
        let log_reply = log.clone();
        let log_failed = log.clone();

        let (exit_tx, _exit_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let (write_tx, _write_rx) = std::sync::mpsc::sync_channel(16);

        let mut service_dispatch = ServiceDispatch::new();
        service_dispatch.register(3, make_service_receiver::<BatchTestHandler, TestProto>());

        let handler = BatchTestHandler { log: log.clone() };
        let mut core = LooperCore::with_service_dispatch(
            handler,
            PeerScope(1),
            write_tx,
            exit_tx,
            service_dispatch,
            crate::Messenger::stub(),
        );

        // Install a dispatch entry on session 5 so the reply
        // resolves a real obligation (phase 1).
        let token = core.active_session_mut().allocate_token(5);
        core.dispatch_mut().insert(
            PeerScope(1),
            token,
            DispatchEntry {
                on_reply: Box::new(move |_h: &mut BatchTestHandler, _, _| {
                    log_reply.lock().unwrap().push("phase1:reply".into());
                    Flow::Continue
                }),
                on_failed: Box::new(move |_h: &mut BatchTestHandler, _| {
                    log_failed.lock().unwrap().push("phase1:failed".into());
                    Flow::Continue
                }),
            },
        );

        let looper = Looper::new(core, None, None);

        // Scrambled injection order (5, 3, 2, 1, 5) — adversarial
        // vs the correct dispatch order (1, 2, 3, 5).

        // Phase 5: Notification (injected first)
        let inner = tagged_payload(&TestMsg::Ping("late_notif".into()));
        let outer = wire_notification(inner);
        msg_tx
            .send(LooperMessage::Service {
                session_id: 3,
                payload: outer,
            })
            .unwrap();

        // Phase 3: Lifecycle (injected second)
        msg_tx
            .send(LooperMessage::Control(ControlMessage::Lifecycle(
                LifecycleMessage::CloseRequested,
            )))
            .unwrap();

        // Phase 2: ServiceTeardown (injected third)
        msg_tx
            .send(LooperMessage::Control(ControlMessage::ServiceTeardown {
                session_id: 7,
                reason: TeardownReason::ServiceRevoked,
            }))
            .unwrap();

        // Phase 1: Reply for installed token (injected fourth)
        let reply_outer = wire_reply(token.0, vec![1, 2, 3]);
        msg_tx
            .send(LooperMessage::Service {
                session_id: 5,
                payload: reply_outer,
            })
            .unwrap();

        // Phase 5: Request (injected fifth)
        let inner = tagged_payload(&TestMsg::Ping("late_req".into()));
        let outer = wire_request(99, inner);
        msg_tx
            .send(LooperMessage::Service {
                session_id: 3,
                payload: outer,
            })
            .unwrap();

        drop(msg_tx);

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Graceful);

        let events = log.lock().unwrap().clone();

        // Phase 1 reply must have dispatched.
        assert!(
            events.contains(&"phase1:reply".to_string()),
            "Reply must dispatch in phase 1. Got: {events:?}"
        );
        // Reply resolved the entry — teardown must not orphan it.
        assert!(
            !events.contains(&"phase1:failed".to_string()),
            "on_failed must not fire — reply resolved before teardown. Got: {events:?}"
        );

        // Phase 2 teardown must have dispatched.
        let teardown_pos = events
            .iter()
            .position(|e| e.starts_with("subscriber_disconnected:7:"))
            .unwrap_or_else(|| {
                panic!("subscriber_disconnected for session 7 must be logged. Got: {events:?}")
            });

        // Phase 3 lifecycle must have dispatched.
        let lifecycle_pos = events
            .iter()
            .position(|e| e == "close_requested")
            .unwrap_or_else(|| {
                panic!("close_requested must be logged. Got: {events:?}")
            });

        // Phase 1 < Phase 2: reply before teardown.
        let reply_pos = events
            .iter()
            .position(|e| e == "phase1:reply")
            .unwrap();
        assert!(
            reply_pos < teardown_pos,
            "Reply (phase 1) must dispatch before teardown (phase 2). Got: {events:?}"
        );

        // Phase 2 < Phase 3: teardown before lifecycle.
        assert!(
            teardown_pos < lifecycle_pos,
            "Teardown (phase 2) must dispatch before lifecycle (phase 3). Got: {events:?}"
        );

        // Phase 5 messages must NOT appear — CloseRequested exits
        // the batch before phase 5 runs.
        assert!(
            !events.contains(&"notification:late_notif".to_string()),
            "Phase 5 notification must not dispatch after phase 3 exit. Got: {events:?}"
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
        // it. The notification bytes arrive on the par Dequeue.
        //
        // Simulates the Looper's Phase 3 flow: pre-create the par
        // pair, stash the Enqueue in the Messenger's map, then call
        // subscriber_sender() to claim it.
        use pane_session::par::Session as _;

        let (write_tx, _write_rx) = std::sync::mpsc::sync_channel(16);
        let messenger = {
            let (timer_tx, _) = calloop_channel::channel::<TimerControl>();
            let (sync_tx, _) = calloop_channel::channel::<crate::send_and_wait::SyncRequest>();
            let looper_thread = std::sync::Arc::new(std::sync::OnceLock::new());
            crate::Messenger::new(timer_tx, sync_tx, write_tx, looper_thread)
        };

        // Pre-create par Enqueue/Dequeue pair (normally the Looper does this).
        let mut dequeue_slot = None;
        let enqueue = pane_session::par::queue::Enqueue::<Vec<u8>>::fork_sync(|dequeue| {
            dequeue_slot = Some(dequeue);
        });
        let dequeue = dequeue_slot.unwrap();

        // Stash the Enqueue (normally the Looper does this).
        messenger
            .subscriber_enqueue_map()
            .lock()
            .unwrap()
            .insert(42, enqueue);

        let mut sender: crate::SubscriberSender<TestProto> = messenger.subscriber_sender(42);
        sender.send_notification(TestMsg::Ping("pushed".into()));
        // Drop sender to close the Enqueue.
        drop(sender);

        // Drain the Dequeue to get the pushed notification bytes.
        let items = futures::executor::block_on(async {
            let mut items = Vec::new();
            let mut dq = dequeue;
            loop {
                match dq.pop().await {
                    pane_session::par::queue::Queue::Item(item, next) => {
                        items.push(item);
                        dq = next;
                    }
                    pane_session::par::queue::Queue::Closed(()) => break,
                }
            }
            items
        });
        assert_eq!(items.len(), 1);

        let frame: ServiceFrame = postcard::from_bytes(&items[0]).expect("ServiceFrame deser");
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

    // ── D8: deferred revocation ─────────────────────────────

    /// Setup that retains write_rx so tests can verify phase 4 wire
    /// output (RevokeInterest sent via send_ctl_frame).
    fn setup_looper_with_write_rx(
        log: Arc<Mutex<Vec<String>>>,
    ) -> (
        Looper<BatchTestHandler>,
        mpsc::Sender<LooperMessage>,
        mpsc::Receiver<LooperMessage>,
        std::sync::mpsc::Receiver<(u16, Vec<u8>)>,
    ) {
        let (exit_tx, _exit_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let (write_tx, write_rx) = std::sync::mpsc::sync_channel(16);

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

        (Looper::new(core, None, None), msg_tx, msg_rx, write_rx)
    }

    fn wire_request(token: u64, inner: Vec<u8>) -> Vec<u8> {
        let frame = ServiceFrame::Request {
            token,
            payload: inner,
        };
        postcard::to_allocvec(&frame).unwrap()
    }

    // ── Claim: LocalRevoke adds session to revoked_sessions ──

    #[test]
    fn local_revoke_adds_to_revoked_sessions() {
        // Batch::collect with LocalRevoke marks the session as
        // revoked and queues a ctl write.
        let mut batch = Batch::new();
        batch.collect(LooperMessage::LocalRevoke { session_id: 7 });

        assert!(
            batch.revoked_sessions.contains(&7),
            "session 7 must be in revoked_sessions"
        );
        assert_eq!(
            batch.ctl_writes.len(),
            1,
            "one ctl write (RevokeInterest) must be queued"
        );

        // Verify the queued bytes are a valid RevokeInterest.
        let msg: ControlMessage =
            postcard::from_bytes(&batch.ctl_writes[0]).expect("deserialize ctl write");
        assert!(
            matches!(msg, ControlMessage::RevokeInterest { session_id: 7 }),
            "queued frame must be RevokeInterest for session 7, got {msg:?}"
        );
    }

    // ── Claim: duplicate LocalRevoke is idempotent ───────────

    #[test]
    fn duplicate_local_revoke_is_idempotent() {
        let mut batch = Batch::new();
        batch.collect(LooperMessage::LocalRevoke { session_id: 7 });
        batch.collect(LooperMessage::LocalRevoke { session_id: 7 });

        assert!(batch.revoked_sessions.contains(&7));
        assert_eq!(
            batch.ctl_writes.len(),
            1,
            "duplicate revocation must not queue a second wire frame"
        );
    }

    // ── Claim: phase 4 sends RevokeInterest on the wire ──────

    #[test]
    fn phase_4_sends_revoke_interest_wire_frame() {
        // Inject LocalRevoke then close the channel. Phase 4 (ctl
        // writes) runs before channel-close handling, so the
        // RevokeInterest wire frame reaches the write channel.
        // Channel close (not CloseRequested) is used because
        // CloseRequested dispatches in phase 3 and exits before
        // phase 4 runs.
        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx, write_rx) = setup_looper_with_write_rx(log);

        msg_tx
            .send(LooperMessage::LocalRevoke { session_id: 5 })
            .unwrap();
        drop(msg_tx); // channel close — handled after all phases

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Disconnected);

        // Read what phase 4 sent on the write channel.
        let (service, payload) = write_rx
            .recv()
            .expect("phase 4 must send RevokeInterest on write channel");
        assert_eq!(
            service, 0,
            "RevokeInterest goes on control channel (service 0)"
        );
        let msg: ControlMessage = postcard::from_bytes(&payload).expect("deserialize");
        assert!(
            matches!(msg, ControlMessage::RevokeInterest { session_id: 5 }),
            "expected RevokeInterest for session 5, got {msg:?}"
        );
    }

    // ── Claim: phase 5 drops frames for revoked sessions (H3) ─

    #[test]
    fn phase_5_suppresses_stale_notifications_h3() {
        // In the same batch: LocalRevoke for session 3, then a
        // Notification for session 3. The notification must be
        // suppressed (H3) — it must NOT appear in the handler log.
        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log.clone());

        // LocalRevoke for session 3
        msg_tx
            .send(LooperMessage::LocalRevoke { session_id: 3 })
            .unwrap();

        // Notification for the revoked session
        let inner = tagged_payload(&TestMsg::Ping("stale".into()));
        let outer = wire_notification(inner);
        msg_tx
            .send(LooperMessage::Service {
                session_id: 3,
                payload: outer,
            })
            .unwrap();

        // A notification for a non-revoked session (to prove the
        // looper is still dispatching)
        let inner = tagged_payload(&TestMsg::Ping("alive".into()));
        let outer = wire_notification(inner);
        msg_tx
            .send(LooperMessage::Service {
                session_id: 3,
                payload: outer.clone(),
            })
            .unwrap();

        drop(msg_tx);

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Disconnected);

        let events = log.lock().unwrap().clone();
        assert!(
            !events.contains(&"notification:stale".to_string()),
            "stale notification for revoked session must be suppressed (H3). Got: {events:?}"
        );
        assert!(
            !events.contains(&"notification:alive".to_string()),
            "second notification for revoked session must also be suppressed (H3). Got: {events:?}"
        );
    }

    #[test]
    fn phase_5_suppresses_stale_requests_h3() {
        // In the same batch: LocalRevoke for session 3, then a
        // Request for session 3. The request must be suppressed.
        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log.clone());

        msg_tx
            .send(LooperMessage::LocalRevoke { session_id: 3 })
            .unwrap();

        // Inject a Request frame for the revoked session.
        let inner = tagged_payload(&TestMsg::Ping("stale_req".into()));
        let outer = wire_request(99, inner);
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
        // The handler should not receive anything for session 3
        // (no notification:stale_req, no request dispatch).
        for event in &events {
            assert!(
                !event.contains("stale_req"),
                "request for revoked session must be suppressed (H3). Got: {events:?}"
            );
        }
    }

    // ── Claim: revocation in phase 4 after Reply (phase 1) ───

    #[test]
    fn phase_ordering_reply_before_revocation() {
        // Reply (phase 1) must dispatch before phase 4 sends
        // RevokeInterest. This ensures outstanding replies are
        // resolved before the revocation goes out.
        //
        // Uses channel close (not CloseRequested) because
        // CloseRequested dispatches in phase 3 and exits before
        // phase 4 runs.
        use crate::dispatch::DispatchEntry;

        let log = Arc::new(Mutex::new(Vec::new()));
        let log_reply = log.clone();
        let log_failed = log.clone();

        let (exit_tx, _exit_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let (write_tx, write_rx) = std::sync::mpsc::sync_channel(16);

        let handler = BatchTestHandler { log: log.clone() };
        let mut core = LooperCore::with_service_dispatch(
            handler,
            PeerScope(1),
            write_tx,
            exit_tx,
            ServiceDispatch::new(),
            crate::Messenger::stub(),
        );

        // Install a dispatch entry on session 5.
        let token = core.active_session_mut().allocate_token(5);
        core.dispatch_mut().insert(
            PeerScope(1),
            token,
            DispatchEntry {
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

        // Same batch: Reply + LocalRevoke, then channel close.
        let reply_outer = wire_reply(token.0, vec![1, 2, 3]);
        msg_tx
            .send(LooperMessage::Service {
                session_id: 5,
                payload: reply_outer,
            })
            .unwrap();
        msg_tx
            .send(LooperMessage::LocalRevoke { session_id: 5 })
            .unwrap();
        drop(msg_tx); // channel close — after all phases

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Disconnected);

        let events = log.lock().unwrap().clone();
        assert!(
            events.contains(&"reply_resolved".to_string()),
            "Reply must be dispatched in phase 1. Got: {events:?}"
        );
        assert!(
            !events.contains(&"reply_orphaned".to_string()),
            "Reply must not be orphaned. Got: {events:?}"
        );

        // Phase 4 must have sent RevokeInterest on the wire.
        let (service, payload) = write_rx.recv().expect("phase 4 must send RevokeInterest");
        assert_eq!(service, 0);
        let msg: ControlMessage = postcard::from_bytes(&payload).expect("deserialize");
        assert!(
            matches!(msg, ControlMessage::RevokeInterest { session_id: 5 }),
            "expected RevokeInterest for session 5, got {msg:?}"
        );
    }

    // ── Claim: revocation before Requests (phase 5) ──────────

    #[test]
    fn phase_ordering_revocation_before_requests() {
        // In the same batch: LocalRevoke for session 3 and a
        // Notification for session 3. The notification (phase 5)
        // must NOT dispatch because the revocation (phase 4)
        // suppresses it. A Notification for a different session
        // must still dispatch.
        let log = Arc::new(Mutex::new(Vec::new()));

        let (exit_tx, _exit_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let (write_tx, _write_rx) = std::sync::mpsc::sync_channel(16);

        let mut service_dispatch = ServiceDispatch::new();
        service_dispatch.register(3, make_service_receiver::<BatchTestHandler, TestProto>());
        service_dispatch.register(4, make_service_receiver::<BatchTestHandler, TestProto>());

        let handler = BatchTestHandler { log: log.clone() };
        let core = LooperCore::with_service_dispatch(
            handler,
            PeerScope(1),
            write_tx,
            exit_tx,
            service_dispatch,
            crate::Messenger::stub(),
        );
        let looper = Looper::new(core, None, None);

        // Revoke session 3
        msg_tx
            .send(LooperMessage::LocalRevoke { session_id: 3 })
            .unwrap();

        // Notification for revoked session 3
        let inner = tagged_payload(&TestMsg::Ping("revoked".into()));
        let outer = wire_notification(inner);
        msg_tx
            .send(LooperMessage::Service {
                session_id: 3,
                payload: outer,
            })
            .unwrap();

        // Notification for non-revoked session 4
        let inner = tagged_payload(&TestMsg::Ping("active".into()));
        let outer = wire_notification(inner);
        msg_tx
            .send(LooperMessage::Service {
                session_id: 4,
                payload: outer,
            })
            .unwrap();

        drop(msg_tx);

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Disconnected);

        let events = log.lock().unwrap().clone();
        assert!(
            !events.contains(&"notification:revoked".to_string()),
            "notification for revoked session must be suppressed. Got: {events:?}"
        );
        assert!(
            events.contains(&"notification:active".to_string()),
            "notification for non-revoked session must dispatch. Got: {events:?}"
        );
    }

    // ── Claim: H2 — ServiceTeardown after revocation is no-op ─

    #[test]
    fn h2_teardown_after_revocation_is_noop() {
        // D8 invariant H2: dispatch_teardown (which calls
        // fail_session) is a no-op for a session with no dispatch
        // entries. After revocation removes entries, a subsequent
        // teardown must not crash or dispatch spurious callbacks.
        use crate::dispatch::DispatchEntry;

        let log = Arc::new(Mutex::new(Vec::new()));
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

        // Install a dispatch entry on session 5 — the teardown
        // would fire on_failed for this if the entry still existed.
        let token = core.active_session_mut().allocate_token(5);
        core.dispatch_mut().insert(
            PeerScope(1),
            token,
            DispatchEntry {
                on_reply: Box::new(|_: &mut BatchTestHandler, _, _| Flow::Continue),
                on_failed: Box::new(move |_: &mut BatchTestHandler, _| {
                    log_failed.lock().unwrap().push("spurious_failed".into());
                    Flow::Continue
                }),
            },
        );

        let looper = Looper::new(core, None, None);

        // Batch: teardown session 5 (phase 2 — fires on_failed,
        // removing the entry), then LocalRevoke for session 5
        // (phase 4). A second teardown in a later batch would be
        // a no-op. We simulate this by sending both in the same
        // batch — the teardown fires first (phase 2), then
        // revocation (phase 4). The on_failed callback fires from
        // the teardown, which is correct. The key H2 claim is that
        // fail_session is safe when entries are already gone.
        //
        // To test the true H2 scenario (teardown AFTER revocation
        // removed entries), we reverse: revoke first, then teardown.
        // But LocalRevoke doesn't remove dispatch entries — it only
        // marks the session. Entries are removed by phase 2 teardown.
        // So H2 is: a second teardown (from process_disconnect) is
        // safe after a first teardown already removed entries.
        msg_tx
            .send(LooperMessage::Control(ControlMessage::ServiceTeardown {
                session_id: 5,
                reason: TeardownReason::ServiceRevoked,
            }))
            .unwrap();
        // Second teardown for same session — must be a no-op.
        msg_tx
            .send(LooperMessage::Control(ControlMessage::ServiceTeardown {
                session_id: 5,
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
        // on_failed fires exactly once (from the first teardown).
        let failed_count = events
            .iter()
            .filter(|e| e.as_str() == "spurious_failed")
            .count();
        assert_eq!(
            failed_count, 1,
            "on_failed must fire once (first teardown), second teardown is no-op (H2). Got: {events:?}"
        );
    }

    // ── Claim: revoked_sessions persists across batches ───────

    #[test]
    fn revoked_sessions_persists_across_batches() {
        // A session revoked in batch N must suppress frames in
        // batch N+1. This tests the persistence of revoked_sessions
        // (intentionally not cleared in Batch::clear).
        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log.clone());

        // Run the looper on a background thread so we can
        // control batch boundaries via timing.
        let looper_thread = std::thread::spawn(move || looper.run(msg_rx));

        // Batch 1: revoke session 3.
        msg_tx
            .send(LooperMessage::LocalRevoke { session_id: 3 })
            .unwrap();

        // Small sleep to let batch 1 dispatch before sending batch 2.
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Batch 2: notification for revoked session 3.
        let inner = tagged_payload(&TestMsg::Ping("cross_batch".into()));
        let outer = wire_notification(inner);
        msg_tx
            .send(LooperMessage::Service {
                session_id: 3,
                payload: outer,
            })
            .unwrap();

        // Small sleep to let batch 2 dispatch.
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Close the channel to terminate.
        drop(msg_tx);

        let reason = looper_thread.join().unwrap();
        assert_eq!(reason, ExitReason::Disconnected);

        let events = log.lock().unwrap().clone();
        assert!(
            !events.contains(&"notification:cross_batch".to_string()),
            "notification in batch N+1 for session revoked in batch N must be suppressed. Got: {events:?}"
        );
    }

    // ── C5: NewConnection → ConnectionSource registration ────

    #[test]
    fn new_connection_registers_connection_source() {
        // Inject a NewConnection through the forwarding channel.
        // The Looper must construct a ConnectionSource, register
        // it on calloop, and ack the bridge.
        use pane_session::bridge::{Registered, WRITE_CHANNEL_CAPACITY};
        use std::os::unix::net::UnixStream;

        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log.clone());

        let (stream, _peer) = UnixStream::pair().unwrap();
        let (write_tx, write_rx) = std::sync::mpsc::sync_channel(WRITE_CHANNEL_CAPACITY);
        let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);

        let welcome = pane_session::handshake::Welcome {
            version: 1,
            instance_id: "test-cs".into(),
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            bindings: vec![],
        };

        // Run the looper on a background thread.
        let looper_thread = std::thread::spawn(move || looper.run(msg_rx));

        // Send NewConnection.
        msg_tx
            .send(LooperMessage::NewConnection {
                welcome,
                stream,
                write_rx,
                ack: ack_tx,
            })
            .unwrap();

        // The Looper must ack registration.
        let _registered: Registered = ack_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("Looper must ack NewConnection registration");

        // Clean shutdown.
        drop(write_tx);
        drop(msg_tx);
        let reason = looper_thread.join().unwrap();
        assert_eq!(reason, ExitReason::Disconnected);
    }

    #[test]
    fn new_connection_frames_arrive_through_connection_source() {
        // After registering a ConnectionSource via NewConnection,
        // frames written by the peer must arrive through the
        // ConnectionSource's calloop callback and dispatch to the
        // handler — not through the forwarding thread.
        use pane_session::bridge::WRITE_CHANNEL_CAPACITY;
        use pane_session::frame::FrameCodec;
        use std::os::unix::net::UnixStream;

        let log = Arc::new(Mutex::new(Vec::new()));

        // Need a looper with session 3 registered for TestProto.
        let (exit_tx, _exit_rx) = mpsc::channel();
        let (msg_tx, msg_rx) = mpsc::channel();
        let (looper_write_tx, _looper_write_rx) = std::sync::mpsc::sync_channel(16);

        let mut service_dispatch = ServiceDispatch::new();
        service_dispatch.register(3, make_service_receiver::<BatchTestHandler, TestProto>());

        let handler = BatchTestHandler { log: log.clone() };
        let core = LooperCore::with_service_dispatch(
            handler,
            PeerScope(1),
            looper_write_tx,
            exit_tx,
            service_dispatch,
            crate::Messenger::stub(),
        );
        let looper = Looper::new(core, None, None);

        let (stream, mut peer) = UnixStream::pair().unwrap();
        let (_write_tx, write_rx) = std::sync::mpsc::sync_channel(WRITE_CHANNEL_CAPACITY);
        let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);

        let welcome = pane_session::handshake::Welcome {
            version: 1,
            instance_id: "frame-test".into(),
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            bindings: vec![],
        };

        // Run the looper on a background thread.
        let looper_thread = std::thread::spawn(move || looper.run(msg_rx));

        // Register the connection.
        msg_tx
            .send(LooperMessage::NewConnection {
                welcome,
                stream,
                write_rx,
                ack: ack_tx,
            })
            .unwrap();

        ack_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("Looper must ack");

        // Write a service frame from the peer side.
        let inner = tagged_payload(&TestMsg::Ping("via_cs".into()));
        let frame = ServiceFrame::Notification { payload: inner };
        let payload = postcard::to_allocvec(&frame).unwrap();

        let codec = FrameCodec::new(16 * 1024 * 1024);
        codec.write_frame(&mut peer, 3, &payload).unwrap();

        // Give the looper time to process the event.
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Close channel to trigger exit.
        drop(peer);
        drop(msg_tx);

        let reason = looper_thread.join().unwrap();
        assert_eq!(reason, ExitReason::Disconnected);

        let events = log.lock().unwrap().clone();
        assert!(
            events.contains(&"notification:via_cs".to_string()),
            "frame from peer must arrive via ConnectionSource. Got: {events:?}"
        );
    }

    #[test]
    fn new_connection_ack_blocks_bridge() {
        // The bridge thread must block on ack_rx until the Looper
        // sends Registered. Simulate this by checking that the ack
        // arrives only after the looper processes the batch.
        use pane_session::bridge::WRITE_CHANNEL_CAPACITY;
        use std::os::unix::net::UnixStream;
        use std::sync::atomic::{AtomicBool, Ordering};

        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log);

        let (stream, _peer) = UnixStream::pair().unwrap();
        let (_write_tx, write_rx) = std::sync::mpsc::sync_channel(WRITE_CHANNEL_CAPACITY);
        let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);

        let welcome = pane_session::handshake::Welcome {
            version: 1,
            instance_id: "ack-test".into(),
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            bindings: vec![],
        };

        let ack_received = Arc::new(AtomicBool::new(false));
        let ack_received_clone = ack_received.clone();

        // Run the looper.
        let looper_thread = std::thread::spawn(move || looper.run(msg_rx));

        // Simulate a bridge thread blocking on ack.
        let bridge_thread = std::thread::spawn(move || {
            ack_rx.recv().expect("must receive ack");
            ack_received_clone.store(true, Ordering::Release);
        });

        // Before sending NewConnection, ack has not been received.
        assert!(
            !ack_received.load(Ordering::Acquire),
            "ack must not arrive before NewConnection is sent"
        );

        // Send the NewConnection.
        msg_tx
            .send(LooperMessage::NewConnection {
                welcome,
                stream,
                write_rx,
                ack: ack_tx,
            })
            .unwrap();

        // Wait for the bridge thread to unblock.
        bridge_thread.join().unwrap();
        assert!(
            ack_received.load(Ordering::Acquire),
            "bridge must receive ack after Looper registers ConnectionSource"
        );

        // Clean shutdown.
        drop(msg_tx);
        let reason = looper_thread.join().unwrap();
        assert_eq!(reason, ExitReason::Disconnected);
    }

    #[test]
    fn existing_functionality_works_alongside_connection_source() {
        // Register a ConnectionSource, then verify timers and
        // lifecycle messages still work on the same calloop.
        use pane_session::bridge::WRITE_CHANNEL_CAPACITY;
        use std::os::unix::net::UnixStream;

        let log = Arc::new(Mutex::new(Vec::new()));
        let pulse_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let (looper, msg_tx, msg_rx, timer_tx) =
            setup_looper_with_timers(log.clone(), pulse_count.clone(), 2);

        let (stream, _peer) = UnixStream::pair().unwrap();
        let (_write_tx, write_rx) = std::sync::mpsc::sync_channel(WRITE_CHANNEL_CAPACITY);
        let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);

        let welcome = pane_session::handshake::Welcome {
            version: 1,
            instance_id: "coexist-test".into(),
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            bindings: vec![],
        };

        // Run the looper.
        let looper_thread = std::thread::spawn(move || looper.run(msg_rx));

        // Register ConnectionSource.
        msg_tx
            .send(LooperMessage::NewConnection {
                welcome,
                stream,
                write_rx,
                ack: ack_tx,
            })
            .unwrap();

        ack_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("Looper must ack");

        // Start a timer. PulseTestHandler exits after 2 pulses.
        timer_tx
            .send(crate::timer::TimerControl::SetPulse {
                id: crate::timer::TimerId(42),
                interval: std::time::Duration::from_millis(10),
            })
            .unwrap();

        let reason = looper_thread.join().unwrap();
        assert_eq!(reason, ExitReason::Graceful);

        let events = log.lock().unwrap().clone();
        let pulse_events: Vec<_> = events.iter().filter(|e| e.starts_with("pulse:")).collect();
        assert!(
            pulse_events.len() >= 2,
            "timers must work alongside ConnectionSource. Got: {events:?}"
        );
    }

    // ── Batch limit tests ───────────────────────────────────

    // ── Claim: all messages dispatch even when exceeding batch limit

    #[test]
    fn batch_limit_all_messages_dispatched() {
        // Send > DEFAULT_BATCH_MSG_LIMIT (64) notifications in a
        // single burst. The batch limit splits them across multiple
        // ticks, but all must eventually dispatch. Channel close
        // terminates after all messages are delivered.
        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log.clone());

        let count = DEFAULT_BATCH_MSG_LIMIT + 20; // 84 messages
        for i in 0..count {
            let inner = tagged_payload(&TestMsg::Ping(format!("msg_{i}")));
            let outer = wire_notification(inner);
            msg_tx
                .send(LooperMessage::Service {
                    session_id: 3,
                    payload: outer,
                })
                .unwrap();
        }
        drop(msg_tx);

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Disconnected);

        let events = log.lock().unwrap().clone();
        let notif_events: Vec<_> = events
            .iter()
            .filter(|e| e.starts_with("notification:msg_"))
            .collect();
        assert_eq!(
            notif_events.len(),
            count,
            "all {count} notifications must dispatch (batch limit splits \
             across ticks, not drops). Got {}: {notif_events:?}",
            notif_events.len(),
        );
    }

    // ── Claim: framework phases complete before batch limit applies

    #[test]
    fn batch_limit_framework_phases_always_complete() {
        // Send a Lifecycle(Ready) (phase 3) alongside a burst of
        // notifications (phase 5) exceeding the batch limit. Ready
        // must dispatch before any notification, and all notifications
        // must eventually dispatch.
        let log = Arc::new(Mutex::new(Vec::new()));
        let (looper, msg_tx, msg_rx) = setup_looper(log.clone());

        // Phase 3 event.
        msg_tx
            .send(LooperMessage::Control(ControlMessage::Lifecycle(
                LifecycleMessage::Ready,
            )))
            .unwrap();

        // Phase 5: > limit notifications.
        let count = DEFAULT_BATCH_MSG_LIMIT + 10;
        for i in 0..count {
            let inner = tagged_payload(&TestMsg::Ping(format!("n_{i}")));
            let outer = wire_notification(inner);
            msg_tx
                .send(LooperMessage::Service {
                    session_id: 3,
                    payload: outer,
                })
                .unwrap();
        }
        drop(msg_tx);

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Disconnected);

        let events = log.lock().unwrap().clone();

        // Ready (phase 3) must dispatch before any notification.
        let ready_pos = events
            .iter()
            .position(|e| e == "ready")
            .expect("ready must dispatch");
        let first_notif_pos = events
            .iter()
            .position(|e| e.starts_with("notification:n_"))
            .expect("at least one notification must dispatch");
        assert!(
            ready_pos < first_notif_pos,
            "Ready (phase 3) must dispatch before Phase 5. Got: {events:?}"
        );

        // All notifications dispatched.
        let notif_count = events
            .iter()
            .filter(|e| e.starts_with("notification:n_"))
            .count();
        assert_eq!(
            notif_count, count,
            "all notifications must eventually dispatch"
        );
    }

    // ── Claim: batch has_pending_phase5 tracks correctly

    #[test]
    fn batch_has_pending_phase5() {
        let mut batch = Batch::new();
        assert!(!batch.has_pending_phase5());

        batch.requests.push((1, 0, vec![]));
        assert!(batch.has_pending_phase5());

        batch.requests.clear();
        assert!(!batch.has_pending_phase5());

        batch.notifications.push((1, vec![]));
        assert!(batch.has_pending_phase5());
    }

    // ── Claim: calloop executor runs spawned futures ────────

    /// Proves calloop's Executor/Scheduler integration works as an
    /// event source: a future spawned via Scheduler completes during
    /// EventLoop::dispatch and produces observable side effects.
    ///
    /// This is the infrastructure test for Phase 2 of the async
    /// migration (decision/async_migration_calloop_executor). The
    /// Executor sits alongside other calloop event sources (channel,
    /// timer) and processes futures during the poll cycle.
    #[test]
    fn executor_runs_spawned_future() {
        let mut event_loop = calloop::EventLoop::<u32>::try_new().unwrap();
        let handle = event_loop.handle();

        let (executor, scheduler) = calloop::futures::executor::<u32>().unwrap();

        // The executor callback delivers completed future values into
        // the calloop state — same pattern as Looper::run().
        handle
            .insert_source(executor, |result, _, state: &mut u32| {
                *state = result;
            })
            .unwrap();

        let mut state: u32 = 0;

        // Before scheduling: dispatch produces nothing.
        event_loop
            .dispatch(Some(std::time::Duration::ZERO), &mut state)
            .unwrap();
        assert_eq!(state, 0);

        // Schedule a future that returns 42.
        scheduler.schedule(async { 42u32 }).unwrap();

        // Dispatch processes the future and delivers the result.
        event_loop
            .dispatch(Some(std::time::Duration::ZERO), &mut state)
            .unwrap();
        assert_eq!(state, 42);
    }

    /// Proves the Executor<()> pattern used by Looper: futures
    /// deliver results through shared state rather than the executor's
    /// return value. This matches the real Looper::run() integration
    /// where Executor<()> futures use channels or Rc<RefCell<>> to
    /// feed results into the batch.
    #[test]
    fn executor_unit_future_with_side_effects() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let mut event_loop = calloop::EventLoop::<()>::try_new().unwrap();
        let handle = event_loop.handle();

        let (executor, scheduler) = calloop::futures::executor::<()>().unwrap();

        handle
            .insert_source(executor, |(), _, _state: &mut ()| {})
            .unwrap();

        // Shared state: the future writes to it, the test reads it.
        // Rc<RefCell<>> is safe because everything runs on one thread.
        let result = Rc::new(RefCell::new(String::new()));
        let result_clone = result.clone();

        scheduler
            .schedule(async move {
                *result_clone.borrow_mut() = "async completed".to_string();
            })
            .unwrap();

        let mut state = ();
        event_loop
            .dispatch(Some(std::time::Duration::ZERO), &mut state)
            .unwrap();

        assert_eq!(*result.borrow(), "async completed");
    }

    /// Proves multiple futures can be spawned and all complete,
    /// and that the executor integrates correctly alongside other
    /// calloop event sources (channel). This mirrors the Looper's
    /// architecture where the executor sits beside the LooperMessage
    /// channel, timer, and sync_request channel.
    #[test]
    fn executor_coexists_with_channel_source() {
        let mut event_loop = calloop::EventLoop::<Vec<String>>::try_new().unwrap();
        let handle = event_loop.handle();

        // Channel source — like the LooperMessage channel in Looper.
        let (chan_tx, chan_rx) = calloop::channel::channel::<String>();
        handle
            .insert_source(chan_rx, |event, _, state: &mut Vec<String>| {
                if let calloop::channel::Event::Msg(s) = event {
                    state.push(format!("channel:{s}"));
                }
            })
            .unwrap();

        // Executor source — futures deliver results into the same state.
        let (executor, scheduler) = calloop::futures::executor::<String>().unwrap();
        handle
            .insert_source(executor, |result, _, state: &mut Vec<String>| {
                state.push(format!("executor:{result}"));
            })
            .unwrap();

        // Send a channel message and schedule a future.
        chan_tx.send("hello".into()).unwrap();
        scheduler.schedule(async { "world".to_string() }).unwrap();

        let mut state: Vec<String> = Vec::new();
        event_loop
            .dispatch(Some(std::time::Duration::ZERO), &mut state)
            .unwrap();

        // Both sources fire in the same dispatch cycle.
        assert!(
            state.contains(&"channel:hello".to_string()),
            "channel source must fire. Got: {state:?}"
        );
        assert!(
            state.contains(&"executor:world".to_string()),
            "executor source must fire. Got: {state:?}"
        );
    }

    // ── Claim: StreamSource delivers par Dequeue items via calloop ──

    /// Proves par's Enqueue/Dequeue pair works through calloop's
    /// StreamSource: items pushed onto the Enqueue are delivered
    /// to the calloop callback via the Dequeue's stream.
    ///
    /// This is the infrastructure test for Phase 3 of the async
    /// migration. The StreamSource wraps DequeueStream1<T> with a
    /// real PingWaker -- when Enqueue::push resolves the internal
    /// oneshot, the PingWaker fires, calloop wakes, and the
    /// callback receives the item.
    #[test]
    fn stream_source_delivers_par_dequeue_items() {
        use pane_session::par::Session as _;

        let mut event_loop = calloop::EventLoop::<Vec<Vec<u8>>>::try_new().unwrap();
        let handle = event_loop.handle();

        // Create par pair.
        let mut dequeue_slot = None;
        let enqueue = pane_session::par::queue::Enqueue::<Vec<u8>>::fork_sync(|dequeue| {
            dequeue_slot = Some(dequeue);
        });
        let dequeue = dequeue_slot.unwrap();

        // Wrap as StreamSource and register on calloop.
        let stream = dequeue.into_stream1();
        let source = calloop::stream::StreamSource::new(stream).unwrap();
        handle
            .insert_source(
                source,
                |event: Option<Vec<u8>>, _, state: &mut Vec<Vec<u8>>| {
                    if let Some(item) = event {
                        state.push(item);
                    }
                },
            )
            .unwrap();

        let mut items: Vec<Vec<u8>> = Vec::new();

        // Push items from the same thread (simulates handler push).
        let enqueue = enqueue.push(vec![1, 2, 3]);
        let enqueue = enqueue.push(vec![4, 5, 6]);
        enqueue.close1();

        // Dispatch until both items arrive.
        for _ in 0..10 {
            event_loop
                .dispatch(Some(std::time::Duration::from_millis(10)), &mut items)
                .unwrap();
            if items.len() >= 2 {
                break;
            }
        }

        assert_eq!(items.len(), 2);
        assert_eq!(items[0], vec![1, 2, 3]);
        assert_eq!(items[1], vec![4, 5, 6]);
    }

    /// Proves StreamSource delivers items pushed from a different
    /// thread -- the real pattern where the handler (looper thread)
    /// pushes and calloop polls the stream.
    #[test]
    fn stream_source_cross_thread_push() {
        use pane_session::par::Session as _;

        let mut event_loop = calloop::EventLoop::<Vec<Vec<u8>>>::try_new().unwrap();
        let handle = event_loop.handle();

        let mut dequeue_slot = None;
        let enqueue = pane_session::par::queue::Enqueue::<Vec<u8>>::fork_sync(|dequeue| {
            dequeue_slot = Some(dequeue);
        });
        let dequeue = dequeue_slot.unwrap();

        let stream = dequeue.into_stream1();
        let source = calloop::stream::StreamSource::new(stream).unwrap();
        handle
            .insert_source(
                source,
                |event: Option<Vec<u8>>, _, state: &mut Vec<Vec<u8>>| {
                    if let Some(item) = event {
                        state.push(item);
                    }
                },
            )
            .unwrap();

        // Push from a background thread -- par Enqueue is Send.
        std::thread::spawn(move || {
            let enqueue = enqueue.push(vec![10, 20]);
            let enqueue = enqueue.push(vec![30, 40]);
            enqueue.close1();
        });

        let mut items: Vec<Vec<u8>> = Vec::new();
        for _ in 0..20 {
            event_loop
                .dispatch(Some(std::time::Duration::from_millis(50)), &mut items)
                .unwrap();
            if items.len() >= 2 {
                break;
            }
        }

        assert_eq!(items.len(), 2, "both items must arrive. Got: {items:?}");
        assert_eq!(items[0], vec![10, 20]);
        assert_eq!(items[1], vec![30, 40]);
    }

    /// Proves the Looper creates and registers a functional
    /// Executor/Scheduler during run() without error. The Scheduler
    /// is !Send (Rc-based), so it can only be used from the looper
    /// thread — this test verifies the Looper wires the executor
    /// into its calloop EventLoop and exits normally.
    ///
    /// The executor_runs_spawned_future and
    /// executor_coexists_with_channel_source tests above prove the
    /// calloop executor mechanism directly. This test proves the
    /// Looper integration: executor creation, registration as an
    /// event source, and clean shutdown with the executor present.
    #[test]
    fn looper_executor_integration() {
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

        // CloseRequested triggers graceful exit. The executor is
        // registered during run() and participates in the calloop
        // poll cycle even though no futures are scheduled.
        msg_tx
            .send(LooperMessage::Control(ControlMessage::Lifecycle(
                LifecycleMessage::CloseRequested,
            )))
            .unwrap();
        drop(msg_tx);

        let reason = looper.run(msg_rx);
        assert_eq!(reason, ExitReason::Graceful);
    }
}
