//! Stress tests for looper, dispatch, and handler layers.
//!
//! Every test validates a specific invariant under adversarial
//! conditions: panicking handlers, type erasure boundaries,
//! repeated failures. Tests are #[ignore] because they are slow —
//! run with `cargo test -- --ignored` or by name.
//!
//! These tests work through the public API only: LooperCore::dispatch,
//! LooperCore::run, ServiceDispatch, etc. Internal dispatch state
//! (Dispatch, DispatchEntry) is not accessible from integration tests.

use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use pane_app::dispatch::PeerScope;
use pane_app::exit_reason::ExitReason;
use pane_app::looper_core::{DispatchOutcome, LooperCore};
use pane_proto::Flow;

// ── Shared test handler ───────────────────────────────────────

struct StressHandler {
    log: Arc<Mutex<Vec<String>>>,
    should_panic_on_ready: bool,
}

impl StressHandler {
    fn new(log: Arc<Mutex<Vec<String>>>) -> Self {
        StressHandler {
            log,
            should_panic_on_ready: false,
        }
    }
}

impl pane_proto::Handler for StressHandler {
    fn ready(&mut self) -> Flow {
        self.log.lock().unwrap().push("ready".into());
        if self.should_panic_on_ready {
            panic!("handler crashed in ready()");
        }
        Flow::Continue
    }

    fn disconnected(&mut self) -> Flow {
        self.log.lock().unwrap().push("disconnected".into());
        Flow::Stop
    }
}

impl Drop for StressHandler {
    fn drop(&mut self) {
        self.log.lock().unwrap().push("handler_dropped".into());
    }
}

fn setup() -> (
    Arc<Mutex<Vec<String>>>,
    mpsc::Sender<ExitReason>,
    mpsc::Receiver<ExitReason>,
) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let (exit_tx, exit_rx) = mpsc::channel();
    (log, exit_tx, exit_rx)
}

fn make_core(
    log: Arc<Mutex<Vec<String>>>,
    exit_tx: mpsc::Sender<ExitReason>,
) -> LooperCore<StressHandler> {
    let handler = StressHandler::new(log);
    LooperCore::new(handler, PeerScope(1), exit_tx)
}

// ════════════════════════════════════════════════════════════════
// S2: on_failed panic propagation
// ════════════════════════════════════════════════════════════════

/// S2: If a handler panics during a callback invoked by
/// run_destruction's fail_connection step, the panic propagates
/// out of dispatch() because run_destruction is NOT inside
/// catch_unwind.
///
/// This test demonstrates the behavior through the public API:
/// we set up a LooperCore with dispatch entries (via the run()
/// path with a handler that panics), then observe that the panic
/// from on_failed during destruction propagates.
///
/// Note: accessing fail_connection directly requires pub(crate),
/// which integration tests can't do. Instead, we test the
/// observable consequence: a panic in any handler callback
/// triggers the destruction sequence, and if destruction itself
/// panics, the result is an unwind.
///
/// Since we can't install dispatch entries from outside the crate,
/// we focus on the observable behavior: dispatch() catching
/// handler panics and returning Exit(Failed).
///
/// The on_failed panic specifically requires internal dispatch
/// entry access. We document this as a known gap and test what
/// we can.
#[test]
#[ignore]
fn destruction_sequence_survives_handler_panic() {
    let (log, exit_tx, exit_rx) = setup();
    let mut core = make_core(log.clone(), exit_tx);

    // Handler panic is caught by dispatch()
    let outcome = core.dispatch(|_h| {
        panic!("handler crash");
    });

    assert_eq!(outcome, DispatchOutcome::Exit(ExitReason::Failed));

    // Destruction ran: exit_tx received Failed
    assert_eq!(exit_rx.recv().unwrap(), ExitReason::Failed);

    // Handler is dropped during shutdown
    core.shutdown();
    let events = log.lock().unwrap().clone();
    assert!(
        events.contains(&"handler_dropped".to_string()),
        "handler must be dropped. Got: {:?}",
        events
    );
}

// ════════════════════════════════════════════════════════════════
// S8: Cross-protocol payload injection
// ════════════════════════════════════════════════════════════════

