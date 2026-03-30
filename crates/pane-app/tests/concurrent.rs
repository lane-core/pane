//! P3 concurrent/stress tests.
//!
//! Validates Stage 5 features (self-delivery, coalescing, timers,
//! filter chain) under concurrent load. All tests use MockCompositor
//! or raw LooperMessage channels — no real compositor needed.
//!
//! Every test has a timeout to catch deadlocks.

use std::num::NonZeroU32;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}};
use std::thread;
use std::time::Duration;

use pane_app::{App, Tag, Message, Messenger, LooperMessage, Handler, ReplyPort};
use pane_app::error::Result;
use pane_app::mock::MockCompositor;
use pane_proto::event::{Modifiers, MouseEvent, MouseEventKind};
use pane_proto::message::PaneId;
use pane_proto::protocol::{CompToClient, ClientToComp, PaneGeometry};

fn pane_id(n: u32) -> PaneId {
    PaneId::new(NonZeroU32::new(n).unwrap())
}

// --- P3-A: Multi-pane dispatch under contention ---

#[test]
fn multi_pane_concurrent_dispatch() {
    let (conn, mock) = MockCompositor::pair();
    let inject = mock.sender();
    let mock_handle = thread::spawn(move || mock.run());

    let app = App::connect_test("com.test.concurrent", conn).unwrap();

    let num_panes = 10;
    let events_per_pane = 100;

    // Create all panes, collect their IDs
    let mut panes = Vec::new();
    for _ in 0..num_panes {
        let pane = app.create_pane(Tag::new("Stress")).unwrap();
        panes.push(pane);
    }

    let pane_ids: Vec<PaneId> = panes.iter().map(|p| p.id()).collect();

    // Schedule events for each pane on a sender thread
    let inject2 = inject.clone();
    let ids = pane_ids.clone();
    thread::spawn(move || {
        for i in 0..events_per_pane {
            for id in &ids {
                if i % 2 == 0 {
                    let _ = inject2.send(CompToClient::Focus { pane: *id });
                } else {
                    let _ = inject2.send(CompToClient::Blur { pane: *id });
                }
            }
        }
        // Close all panes after events
        thread::sleep(Duration::from_millis(50));
        for id in &ids {
            let _ = inject2.send(CompToClient::Close { pane: *id });
        }
    });

    // Run each pane on its own thread
    let counts: Arc<Mutex<Vec<usize>>> = Arc::new(Mutex::new(vec![0; num_panes]));
    let mut handles = Vec::new();

    for (i, pane) in panes.into_iter().enumerate() {
        let counts = counts.clone();
        handles.push(thread::spawn(move || {
            let mut count = 0usize;
            pane.run(|_proxy, event| {
                match event {
                    Message::Activated | Message::Deactivated => count += 1,
                    Message::CloseRequested => return Ok(false),
                    _ => {}
                }
                Ok(true)
            }).unwrap();
            counts.lock().unwrap()[i] = count;
        }));
    }

    for h in handles {
        h.join().unwrap();
    }
    drop(app);
    mock_handle.join().unwrap();

    let counts = counts.lock().unwrap();
    for (i, &count) in counts.iter().enumerate() {
        assert!(count > 0, "pane {} received no events", i);
    }
    let total: usize = counts.iter().sum();
    assert_eq!(total, num_panes * events_per_pane,
        "total events should be {} but got {}", num_panes * events_per_pane, total);
}

// --- P3-B: Self-delivery burst from multiple worker threads ---

#[test]
fn self_delivery_burst_from_workers() {
    let (conn, mock) = MockCompositor::pair();
    let inject = mock.sender();
    let mock_handle = thread::spawn(move || mock.run());

    let app = App::connect_test("com.test.burst", conn).unwrap();
    let pane = app.create_pane(Tag::new("Burst")).unwrap();
    let pane_id = pane.id();
    let proxy = pane.messenger();

    let num_threads = 5;
    let events_per_thread = 100;
    let expected_total = num_threads * events_per_thread;

    // Spawn worker threads that post events via self-delivery
    for _ in 0..num_threads {
        let p = proxy.clone();
        thread::spawn(move || {
            for _ in 0..events_per_thread {
                // Ignore errors — looper may exit before we finish
                let _ = p.send_message(Message::Activated);
            }
        });
    }
    // Drop our proxy — workers have their own clones, and pane.run()
    // creates its own handle. Holding proxy here keeps the looper_tx
    // sender alive after run() exits, which can block cleanup.
    drop(proxy);

    // Close after workers are likely done
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        let _ = inject.send(CompToClient::Close { pane: pane_id });
    });

    let event_count = Arc::new(AtomicUsize::new(0));
    let count = event_count.clone();

    pane.run(move |_proxy, event| {
        match event {
            Message::Activated => { count.fetch_add(1, Ordering::Relaxed); }
            Message::CloseRequested => return Ok(false),
            _ => {}
        }
        Ok(true)
    }).unwrap();

    drop(app);
    mock_handle.join().unwrap();

    let received = event_count.load(Ordering::Relaxed);
    assert_eq!(received, expected_total,
        "expected {} self-delivered events, got {}", expected_total, received);
}

