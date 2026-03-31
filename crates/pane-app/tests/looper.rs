use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use calloop::channel::{self, Sender};
use pane_app::{Message, Handler, MessageFilter, FilterAction, Messenger, LooperMessage};
use pane_app::error::Result;
use pane_proto::event::{KeyEvent, Key, NamedKey, Modifiers, KeyState};
use pane_proto::message::PaneId;
use pane_proto::protocol::{ClientToComp, CompToClient, PaneGeometry};

fn pane_id(n: u32) -> PaneId {
    PaneId::from_uuid(uuid::Uuid::from_u128(n as u128))
}

fn make_proxy(id: PaneId) -> Messenger {
    // Create a dummy proxy for testing — the sender goes nowhere
    let (tx, _rx) = mpsc::channel::<ClientToComp>();
    Messenger::new(id, tx)
}

/// Send a CompToClient message through a LooperMessage channel.
fn send_comp(tx: &Sender<LooperMessage>, msg: CompToClient) {
    tx.send(LooperMessage::FromComp(msg)).unwrap();
}

fn escape_key() -> CompToClient {
    CompToClient::Key {
        pane: pane_id(1),
        event: KeyEvent {
            key: Key::Named(NamedKey::Escape),
            modifiers: Modifiers::empty(),
            state: KeyState::Press,
            timestamp: None,
        },
    }
}

fn close_msg() -> CompToClient {
    CompToClient::Close { pane: pane_id(1) }
}

// --- Closure-based looper tests ---

#[test]
fn closure_receives_key_and_exits() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, escape_key());
    drop(tx);

    let mut got_key = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_proxy, event| {
        if let Message::Key(k) = &event {
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
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, close_msg());
    drop(tx);

    let mut got_close = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_proxy, event| {
        if matches!(event, Message::CloseRequested) {
            got_close = true;
            return Ok(false);
        }
        Ok(true)
    }).unwrap();

    assert!(got_close);
}

#[test]
fn closure_handles_channel_close_as_disconnect() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    drop(tx);

    let mut got_disconnect = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_proxy, event| {
        if matches!(event, Message::Disconnected) {
            got_disconnect = true;
        }
        Ok(true)
    }).unwrap();

    assert!(got_disconnect);
}

#[test]
fn closure_ignores_wrong_pane_id() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, CompToClient::Focus { pane: pane_id(2) });
    send_comp(&tx, close_msg());
    drop(tx);

    let mut events_received = 0;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_proxy, event| {
        events_received += 1;
        if matches!(event, Message::CloseRequested) {
            return Ok(false);
        }
        Ok(true)
    }).unwrap();

    assert_eq!(events_received, 1);
}

// --- Filter tests ---

struct ConsumeEscapeFilter;

impl MessageFilter for ConsumeEscapeFilter {
    fn filter(&mut self, event: Message) -> FilterAction {
        if let Message::Key(ref k) = event {
            if k.is_escape() {
                return FilterAction::Consume;
            }
        }
        FilterAction::Pass(event)
    }
}

