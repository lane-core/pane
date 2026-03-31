//! Handler trait default method tests.
//! Verifies that the default implementations return the correct values
//! without any overrides — the contract that developers depend on.

use std::any::Any;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use calloop::channel::{self, Sender};
use pane_app::{Message, Messenger, Handler, LooperMessage, ReplyPort, PaneError};
use pane_app::error::Result;
use pane_proto::event::{KeyEvent, Key, Modifiers, KeyState};
use pane_proto::message::PaneId;
use pane_proto::protocol::{ClientToComp, CompToClient};

fn pane_id(n: u32) -> PaneId {
    PaneId::from_uuid(uuid::Uuid::from_u128(n as u128))
}

fn make_handle(id: PaneId) -> (Messenger, mpsc::Receiver<ClientToComp>) {
    let (tx, rx) = mpsc::channel();
    (Messenger::new(id, tx), rx)
}

fn send_comp(tx: &Sender<LooperMessage>, msg: CompToClient) {
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
    let (tx, rx) = channel::channel::<LooperMessage>();
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
    let (tx, rx) = channel::channel::<LooperMessage>();
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
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (handle, _comp_rx) = make_handle(pane_id(1));

    drop(tx); // explicitly drop sender to simulate disconnect
    pane_app::looper::run_handler(pane_id(1), rx, filters, handle, DefaultHandler).unwrap();
    // Loop exited on disconnect — default worked
}