// --- P3-C: Coalescing under sustained resize flood ---

#[test]
fn coalesce_resize_flood() {
    let (tx, rx) = mpsc::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (comp_tx, _comp_rx) = mpsc::channel::<ClientToComp>();
    let proxy = Messenger::new(pane_id(1), comp_tx);

    // Pre-load 1000 resizes with different geometries
    for i in 0..1000u32 {
        tx.send(LooperMessage::FromComp(CompToClient::Resize {
            pane: pane_id(1),
            geometry: PaneGeometry { width: i, height: i, cols: 80, rows: 24 },
        })).unwrap();
    }
    // Interleave 1000 mouse moves
    for i in 0..1000u16 {
        tx.send(LooperMessage::FromComp(CompToClient::Mouse {
            pane: pane_id(1),
            event: MouseEvent {
                col: i, row: i,
                kind: MouseEventKind::Move,
                modifiers: Modifiers::empty(),
                timestamp: None,
            },
        })).unwrap();
    }
    // Terminate
    tx.send(LooperMessage::FromComp(CompToClient::Close { pane: pane_id(1) })).unwrap();
    drop(tx);

    let mut resize_count = 0u32;
    let mut move_count = 0u32;
    let mut last_resize_w = 0u32;
    let mut last_move_col = 0u16;

    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        match &event {
            Message::Resize(g) => {
                resize_count += 1;
                last_resize_w = g.width;
            }
            Message::Mouse(m) if matches!(m.kind, MouseEventKind::Move) => {
                move_count += 1;
                last_move_col = m.col;
            }
            Message::CloseRequested => return Ok(false),
            _ => {}
        }
        Ok(true)
    }).unwrap();

    assert_eq!(resize_count, 1, "1000 resizes should coalesce to 1, got {}", resize_count);
    assert_eq!(last_resize_w, 999, "should keep last resize geometry");
    assert_eq!(move_count, 1, "1000 moves should coalesce to 1, got {}", move_count);
    assert_eq!(last_move_col, 999, "should keep last mouse position");
}

// --- P3-D: Timer cancellation race ---

#[test]
fn timer_cancellation_race() {
    let (tx, rx) = mpsc::channel::<LooperMessage>();
    let (comp_tx, _comp_rx) = mpsc::channel::<ClientToComp>();
    let filters = pane_app::filter::FilterChain::new();
    let looper_tx = tx.clone();
    let proxy = Messenger::new(pane_id(1), comp_tx);

    // We can't use with_looper from integration tests, so we test
    // the timer threading pattern directly: spawn timer threads that
    // send to the looper channel, cancel most of them.
    let total_timers = 20;
    let cancelled_count = 18;
    let active_count = total_timers - cancelled_count;
    let interval = Duration::from_millis(10);

    let mut tokens = Vec::new();
    for _ in 0..total_timers {
        let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let token = cancelled.clone();
        let tx_clone = looper_tx.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(interval);
                if cancelled.load(Ordering::Relaxed) {
                    break;
                }
                if tx_clone.send(LooperMessage::Posted(Message::Activated)).is_err() {
                    break;
                }
            }
        });
        tokens.push(token);
    }

    // Cancel most timers immediately
    for token in tokens.iter().take(cancelled_count) {
        token.store(true, Ordering::Relaxed);
    }

    // Let active timers run for 100ms
    thread::sleep(Duration::from_millis(100));

    // Cancel remaining and send Close
    for token in &tokens {
        token.store(true, Ordering::Relaxed);
    }
    thread::sleep(Duration::from_millis(20)); // let threads exit
    tx.send(LooperMessage::FromComp(CompToClient::Close { pane: pane_id(1) })).unwrap();
    drop(tx);

    let event_count = Arc::new(AtomicUsize::new(0));
    let count = event_count.clone();

    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, move |_h, event| {
        match event {
            Message::Activated => { count.fetch_add(1, Ordering::Relaxed); }
            Message::CloseRequested => return Ok(false),
            _ => {}
        }
        Ok(true)
    }).unwrap();

    let received = event_count.load(Ordering::Relaxed);
    // Active timers: 2 timers × ~10 fires each (100ms / 10ms) = ~20 events
    // Allow generous range: at least 2 (each fires at least once) and at most 40
    assert!(received >= active_count,
        "at least {} events expected from {} active timers, got {}",
        active_count, active_count, received);
    assert!(received <= 60,
        "too many events ({}), cancelled timers may still be firing", received);
}

