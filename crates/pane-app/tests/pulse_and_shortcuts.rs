//! Tests for Pulse and ShortcutFilter.

use std::num::NonZeroU32;
use std::sync::mpsc;
use std::sync::Arc;

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

    // Start a 40ms pulse
    proxy.set_pulse_rate(Duration::from_millis(40)).unwrap();

    // Wait for at least one pulse to confirm it's running
    std::thread::sleep(Duration::from_millis(60));

    // Replace with a 500ms pulse (effectively silencing it for our test window)
    proxy.set_pulse_rate(Duration::from_millis(500)).unwrap();

    // Drain any pulses that arrived before cancellation
    while looper_rx.try_recv().is_ok() {}

    // Wait 150ms — if the old 40ms timer were still alive, we'd see ~3 pulses.
    // With only the 500ms timer, we should see zero.
    std::thread::sleep(Duration::from_millis(150));

    let mut stale_pulses = 0;
    while looper_rx.try_recv().is_ok() {
        stale_pulses += 1;
    }

    assert_eq!(stale_pulses, 0, "old pulse timer should have been cancelled");
}

#[test]
fn set_pulse_rate_zero_disables() {
    use std::time::Duration;

    let (comp_tx, _comp_rx) = mpsc::channel::<ClientToComp>();
    let (looper_tx, looper_rx) = mpsc::sync_channel::<LooperMessage>(256);
    let proxy = Messenger::new(pane_id(1), comp_tx).with_looper(looper_tx);

    // Start a 30ms pulse
    proxy.set_pulse_rate(Duration::from_millis(30)).unwrap();

    // Wait for at least one pulse
    std::thread::sleep(Duration::from_millis(50));

    // Disable
    proxy.set_pulse_rate(Duration::ZERO).unwrap();

    // Drain
    while looper_rx.try_recv().is_ok() {}

    // Wait — should see no more pulses
    std::thread::sleep(Duration::from_millis(100));

    let mut count = 0;
    while looper_rx.try_recv().is_ok() {
        count += 1;
    }

    assert_eq!(count, 0, "ZERO rate should disable pulse timer");
}
