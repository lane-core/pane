use std::num::NonZeroU32;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use pane_app::{PaneEvent, Handler, Filter, FilterAction};
use pane_app::error::Result;
use pane_proto::event::{KeyEvent, Key, NamedKey, Modifiers, KeyState};
use pane_proto::message::PaneId;
use pane_proto::protocol::{CompToClient, PaneGeometry};

fn pane_id(n: u32) -> PaneId {
    PaneId::new(NonZeroU32::new(n).unwrap())
}

fn geom() -> PaneGeometry {
    PaneGeometry { width: 800, height: 600, cols: 80, rows: 24 }
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

    tx.send(escape_key()).unwrap();
    drop(tx); // close channel after one message

    let mut got_key = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, |event| {
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

    tx.send(close_msg()).unwrap();
    drop(tx);

    let mut got_close = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, |event| {
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

    // Drop sender immediately — simulates compositor death
    drop(tx);

    let mut got_disconnect = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, |event| {
        if matches!(event, PaneEvent::Disconnected) {
            got_disconnect = true;
        }
        Ok(true) // doesn't matter, loop exits on channel close
    }).unwrap();

    assert!(got_disconnect);
}

#[test]
fn closure_ignores_wrong_pane_id() {
    let (tx, rx) = mpsc::channel();
    let filters = pane_app::filter::FilterChain::new();

    // Send a message for pane 2, but looper runs for pane 1
    tx.send(CompToClient::Focus { pane: pane_id(2) }).unwrap();
    // Then send close for pane 1
    tx.send(close_msg()).unwrap();
    drop(tx);

    let mut events_received = 0;
    pane_app::looper::run_closure(pane_id(1), rx, filters, |event| {
        events_received += 1;
        if matches!(event, PaneEvent::Close) {
            return Ok(false);
        }
        Ok(true)
    }).unwrap();

    // Should only receive the Close, not the Focus (wrong pane)
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

    // Send escape (will be consumed) then close (will reach handler)
    tx.send(escape_key()).unwrap();
    tx.send(close_msg()).unwrap();
    drop(tx);

    let mut got_escape = false;
    let mut got_close = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, |event| {
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
    fn ready(&mut self, _geom: PaneGeometry) -> Result<bool> {
        self.log.lock().unwrap().push("ready".into());
        Ok(true)
    }

    fn focused(&mut self) -> Result<bool> {
        self.log.lock().unwrap().push("focused".into());
        Ok(true)
    }

    fn blurred(&mut self) -> Result<bool> {
        self.log.lock().unwrap().push("blurred".into());
        Ok(true)
    }

    fn key(&mut self, event: KeyEvent) -> Result<bool> {
        self.log.lock().unwrap().push(format!("key:{:?}", event.key));
        Ok(true)
    }

    fn command_executed(&mut self, command: &str, args: &str) -> Result<bool> {
        self.log.lock().unwrap().push(format!("cmd:{}:{}", command, args));
        Ok(true)
    }

    fn close_requested(&mut self) -> Result<bool> {
        self.log.lock().unwrap().push("close".into());
        Ok(false) // accept close
    }
}

#[test]
fn handler_dispatches_to_correct_methods() {
    let (tx, rx) = mpsc::channel();
    let filters = pane_app::filter::FilterChain::new();
    let log = Arc::new(Mutex::new(Vec::new()));

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

    pane_app::looper::run_handler(pane_id(1), rx, filters, handler).unwrap();

    let log = log.lock().unwrap();
    assert_eq!(*log, vec!["focused", "blurred", "cmd:save:foo.rs", "close"]);
}