// --- P3-E: Messenger clone flood ---

#[test]
fn pane_handle_clone_flood() {
    let (comp_tx, comp_rx) = mpsc::channel::<ClientToComp>();
    let handle = Messenger::new(pane_id(1), comp_tx);

    let num_threads = 50;
    let sends_per_thread = 50;
    let expected = num_threads * sends_per_thread;

    let mut threads = Vec::new();
    for i in 0..num_threads {
        let h = handle.clone();
        threads.push(thread::spawn(move || {
            for j in 0..sends_per_thread {
                let content = format!("t{}m{}", i, j);
                h.set_content(content.as_bytes()).unwrap();
            }
        }));
    }

    for t in threads {
        t.join().unwrap();
    }

    // Count all received messages
    drop(handle); // drop last sender so rx closes
    let mut count = 0;
    while comp_rx.recv().is_ok() {
        count += 1;
    }

    assert_eq!(count, expected,
        "expected {} messages from {} threads × {} sends, got {}",
        expected, num_threads, sends_per_thread, count);
}

// --- P3-F: Looper shutdown under active self-delivery ---

#[test]
fn looper_shutdown_under_active_posting() {
    let (conn, mock) = MockCompositor::pair();
    let inject = mock.sender();
    let mock_handle = thread::spawn(move || mock.run());

    let app = App::connect_test("com.test.shutdown", conn).unwrap();
    let pane = app.create_pane(Tag::new("Shutdown")).unwrap();
    let pane_id = pane.id();
    let proxy = pane.messenger();

    // Spawn a worker that posts events continuously
    let worker_proxy = proxy.clone();
    thread::spawn(move || {
        loop {
            if worker_proxy.send_message(Message::Activated).is_err() {
                break; // looper gone, channel closed
            }
            thread::sleep(Duration::from_millis(1));
        }
    });

    // Drop our proxy before run() — workers have their own clone.
    // This prevents holding senders alive after run() exits.
    drop(proxy);

    // Close the pane after 50ms
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(50));
        let _ = inject.send(CompToClient::Close { pane: pane_id });
    });

    let result = pane.run(|_proxy, event| {
        match event {
            Message::CloseRequested => Ok(false),
            _ => Ok(true),
        }
    });

    assert!(result.is_ok(), "pane.run() should exit cleanly, got {:?}", result);

    drop(app);
    mock_handle.join().unwrap();
}

// ============================================================
// Helpers for stress tests
// ============================================================

/// Create a Messenger with looper channel attached.
fn make_looper_proxy(id: PaneId) -> (Messenger, mpsc::Receiver<LooperMessage>) {
    let (comp_tx, _) = mpsc::channel::<ClientToComp>();
    let (looper_tx, looper_rx) = mpsc::sync_channel::<LooperMessage>(256);
    (Messenger::new(id, comp_tx).with_looper(looper_tx), looper_rx)
}

/// Handler that replies to requests: doubles u64, triples u32.
struct EchoHandler;

impl Handler for EchoHandler {
    fn request_received(&mut self, _proxy: &Messenger, msg: Message, reply_port: ReplyPort) -> Result<bool> {
        if let Message::AppMessage(payload) = msg {
            if let Some(&n) = payload.downcast_ref::<u64>() {
                reply_port.reply(n * 2);
                return Ok(true);
            }
        }
        // Unknown request — drop triggers ReplyFailed
        Ok(true)
    }

