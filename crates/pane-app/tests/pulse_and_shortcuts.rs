//! Tests for Pulse and ShortcutFilter.

use std::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use calloop::channel::{self, Sender};
use pane_app::{Message, Messenger, Handler, LooperMessage, KeyCombo};
use pane_app::shortcuts::ShortcutFilter;
use pane_app::error::Result;
use pane_proto::event::{Key, KeyEvent, KeyState, Modifiers, NamedKey};
use pane_proto::message::PaneId;
use pane_proto::protocol::{ClientToComp, CompToClient};

fn pane_id(n: u32) -> PaneId {
    PaneId::from_uuid(uuid::Uuid::from_u128(n as u128))
}

fn make_proxy(id: PaneId) -> Messenger {
    let (tx, _rx) = mpsc::channel::<ClientToComp>();
    Messenger::new(id, tx)
}

fn send_comp(tx: &Sender<LooperMessage>, msg: CompToClient) {
    tx.send(LooperMessage::FromComp(msg)).unwrap();
}

// --- Pulse tests ---

#[test]
fn pulse_dispatches_to_handler() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    // Post a Pulse then Close
    tx.send(LooperMessage::Posted(Message::Pulse)).unwrap();
    send_comp(&tx, CompToClient::Close { pane: pane_id(1) });
    drop(tx);

    let mut got_pulse = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_m, msg| {
        if matches!(msg, Message::Pulse) { got_pulse = true; }
        if matches!(msg, Message::CloseRequested) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert!(got_pulse, "Pulse should reach handler");
}

#[test]
fn pulse_handler_method() {
    use std::sync::Mutex;

    struct PulseHandler {
        count: Arc<Mutex<u32>>,
    }

    impl Handler for PulseHandler {
        fn pulse(&mut self, _proxy: &Messenger) -> Result<bool> {
            *self.count.lock().unwrap() += 1;
            Ok(true)
        }

        fn close_requested(&mut self, _proxy: &Messenger) -> Result<bool> {
            Ok(false)
        }
    }

    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));
    let count = Arc::new(Mutex::new(0u32));

    let handler = PulseHandler { count: count.clone() };

    tx.send(LooperMessage::Posted(Message::Pulse)).unwrap();
    tx.send(LooperMessage::Posted(Message::Pulse)).unwrap();
    tx.send(LooperMessage::Posted(Message::Pulse)).unwrap();
    send_comp(&tx, CompToClient::Close { pane: pane_id(1) });
    drop(tx);

    pane_app::looper::run_handler(pane_id(1), rx, filters, proxy, handler).unwrap();

    assert_eq!(*count.lock().unwrap(), 3, "should receive 3 pulses");
}

// --- ShortcutFilter tests ---

#[test]
fn shortcut_filter_transforms_key_to_command() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let mut filters = pane_app::filter::FilterChain::new();

    let mut shortcuts = ShortcutFilter::new();
    shortcuts.add(
        KeyCombo::new(Key::Char('s'), Modifiers::CTRL),
        "save", "",
    );
    filters.add(shortcuts);

    let proxy = make_proxy(pane_id(1));

    // Send Ctrl+S
    send_comp(&tx, CompToClient::Key {
        pane: pane_id(1),
        event: KeyEvent {
            key: Key::Char('s'),
            modifiers: Modifiers::CTRL,
            state: KeyState::Press,
            timestamp: None,
        },
    });
    send_comp(&tx, CompToClient::Close { pane: pane_id(1) });
    drop(tx);

    let mut got_save = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_m, msg| {
        if let Message::CommandExecuted { ref command, .. } = msg {
            if command == "save" { got_save = true; }
        }
        if matches!(msg, Message::CloseRequested) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert!(got_save, "Ctrl+S should be transformed into CommandExecuted(save)");
}

#[test]
fn shortcut_filter_passes_unmatched_keys() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let mut filters = pane_app::filter::FilterChain::new();

    let mut shortcuts = ShortcutFilter::new();
    shortcuts.add(
        KeyCombo::new(Key::Char('s'), Modifiers::CTRL),
        "save", "",
    );
    filters.add(shortcuts);

    let proxy = make_proxy(pane_id(1));

    // Send plain 'a' (no shortcut match)
    send_comp(&tx, CompToClient::Key {
        pane: pane_id(1),
        event: KeyEvent {
            key: Key::Char('a'),
            modifiers: Modifiers::empty(),
            state: KeyState::Press,
            timestamp: None,
        },
    });
    send_comp(&tx, CompToClient::Close { pane: pane_id(1) });
    drop(tx);

    let mut got_key = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_m, msg| {
        if matches!(msg, Message::Key(_)) { got_key = true; }
        if matches!(msg, Message::CloseRequested) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert!(got_key, "unmatched key should pass through as Key message");
}

