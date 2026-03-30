//! Handler trait default method tests.
//! Verifies that the default implementations return the correct values
//! without any overrides — the contract that developers depend on.

use std::any::Any;
use std::num::NonZeroU32;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pane_app::{Message, Messenger, Handler, LooperMessage, ReplyPort};
use pane_app::error::Result;
use pane_proto::event::{KeyEvent, Key, Modifiers, KeyState};
use pane_proto::message::PaneId;
use pane_proto::protocol::{ClientToComp, CompToClient};

fn pane_id(n: u32) -> PaneId {
    PaneId::new(NonZeroU32::new(n).unwrap())
}

fn make_handle(id: PaneId) -> (Messenger, mpsc::Receiver<ClientToComp>) {
    let (tx, rx) = mpsc::channel();
    (Messenger::new(id, tx), rx)
}

fn send_comp(tx: &mpsc::Sender<LooperMessage>, msg: CompToClient) {
    tx.send(LooperMessage::FromComp(msg)).unwrap();
}

/// A Handler with NO overrides — uses all defaults.
struct DefaultHandler;
impl Handler for DefaultHandler {}

/// A Handler that overrides close_requested to continue (Ok(true)).
struct NeverCloseHandler;
impl Handler for NeverCloseHandler {
    fn close_requested(&mut self, _handle: &Messenger) -> Result<bool> {
        Ok(true) // refuse to close
    }
}

// --- P1-4: Handler default methods ---

#[test]
fn handler_default_close_returns_false() {
    // Default close_requested returns Ok(false) — the loop should exit
    let (tx, rx) = mpsc::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (handle, _comp_rx) = make_handle(pane_id(1));

    send_comp(&tx, CompToClient::Close { pane: pane_id(1) });
    drop(tx);

    pane_app::looper::run_handler(pane_id(1), rx, filters, handle, DefaultHandler).unwrap();
    // If we get here, the loop exited on Close — default worked
}

#[test]
fn handler_default_key_returns_true() {
    // Default key returns Ok(true) — the loop should continue
    let (tx, rx) = mpsc::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (handle, _comp_rx) = make_handle(pane_id(1));

    // Send a key, then a close to terminate
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

    pane_app::looper::run_handler(pane_id(1), rx, filters, handle, DefaultHandler).unwrap();
    // If we get here, key didn't stop the loop — default worked
}

#[test]
fn handler_default_disconnect_returns_false() {
    // Default disconnected returns Ok(false)
    let (tx, rx) = mpsc::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (handle, _comp_rx) = make_handle(pane_id(1));

    drop(tx); // explicitly drop sender to simulate disconnect
    pane_app::looper::run_handler(pane_id(1), rx, filters, handle, DefaultHandler).unwrap();
    // Loop exited on disconnect — default worked
}

#[test]
fn handler_override_close_to_continue() {
    // NeverCloseHandler overrides close to return Ok(true) — loop continues
    let (tx, rx) = mpsc::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (handle, _comp_rx) = make_handle(pane_id(1));

    // Send close (handler will ignore it), then drop to disconnect
    send_comp(&tx, CompToClient::Close { pane: pane_id(1) });
    drop(tx);

    pane_app::looper::run_handler(pane_id(1), rx, filters, handle, NeverCloseHandler).unwrap();
    // Loop exited on disconnect (after ignoring close) — override worked
}

// --- App message tests ---

#[derive(Debug)]
struct DownloadComplete {
    path: String,
    bytes: u64,
}

struct AppMessageHandler {
    received: Arc<Mutex<Option<(String, u64)>>>,
}

impl Handler for AppMessageHandler {
    fn message_received(&mut self, _proxy: &Messenger, msg: Box<dyn Any + Send>) -> Result<bool> {
        if let Some(dl) = msg.downcast_ref::<DownloadComplete>() {
            *self.received.lock().unwrap() = Some((dl.path.clone(), dl.bytes));
        }
        Ok(true)
    }

    fn close_requested(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(false)
    }
}

#[test]
fn app_message_delivered_to_handler() {
    let (tx, rx) = mpsc::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (comp_tx, _comp_rx) = mpsc::channel::<ClientToComp>();
    let (looper_tx, _) = mpsc::sync_channel::<LooperMessage>(256);
    let handle = Messenger::new(pane_id(1), comp_tx).with_looper(looper_tx);

    let received = Arc::new(Mutex::new(None));
    let handler = AppMessageHandler { received: received.clone() };

    // Post an app message, then close
    tx.send(LooperMessage::Posted(Message::App(
        Box::new(DownloadComplete { path: "/tmp/file.zip".into(), bytes: 42000 }),
    ))).unwrap();
    send_comp(&tx, CompToClient::Close { pane: pane_id(1) });
    drop(tx);

    pane_app::looper::run_handler(pane_id(1), rx, filters, handle, handler).unwrap();

    let result = received.lock().unwrap();
    assert_eq!(result.as_ref().unwrap().0, "/tmp/file.zip");
    assert_eq!(result.as_ref().unwrap().1, 42000);
}