#[test]
fn filter_consumes_event() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let mut filters = pane_app::filter::FilterChain::new();
    filters.add(ConsumeEscapeFilter);
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, escape_key());
    send_comp(&tx, close_msg());
    drop(tx);

    let mut got_escape = false;
    let mut got_close = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_proxy, event| {
        match event {
            Message::Key(ref k) if k.is_escape() => got_escape = true,
            Message::CloseRequested => {
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
    fn ready(&mut self, _proxy: &Messenger, _geom: PaneGeometry) -> Result<bool> {
        self.log.lock().unwrap().push("ready".into());
        Ok(true)
    }

    fn activated(&mut self, _proxy: &Messenger) -> Result<bool> {
        self.log.lock().unwrap().push("activated".into());
        Ok(true)
    }

    fn deactivated(&mut self, _proxy: &Messenger) -> Result<bool> {
        self.log.lock().unwrap().push("deactivated".into());
        Ok(true)
    }

    fn key(&mut self, _proxy: &Messenger, event: KeyEvent) -> Result<bool> {
        self.log.lock().unwrap().push(format!("key:{:?}", event.key));
        Ok(true)
    }

    fn command_executed(&mut self, _proxy: &Messenger, command: &str, args: &str) -> Result<bool> {
        self.log.lock().unwrap().push(format!("cmd:{}:{}", command, args));
        Ok(true)
    }

    fn close_requested(&mut self, _proxy: &Messenger) -> Result<bool> {
        self.log.lock().unwrap().push("close".into());
        Ok(false)
    }
}

#[test]
fn handler_dispatches_to_correct_methods() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let log = Arc::new(Mutex::new(Vec::new()));
    let proxy = make_proxy(pane_id(1));

    let handler = TestHandler { log: log.clone() };

    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    send_comp(&tx, CompToClient::Blur { pane: pane_id(1) });
    send_comp(&tx, CompToClient::CommandExecuted {
        pane: pane_id(1),
        command: "save".into(),
        args: "foo.rs".into(),
    });
    send_comp(&tx, close_msg());
    drop(tx);

    pane_app::looper::run_handler(pane_id(1), rx, filters, proxy, handler).unwrap();

    let log = log.lock().unwrap();
    assert_eq!(*log, vec!["activated", "deactivated", "cmd:save:foo.rs", "close"]);
}

// --- P1-3: Filter chain edge cases ---

#[test]
fn empty_filter_chain_passes_all() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new(); // no filters
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    send_comp(&tx, close_msg());
    drop(tx);

    let mut got_focus = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        if matches!(event, Message::Activated) { got_focus = true; }
        if matches!(event, Message::CloseRequested) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert!(got_focus, "focus should pass through empty filter chain");
}

struct TransformFilter;
impl MessageFilter for TransformFilter {
    fn filter(&mut self, event: Message) -> FilterAction {
        // Transform Focus into Blur
        if matches!(event, Message::Activated) {
            return FilterAction::Pass(Message::Deactivated);
        }
        FilterAction::Pass(event)
    }
}

#[test]
fn filter_chain_ordering_transforms() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let mut filters = pane_app::filter::FilterChain::new();
    filters.add(TransformFilter); // transforms Focus → Blur
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    send_comp(&tx, close_msg());
    drop(tx);

    let mut got_blur = false;
    let mut got_focus = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        match event {
            Message::Activated => got_focus = true,
            Message::Deactivated => got_blur = true,
            Message::CloseRequested => return Ok(false),
            _ => {}
        }
        Ok(true)
    }).unwrap();

    assert!(!got_focus, "focus should have been transformed");
    assert!(got_blur, "should have received blur (transformed from focus)");
}

struct ConsumeAllFilter;
impl MessageFilter for ConsumeAllFilter {
    fn filter(&mut self, _event: Message) -> FilterAction {
        FilterAction::Consume
    }
}

#[test]
fn filter_consume_all_then_disconnect() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let mut filters = pane_app::filter::FilterChain::new();
    filters.add(ConsumeAllFilter);
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    send_comp(&tx, close_msg());
    drop(tx); // disconnect after queued messages

    let mut events_seen = 0;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        // Only Disconnected should reach the handler — it's generated
        // by the looper itself, not from CompToClient (so not filtered)
        assert!(matches!(event, Message::Disconnected),
            "only Disconnected should pass through — got {:?}", event);
        events_seen += 1;
        Ok(true)
    }).unwrap();

    assert_eq!(events_seen, 1, "only Disconnected should reach handler (generated by looper, not filtered)");
}

// --- P1-5: Looper lifecycle ---

#[test]
fn looper_handler_error_propagates() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    drop(tx);

    let result = pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, _event| {
        Err(pane_app::error::Error::Pane(pane_app::error::PaneError::Refused))
    });

    assert!(result.is_err(), "handler error should propagate");
}

#[test]
fn looper_processes_all_queued_before_disconnect() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    // Queue 3 events then drop sender
    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    send_comp(&tx, CompToClient::Blur { pane: pane_id(1) });
    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    drop(tx);

    let mut count = 0;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        if !matches!(event, Message::Disconnected) {
            count += 1;
        }
        Ok(true)
    }).unwrap();

    assert_eq!(count, 3, "all 3 queued events should be processed before disconnect");
}

#[test]
fn looper_error_stops_processing() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    send_comp(&tx, CompToClient::Blur { pane: pane_id(1) }); // won't reach this
    send_comp(&tx, close_msg()); // won't reach this
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