/// S8: Feed protocol A's serialized bytes to protocol B's closure.
/// Current code panics (expect on deserialize).
///
/// Sub-case 1: gibberish payload.
///
/// The invariant being checked is that the system fails loudly
/// (panic, not silent corruption) when type erasure is violated.
/// The make_service_receiver closure does
/// `postcard::from_bytes(payload).expect(...)` — so mismatched
/// types produce a panic.
///
/// This is technically correct (the panic is caught by catch_unwind
/// in the looper), but it's not graceful. A malicious or buggy
/// peer can crash the handler loop.
#[test]
#[ignore]
fn cross_protocol_gibberish_payload_panics() {
    use pane_app::service_dispatch::{make_service_receiver, ServiceDispatch};
    use serde::{Deserialize, Serialize};

    struct ProtoA;
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct MsgA {
        field_a: String,
        field_b: u64,
    }
    impl pane_proto::Protocol for ProtoA {
        fn service_id() -> pane_proto::ServiceId {
            pane_proto::ServiceId::new("com.test.proto_a")
        }
        type Message = MsgA;
    }

    struct CrossHandler;
    impl pane_proto::Handler for CrossHandler {}
    impl pane_proto::Handles<ProtoA> for CrossHandler {
        fn receive(&mut self, _msg: MsgA) -> Flow {
            Flow::Continue
        }
    }

    let mut dispatch = ServiceDispatch::<CrossHandler>::new();
    dispatch.register(1, make_service_receiver::<CrossHandler, ProtoA>());

    // To dispatch, we need a Messenger — but Messenger::new() is pub(crate).
    // Instead, we test the ServiceDispatch in isolation using the
    // dispatch_notification method, which needs a &Messenger.
    //
    // We can get a Messenger from a LooperCore through its public
    // messenger() method. But we need H = CrossHandler for that core.
    let (exit_tx, _) = mpsc::channel();
    let core = LooperCore::new(CrossHandler, PeerScope(1), exit_tx);
    let messenger = core.messenger().clone();

    let mut handler = CrossHandler;

    // Sub-case 1: gibberish bytes
    let gibberish = vec![0xFF, 0xDE, 0xAD, 0xBE, 0xEF];
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dispatch.dispatch_notification(1, &mut handler, &messenger, &gibberish);
    }));

    assert!(
        result.is_err(),
        "gibberish payload must cause a panic (expect on deserialize), \
         not silent corruption"
    );
}

/// S8 sub-case 2: structurally-compatible-but-wrong-type payload.
/// Protocol A expects MsgA { field_a: String, field_b: u64 }.
/// We serialize MsgB { name: String, value: u64 } — same wire
/// layout via postcard. This may silently succeed (no panic)
/// because postcard doesn't carry type names, just structure.
///
/// This documents the type-erasure boundary: the system relies on
/// the server routing frames to the correct protocol (by session_id).
/// If routing is correct, type confusion can't happen. If routing
/// is wrong (bug), the behavior depends on postcard's structural
/// compatibility.
#[test]
#[ignore]
fn cross_protocol_structural_match_may_not_panic() {
    use pane_app::service_dispatch::{make_service_receiver, ServiceDispatch};
    use serde::{Deserialize, Serialize};

    struct ProtoA;
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct MsgA {
        field_a: String,
        field_b: u64,
    }
    impl pane_proto::Protocol for ProtoA {
        fn service_id() -> pane_proto::ServiceId {
            pane_proto::ServiceId::new("com.test.proto_a2")
        }
        type Message = MsgA;
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct MsgB {
        name: String,
        value: u64,
    }

    struct CrossHandler2 {
        received_a: Vec<MsgA>,
    }
    impl pane_proto::Handler for CrossHandler2 {}
    impl pane_proto::Handles<ProtoA> for CrossHandler2 {
        fn receive(&mut self, msg: MsgA) -> Flow {
            self.received_a.push(msg);
            Flow::Continue
        }
    }

    let mut dispatch = ServiceDispatch::<CrossHandler2>::new();
    dispatch.register(1, make_service_receiver::<CrossHandler2, ProtoA>());

    let (exit_tx, _) = mpsc::channel();
    let core = LooperCore::new(CrossHandler2 { received_a: vec![] }, PeerScope(1), exit_tx);
    let messenger = core.messenger().clone();

    let mut handler = CrossHandler2 { received_a: vec![] };

    // Serialize MsgB — structurally compatible with MsgA
    let msg_b = MsgB {
        name: "injected".into(),
        value: 42,
    };
    let payload = postcard::to_allocvec(&msg_b).unwrap();

    // This may or may not panic depending on postcard's wire format.
    // postcard serializes structs as sequential fields, so String + u64
    // from MsgB should deserialize into MsgA's String + u64 fields.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dispatch.dispatch_notification(1, &mut handler, &messenger, &payload);
    }));

    if result.is_ok() {
        // Silent type confusion — postcard treated MsgB bytes as MsgA.
        // This is expected and documents the type-erasure boundary:
        // correctness depends on routing, not on wire-level type tags.
        assert_eq!(handler.received_a.len(), 1);
        assert_eq!(handler.received_a[0].field_a, "injected");
        assert_eq!(handler.received_a[0].field_b, 42);
    }
    // If it panicked, that's also acceptable — the point is
    // it didn't silently corrupt memory or produce UB.
}

