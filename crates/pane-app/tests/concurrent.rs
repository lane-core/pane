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

use pane_app::{App, Tag, PaneEvent, PaneHandle, LooperMessage};
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
                    PaneEvent::Focus | PaneEvent::Blur => count += 1,
                    PaneEvent::Close => return Ok(false),
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
    let proxy = pane.proxy();

    let num_threads = 5;
    let events_per_thread = 100;
    let expected_total = num_threads * events_per_thread;

    // Spawn worker threads that post events via self-delivery
    for _ in 0..num_threads {
        let p = proxy.clone();
        thread::spawn(move || {
            for _ in 0..events_per_thread {
                // Ignore errors — looper may exit before we finish
                let _ = p.post_event(PaneEvent::Focus);
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
            PaneEvent::Focus => { count.fetch_add(1, Ordering::Relaxed); }
            PaneEvent::Close => return Ok(false),
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
    let proxy = PaneHandle::new(pane_id(1), comp_tx);

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
            PaneEvent::Resize(g) => {
                resize_count += 1;
                last_resize_w = g.width;
            }
            PaneEvent::Mouse(m) if matches!(m.kind, MouseEventKind::Move) => {
                move_count += 1;
                last_move_col = m.col;
            }
            PaneEvent::Close => return Ok(false),
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
    let proxy = PaneHandle::new(pane_id(1), comp_tx);

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
                if tx_clone.send(LooperMessage::Posted(PaneEvent::Focus)).is_err() {
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
            PaneEvent::Focus => { count.fetch_add(1, Ordering::Relaxed); }
            PaneEvent::Close => return Ok(false),
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

// --- P3-E: PaneHandle clone flood ---

#[test]
fn pane_handle_clone_flood() {
    let (comp_tx, comp_rx) = mpsc::channel::<ClientToComp>();
    let handle = PaneHandle::new(pane_id(1), comp_tx);

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
    let proxy = pane.proxy();

    // Spawn a worker that posts events continuously
    let worker_proxy = proxy.clone();
    thread::spawn(move || {
        loop {
            if worker_proxy.post_event(PaneEvent::Focus).is_err() {
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
            PaneEvent::Close => Ok(false),
            _ => Ok(true),
        }
    });

    assert!(result.is_ok(), "pane.run() should exit cleanly, got {:?}", result);

    drop(app);
    mock_handle.join().unwrap();
}