// --- Commit 2: Filter wants() tests ---

/// A filter that consumes all events it actually sees, but only
/// wants non-Focus events (skips Focus via wants()).
struct SkipFocusFilter;

impl MessageFilter for SkipFocusFilter {
    fn filter(&mut self, _event: Message) -> FilterAction {
        FilterAction::Consume
    }

    fn matches(&self, event: &Message) -> bool {
        !matches!(event, Message::Activated)
    }
}

#[test]
fn filter_wants_skips_uninterested() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let mut filters = pane_app::filter::FilterChain::new();
    filters.add(SkipFocusFilter);
    let proxy = make_proxy(pane_id(1));

    // Focus should bypass the filter (wants returns false) → reaches handler
    // Close should be consumed by the filter
    // Disconnect will terminate
    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    send_comp(&tx, close_msg());
    drop(tx);

    let mut got_focus = false;
    let mut got_close = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        match event {
            Message::Activated => got_focus = true,
            Message::CloseRequested => { got_close = true; return Ok(false); }
            Message::Disconnected => return Ok(false),
            _ => {}
        }
        Ok(true)
    }).unwrap();

    assert!(got_focus, "Focus should pass through — filter doesn't want it");
    assert!(!got_close, "Close should be consumed by filter");
}

#[test]
fn filter_wants_default_true() {
    // A filter with no wants() override should see all events (backward compat)
    let (tx, rx) = channel::channel::<LooperMessage>();
    let mut filters = pane_app::filter::FilterChain::new();
    filters.add(ConsumeAllFilter);
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    drop(tx);

    let mut events_seen = 0;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        events_seen += 1;
        if matches!(event, Message::Disconnected) { return Ok(false); }
        Ok(true)
    }).unwrap();

    // ConsumeAllFilter has default wants() = true, so it consumes Focus.
    // Only Disconnected (looper-generated) reaches handler.
    assert_eq!(events_seen, 1, "only Disconnected should reach handler");
}

// --- Commit 3: Self-delivery tests ---

#[test]
fn self_delivery_reaches_handler() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    // Post a self-delivered Focus, then a comp Close to terminate
    tx.send(LooperMessage::Posted(Message::Activated)).unwrap();
    send_comp(&tx, close_msg());
    drop(tx);

    let mut got_focus = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        if matches!(event, Message::Activated) { got_focus = true; }
        if matches!(event, Message::CloseRequested) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert!(got_focus, "self-delivered Focus should reach the handler");
}

#[test]
fn self_delivery_interleaved_with_comp() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    tx.send(LooperMessage::Posted(Message::Deactivated)).unwrap();
    send_comp(&tx, close_msg());
    drop(tx);

    let mut log = Vec::new();
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        log.push(format!("{:?}", std::mem::discriminant(&event)));
        if matches!(event, Message::CloseRequested) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert_eq!(log.len(), 3, "should see Focus, Blur, Close");
}

#[test]
fn self_delivery_filters_apply() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let mut filters = pane_app::filter::FilterChain::new();
    filters.add(ConsumeEscapeFilter);
    let proxy = make_proxy(pane_id(1));

    // Self-deliver an Escape key event — filter should consume it
    let esc = KeyEvent {
        key: Key::Named(NamedKey::Escape),
        modifiers: Modifiers::empty(),
        state: KeyState::Press,
        timestamp: None,
    };
    tx.send(LooperMessage::Posted(Message::Key(esc))).unwrap();
    send_comp(&tx, close_msg());
    drop(tx);

    let mut got_escape = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        if matches!(event, Message::Key(_)) { got_escape = true; }
        if matches!(event, Message::CloseRequested) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert!(!got_escape, "self-delivered escape should be consumed by filter");
}

// --- Commit 4: Message coalescing tests ---

use pane_proto::event::{MouseEvent, MouseButton, MouseEventKind};

fn resize_msg(w: u32, h: u32) -> CompToClient {
    CompToClient::Resize {
        pane: pane_id(1),
        geometry: PaneGeometry { width: w, height: h, cols: 80, rows: 24 },
    }
}