#[test]
fn shortcut_filter_ignores_key_release() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let mut filters = pane_app::filter::FilterChain::new();

    let mut shortcuts = ShortcutFilter::new();
    shortcuts.add(
        KeyCombo::new(Key::Char('s'), Modifiers::CTRL),
        "save", "",
    );
    filters.add(shortcuts);

    let proxy = make_proxy(pane_id(1));

    // Send Ctrl+S Release (not Press)
    send_comp(&tx, CompToClient::Key {
        pane: pane_id(1),
        event: KeyEvent {
            key: Key::Char('s'),
            modifiers: Modifiers::CTRL,
            state: KeyState::Release,
            timestamp: None,
        },
    });
    send_comp(&tx, CompToClient::Close { pane: pane_id(1) });
    drop(tx);

    let mut got_command = false;
    let mut got_key = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_m, msg| {
        if matches!(msg, Message::CommandExecuted { .. }) { got_command = true; }
        if matches!(msg, Message::Key(_)) { got_key = true; }
        if matches!(msg, Message::CloseRequested) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert!(!got_command, "key release should not trigger shortcut");
    assert!(got_key, "key release should pass through as Key");
}

#[test]
fn shortcut_filter_with_escape() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let mut filters = pane_app::filter::FilterChain::new();

    let mut shortcuts = ShortcutFilter::new();
    shortcuts.add(
        KeyCombo::new(Key::Named(NamedKey::Escape), Modifiers::empty()),
        "close", "",
    );
    filters.add(shortcuts);

    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, CompToClient::Key {
        pane: pane_id(1),
        event: KeyEvent {
            key: Key::Named(NamedKey::Escape),
            modifiers: Modifiers::empty(),
            state: KeyState::Press,
            timestamp: None,
        },
    });
    send_comp(&tx, CompToClient::Close { pane: pane_id(1) });
    drop(tx);

    let mut got_close_cmd = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_m, msg| {
        if let Message::CommandExecuted { ref command, .. } = msg {
            if command == "close" { got_close_cmd = true; }
        }
        if matches!(msg, Message::CloseRequested) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert!(got_close_cmd, "Escape should be transformed to close command");
}

// --- Pulse cancellation tests ---

#[test]
fn set_pulse_rate_cancels_previous() {
    use std::time::Duration;

    let (comp_tx, _comp_rx) = mpsc::channel::<ClientToComp>();
    let (looper_tx, looper_rx) = channel::channel::<LooperMessage>();
    let proxy = Messenger::new(pane_id(1), comp_tx).with_looper(looper_tx);
    let outer_proxy = proxy.clone();

    // Track pulse count and timing: after switching from 30ms to 500ms,
    // we should stop seeing rapid pulses.
    let fast_count = Arc::new(AtomicU32::new(0));
    let slow_count = Arc::new(AtomicU32::new(0));
    let switched = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let fc = fast_count.clone();
    let sc = slow_count.clone();
    let sw = switched.clone();

    let handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_closure(pane_id(1), looper_rx, filters, proxy, move |_m, msg| {
            if matches!(msg, Message::Pulse) {
                if sw.load(Ordering::Relaxed) {
                    sc.fetch_add(1, Ordering::Relaxed);
                } else {
                    fc.fetch_add(1, Ordering::Relaxed);
                }
            }
            if matches!(msg, Message::CloseRequested) { return Ok(false); }
            Ok(true)
        })
    });

    // Start a 30ms pulse, let it run for a bit
    outer_proxy.set_pulse_rate(Duration::from_millis(30)).unwrap();
    std::thread::sleep(Duration::from_millis(120));

    // Switch to 500ms — mark the switchover
    switched.store(true, Ordering::Relaxed);
    outer_proxy.set_pulse_rate(Duration::from_millis(500)).unwrap();

    // Wait 150ms — if the old 30ms timer survived, we'd see ~5 pulses.
    // With 500ms timer only, we should see 0.
    std::thread::sleep(Duration::from_millis(150));

    // Stop the looper
    outer_proxy.send_message(Message::CloseRequested).unwrap();
    handle.join().unwrap().unwrap();

    let fast = fast_count.load(Ordering::Relaxed);
    let slow = slow_count.load(Ordering::Relaxed);

    assert!(fast >= 2, "should have seen fast pulses before switch, got {fast}");
    assert_eq!(slow, 0, "old timer should be cancelled — no fast pulses after switch, got {slow}");
}