#[test]
fn handler_override_close_to_continue() {
    // NeverCloseHandler overrides close to return Ok(true) — loop continues
    let (tx, rx) = channel::channel::<LooperMessage>();
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
    fn app_message(&mut self, _proxy: &Messenger, msg: Box<dyn Any + Send>) -> Result<bool> {
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
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (comp_tx, _comp_rx) = mpsc::channel::<ClientToComp>();
    let (looper_tx, _looper_rx) = channel::channel::<LooperMessage>();
    let handle = Messenger::new(pane_id(1), comp_tx).with_looper(looper_tx);

    let received = Arc::new(Mutex::new(None));
    let handler = AppMessageHandler { received: received.clone() };

    // Post an app message, then close
    tx.send(LooperMessage::Posted(Message::AppMessage(
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
    let (tx, rx) = channel::channel::<LooperMessage>();
    let filters = pane_app::filter::FilterChain::new();
    let (comp_tx, _comp_rx) = mpsc::channel::<ClientToComp>();
    let (looper_tx, _looper_rx) = channel::channel::<LooperMessage>();
    let handle = Messenger::new(pane_id(1), comp_tx).with_looper(looper_tx);

    let received = Arc::new(Mutex::new(None));
    let handler = AppMessageHandler { received: received.clone() };

    // Post a String (handler expects DownloadComplete), then close
    tx.send(LooperMessage::Posted(Message::AppMessage(
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
    let (looper_tx, looper_rx) = channel::channel::<LooperMessage>();
    let proxy = Messenger::new(pane_id(1), comp_tx).with_looper(looper_tx);

    proxy.post_app_message(DownloadComplete {
        path: "/tmp/test".into(),
        bytes: 100,
    }).unwrap();

    let msg = recv_one(looper_rx, Duration::from_secs(1))
        .expect("should receive app message");
    match msg {
        LooperMessage::Posted(Message::AppMessage(payload)) => {
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
    fn request_received(&mut self, _proxy: &Messenger, msg: Message, reply_port: ReplyPort) -> Result<bool> {
        if let Message::AppMessage(payload) = msg {
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

/// Receive one message from a calloop channel (test spy helper).
fn recv_one(
    ch: channel::Channel<LooperMessage>,
    timeout: Duration,
) -> Option<LooperMessage> {
    let mut event_loop: calloop::EventLoop<'_, Option<LooperMessage>> =
        calloop::EventLoop::try_new().unwrap();
    event_loop.handle().insert_source(ch, |event, _, result| {
        if let calloop::channel::Event::Msg(msg) = event {
            if result.is_none() {
                *result = Some(msg);
            }
        }
    }).unwrap();
    let mut result = None;
    event_loop.dispatch(Some(timeout), &mut result).unwrap();
    result
}

/// Helper: create a proxy with looper channel, return (proxy, channel).
/// The proxy can be cloned and shared across threads.
fn make_looper_proxy(id: PaneId) -> (Messenger, channel::Channel<LooperMessage>) {
    let (comp_tx, _) = mpsc::channel::<ClientToComp>();
    let (looper_tx, looper_rx) = channel::channel::<LooperMessage>();
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
        Message::AppMessage(Box::new(21u64)),
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
        fn request_received(&mut self, _proxy: &Messenger, _msg: Message, _reply_port: ReplyPort) -> Result<bool> {
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
        Message::AppMessage(Box::new(42u64)),
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

    let token = caller_proxy.send_request(&target_for_send, Message::AppMessage(Box::new(21u64))).unwrap();

    // The reply should arrive on the caller's looper channel
    let reply_msg = recv_one(caller_rx, Duration::from_secs(2))
        .expect("should receive Reply on caller's looper channel");
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

// --- Reply adversarial tests ---

#[test]
fn send_and_wait_timeout() {
    /// Handler that sleeps forever on any request (never replies).
    struct BlackHoleHandler;
    impl Handler for BlackHoleHandler {
        fn request_received(&mut self, _proxy: &Messenger, _msg: Message, reply_port: ReplyPort) -> Result<bool> {
            // Hold the reply port but never reply. It will be dropped
            // when the handler struct is dropped (looper exit), but the
            // caller should time out before that.
            std::mem::forget(reply_port);
            // Block this looper so it doesn't process the Close
            std::thread::sleep(Duration::from_secs(5));
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
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, BlackHoleHandler)
    });

    let (caller_proxy, _) = make_looper_proxy(pane_id(1));

    let start = std::time::Instant::now();
    let result = caller_proxy.send_and_wait(
        &target_for_send,
        Message::AppMessage(Box::new(42u64)),
        Duration::from_millis(100),
    );
    let elapsed = start.elapsed();

    assert!(result.is_err(), "should error when target never replies");
    match result.unwrap_err() {
        PaneError::Timeout => {} // expected
        other => panic!("expected Timeout, got {:?}", other),
    }
    assert!(
        elapsed >= Duration::from_millis(80),
        "timeout should wait at least ~100ms, but returned in {:?}", elapsed,
    );
    assert!(
        elapsed < Duration::from_millis(500),
        "timeout should not wait much longer than 100ms, but took {:?}", elapsed,
    );

    // Clean up: dropping the sender will disconnect the target looper.
    // The target thread is blocked in sleep(5s); we detach it.
    drop(target_for_send);
    drop(caller_proxy);
    drop(target_handle);
}

#[test]
fn target_exit_mid_request() {
    /// Handler that closes immediately on receiving a request, dropping the ReplyPort.
    struct ExitOnRequestHandler;
    impl Handler for ExitOnRequestHandler {
        fn request_received(&mut self, _proxy: &Messenger, _msg: Message, _reply_port: ReplyPort) -> Result<bool> {
            // Return false to exit the looper. reply_port is dropped -> sends ReplyFailed.
            Ok(false)
        }
        fn close_requested(&mut self, _proxy: &Messenger) -> Result<bool> {
            Ok(false)
        }
    }

    let (target_proxy, target_rx) = make_looper_proxy(pane_id(2));
    let target_for_send = target_proxy.clone();

    let target_handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, ExitOnRequestHandler)
    });

    let (caller_proxy, _) = make_looper_proxy(pane_id(1));

    let result = caller_proxy.send_and_wait(
        &target_for_send,
        Message::AppMessage(Box::new(99u64)),
        Duration::from_secs(2),
    );

    assert!(result.is_err(), "should get error when target exits mid-request");
    // The reply_port drop sends ReplyFailed, which maps to PaneError::Refused
    match result.unwrap_err() {
        PaneError::Refused => {} // expected: ReplyPort dropped without reply
        other => panic!("expected Refused (from dropped ReplyPort), got {:?}", other),
    }

    target_handle.join().unwrap().unwrap();
}

#[test]
fn multiple_concurrent_requests() {
    let (target_proxy, target_rx) = make_looper_proxy(pane_id(2));
    let target_for_send = target_proxy.clone();

    let target_handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, EchoHandler)
    });

    let num_threads = 10u64;
    let mut handles = Vec::new();

    for i in 0..num_threads {
        let target = target_for_send.clone();
        let (caller, _rx) = make_looper_proxy(pane_id(10 + i as u32));
        handles.push(std::thread::spawn(move || {
            let result = caller.send_and_wait(
                &target,
                Message::AppMessage(Box::new(i)),
                Duration::from_secs(5),
            );
            let payload = result.unwrap_or_else(|e| panic!("thread {i} should get a reply, got {e:?}"));
            let value = *payload.downcast_ref::<u64>().expect("reply should be u64");
            assert_eq!(value, i * 2, "thread {i} should get {}, got {value}", i * 2);
        }));
    }

    for (i, h) in handles.into_iter().enumerate() {
        h.join().unwrap_or_else(|_| panic!("thread {i} panicked"));
    }

    target_for_send.send_message(Message::CloseRequested).unwrap();
    target_handle.join().unwrap().unwrap();
}

#[test]
fn reply_after_requestor_gone() {
    // The caller sends a blocking request with a short timeout and gives up.
    // The target holds the ReplyPort and replies later (on pulse).
    // The reply_fn's send should fail silently — the target must not panic.

    /// Handler that stores the ReplyPort and replies on the next pulse.
    struct DelayedReplyHandler {
        saved_port: Option<ReplyPort>,
    }
    impl Handler for DelayedReplyHandler {
        fn request_received(&mut self, _proxy: &Messenger, _msg: Message, reply_port: ReplyPort) -> Result<bool> {
            self.saved_port = Some(reply_port);
            Ok(true)
        }
        fn pulse(&mut self, _proxy: &Messenger) -> Result<bool> {
            if let Some(port) = self.saved_port.take() {
                port.reply(42u64);
            }
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
        let handler = DelayedReplyHandler { saved_port: None };
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, handler)
    });

    // Caller sends a blocking request with a very short timeout.
    // The target won't reply within the timeout (it waits for pulse).
    let (caller_proxy, _) = make_looper_proxy(pane_id(1));
    let result = caller_proxy.send_and_wait(
        &target_for_send,
        Message::AppMessage(Box::new(1u64)),
        Duration::from_millis(10), // very short timeout — will expire
    );
    // Caller times out — the reply channel (recv side) is now dropped
    assert!(result.is_err(), "caller should time out");
    drop(caller_proxy); // ensure all caller state is gone

    // Now trigger pulses so the target tries to reply to the dead channel
    target_for_send.set_pulse_rate(Duration::from_millis(10)).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    // If the target panicked on the dead channel, join will fail
    target_for_send.send_message(Message::CloseRequested).unwrap();
    let result = target_handle.join();
    assert!(result.is_ok(), "target should not panic when replying to gone requestor");
    assert!(result.unwrap().is_ok(), "target looper should exit cleanly");
}

#[test]
fn send_request_async_reply_failed() {
    /// Handler that drops the ReplyPort without replying.
    struct DropReplyHandler;
    impl Handler for DropReplyHandler {
        fn request_received(&mut self, _proxy: &Messenger, _msg: Message, _reply_port: ReplyPort) -> Result<bool> {
            // reply_port dropped -> sends ReplyFailed to caller's looper
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
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, DropReplyHandler)
    });

    let (caller_proxy, caller_rx) = make_looper_proxy(pane_id(1));

    let token = caller_proxy.send_request(
        &target_for_send,
        Message::AppMessage(Box::new(42u64)),
    ).unwrap();

    // The ReplyFailed should arrive on the caller's looper channel
    let reply_msg = recv_one(caller_rx, Duration::from_secs(2))
        .expect("should receive ReplyFailed on caller's looper channel");

    match reply_msg {
        LooperMessage::Posted(Message::ReplyFailed { token: t }) => {
            assert_eq!(t, token, "ReplyFailed token should match request token");
        }
        other => panic!("expected Posted(ReplyFailed), got {:?}", other),
    }

    target_for_send.send_message(Message::CloseRequested).unwrap();
    target_handle.join().unwrap().unwrap();
}

#[test]
fn nested_request_no_deadlock() {
    // Target A handles requests by forwarding an async request to B,
    // then replying once B's reply arrives. This verifies that async
    // requests don't block the looper.

    /// Handler for target B: echoes back the value tripled.
    struct TripleHandler;
    impl Handler for TripleHandler {
        fn request_received(&mut self, _proxy: &Messenger, msg: Message, reply_port: ReplyPort) -> Result<bool> {
            if let Message::AppMessage(payload) = msg {
                if let Some(&n) = payload.downcast_ref::<u64>() {
                    reply_port.reply(n * 3);
                    return Ok(true);
                }
            }
            Ok(true)
        }
        fn close_requested(&mut self, _proxy: &Messenger) -> Result<bool> {
            Ok(false)
        }
    }

    /// Handler for target A: on request, forwards to B asynchronously,
    /// then replies to the original when B's reply arrives.
    struct ForwardingHandler {
        b_proxy: Messenger,
        /// Pending: maps B's reply token to the original ReplyPort
        pending: std::collections::HashMap<u64, ReplyPort>,
    }
    impl Handler for ForwardingHandler {
        fn request_received(&mut self, proxy: &Messenger, msg: Message, reply_port: ReplyPort) -> Result<bool> {
            if let Message::AppMessage(ref payload) = msg {
                if let Some(&n) = payload.downcast_ref::<u64>() {
                    let b_token = proxy.send_request(&self.b_proxy, Message::AppMessage(Box::new(n))).unwrap();
                    self.pending.insert(b_token, reply_port);
                    return Ok(true);
                }
            }
            Ok(true)
        }
        fn reply_received(&mut self, _proxy: &Messenger, token: u64, payload: Box<dyn Any + Send>) -> Result<bool> {
            if let Some(port) = self.pending.remove(&token) {
                if let Some(&val) = payload.downcast_ref::<u64>() {
                    port.reply(val);
                }
            }
            Ok(true)
        }
        fn close_requested(&mut self, _proxy: &Messenger) -> Result<bool> {
            Ok(false)
        }
    }

    // Set up B
    let (b_proxy, b_rx) = make_looper_proxy(pane_id(3));
    let b_for_a = b_proxy.clone();
    let b_for_close = b_proxy.clone();
    let b_handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_handler(pane_id(3), b_rx, filters, b_proxy, TripleHandler)
    });

    // Set up A (with a reference to B's proxy for forwarding)
    let (a_proxy, a_rx) = make_looper_proxy(pane_id(2));
    let a_for_send = a_proxy.clone();
    let a_for_close = a_proxy.clone();
    let a_handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        let handler = ForwardingHandler {
            b_proxy: b_for_a,
            pending: std::collections::HashMap::new(),
        };
        pane_app::looper::run_handler(pane_id(2), a_rx, filters, a_proxy, handler)
    });

    // Caller sends a blocking request to A with a generous timeout
    let (caller_proxy, _) = make_looper_proxy(pane_id(1));
    let result = caller_proxy.send_and_wait(
        &a_for_send,
        Message::AppMessage(Box::new(7u64)),
        Duration::from_secs(5),
    );

    let payload = result.expect("nested request chain should complete without deadlock");
    let value = *payload.downcast_ref::<u64>().expect("reply should be u64");
    assert_eq!(value, 21, "7 * 3 = 21, got {value}");

    // Clean up
    a_for_close.send_message(Message::CloseRequested).unwrap();
    b_for_close.send_message(Message::CloseRequested).unwrap();
    a_handle.join().unwrap().unwrap();
    b_handle.join().unwrap().unwrap();
}

// --- Deadlock guard test ---

#[test]
fn send_and_wait_from_looper_returns_would_deadlock() {
    // If send_and_wait is called from a looper thread, it should
    // return WouldDeadlock instead of deadlocking.
    let (target_proxy, target_rx) = make_looper_proxy(pane_id(2));
    let target_for_send = target_proxy.clone();

    let target_handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_handler(pane_id(2), target_rx, filters, target_proxy, EchoHandler)
    });

    let (proxy, looper_rx) = make_looper_proxy(pane_id(1));
    let outer_proxy = proxy.clone();

    let result_holder = Arc::new(Mutex::new(None::<std::result::Result<Box<dyn Any + Send>, PaneError>>));
    let rh = result_holder.clone();
    let target_clone = target_for_send.clone();

    let handle = std::thread::spawn(move || {
        let filters = pane_app::filter::FilterChain::new();
        pane_app::looper::run_closure(pane_id(1), looper_rx, filters, proxy, move |_m, msg| {
            if matches!(msg, Message::Activated) {
                // Try send_and_wait from INSIDE the looper — should be caught
                let result = _m.send_and_wait(
                    &target_clone,
                    Message::AppMessage(Box::new(42u64)),
                    Duration::from_millis(100),
                );
                *rh.lock().unwrap() = Some(result);
                return Ok(false); // exit after testing
            }
            if matches!(msg, Message::CloseRequested) { return Ok(false); }
            Ok(true)
        })
    });

    // Trigger the test by posting Activated
    outer_proxy.send_message(Message::Activated).unwrap();

    handle.join().unwrap().unwrap();

    let result = result_holder.lock().unwrap().take()
        .expect("handler should have attempted send_and_wait");
    match result {
        Err(PaneError::WouldDeadlock) => {} // correct
        Err(other) => panic!("expected WouldDeadlock, got {:?}", other),
        Ok(_) => panic!("send_and_wait from looper should not succeed"),
    }

    // Clean up target
    target_for_send.send_message(Message::CloseRequested).unwrap();
    target_handle.join().unwrap().unwrap();
}