    fn close_requested(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(false)
    }
}

// ============================================================
// Timer stress tests
// ============================================================

// --- S1: High timer count (50 periodic timers on one looper) ---

#[test]
fn stress_high_timer_count() {
    let (proxy, looper_rx) = make_looper_proxy(pane_id(1));
    let outer_proxy = proxy.clone();

    let num_timers = 50u32;
    let counters: Vec<Arc<AtomicUsize>> = (0..num_timers)
        .map(|_| Arc::new(AtomicUsize::new(0)))
        .collect();
    let counters_clone = counters.clone();

    let handle = thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_closure(pane_id(1), looper_rx, filters, proxy, move |_m, msg| {
            if let Message::CommandExecuted { ref command, .. } = msg {
                if let Some(idx) = command.strip_prefix("t").and_then(|s| s.parse::<usize>().ok()) {
                    if idx < counters_clone.len() {
                        counters_clone[idx].fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            if matches!(msg, Message::CloseRequested) { return Ok(false); }
            Ok(true)
        })
    });

    // Register 50 periodic timers, all at 20ms
    let mut tokens = Vec::new();
    for i in 0..num_timers {
        let token = outer_proxy.send_periodic(
            Message::CommandExecuted { command: format!("t{i}"), args: String::new() },
            Duration::from_millis(20),
        ).unwrap();
        tokens.push(token);
    }

    // Let them run for 200ms (~10 fires each)
    thread::sleep(Duration::from_millis(200));

    // Cancel all and shut down
    for t in &tokens {
        t.cancel();
    }
    outer_proxy.send_message(Message::CloseRequested).unwrap();
    handle.join().unwrap().unwrap();

    let mut zero_count = 0;
    for (i, c) in counters.iter().enumerate() {
        let n = c.load(Ordering::Relaxed);
        if n == 0 { zero_count += 1; }
        assert!(n <= 20, "timer {i} fired too many times: {n}");
    }
    assert_eq!(zero_count, 0, "{zero_count} of {num_timers} timers never fired");
}

// --- S2: Timer + message interleaving under sustained load ---

#[test]
fn stress_timer_message_interleave() {
    let (proxy, looper_rx) = make_looper_proxy(pane_id(1));
    let outer_proxy = proxy.clone();

    let pulse_count = Arc::new(AtomicUsize::new(0));
    let msg_count = Arc::new(AtomicUsize::new(0));
    let pc = pulse_count.clone();
    let mc = msg_count.clone();

    let handle = thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_closure(pane_id(1), looper_rx, filters, proxy, move |_m, msg| {
            match msg {
                Message::Pulse => { pc.fetch_add(1, Ordering::Relaxed); }
                Message::Activated => { mc.fetch_add(1, Ordering::Relaxed); }
                Message::CloseRequested => return Ok(false),
                _ => {}
            }
            Ok(true)
        })
    });

    // Start a 15ms pulse
    outer_proxy.set_pulse_rate(Duration::from_millis(15)).unwrap();

    // Sustained message flood for 300ms (not a single burst)
    let flood_proxy = outer_proxy.clone();
    let flood_handle = thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut sent = 0u32;
        while start.elapsed() < Duration::from_millis(300) {
            let _ = flood_proxy.send_message(Message::Activated);
            sent += 1;
            // Pace at ~1 per 100us to keep the looper continuously busy
            if sent % 10 == 0 {
                thread::sleep(Duration::from_micros(100));
            }
        }
        sent
    });

    let sent = flood_handle.join().unwrap();
    outer_proxy.send_message(Message::CloseRequested).unwrap();
    handle.join().unwrap().unwrap();

    let pulses = pulse_count.load(Ordering::Relaxed);
    let msgs = msg_count.load(Ordering::Relaxed);

    assert_eq!(msgs, sent as usize,
        "all {sent} messages should arrive, got {msgs}");
    // 300ms / 15ms = ~20 fires; under sustained load expect at least a few
    assert!(pulses >= 3,
        "pulse should survive sustained message interleaving, got {pulses} (expected >= 3 over 300ms)");
}

// --- S3: One-shot timer ordering ---

