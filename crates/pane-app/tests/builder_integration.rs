//! Integration test: PaneBuilder connects to ProtocolServer,
//! opens a service via DeclareInterest, gets a real ServiceHandle.

use std::sync::Arc;

use pane_app::pane::{Pane, Tag};
use pane_proto::control::ControlMessage;
use pane_proto::protocol::ServiceId;
use pane_proto::service_frame::ServiceFrame;
use pane_proto::{Flow, Handler, Handles, Protocol};
use pane_session::bridge::HANDSHAKE_MAX_MESSAGE_SIZE;
use pane_session::frame::{Frame, FrameCodec};
use pane_session::handshake::{Hello, Rejection, ServiceProvision, Welcome};
use pane_session::server::ProtocolServer;
use pane_session::transport::MemoryTransport;

use serde::{Deserialize, Serialize};

fn echo_service_id() -> ServiceId {
    ServiceId::new("com.pane.echo")
}

struct EchoService;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum EchoMessage {
    Ping(String),
    Pong(String),
}

impl Protocol for EchoService {
    fn service_id() -> ServiceId {
        echo_service_id()
    }
    type Message = EchoMessage;
}

struct ConsumerHandler;
impl Handler for ConsumerHandler {}
impl Handles<EchoService> for ConsumerHandler {
    fn receive(&mut self, _msg: EchoMessage) -> Flow {
        Flow::Continue
    }
}

/// Manual client for the provider side (reused from two_pane_echo).
struct ClientConn {
    transport: MemoryTransport,
    codec: FrameCodec,
}

impl ClientConn {
    fn connect(mut transport: MemoryTransport, hello: Hello) -> (Self, Welcome) {
        let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);
        let bytes = postcard::to_allocvec(&hello).unwrap();
        codec.write_frame(&mut transport, 0, &bytes).unwrap();
        let frame = codec.read_frame(&mut transport).unwrap();
        let payload = match frame {
            Frame::Message {
                service: 0,
                payload,
            } => payload,
            other => panic!("unexpected frame: {other:?}"),
        };
        let decision: Result<Welcome, Rejection> = postcard::from_bytes(&payload).unwrap();
        let welcome = decision.expect("rejected");
        let mut codec = codec;
        codec.set_max_message_size(welcome.max_message_size);
        (ClientConn { transport, codec }, welcome)
    }

    fn read_control(&mut self) -> ControlMessage {
        let frame = self.codec.read_frame(&mut self.transport).unwrap();
        match frame {
            Frame::Message {
                service: 0,
                payload,
            } => postcard::from_bytes(&payload).unwrap(),
            other => panic!("expected Control, got {other:?}"),
        }
    }

    fn expect_ready(&mut self) {
        match self.read_control() {
            ControlMessage::Lifecycle(
                pane_proto::protocols::lifecycle::LifecycleMessage::Ready,
            ) => {}
            other => panic!("expected Ready, got {other:?}"),
        }
    }

    fn register_session(&mut self, session_id: u16) {
        self.codec.register_service(session_id);
    }

    fn read_service(&mut self) -> (u16, ServiceFrame) {
        let frame = self.codec.read_frame(&mut self.transport).unwrap();
        match frame {
            Frame::Message { service, payload } if service != 0 => {
                let sf: ServiceFrame = postcard::from_bytes(&payload).unwrap();
                (service, sf)
            }
            other => panic!("expected service frame, got {other:?}"),
        }
    }
}

fn accept_on_thread(
    server: &Arc<ProtocolServer>,
    transport: MemoryTransport,
) -> std::thread::JoinHandle<pane_session::server::ConnectionHandle> {
    let server = Arc::clone(server);
    std::thread::spawn(move || {
        let (r, w) = transport.split();
        server.accept(r, w).expect("accept failed")
    })
}