#[test]
fn set_pulse_rate_zero_disables() {
    use std::time::Duration;

    let (comp_tx, _comp_rx) = mpsc::channel::<ClientToComp>();
    let (looper_tx, looper_rx) = channel::channel::<LooperMessage>();
    let proxy = Messenger::new(pane_id(1), comp_tx).with_looper(looper_tx);
    let outer_proxy = proxy.clone();

    let count_before = Arc::new(AtomicU32::new(0));
    let count_after = Arc::new(AtomicU32::new(0));
    let disabled = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let cb = count_before.clone();
    let ca = count_after.clone();
    let d = disabled.clone();

    let handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_closure(pane_id(1), looper_rx, filters, proxy, move |_m, msg| {
            if matches!(msg, Message::Pulse) {
                if d.load(Ordering::Relaxed) {
                    ca.fetch_add(1, Ordering::Relaxed);
                } else {
                    cb.fetch_add(1, Ordering::Relaxed);
                }
            }
            if matches!(msg, Message::CloseRequested) { return Ok(false); }
            Ok(true)
        })
    });

    // Start a 30ms pulse
    outer_proxy.set_pulse_rate(Duration::from_millis(30)).unwrap();
    std::thread::sleep(Duration::from_millis(120));

    // Disable
    disabled.store(true, Ordering::Relaxed);
    outer_proxy.set_pulse_rate(Duration::ZERO).unwrap();

    // Wait — should see no more pulses
    std::thread::sleep(Duration::from_millis(120));

    outer_proxy.send_message(Message::CloseRequested).unwrap();
    handle.join().unwrap().unwrap();

    let before = count_before.load(Ordering::Relaxed);
    let after = count_after.load(Ordering::Relaxed);

    assert!(before >= 2, "should have seen pulses before disable, got {before}");
    assert_eq!(after, 0, "ZERO rate should disable — no pulses after, got {after}");
}

#[test]
fn pulse_fires_through_looper_timers() {
    use std::time::Duration;

    let (comp_tx, _comp_rx) = mpsc::channel::<ClientToComp>();
    let (looper_tx, looper_rx) = channel::channel::<LooperMessage>();
    let proxy = Messenger::new(pane_id(1), comp_tx).with_looper(looper_tx);
    let outer_proxy = proxy.clone();

    let pulse_count = Arc::new(AtomicU32::new(0));
    let count_clone = pulse_count.clone();

    // Run the looper on a thread — it will fire pulse timers internally
    let handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_closure(pane_id(1), looper_rx, filters, proxy, move |_m, msg| {
            if matches!(msg, Message::Pulse) {
                let n = count_clone.fetch_add(1, Ordering::Relaxed) + 1;
                if n >= 3 {
                    return Ok(false); // exit after 3 pulses
                }
            }
            Ok(true)
        })
    });

    // Set pulse rate from outside the looper
    outer_proxy.set_pulse_rate(Duration::from_millis(30)).unwrap();

    // Wait for the looper to exit (after 3 pulses), with a safety timeout
    let result = handle.join().unwrap();
    assert!(result.is_ok(), "looper should exit cleanly");
    assert!(
        pulse_count.load(Ordering::Relaxed) >= 3,
        "should have received at least 3 pulses through the looper timer, got {}",
        pulse_count.load(Ordering::Relaxed),
    );
}

// --- Timer adversarial tests ---

/// Helper: create a Messenger with looper channel, return (proxy, looper_channel).
fn make_looper_proxy(id: PaneId) -> (Messenger, channel::Channel<LooperMessage>) {
    let (comp_tx, _) = mpsc::channel::<ClientToComp>();
    let (looper_tx, looper_rx) = channel::channel::<LooperMessage>();
    (Messenger::new(id, comp_tx).with_looper(looper_tx), looper_rx)
}