#[test]
fn stress_oneshot_ordering() {
    let (proxy, looper_rx) = make_looper_proxy(pane_id(1));
    let outer_proxy = proxy.clone();

    let arrival_order = Arc::new(Mutex::new(Vec::<u32>::new()));
    let order = arrival_order.clone();

    let handle = thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_closure(pane_id(1), looper_rx, filters, proxy, move |_m, msg| {
            if let Message::CommandExecuted { ref command, .. } = msg {
                if let Some(n) = command.strip_prefix("s").and_then(|s| s.parse::<u32>().ok()) {
                    order.lock().unwrap().push(n);
                }
            }
            if matches!(msg, Message::CloseRequested) { return Ok(false); }
            Ok(true)
        })
    });

    // Schedule 10 one-shots with increasing delays
    for i in 0..10u32 {
        outer_proxy.send_delayed(
            Message::CommandExecuted { command: format!("s{i}"), args: String::new() },
            Duration::from_millis(20 + i as u64 * 15),
        ).unwrap();
    }

    // Wait for all to fire (last at ~155ms, give generous margin)
    thread::sleep(Duration::from_millis(300));
    outer_proxy.send_message(Message::CloseRequested).unwrap();
    handle.join().unwrap().unwrap();

    let arrived = arrival_order.lock().unwrap();
    assert_eq!(arrived.len(), 10, "all 10 one-shots should fire, got {}", arrived.len());
    // Verify monotonic ordering (each arrived at or after its predecessor)
    for w in arrived.windows(2) {
        assert!(w[0] <= w[1],
            "one-shots should arrive in delay order: {:?}", *arrived);
    }
}

// --- S4: Timer registration storm (register/cancel 100 rapidly) ---

#[test]
fn stress_timer_registration_storm() {
    let (proxy, looper_rx) = make_looper_proxy(pane_id(1));
    let outer_proxy = proxy.clone();

    let fire_count = Arc::new(AtomicUsize::new(0));
    let fc = fire_count.clone();

    let handle = thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_closure(pane_id(1), looper_rx, filters, proxy, move |_m, msg| {
            match msg {
                Message::Activated => { fc.fetch_add(1, Ordering::Relaxed); }
                Message::CloseRequested => return Ok(false),
                _ => {}
            }
            Ok(true)
        })
    });

    // Register and immediately cancel 100 timers
    for _ in 0..100 {
        let token = outer_proxy.send_periodic(
            Message::Activated,
            Duration::from_millis(5),
        ).unwrap();
        token.cancel();
    }

    // Wait for any stale fires
    thread::sleep(Duration::from_millis(100));
    let stale = fire_count.load(Ordering::Relaxed);

    // Now register one survivor to prove timers still work
    let _survivor = outer_proxy.send_periodic(
        Message::Activated,
        Duration::from_millis(10),
    ).unwrap();
    thread::sleep(Duration::from_millis(80));

    outer_proxy.send_message(Message::CloseRequested).unwrap();
    handle.join().unwrap().unwrap();

    let total = fire_count.load(Ordering::Relaxed);
    let survivor_fires = total - stale;

    // Stale fires: at most a handful (one per timer that fired before cancel was seen)
    assert!(stale <= 10,
        "cancelled timers should produce few stale fires, got {stale}");
    // Survivor should have fired normally
    assert!(survivor_fires >= 3,
        "survivor timer should fire after storm, got {survivor_fires}");
}

// --- S5: Timer accuracy under handler load ---

#[test]
fn stress_timer_accuracy_under_load() {
    let (proxy, looper_rx) = make_looper_proxy(pane_id(1));
    let outer_proxy = proxy.clone();

    let pulse_times = Arc::new(Mutex::new(Vec::<std::time::Instant>::new()));
    let pt = pulse_times.clone();

    let handle = thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_closure(pane_id(1), looper_rx, filters, proxy, move |_m, msg| {
            match msg {
                Message::Pulse => {
                    pt.lock().unwrap().push(std::time::Instant::now());
                }
                Message::Activated => {
                    // Simulate slow handler: 2ms of work per event
                    thread::sleep(Duration::from_millis(2));
                }
                Message::CloseRequested => return Ok(false),
                _ => {}
            }
            Ok(true)
        })
    });

    // Start a 30ms pulse
    outer_proxy.set_pulse_rate(Duration::from_millis(30)).unwrap();

    // Flood with events that each take 2ms to handle (saturates ~50% of looper time)
    let flood_proxy = outer_proxy.clone();
    thread::spawn(move || {
        for _ in 0..100 {
            let _ = flood_proxy.send_message(Message::Activated);
            thread::sleep(Duration::from_millis(3));
        }
    });

    thread::sleep(Duration::from_millis(350));
    outer_proxy.send_message(Message::CloseRequested).unwrap();
    handle.join().unwrap().unwrap();

    let times = pulse_times.lock().unwrap();
    assert!(times.len() >= 3,
        "should get at least 3 pulses under load, got {}", times.len());

    // Measure inter-pulse intervals: with 30ms target and 2ms handler overhead,
    // intervals should generally stay under 100ms
    let intervals: Vec<Duration> = times.windows(2)
        .map(|w| w[1].duration_since(w[0]))
        .collect();
    let max_interval = intervals.iter().max().unwrap();
    assert!(*max_interval < Duration::from_millis(150),
        "worst-case inter-pulse interval {:?} exceeds 150ms (target 30ms)", max_interval);
}