fn mouse_move_msg(col: u16, row: u16) -> CompToClient {
    CompToClient::Mouse {
        pane: pane_id(1),
        event: MouseEvent {
            col, row,
            kind: MouseEventKind::Move,
            modifiers: Modifiers::empty(),
            timestamp: None,
        },
    }
}

#[test]
fn coalesce_resize_keeps_last() {
    // Pre-load 3 resizes + close before starting the looper.
    // All messages are buffered, so drain_and_coalesce sees them all.
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, resize_msg(100, 100));
    send_comp(&tx, resize_msg(200, 200));
    send_comp(&tx, resize_msg(300, 300));
    send_comp(&tx, close_msg());
    drop(tx);

    let mut resize_count = 0;
    let mut last_width = 0;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        if let Message::Resize(g) = &event {
            resize_count += 1;
            last_width = g.width;
        }
        if matches!(event, Message::CloseRequested) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert_eq!(resize_count, 1, "should coalesce to 1 resize");
    assert_eq!(last_width, 300, "should keep the last geometry");
}

#[test]
fn coalesce_mouse_move_keeps_last() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, mouse_move_msg(10, 10));
    send_comp(&tx, mouse_move_msg(20, 20));
    send_comp(&tx, mouse_move_msg(30, 30));
    send_comp(&tx, close_msg());
    drop(tx);

    let mut move_count = 0;
    let mut last_col = 0;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        if let Message::Mouse(m) = &event {
            if matches!(m.kind, MouseEventKind::Move) {
                move_count += 1;
                last_col = m.col;
            }
        }
        if matches!(event, Message::CloseRequested) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert_eq!(move_count, 1, "should coalesce to 1 mouse move");
    assert_eq!(last_col, 30, "should keep the last position");
}

#[test]
fn coalesce_preserves_press_release() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, CompToClient::Mouse {
        pane: pane_id(1),
        event: MouseEvent {
            col: 5, row: 5,
            kind: MouseEventKind::Press(MouseButton::Left),
            modifiers: Modifiers::empty(),
            timestamp: None,
        },
    });
    send_comp(&tx, mouse_move_msg(10, 10));
    send_comp(&tx, mouse_move_msg(20, 20));
    send_comp(&tx, CompToClient::Mouse {
        pane: pane_id(1),
        event: MouseEvent {
            col: 20, row: 20,
            kind: MouseEventKind::Release(MouseButton::Left),
            modifiers: Modifiers::empty(),
            timestamp: None,
        },
    });
    send_comp(&tx, close_msg());
    drop(tx);

    let mut event_types = Vec::new();
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        match &event {
            Message::Mouse(m) => event_types.push(format!("{:?}", m.kind)),
            Message::CloseRequested => { event_types.push("Close".into()); return Ok(false); }
            _ => {}
        }
        Ok(true)
    }).unwrap();

    // Press, Move(coalesced), Release, Close
    assert_eq!(event_types.len(), 4);
    assert!(event_types[0].contains("Press"));
    assert!(event_types[1].contains("Move"));
    assert!(event_types[2].contains("Release"));
    assert_eq!(event_types[3], "Close");
}

#[test]
fn coalesce_no_coalesce_focus_blur() {
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    send_comp(&tx, CompToClient::Blur { pane: pane_id(1) });
    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    send_comp(&tx, close_msg());
    drop(tx);

    let mut count = 0;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        if !matches!(event, Message::CloseRequested | Message::Disconnected) {
            count += 1;
        }
        if matches!(event, Message::CloseRequested) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert_eq!(count, 3, "Focus, Blur, Focus should all be delivered");
}

#[test]
fn single_message_no_coalesce() {
    // Regression guard: a single message should pass through unmodified
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let proxy = make_proxy(pane_id(1));

    send_comp(&tx, CompToClient::Focus { pane: pane_id(1) });
    send_comp(&tx, close_msg());
    drop(tx);

    let mut got_focus = false;
    pane_app::looper::run_closure(pane_id(1), rx, filters, proxy, |_h, event| {
        if matches!(event, Message::Activated) { got_focus = true; }
        if matches!(event, Message::CloseRequested) { return Ok(false); }
        Ok(true)
    }).unwrap();

    assert!(got_focus, "single message should pass through");
}