#[test]
fn app_message_wrong_type_continues() {
    let (tx, rx) = mpsc::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (comp_tx, _comp_rx) = mpsc::channel::<ClientToComp>();
    let (looper_tx, _) = mpsc::sync_channel::<LooperMessage>(256);
    let handle = Messenger::new(pane_id(1), comp_tx).with_looper(looper_tx);

    let received = Arc::new(Mutex::new(None));
    let handler = AppMessageHandler { received: received.clone() };

    // Post a String (handler expects DownloadComplete), then close
    tx.send(LooperMessage::Posted(Message::App(
        Box::new("not a DownloadComplete".to_string()),
    ))).unwrap();
    send_comp(&tx, CompToClient::Close { pane: pane_id(1) });
    drop(tx);

    pane_app::looper::run_handler(pane_id(1), rx, filters, handle, handler).unwrap();

    // Handler continued past the wrong type (default Ok(true) after failed downcast)
    assert!(received.lock().unwrap().is_none());
}

#[test]
fn post_app_message_round_trip() {
    let (comp_tx, _comp_rx) = mpsc::channel::<ClientToComp>();
    let (looper_tx, looper_rx) = mpsc::sync_channel::<LooperMessage>(256);
    let proxy = Messenger::new(pane_id(1), comp_tx).with_looper(looper_tx);

    proxy.post_app_message(DownloadComplete {
        path: "/tmp/test".into(),
        bytes: 100,
    }).unwrap();

    let msg = looper_rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap();
    match msg {
        LooperMessage::Posted(Message::App(payload)) => {
            let dl = payload.downcast_ref::<DownloadComplete>().unwrap();
            assert_eq!(dl.path, "/tmp/test");
            assert_eq!(dl.bytes, 100);
        }
        other => panic!("expected App message, got {:?}", other),
    }
}

// --- Reply mechanism tests ---

/// Handler that replies to requests with the request data doubled.
struct EchoHandler;

impl Handler for EchoHandler {
    fn handle_request(&mut self, _proxy: &Messenger, msg: Message, reply_port: ReplyPort) -> Result<bool> {
        if let Message::App(payload) = msg {
            if let Some(n) = payload.downcast_ref::<u64>() {
                reply_port.reply(n * 2);
                return Ok(true);
            }
        }
        // Unknown request — drop reply_port (sends ReplyFailed)
        Ok(true)
    }

    fn close_requested(&mut self, _proxy: &Messenger) -> Result<bool> {
        Ok(false)
    }
}

/// Helper: create a proxy with looper channel, return (proxy, receiver).
/// The proxy can be cloned and shared across threads.
fn make_looper_proxy(id: PaneId) -> (Messenger, mpsc::Receiver<LooperMessage>) {
    let (comp_tx, _) = mpsc::channel::<ClientToComp>();
    let (looper_tx, looper_rx) = mpsc::sync_channel::<LooperMessage>(256);
    (Messenger::new(id, comp_tx).with_looper(looper_tx), looper_rx)
}

#[test]
fn send_and_wait_round_trip() {
    let (target_proxy, target_rx) = make_looper_proxy(pane_id(2));
    let target_for_send = target_proxy.clone();

    let target_handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, EchoHandler)
    });

    let (caller_proxy, _caller_rx) = make_looper_proxy(pane_id(1));

    let reply = caller_proxy.send_and_wait(
        &target_for_send,
        Message::App(Box::new(21u64)),
        Duration::from_secs(2),
    ).unwrap();

    let value = reply.downcast_ref::<u64>().unwrap();
    assert_eq!(*value, 42, "echo handler should double the value");

    target_for_send.send_message(Message::CloseRequested).unwrap();
    target_handle.join().unwrap().unwrap();
}

#[test]
fn reply_port_drop_sends_reply_failed() {
    struct IgnoreHandler;
    impl Handler for IgnoreHandler {
        fn handle_request(&mut self, _proxy: &Messenger, _msg: Message, _reply_port: ReplyPort) -> Result<bool> {
            // reply_port dropped here → sends ReplyFailed
            Ok(true)
        }
        fn close_requested(&mut self, _proxy: &Messenger) -> Result<bool> {
            Ok(false)
        }
    }

    let (target_proxy, target_rx) = make_looper_proxy(pane_id(2));
    let target_for_send = target_proxy.clone();

    let target_handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, IgnoreHandler)
    });

    let (caller_proxy, _) = make_looper_proxy(pane_id(1));

    let result = caller_proxy.send_and_wait(
        &target_for_send,
        Message::App(Box::new(42u64)),
        Duration::from_secs(2),
    );

    assert!(result.is_err(), "should get ReplyFailed when reply port is dropped");

    target_for_send.send_message(Message::CloseRequested).unwrap();
    target_handle.join().unwrap().unwrap();
}

#[test]
fn send_request_async_round_trip() {
    let (target_proxy, target_rx) = make_looper_proxy(pane_id(2));
    let target_for_send = target_proxy.clone();

    let target_handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, EchoHandler)
    });

    let (caller_proxy, caller_rx) = make_looper_proxy(pane_id(1));

    let token = caller_proxy.send_request(&target_for_send, Message::App(Box::new(21u64))).unwrap();

    // The reply should arrive on the caller's looper channel
    let reply_msg = caller_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    match reply_msg {
        LooperMessage::Posted(Message::Reply { token: t, payload }) => {
            assert_eq!(t, token);
            let value = payload.downcast_ref::<u64>().unwrap();
            assert_eq!(*value, 42);
        }
        other => panic!("expected Reply, got {:?}", other),
    }

    target_for_send.send_message(Message::CloseRequested).unwrap();
    target_handle.join().unwrap().unwrap();
}