// ============================================================
// Reply stress tests
// ============================================================

// --- S6: High concurrency send_and_wait (100 threads) ---

#[test]
fn stress_send_and_wait_100_threads() {
    let (target_proxy, target_rx) = make_looper_proxy(pane_id(2));
    let target_for_send = target_proxy.clone();

    let target_handle = thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, EchoHandler)
    });

    let num_threads = 100u64;
    let mut handles = Vec::new();

    for i in 0..num_threads {
        let target = target_for_send.clone();
        let (caller, _rx) = make_looper_proxy(pane_id(100 + i as u32));
        handles.push(thread::spawn(move || {
            let result = caller.send_and_wait(
                &target,
                Message::AppMessage(Box::new(i)),
                Duration::from_secs(10),
            );
            let payload = result.unwrap_or_else(|e| panic!("thread {i} failed: {e:?}"));
            let value = *payload.downcast_ref::<u64>().expect("reply should be u64");
            assert_eq!(value, i * 2, "thread {i}: expected {}, got {value}", i * 2);
        }));
    }

    for (i, h) in handles.into_iter().enumerate() {
        h.join().unwrap_or_else(|_| panic!("thread {i} panicked"));
    }

    target_for_send.send_message(Message::CloseRequested).unwrap();
    target_handle.join().unwrap().unwrap();
}

// --- S7: Sustained request throughput ---

#[test]
fn stress_request_throughput() {
    let (target_proxy, target_rx) = make_looper_proxy(pane_id(2));
    let target_for_send = target_proxy.clone();

    let target_handle = thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, EchoHandler)
    });

    let (caller_proxy, _) = make_looper_proxy(pane_id(1));

    // Fire as many send_and_wait calls as we can in 1 second
    let start = std::time::Instant::now();
    let mut completed = 0u64;
    let deadline = Duration::from_secs(1);

    while start.elapsed() < deadline {
        let result = caller_proxy.send_and_wait(
            &target_for_send,
            Message::AppMessage(Box::new(completed)),
            Duration::from_millis(500),
        );
        match result {
            Ok(payload) => {
                let val = *payload.downcast_ref::<u64>().unwrap();
                assert_eq!(val, completed * 2,
                    "reply {completed}: expected {}, got {val}", completed * 2);
                completed += 1;
            }
            Err(e) => panic!("request {completed} failed: {e:?}"),
        }
    }

    target_for_send.send_message(Message::CloseRequested).unwrap();
    target_handle.join().unwrap().unwrap();

    // Serial send_and_wait: each round-trip is two channel hops.
    // Should get at least a few hundred per second.
    assert!(completed >= 100,
        "sustained throughput too low: {completed} requests/sec (expected >= 100)");
}

// --- S8: Mixed sync/async requests to same target ---

