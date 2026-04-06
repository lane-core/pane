//! Integration test: PaneBuilder connects to ProtocolServer,
//! opens a service via DeclareInterest, gets a real ServiceHandle.

use std::sync::Arc;

use pane_proto::control::ControlMessage;
use pane_proto::protocol::ServiceId;
use pane_proto::service_frame::ServiceFrame;
use pane_proto::{Flow, Handler, Handles, Protocol};
use pane_session::frame::{Frame, FrameCodec};
use pane_session::handshake::{Hello, Welcome, Rejection, ServiceProvision};
use pane_session::server::ProtocolServer;
use pane_session::transport::MemoryTransport;
use pane_session::bridge::HANDSHAKE_MAX_MESSAGE_SIZE;
use pane_app::pane::{Pane, Tag};

use serde::{Serialize, Deserialize};

fn echo_service_id() -> ServiceId {
    ServiceId::new("com.pane.echo")
}

struct EchoService;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum EchoMessage { Ping(String), Pong(String) }

impl Protocol for EchoService {
    fn service_id() -> ServiceId { echo_service_id() }
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
            Frame::Message { service: 0, payload } => payload,
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
            Frame::Message { service: 0, payload } =>
                postcard::from_bytes(&payload).unwrap(),
            other => panic!("expected Control, got {other:?}"),
        }
    }

    fn register_session(&mut self, session_id: u8) {
        self.codec.register_service(session_id);
    }

    fn read_service(&mut self) -> (u8, ServiceFrame) {
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
    let (mut provider, _) = ClientConn::connect(b_client, Hello {
        version: 1,
        max_message_size: 16 * 1024 * 1024,
        interests: vec![],
        provides: vec![ServiceProvision { service: echo_service_id(), version: 1 }],
    });
    let conn_b = accept_b.join().unwrap();

    // Consumer: PaneBuilder
    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);

    let pane = Pane::new(Tag::new("consumer"));
    let mut builder = pane.setup::<ConsumerHandler>();
    builder.connect(a_client).expect("connect failed");
    let conn_a = accept_a.join().unwrap();

    // open_service sends DeclareInterest, blocks for InterestAccepted
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

    // Provider receives it
    let (recv_session, recv_frame) = provider.read_service();
    assert_eq!(recv_session, b_session);
    match recv_frame {
        ServiceFrame::Notification { payload } => {
            let msg: EchoMessage = postcard::from_bytes(&payload).unwrap();
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
    assert!(handle.is_none(), "open_service should return None when declined");

    drop(builder);
    conn_a.wait();
}