// ════════════════════════════════════════════════════════════════
// S10: Handler panic storm
// ════════════════════════════════════════════════════════════════

/// S10: Handler panics on every 3rd dispatch, 300 messages total.
/// Validates: catch_unwind fires destruction sequence on the first
/// panic, exit_tx receives exactly one ExitReason.
///
/// The LooperCore's `exited` flag prevents re-dispatch after the
/// first panic. After the first panic, dispatch() returns
/// Exit(Failed) immediately without running the callback.
///
/// Invariant: I1 — panic=unwind, catch_unwind catches it.
/// After the first panic, the handler is never touched again.
/// The exit_tx receives exactly one ExitReason::Failed.
#[test]
#[ignore]
fn handler_panic_storm_single_exit_reason() {
    let (log, exit_tx, exit_rx) = setup();
    let mut core = make_core(log.clone(), exit_tx);

    let mut panic_iteration = None;
    let mut continue_count = 0;
    let mut exit_count = 0;

    for i in 0..300 {
        let outcome = core.dispatch(|h| {
            if i % 3 == 2 {
                panic!("handler panic at iteration {i}");
            }
            h.log.lock().unwrap().push(format!("dispatch_{i}"));
            Flow::Continue
        });

        match outcome {
            DispatchOutcome::Continue => {
                continue_count += 1;
            }
            DispatchOutcome::Exit(ExitReason::Failed) => {
                if panic_iteration.is_none() {
                    panic_iteration = Some(i);
                }
                exit_count += 1;
            }
            DispatchOutcome::Exit(other) => {
                panic!("unexpected exit reason at iteration {i}: {other:?}");
            }
        }
    }

    // The first panic should happen at iteration 2 (0-indexed,
    // i % 3 == 2).
    assert_eq!(
        panic_iteration,
        Some(2),
        "first panic should be at iteration 2"
    );

    // Iterations 0 and 1 succeeded (Continue).
    assert_eq!(
        continue_count, 2,
        "only 2 iterations should succeed before the first panic"
    );

    // All subsequent iterations return Exit(Failed) immediately.
    assert_eq!(
        exit_count, 298,
        "all 298 remaining iterations should return Exit(Failed)"
    );

    // exit_tx should have received exactly one ExitReason::Failed
    let mut exit_reasons = vec![];
    while let Ok(r) = exit_rx.try_recv() {
        exit_reasons.push(r);
    }
    assert_eq!(
        exit_reasons,
        vec![ExitReason::Failed],
        "exit_tx should receive exactly one Failed"
    );
}

/// S10 variant: verify the handler is dropped exactly once even
/// under panic storm conditions.
#[test]
#[ignore]
fn handler_panic_storm_handler_dropped_once() {
    let (log, exit_tx, _exit_rx) = setup();
    let mut core = make_core(log.clone(), exit_tx);

    // Dispatch 100 times, panicking on every 3rd
    for i in 0..100 {
        core.dispatch(|_h| {
            if i % 3 == 2 {
                panic!("storm panic {i}");
            }
            Flow::Continue
        });
    }

    // Shutdown drops the handler
    core.shutdown();

    let events = log.lock().unwrap().clone();
    let drop_count = events.iter().filter(|e| *e == "handler_dropped").count();
    assert_eq!(
        drop_count, 1,
        "handler must be dropped exactly once. Got: {:?}",
        events
    );
}

// ════════════════════════════════════════════════════════════════
// S10 via run(): handler panic during run() exits correctly
// ════════════════════════════════════════════════════════════════

/// S10 via run(): the full run() loop with a handler that panics
/// on the Ready lifecycle message. Verifies: run() returns
/// ExitReason::Failed, the handler is dropped, and the exit_tx
/// receives Failed.
#[test]
#[ignore]
fn run_handler_panic_exits_failed() {
    use pane_session::bridge::LooperMessage;

    let log = Arc::new(Mutex::new(Vec::new()));
    let (exit_tx, exit_rx) = mpsc::channel();

    let mut handler = StressHandler::new(log.clone());
    handler.should_panic_on_ready = true;

    let core = LooperCore::new(handler, PeerScope(1), exit_tx);

    let (msg_tx, msg_rx) = mpsc::channel();

    // Send Ready — the handler will panic
    msg_tx
        .send(LooperMessage::Control(
            pane_proto::control::ControlMessage::Lifecycle(
                pane_proto::protocols::lifecycle::LifecycleMessage::Ready,
            ),
        ))
        .unwrap();

    // Drop the sender to close the channel after the Ready
    drop(msg_tx);

    let exit_reason = core.run(msg_rx);
    assert_eq!(
        exit_reason,
        ExitReason::Failed,
        "run() must return Failed after handler panic"
    );

    let events = log.lock().unwrap().clone();
    assert!(
        events.contains(&"ready".to_string()),
        "handler.ready() must have been called. Got: {:?}",
        events
    );
    assert!(
        events.contains(&"handler_dropped".to_string()),
        "handler must be dropped. Got: {:?}",
        events
    );

    // exit_tx received Failed
    let mut reasons = vec![];
    while let Ok(r) = exit_rx.try_recv() {
        reasons.push(r);
    }
    assert!(
        reasons.contains(&ExitReason::Failed),
        "exit_tx must receive Failed. Got: {:?}",
        reasons
    );
}