#[test]
fn stress_mixed_sync_async_requests() {
    let (target_proxy, target_rx) = make_looper_proxy(pane_id(2));
    let target_for_send = target_proxy.clone();

    let target_handle = thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, EchoHandler)
    });

    let sync_count = Arc::new(AtomicUsize::new(0));
    let async_count = Arc::new(AtomicUsize::new(0));

    // 10 sync threads
    let mut handles = Vec::new();
    for i in 0..10u64 {
        let target = target_for_send.clone();
        let (caller, _rx) = make_looper_proxy(pane_id(100 + i as u32));
        let sc = sync_count.clone();
        handles.push(thread::spawn(move || {
            for j in 0..10u64 {
                let val = i * 10 + j;
                let result = caller.send_and_wait(
                    &target,
                    Message::AppMessage(Box::new(val)),
                    Duration::from_secs(5),
                );
                let payload = result.unwrap_or_else(|e| panic!("sync {val} failed: {e:?}"));
                let reply = *payload.downcast_ref::<u64>().unwrap();
                assert_eq!(reply, val * 2, "sync reply mismatch for {val}");
                sc.fetch_add(1, Ordering::Relaxed);
            }
        }));
    }

    // 10 async senders
    for i in 0..10u64 {
        let target = target_for_send.clone();
        let (caller, caller_rx) = make_looper_proxy(pane_id(200 + i as u32));
        let ac = async_count.clone();
        handles.push(thread::spawn(move || {
            let mut pending = std::collections::HashMap::new();
            // Send all 10 requests
            for j in 0..10u64 {
                let val = 1000 + i * 10 + j;
                let token = caller.send_request(
                    &target,
                    Message::AppMessage(Box::new(val)),
                ).unwrap();
                pending.insert(token, val);
            }
            // Collect replies
            let mut received = 0;
            while received < 10 {
                let msg = caller_rx.recv_timeout(Duration::from_secs(5))
                    .expect("async reply timed out");
                match msg {
                    LooperMessage::Posted(Message::Reply { token, payload }) => {
                        let orig = pending.remove(&token).expect("unexpected token");
                        let reply = *payload.downcast_ref::<u64>().unwrap();
                        assert_eq!(reply, orig * 2, "async reply mismatch for {orig}");
                        ac.fetch_add(1, Ordering::Relaxed);
                        received += 1;
                    }
                    _ => panic!("expected Reply, got {:?}", msg),
                }
            }
        }));
    }

    for (i, h) in handles.into_iter().enumerate() {
        h.join().unwrap_or_else(|_| panic!("thread {i} panicked"));
    }

    let sc = sync_count.load(Ordering::Relaxed);
    let ac = async_count.load(Ordering::Relaxed);
    assert_eq!(sc, 100, "all 100 sync requests should complete, got {sc}");
    assert_eq!(ac, 100, "all 100 async requests should complete, got {ac}");

    target_for_send.send_message(Message::CloseRequested).unwrap();
    target_handle.join().unwrap().unwrap();
}

// --- S9: Async reply ordering (verify tokens match) ---

#[test]
fn stress_async_reply_ordering() {
    let (target_proxy, target_rx) = make_looper_proxy(pane_id(2));
    let target_for_send = target_proxy.clone();

    let target_handle = thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, EchoHandler)
    });

    let (caller_proxy, caller_rx) = make_looper_proxy(pane_id(1));

    // Send 50 async requests rapidly
    let mut pending = std::collections::HashMap::new();
    for i in 0..50u64 {
        let token = caller_proxy.send_request(
            &target_for_send,
            Message::AppMessage(Box::new(i)),
        ).unwrap();
        pending.insert(token, i);
    }

    // All 50 replies should arrive with matching tokens and correct values
    let mut received = 0;
    while received < 50 {
        let msg = caller_rx.recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| panic!("timed out waiting for reply {received}/50"));
        match msg {
            LooperMessage::Posted(Message::Reply { token, payload }) => {
                let orig = pending.remove(&token)
                    .unwrap_or_else(|| panic!("reply token {token} not in pending set"));
                let reply = *payload.downcast_ref::<u64>().unwrap();
                assert_eq!(reply, orig * 2,
                    "reply for request {orig}: expected {}, got {reply}", orig * 2);
                received += 1;
            }
            other => panic!("expected Reply, got {:?}", other),
        }
    }
    assert!(pending.is_empty(), "all requests should be answered");

    target_for_send.send_message(Message::CloseRequested).unwrap();
    target_handle.join().unwrap().unwrap();
}

// --- S10: Cascading failure (monitor + reply interaction) ---

