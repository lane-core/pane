use std::num::NonZeroU32;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use pane_app::{PaneEvent, Handler, Filter, FilterAction, PaneHandle};
use pane_app::error::Result;
use pane_proto::event::{KeyEvent, Key, NamedKey, Modifiers, KeyState};
use pane_proto::message::PaneId;
use pane_proto::protocol::{ClientToComp, CompToClient, PaneGeometry};

fn pane_id(n: u32) -> PaneId {
    PaneId::new(NonZeroU32::new(n).unwrap())
}

fn make_proxy(id: PaneId) -> PaneHandle {
    // Create a dummy proxy for testing — the sender goes nowhere
    let (tx, _rx) = mpsc::channel::<ClientToComp>();
    PaneHandle::new(id, tx)
}

fn escape_key() -> CompToClient {
    CompToClient::Key {
        pane: pane_id(1),
        event: KeyEvent {
            key: Key::Named(NamedKey::Escape),
            modifiers: Modifiers::empty(),
            state: KeyState::Press,
        },
    }
}

fn close_msg() -> CompToClient {
    CompToClient::Close { pane: pane_id(1) }
}

// --- Closure-based looper tests ---

#[test]
fn closure_receives_key_and_exits() {
    let (tx, rx) = mpsc::channel();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    tx.send(escape_key()).unwrap();
    drop(tx);

    let mut got_key = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_proxy, event| {
        if let PaneEvent::Key(k) = &event {
            if k.is_escape() {
                got_key = true;
                return Ok(false);
            }
        }
        Ok(true)
    }).unwrap();

    assert!(got_key);
}

#[test]
fn closure_handles_close() {
    let (tx, rx) = mpsc::channel();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    tx.send(close_msg()).unwrap();
    drop(tx);

    let mut got_close = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_proxy, event| {
        if matches!(event, PaneEvent::Close) {
            got_close = true;
            return Ok(false);
        }
        Ok(true)
    }).unwrap();

    assert!(got_close);
}

#[test]
fn closure_handles_channel_close_as_disconnect() {
    let (tx, rx) = mpsc::channel::<CompToClient>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    drop(tx);

    let mut got_disconnect = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_proxy, event| {
        if matches!(event, PaneEvent::Disconnected) {
            got_disconnect = true;
        }
        Ok(true)
    }).unwrap();

    assert!(got_disconnect);
}

#[test]
fn closure_ignores_wrong_pane_id() {
    let (tx, rx) = mpsc::channel();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    tx.send(CompToClient::Focus { pane: pane_id(2) }).unwrap();
    tx.send(close_msg()).unwrap();
    drop(tx);

    let mut events_received = 0;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_proxy, event| {
        events_received += 1;
        if matches!(event, PaneEvent::Close) {
            return Ok(false);
        }
        Ok(true)
    }).unwrap();

    assert_eq!(events_received, 1);
}

// --- Filter tests ---

struct ConsumeEscapeFilter;

impl Filter for ConsumeEscapeFilter {
    fn filter(&mut self, event: PaneEvent) -> FilterAction {
        if let PaneEvent::Key(ref k) = event {
            if k.is_escape() {
                return FilterAction::Consume;
            }
        }
        FilterAction::Pass(event)
    }
}

#[test]
fn filter_consumes_event() {
    let (tx, rx) = mpsc::channel();
    let mut filters = pane_app::filter::FilterChain::new();
    filters.add(ConsumeEscapeFilter);
    let proxy = make_proxy(pane_id(1));

    tx.send(escape_key()).unwrap();
    tx.send(close_msg()).unwrap();
    drop(tx);

    let mut got_escape = false;
    let mut got_close = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_proxy, event| {
        match event {
            PaneEvent::Key(ref k) if k.is_escape() => got_escape = true,
            PaneEvent::Close => {
                got_close = true;
                return Ok(false);
            }
            _ => {}
        }
        Ok(true)
    }).unwrap();

    assert!(!got_escape, "escape should have been consumed by filter");
    assert!(got_close);
}

// --- Handler trait tests ---

struct TestHandler {
    log: Arc<Mutex<Vec<String>>>,
}

impl Handler for TestHandler {
    fn ready(&mut self, _proxy: &PaneHandle, _geom: PaneGeometry) -> Result<bool> {
        self.log.lock().unwrap().push("ready".into());
        Ok(true)
    }

    fn focused(&mut self, _proxy: &PaneHandle) -> Result<bool> {
        self.log.lock().unwrap().push("focused".into());
        Ok(true)
    }

    fn blurred(&mut self, _proxy: &PaneHandle) -> Result<bool> {
        self.log.lock().unwrap().push("blurred".into());
        Ok(true)
    }

    fn key(&mut self, _proxy: &PaneHandle, event: KeyEvent) -> Result<bool> {
        self.log.lock().unwrap().push(format!("key:{:?}", event.key));
        Ok(true)
    }

    fn command_executed(&mut self, _proxy: &PaneHandle, command: &str, args: &str) -> Result<bool> {
        self.log.lock().unwrap().push(format!("cmd:{}:{}", command, args));
        Ok(true)
    }

    fn close_requested(&mut self, _proxy: &PaneHandle) -> Result<bool> {
        self.log.lock().unwrap().push("close".into());
        Ok(false)
    }
}