#[test]
fn send_delayed_fires_through_looper() {
    let (proxy, looper_rx) = make_looper_proxy(pane_id(1));
    let outer_proxy = proxy.clone();

    let received_at = Arc::new(std::sync::Mutex::new(None::<Instant>));
    let received_clone = received_at.clone();
    let start = Instant::now();

    let handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_closure(pane_id(1), looper_rx, filters, proxy, move |_m, msg| {
            // Use CommandExecuted as the delayed event (it's clonable, unlike App)
            if matches!(msg, Message::CommandExecuted { .. }) {
                *received_clone.lock().unwrap() = Some(Instant::now());
                return Ok(false); // exit after receiving the delayed event
            }
            if matches!(msg, Message::CloseRequested) { return Ok(false); }
            Ok(true)
        })
    });

    // Schedule a delayed event for 50ms from now.
    // send_delayed takes ownership. We use CommandExecuted as a distinguishable marker —
    // we use CommandExecuted rather than App.
    outer_proxy.send_delayed(
        Message::CommandExecuted { command: "delayed".into(), args: String::new() },
        Duration::from_millis(50),
    ).unwrap();

    // Safety timeout: if the looper doesn't exit in 2 seconds, something is wrong
    let join_result = handle.join();
    assert!(join_result.is_ok(), "looper thread should not panic");
    assert!(join_result.unwrap().is_ok(), "looper should exit cleanly");

    let arrival = received_at.lock().unwrap()
        .expect("delayed event should have been received");
    let elapsed = arrival.duration_since(start);
    assert!(
        elapsed >= Duration::from_millis(40),
        "delayed event arrived too early: {:?} (expected >= 40ms)", elapsed,
    );
    assert!(
        elapsed < Duration::from_millis(200),
        "delayed event arrived too late: {:?} (expected < 200ms)", elapsed,
    );
}

#[test]
fn multiple_concurrent_timers() {
    let (proxy, looper_rx) = make_looper_proxy(pane_id(1));
    let outer_proxy = proxy.clone();

    // Three counters for three different periodic timers.
    // Timer events must be Clone, so we use CommandExecuted with
    // distinct command strings to distinguish them.
    let count_20 = Arc::new(AtomicU32::new(0));
    let count_40 = Arc::new(AtomicU32::new(0));
    let count_80 = Arc::new(AtomicU32::new(0));
    let c20 = count_20.clone();
    let c40 = count_40.clone();
    let c80 = count_80.clone();

    let handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_closure(pane_id(1), looper_rx, filters, proxy, move |_m, msg| {
            if let Message::CommandExecuted { ref command, .. } = msg {
                match command.as_str() {
                    "t20" => { c20.fetch_add(1, Ordering::Relaxed); }
                    "t40" => { c40.fetch_add(1, Ordering::Relaxed); }
                    "t80" => { c80.fetch_add(1, Ordering::Relaxed); }
                    _ => {}
                }
            }
            if matches!(msg, Message::CloseRequested) { return Ok(false); }
            Ok(true)
        })
    });

    // Start three periodic timers with different intervals
    let _t1 = outer_proxy.send_periodic(
        Message::CommandExecuted { command: "t20".into(), args: String::new() },
        Duration::from_millis(20),
    ).unwrap();
    let _t2 = outer_proxy.send_periodic(
        Message::CommandExecuted { command: "t40".into(), args: String::new() },
        Duration::from_millis(40),
    ).unwrap();
    let _t3 = outer_proxy.send_periodic(
        Message::CommandExecuted { command: "t80".into(), args: String::new() },
        Duration::from_millis(80),
    ).unwrap();

    // Let them run for 200ms
    std::thread::sleep(Duration::from_millis(200));
    outer_proxy.send_message(Message::CloseRequested).unwrap();
    handle.join().unwrap().unwrap();

    let n20 = count_20.load(Ordering::Relaxed);
    let n40 = count_40.load(Ordering::Relaxed);
    let n80 = count_80.load(Ordering::Relaxed);

    // 20ms timer over 200ms: expect ~10 fires (allow 4..20)
    assert!(n20 >= 4, "20ms timer should fire at least 4 times in 200ms, got {n20}");
    assert!(n20 <= 20, "20ms timer fired too many times: {n20}");

    // 40ms timer over 200ms: expect ~5 fires (allow 2..12)
    assert!(n40 >= 2, "40ms timer should fire at least 2 times in 200ms, got {n40}");
    assert!(n40 <= 12, "40ms timer fired too many times: {n40}");

    // 80ms timer over 200ms: expect ~2 fires (allow 1..6)
    assert!(n80 >= 1, "80ms timer should fire at least once in 200ms, got {n80}");
    assert!(n80 <= 6, "80ms timer fired too many times: {n80}");

    // Faster timer should fire more than slower
    assert!(n20 > n80, "20ms timer ({n20}) should fire more often than 80ms timer ({n80})");
}