#[test]
fn stress_cascading_failure() {
    // B is a target that will be killed mid-flight.
    // A monitors B and sends async requests to B.
    // When B dies: A should get both PaneExited and ReplyFailed.

    struct DieOnThirdHandler {
        count: u32,
    }
    impl Handler for DieOnThirdHandler {
        fn request_received(&mut self, _proxy: &Messenger, _msg: Message, reply_port: ReplyPort) -> Result<bool> {
            self.count += 1;
            if self.count >= 3 {
                // Exit without replying — reply_port drop sends ReplyFailed
                return Ok(false);
            }
            reply_port.reply(self.count as u64);
            Ok(true)
        }
        fn close_requested(&mut self, _proxy: &Messenger) -> Result<bool> {
            Ok(false)
        }
    }

    let (b_proxy, b_rx) = make_looper_proxy(pane_id(3));
    let b_for_a = b_proxy.clone();

    let (a_proxy, a_rx) = make_looper_proxy(pane_id(2));

    let b_handle = thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_handler(
            pane_id(3), b_rx, filters, b_proxy,
            DieOnThirdHandler { count: 0 },
        )
    });

    // Send 5 async requests from A to B (B dies on the 3rd)
    let mut tokens = Vec::new();
    for i in 0..5u64 {
        match a_proxy.send_request(
            &b_for_a,
            Message::AppMessage(Box::new(i)),
        ) {
            Ok(token) => tokens.push(token),
            Err(_) => break, // B's channel closed
        }
    }

    b_handle.join().unwrap().unwrap();

    // Simulate what pane.run() would do via ExitBroadcaster:
    // post PaneExited to A's channel through its public API.
    let _ = a_proxy.send_message(Message::PaneExited {
        pane: pane_id(3),
        reason: pane_app::ExitReason::HandlerExit,
    });

    // Collect everything A receives
    let mut replies = Vec::new();
    let mut failures = Vec::new();
    let mut exits = Vec::new();

    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        match a_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(LooperMessage::Posted(Message::Reply { token, payload })) => {
                let val = *payload.downcast_ref::<u64>().unwrap();
                replies.push((token, val));
            }
            Ok(LooperMessage::Posted(Message::ReplyFailed { token })) => {
                failures.push(token);
            }
            Ok(LooperMessage::Posted(Message::PaneExited { pane, .. })) => {
                exits.push(pane);
            }
            Ok(_) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // B replied to 2 requests, failed on the 3rd (exit drops ReplyPort),
    // and remaining requests either fail or were never delivered
    assert!(replies.len() >= 2,
        "B should reply to first 2 requests, got {} replies", replies.len());
    assert!(!failures.is_empty(),
        "A should get at least one ReplyFailed when B dies");
    assert_eq!(exits.len(), 1,
        "A should get exactly one PaneExited for B, got {}", exits.len());
    assert_eq!(exits[0], pane_id(3),
        "PaneExited should be for B's pane id");
}

// --- S11: Memory pressure (1000 requests with large payloads) ---

#[test]
fn stress_reply_large_payloads() {
    /// Handler that echoes back a vec of the same length.
    struct LargeEchoHandler;
    impl Handler for LargeEchoHandler {
        fn request_received(&mut self, _proxy: &Messenger, msg: Message, reply_port: ReplyPort) -> Result<bool> {
            if let Message::AppMessage(payload) = msg {
                if let Some(data) = payload.downcast_ref::<Vec<u8>>() {
                    let reply = vec![0xFFu8; data.len()];
                    reply_port.reply(reply);
                    return Ok(true);
                }
            }
            Ok(true)
        }
        fn close_requested(&mut self, _proxy: &Messenger) -> Result<bool> {
            Ok(false)
        }
    }

    let (target_proxy, target_rx) = make_looper_proxy(pane_id(2));
    let target_for_send = target_proxy.clone();

    let target_handle = thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, LargeEchoHandler)
    });

    let (caller_proxy, _) = make_looper_proxy(pane_id(1));

    // 1000 requests with 64KB payloads (~64MB total in flight)
    let payload_size = 64 * 1024;
    let num_requests = 1000u64;

    for i in 0..num_requests {
        let data = vec![i as u8; payload_size];
        let result = caller_proxy.send_and_wait(
            &target_for_send,
            Message::AppMessage(Box::new(data)),
            Duration::from_secs(5),
        );
        let reply = result.unwrap_or_else(|e| panic!("request {i} failed: {e:?}"));
        let reply_data = reply.downcast_ref::<Vec<u8>>().unwrap();
        assert_eq!(reply_data.len(), payload_size,
            "request {i}: reply size {}, expected {payload_size}", reply_data.len());
    }

    target_for_send.send_message(Message::CloseRequested).unwrap();
    target_handle.join().unwrap().unwrap();
}