#[test]
fn handler_dispatches_to_correct_methods() {
    let (tx, rx) = mpsc::channel();
    let filters = pane_app::filter::FilterChain::new();
    let log = Arc::new(Mutex::new(Vec::new()));
    let proxy = make_proxy(pane_id(1));

    let handler = TestHandler { log: log.clone() };

    tx.send(CompToClient::Focus { pane: pane_id(1) }).unwrap();
    tx.send(CompToClient::Blur { pane: pane_id(1) }).unwrap();
    tx.send(CompToClient::CommandExecuted {
        pane: pane_id(1),
        command: "save".into(),
        args: "foo.rs".into(),
    }).unwrap();
    tx.send(close_msg()).unwrap();
    drop(tx);

    pane_app::looper::run_handler(pane_id(1), rx, filters, proxy, handler).unwrap();

    let log = log.lock().unwrap();
    assert_eq!(*log, vec!["focused", "blurred", "cmd:save:foo.rs", "close"]);
}

// --- P1-3: Filter chain edge cases ---

#[test]
fn empty_filter_chain_passes_all() {
    let (tx, rx) = mpsc::channel();
    let filters = pane_app::filter::FilterChain::new(); // no filters
    let proxy = make_proxy(pane_id(1));

    tx.send(CompToClient::Focus { pane: pane_id(1) }).unwrap();
    tx.send(close_msg()).unwrap();
    drop(tx);

    let mut got_focus = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        if matches!(event, PaneEvent::Focus) { got_focus = true; }
        if matches!(event, PaneEvent::Close) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert!(got_focus, "focus should pass through empty filter chain");
}

struct TransformFilter;
impl Filter for TransformFilter {
    fn filter(&mut self, event: PaneEvent) -> FilterAction {
        // Transform Focus into Blur
        if matches!(event, PaneEvent::Focus) {
            return FilterAction::Pass(PaneEvent::Blur);
        }
        FilterAction::Pass(event)
    }
}

#[test]
fn filter_chain_ordering_transforms() {
    let (tx, rx) = mpsc::channel();
    let mut filters = pane_app::filter::FilterChain::new();
    filters.add(TransformFilter); // transforms Focus → Blur
    let proxy = make_proxy(pane_id(1));

    tx.send(CompToClient::Focus { pane: pane_id(1) }).unwrap();
    tx.send(close_msg()).unwrap();
    drop(tx);

    let mut got_blur = false;
    let mut got_focus = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        match event {
            PaneEvent::Focus => got_focus = true,
            PaneEvent::Blur => got_blur = true,
            PaneEvent::Close => return Ok(false),
            _ => {}
        }
        Ok(true)
    }).unwrap();

    assert!(!got_focus, "focus should have been transformed");
    assert!(got_blur, "should have received blur (transformed from focus)");
}

struct ConsumeAllFilter;
impl Filter for ConsumeAllFilter {
    fn filter(&mut self, _event: PaneEvent) -> FilterAction {
        FilterAction::Consume
    }
}

#[test]
fn filter_consume_all_then_disconnect() {
    let (tx, rx) = mpsc::channel();
    let mut filters = pane_app::filter::FilterChain::new();
    filters.add(ConsumeAllFilter);
    let proxy = make_proxy(pane_id(1));

    tx.send(CompToClient::Focus { pane: pane_id(1) }).unwrap();
    tx.send(close_msg()).unwrap();
    drop(tx); // disconnect after queued messages

    let mut events_seen = 0;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        // Only Disconnected should reach the handler — it's generated
        // by the looper itself, not from CompToClient (so not filtered)
        assert!(matches!(event, PaneEvent::Disconnected),
            "only Disconnected should pass through — got {:?}", event);
        events_seen += 1;
        Ok(true)
    }).unwrap();

    assert_eq!(events_seen, 1, "only Disconnected should reach handler (generated by looper, not filtered)");
}

// --- P1-5: Looper lifecycle ---

#[test]
fn looper_handler_error_propagates() {
    let (tx, rx) = mpsc::channel();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    tx.send(CompToClient::Focus { pane: pane_id(1) }).unwrap();
    drop(tx);

    let result = pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, _event| {
        Err(pane_app::error::Error::Pane(pane_app::error::PaneError::Refused))
    });

    assert!(result.is_err(), "handler error should propagate");
}

#[test]
fn looper_processes_all_queued_before_disconnect() {
    let (tx, rx) = mpsc::channel();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    // Queue 3 events then drop sender
    tx.send(CompToClient::Focus { pane: pane_id(1) }).unwrap();
    tx.send(CompToClient::Blur { pane: pane_id(1) }).unwrap();
    tx.send(CompToClient::Focus { pane: pane_id(1) }).unwrap();
    drop(tx);

    let mut count = 0;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        if !matches!(event, PaneEvent::Disconnected) {
            count += 1;
        }
        Ok(true)
    }).unwrap();

    assert_eq!(count, 3, "all 3 queued events should be processed before disconnect");
}

#[test]
fn looper_error_stops_processing() {
    let (tx, rx) = mpsc::channel();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    tx.send(CompToClient::Focus { pane: pane_id(1) }).unwrap();
    tx.send(CompToClient::Blur { pane: pane_id(1) }).unwrap(); // won't reach this
    tx.send(close_msg()).unwrap(); // won't reach this
    drop(tx);

    let mut count = 0;
    let result = pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, _event| {
        count += 1;
        if count == 1 {
            return Err(pane_app::error::Error::Pane(pane_app::error::PaneError::Refused));
        }
        Ok(true)
    });

    assert!(result.is_err());
    assert_eq!(count, 1, "error on first event should stop processing");
}