#[test]
fn open_service_via_protocol_server() {
    let server = Arc::new(ProtocolServer::new());

    // Provider: manual ClientConn
    let (b_client, b_server) = MemoryTransport::pair();
    let accept_b = accept_on_thread(&server, b_server);
    let (mut provider, _) = ClientConn::connect(
        b_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![ServiceProvision {
                service: echo_service_id(),
                version: 1,
            }],
        },
    );
    let conn_b = accept_b.join().unwrap();
    provider.expect_ready();

    // Consumer: PaneBuilder (Ready is buffered by open_service)
    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);

    let pane = Pane::new(Tag::new("consumer"));
    let mut builder = pane.setup::<ConsumerHandler>();
    builder.connect(a_client).expect("connect failed");
    let conn_a = accept_a.join().unwrap();

    // open_service sends DeclareInterest, blocks for InterestAccepted.
    // Ready (from the server) arrives first and is buffered.
    let handle = builder.open_service::<EchoService>();
    assert!(handle.is_some(), "open_service should succeed");
    let handle = handle.unwrap();
    assert_ne!(handle.session_id(), 0, "session_id should be nonzero");

    // Provider should have received InterestAccepted
    let b_session = match provider.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for provider, got {other:?}"),
    };
    provider.register_session(b_session);

    // Send a notification through the wired handle
    handle.send_notification(EchoMessage::Ping("wired".into()));

    // Provider receives it — inner payload has tag byte (S8).
    let (recv_session, recv_frame) = provider.read_service();
    assert_eq!(recv_session, b_session);
    match recv_frame {
        ServiceFrame::Notification { payload } => {
            let expected_tag = echo_service_id().tag();
            assert_eq!(payload[0], expected_tag, "protocol tag must match");
            let msg: EchoMessage = postcard::from_bytes(&payload[1..]).unwrap();
            assert_eq!(msg, EchoMessage::Ping("wired".into()));
        }
        other => panic!("expected Notification, got {other:?}"),
    }

    drop(handle);
    drop(builder);
    drop(provider);
    conn_a.wait();
    conn_b.wait();
}

#[test]
fn open_service_declined_returns_none() {
    let server = Arc::new(ProtocolServer::new());

    // No provider registered — DeclareInterest will be declined.
    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);

    let pane = Pane::new(Tag::new("consumer"));
    let mut builder = pane.setup::<ConsumerHandler>();
    builder.connect(a_client).expect("connect failed");
    let conn_a = accept_a.join().unwrap();

    let handle = builder.open_service::<EchoService>();
    assert!(
        handle.is_none(),
        "open_service should return None when declined"
    );

    drop(builder);
    conn_a.wait();
}

/// N1: RevokeInterest end-to-end — ServiceHandle Drop sends
/// RevokeInterest through the writer thread to the wire, server
/// cleans route, provider receives ServiceTeardown.
#[test]
fn revoke_interest_end_to_end() {
    let server = Arc::new(ProtocolServer::new());

    // Provider
    let (b_client, b_server) = MemoryTransport::pair();
    let accept_b = accept_on_thread(&server, b_server);
    let (mut provider, _) = ClientConn::connect(
        b_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![ServiceProvision {
                service: echo_service_id(),
                version: 1,
            }],
        },
    );
    let conn_b = accept_b.join().unwrap();
    provider.expect_ready();

    // Consumer via PaneBuilder
    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);
    let pane = Pane::new(Tag::new("consumer"));
    let mut builder = pane.setup::<ConsumerHandler>();
    builder.connect(a_client).expect("connect failed");
    let conn_a = accept_a.join().unwrap();

    let handle = builder.open_service::<EchoService>().expect("open failed");

    // Provider reads InterestAccepted
    let _b_session = match provider.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted, got {other:?}"),
    };

    // Drop the ServiceHandle — should send RevokeInterest
    drop(handle);

    // Give the writer thread time to flush
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Provider should receive ServiceTeardown (from server
    // processing the RevokeInterest)
    let msg = provider.read_control();
    match msg {
        ControlMessage::ServiceTeardown { reason, .. } => {
            assert!(matches!(
                reason,
                pane_proto::control::TeardownReason::ServiceRevoked
            ));
        }
        other => panic!("expected ServiceTeardown after RevokeInterest, got {other:?}"),
    }

    drop(builder);
    drop(provider);
    conn_a.wait();
    conn_b.wait();
}