#[test]
fn timer_survives_message_burst() {
    let (proxy, looper_rx) = make_looper_proxy(pane_id(1));
    let outer_proxy = proxy.clone();

    let pulse_count = Arc::new(AtomicU32::new(0));
    let burst_count = Arc::new(AtomicU32::new(0));
    let pc = pulse_count.clone();
    let bc = burst_count.clone();

    let handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_closure(pane_id(1), looper_rx, filters, proxy, move |_m, msg| {
            match msg {
                Message::Pulse => { pc.fetch_add(1, Ordering::Relaxed); }
                Message::Activated => { bc.fetch_add(1, Ordering::Relaxed); }
                Message::CloseRequested => return Ok(false),
                _ => {}
            }
            Ok(true)
        })
    });

    // Start a 20ms pulse timer
    outer_proxy.set_pulse_rate(Duration::from_millis(20)).unwrap();

    // Let a few pulses fire first
    std::thread::sleep(Duration::from_millis(60));

    // Flood the channel with 100 Posted messages
    for _ in 0..100 {
        let _ = outer_proxy.send_message(Message::Activated);
    }

    // Let pulses continue after the burst
    std::thread::sleep(Duration::from_millis(100));

    outer_proxy.send_message(Message::CloseRequested).unwrap();
    handle.join().unwrap().unwrap();

    let pulses = pulse_count.load(Ordering::Relaxed);
    let bursts = burst_count.load(Ordering::Relaxed);

    assert_eq!(bursts, 100, "all 100 burst messages should arrive, got {bursts}");
    // Over ~160ms with 20ms interval, expect at least a few pulses
    assert!(pulses >= 3, "pulse timer should survive message burst, got only {pulses} pulses");
}

#[test]
fn cancelled_timer_never_fires_again() {
    let (proxy, looper_rx) = make_looper_proxy(pane_id(1));
    let outer_proxy = proxy.clone();

    let fire_count = Arc::new(AtomicU32::new(0));
    let fc = fire_count.clone();

    let handle = std::thread::spawn(move || {
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

    // Start a 20ms periodic timer (Activated is clonable)
    let token = outer_proxy.send_periodic(
        Message::Activated,
        Duration::from_millis(20),
    ).unwrap();

    // Wait for at least 2 fires
    std::thread::sleep(Duration::from_millis(80));
    let count_before_cancel = fire_count.load(Ordering::Relaxed);
    assert!(
        count_before_cancel >= 2,
        "timer should have fired at least twice before cancel, got {count_before_cancel}",
    );

    // Cancel the timer
    token.cancel();

    // Record the count right after cancellation, wait, then check no more fires
    std::thread::sleep(Duration::from_millis(20)); // one tick for the looper to see the cancel
    let count_at_cancel = fire_count.load(Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(100));
    let count_after_wait = fire_count.load(Ordering::Relaxed);

    outer_proxy.send_message(Message::CloseRequested).unwrap();
    handle.join().unwrap().unwrap();

    assert_eq!(
        count_at_cancel, count_after_wait,
        "cancelled timer should not fire again: count at cancel = {count_at_cancel}, after 100ms = {count_after_wait}",
    );
}

#[test]
fn set_pulse_rate_rapid_changes() {
    let (proxy, looper_rx) = make_looper_proxy(pane_id(1));
    let outer_proxy = proxy.clone();

    let pulse_count = Arc::new(AtomicU32::new(0));
    let pc = pulse_count.clone();

    let handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_closure(pane_id(1), looper_rx, filters, proxy, move |_m, msg| {
            match msg {
                Message::Pulse => { pc.fetch_add(1, Ordering::Relaxed); }
                Message::CloseRequested => return Ok(false),
                _ => {}
            }
            Ok(true)
        })
    });

    // Rapidly change pulse rates 5 times. Each call cancels the previous.
    // Rates: 10ms, 15ms, 20ms, 25ms, 500ms (final = very slow)
    outer_proxy.set_pulse_rate(Duration::from_millis(10)).unwrap();
    outer_proxy.set_pulse_rate(Duration::from_millis(15)).unwrap();
    outer_proxy.set_pulse_rate(Duration::from_millis(20)).unwrap();
    outer_proxy.set_pulse_rate(Duration::from_millis(25)).unwrap();
    outer_proxy.set_pulse_rate(Duration::from_millis(500)).unwrap();

    // Wait 200ms. With a 500ms rate, we should get 0 pulses.
    // If any of the fast timers leaked through, we'd see many.
    std::thread::sleep(Duration::from_millis(200));

    let count = pulse_count.load(Ordering::Relaxed);
    outer_proxy.send_message(Message::CloseRequested).unwrap();
    handle.join().unwrap().unwrap();

    assert!(
        count <= 1,
        "only the final 500ms rate should be active; got {count} pulses in 200ms (leaked fast timers?)",
    );
}