// ════════════════════════════════════════════════════════════════
// S8 via LooperCore::run(): gibberish service frame
// ════════════════════════════════════════════════════════════════

/// S8 via run(): Send a gibberish service frame payload through
/// the LooperCore's run() loop. The dispatch_service path parses
/// ServiceFrame first; if THAT parse fails, the frame is silently
/// dropped (DispatchOutcome::Continue). If the outer parse succeeds
/// but the inner payload is wrong, make_service_receiver panics —
/// caught by dispatch_service's catch_unwind boundary.
///
/// This test sends:
/// 1. A frame that can't parse as ServiceFrame → dropped (correct)
/// 2. A valid ServiceFrame::Notification with gibberish inner payload
///    → catch_unwind catches the deserialization panic, run() returns
///    ExitReason::Failed (correct)
#[test]
#[ignore]
fn gibberish_service_frame_exits_failed() {
    use pane_app::service_dispatch::{make_service_receiver, ServiceDispatch};
    use pane_session::bridge::LooperMessage;
    use serde::{Deserialize, Serialize};

    struct TestProto;
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestMsg(String);
    impl pane_proto::Protocol for TestProto {
        fn service_id() -> pane_proto::ServiceId {
            pane_proto::ServiceId::new("com.test.gibberish")
        }
        type Message = TestMsg;
    }

    struct GibberishHandler {
        log: Arc<Mutex<Vec<String>>>,
    }
    impl pane_proto::Handler for GibberishHandler {
        fn close_requested(&mut self) -> Flow {
            self.log.lock().unwrap().push("close_requested".into());
            Flow::Stop
        }
    }
    impl pane_proto::Handles<TestProto> for GibberishHandler {
        fn receive(&mut self, msg: TestMsg) -> Flow {
            self.log.lock().unwrap().push(format!("received:{}", msg.0));
            Flow::Continue
        }
    }
    impl Drop for GibberishHandler {
        fn drop(&mut self) {
            self.log.lock().unwrap().push("handler_dropped".into());
        }
    }

    let log = Arc::new(Mutex::new(Vec::new()));
    let (exit_tx, exit_rx) = mpsc::channel();
    let (msg_tx, msg_rx) = mpsc::channel();

    let mut service_dispatch = ServiceDispatch::new();
    service_dispatch.register(3, make_service_receiver::<GibberishHandler, TestProto>());

    let handler = GibberishHandler { log: log.clone() };
    let core = LooperCore::with_service_dispatch(handler, PeerScope(1), exit_tx, service_dispatch);

    // Case 1: gibberish that can't parse as ServiceFrame.
    // Silently dropped (correct behavior).
    msg_tx
        .send(LooperMessage::Service {
            session_id: 3,
            payload: vec![0xFF, 0xDE, 0xAD],
        })
        .unwrap();

    // Case 2: valid ServiceFrame::Notification with gibberish inner payload.
    // make_service_receiver panics on inner deserialization.
    // catch_unwind in dispatch_service catches it → ExitReason::Failed.
    let bad_notification = pane_proto::ServiceFrame::Notification {
        payload: vec![0xFF, 0xDE, 0xAD, 0xBE, 0xEF],
    };
    let bad_bytes = postcard::to_allocvec(&bad_notification).unwrap();
    msg_tx
        .send(LooperMessage::Service {
            session_id: 3,
            payload: bad_bytes,
        })
        .unwrap();

    drop(msg_tx);

    let reason = core.run(msg_rx);
    assert_eq!(
        reason,
        ExitReason::Failed,
        "gibberish inner payload must trigger catch_unwind → Failed"
    );

    let events = log.lock().unwrap().clone();
    assert!(
        events.contains(&"handler_dropped".to_string()),
        "handler must be dropped during destruction sequence. Got: {:?}",
        events
    );

    // exit_tx received Failed
    assert_eq!(exit_rx.recv().unwrap(), ExitReason::Failed);
}
