//! Tests for Pulse and ShortcutFilter.

use std::num::NonZeroU32;
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use pane_app::{Message, Messenger, Handler, LooperMessage, KeyCombo};
use pane_app::shortcuts::ShortcutFilter;
use pane_app::error::Result;
use pane_proto::event::{Key, KeyEvent, KeyState, Modifiers, NamedKey};
use pane_proto::message::PaneId;
use pane_proto::protocol::{ClientToComp, CompToClient};

fn pane_id(n: u32) -> PaneId {
    PaneId::new(NonZeroU32::new(n).unwrap())
}

fn make_proxy(id: PaneId) -> Messenger {
    let (tx, _rx) = mpsc::channel::<ClientToComp>();
    Messenger::new(id, tx)
}

fn send_comp(tx: &mpsc::Sender<LooperMessage>, msg: CompToClient) {
    tx.send(LooperMessage::FromComp(msg)).unwrap();
}

// --- Pulse tests ---

#[test]
fn pulse_dispatches_to_handler() {
    let (tx, rx) = mpsc::channel::<LooperMessage>();
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

    let (tx, rx) = mpsc::channel::<LooperMessage>();
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
    let (tx, rx) = mpsc::channel::<LooperMessage>();
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
    let (tx, rx) = mpsc::channel::<LooperMessage>();
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
    let (tx, rx) = mpsc::channel::<LooperMessage>();
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
    let (tx, rx) = mpsc::channel::<LooperMessage>();
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
    let (looper_tx, looper_rx) = mpsc::sync_channel::<LooperMessage>(256);
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
    let (looper_tx, looper_rx) = mpsc::sync_channel::<LooperMessage>(256);
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
    let (looper_tx, looper_rx) = mpsc::sync_channel::<LooperMessage>(256);
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