/// N2: Ready is buffered during open_service, drained by run_with.
/// ProtocolServer sends Ready after Welcome. open_service blocks
/// for InterestAccepted, buffering Ready. run_with drains the buffer,
/// dispatching Ready → handler.ready() fires.
#[test]
fn ready_buffered_during_open_service_delivered_by_run_with() {
    use std::sync::Mutex;

    let server = Arc::new(ProtocolServer::new());

    // Provider
    let (b_client, b_server) = MemoryTransport::pair();
    let accept_b = accept_on_thread(&server, b_server);
    let (mut provider, _) = ClientConn::connect(
        b_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![ServiceProvision {
                service: echo_service_id(),
                version: 1,
            }],
        },
    );
    let conn_b = accept_b.join().unwrap();
    provider.expect_ready();

    // Consumer
    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);
    let pane = Pane::new(Tag::new("consumer"));
    let mut builder = pane.setup::<ReadyTracker>();
    builder.connect(a_client).expect("connect failed");
    let _conn_a = accept_a.join().unwrap();

    // open_service blocks for InterestAccepted. Ready arrives first
    // and is buffered by open_service's loop.
    let handle = builder.open_service::<EchoService>().expect("open failed");

    // Provider reads InterestAccepted (Ready already drained above)
    let _b_session = match provider.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted, got {other:?}"),
    };

    let log = Arc::new(Mutex::new(Vec::new()));
    // Handler returns Flow::Stop on ready() — the assertion IS that
    // ready() fires (from the buffered Ready), causing a clean exit.
    let handler = ReadyTracker {
        log: log.clone(),
        _handle: handle,
    };

    let reason = builder.run_with(handler);

    // run_with drained the buffer, dispatched Ready → ready()
    // returned Flow::Stop → looper exited without entering recv loop.
    assert_eq!(reason, pane_app::exit_reason::ExitReason::Graceful);

    let events = log.lock().unwrap();
    assert!(
        events.contains(&"ready".to_string()),
        "handler.ready() must fire from buffered Ready. Got: {:?}",
        *events
    );

    drop(provider);
    conn_b.wait();
}

struct ReadyTracker {
    log: Arc<std::sync::Mutex<Vec<String>>>,
    _handle: pane_app::ServiceHandle<EchoService>,
}

impl Handler for ReadyTracker {
    fn ready(&mut self) -> Flow {
        self.log.lock().unwrap().push("ready".into());
        Flow::Stop // Exit after Ready — the assertion is that it fires
    }
}

impl Handles<EchoService> for ReadyTracker {
    fn receive(&mut self, _msg: EchoMessage) -> Flow {
        Flow::Continue
    }
}

/// N3: Multiple sequential open_service calls with different protocols.
/// Each gets its own DeclareInterest/InterestAccepted exchange and
/// its own session_id in the dispatch table.
#[test]
fn multiple_open_service_sequential() {
    let server = Arc::new(ProtocolServer::new());

    struct PingService;
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    enum PingMsg {
        Ping,
    }
    impl Protocol for PingService {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.ping")
        }
        type Message = PingMsg;
    }

    struct PongService;
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    enum PongMsg {
        Pong,
    }
    impl Protocol for PongService {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.pong")
        }
        type Message = PongMsg;
    }

    struct MultiHandler;
    impl Handler for MultiHandler {}
    impl Handles<PingService> for MultiHandler {
        fn receive(&mut self, _msg: PingMsg) -> Flow {
            Flow::Continue
        }
    }
    impl Handles<PongService> for MultiHandler {
        fn receive(&mut self, _msg: PongMsg) -> Flow {
            Flow::Continue
        }
    }

    // Provider offers both services
    let (b_client, b_server) = MemoryTransport::pair();
    let accept_b = accept_on_thread(&server, b_server);
    let (mut provider, _) = ClientConn::connect(
        b_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![
                ServiceProvision {
                    service: PingService::service_id(),
                    version: 1,
                },
                ServiceProvision {
                    service: PongService::service_id(),
                    version: 1,
                },
            ],
        },
    );
    let _conn_b = accept_b.join().unwrap();
    provider.expect_ready();

    // Consumer opens both
    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);
    let pane = Pane::new(Tag::new("multi-consumer"));
    let mut builder = pane.setup::<MultiHandler>();
    builder.connect(a_client).expect("connect failed");
    let _conn_a = accept_a.join().unwrap();

    let ping_handle = builder.open_service::<PingService>();
    assert!(ping_handle.is_some(), "ping open_service should succeed");
    let ping = ping_handle.unwrap();

    // Provider gets InterestAccepted for ping
    match provider.read_control() {
        ControlMessage::InterestAccepted { .. } => {}
        other => panic!("expected InterestAccepted for ping, got {other:?}"),
    }

    let pong_handle = builder.open_service::<PongService>();
    assert!(pong_handle.is_some(), "pong open_service should succeed");
    let pong = pong_handle.unwrap();

    // Provider gets InterestAccepted for pong
    match provider.read_control() {
        ControlMessage::InterestAccepted { .. } => {}
        other => panic!("expected InterestAccepted for pong, got {other:?}"),
    }

    // Session IDs should differ
    assert_ne!(
        ping.session_id(),
        pong.session_id(),
        "different services must get different session_ids"
    );

    // Both should be nonzero
    assert_ne!(ping.session_id(), 0);
    assert_ne!(pong.session_id(), 0);

    drop(ping);
    drop(pong);
    drop(builder);
    drop(provider);
}
