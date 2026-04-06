//! Integration test: two-pane service routing via ProtocolServer.
//!
//! Proof that the full service registration and inter-pane messaging
//! stack works end-to-end. Uses the single-threaded actor server.

use std::sync::Arc;

use pane_proto::control::ControlMessage;
use pane_proto::protocol::ServiceId;
use pane_proto::service_frame::ServiceFrame;
use pane_session::frame::{Frame, FrameCodec};
use pane_session::handshake::{Hello, Welcome, Rejection, ServiceProvision};
use pane_session::server::ProtocolServer;
use pane_session::transport::MemoryTransport;
use pane_session::bridge::HANDSHAKE_MAX_MESSAGE_SIZE;

use serde::{Serialize, Deserialize};

fn echo_service_id() -> ServiceId {
    ServiceId::new("com.pane.echo")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum EchoMessage {
    Ping(String),
    Pong(String),
}

// -- Helper: manual client-side handshake --

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

    fn send_control(&mut self, msg: &ControlMessage) {
        let bytes = postcard::to_allocvec(msg).unwrap();
        self.codec.write_frame(&mut self.transport, 0, &bytes).unwrap();
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

    fn send_service(&mut self, session_id: u8, frame: &ServiceFrame) {
        let bytes = postcard::to_allocvec(frame).unwrap();
        self.codec.write_frame(&mut self.transport, session_id, &bytes).unwrap();
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

/// Helper: spawn server.accept() on a thread so it doesn't block
/// the client handshake on the same thread.
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

// -- Tests --

#[test]
fn two_pane_echo_roundtrip() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // -- Pane B: echo provider --
    let (b_client, b_server) = MemoryTransport::pair();
    let accept_b = accept_on_thread(&server, b_server);

    let (mut pane_b, _) = ClientConn::connect(b_client, Hello {
        version: 1,
        max_message_size: 16 * 1024 * 1024,
        interests: vec![],
        provides: vec![ServiceProvision { service: echo_id, version: 1 }],
    });
    let conn_b = accept_b.join().unwrap();

    // -- Pane A: echo consumer --
    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);

    let (mut pane_a, _) = ClientConn::connect(a_client, Hello {
        version: 1,
        max_message_size: 16 * 1024 * 1024,
        interests: vec![],
        provides: vec![],
    });
    let conn_a = accept_a.join().unwrap();

    // -- DeclareInterest --
    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });

    let a_session = match pane_a.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for A, got {other:?}"),
    };

    let b_session = match pane_b.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for B, got {other:?}"),
    };

    pane_a.register_session(a_session);
    pane_b.register_session(b_session);

    // -- Request/Reply --
    let ping = EchoMessage::Ping("hello".into());
    let inner = postcard::to_allocvec(&ping).unwrap();
    pane_a.send_service(a_session, &ServiceFrame::Request { token: 42, payload: inner });

    let (recv_session, recv_frame) = pane_b.read_service();
    assert_eq!(recv_session, b_session);
    match recv_frame {
        ServiceFrame::Request { token, payload } => {
            assert_eq!(token, 42);
            let msg: EchoMessage = postcard::from_bytes(&payload).unwrap();
            assert_eq!(msg, EchoMessage::Ping("hello".into()));

            let pong = EchoMessage::Pong("hello".into());
            let reply_bytes = postcard::to_allocvec(&pong).unwrap();
            pane_b.send_service(b_session, &ServiceFrame::Reply { token, payload: reply_bytes });
        }
        other => panic!("expected Request, got {other:?}"),
    }

    let (recv_session, recv_frame) = pane_a.read_service();
    assert_eq!(recv_session, a_session);
    match recv_frame {
        ServiceFrame::Reply { token, payload } => {
            assert_eq!(token, 42);
            let msg: EchoMessage = postcard::from_bytes(&payload).unwrap();
            assert_eq!(msg, EchoMessage::Pong("hello".into()));
        }
        other => panic!("expected Reply, got {other:?}"),
    }

    drop(pane_a);
    drop(pane_b);
    conn_a.wait();
    conn_b.wait();
}

#[test]
fn declare_interest_no_provider_declined() {
    let server = Arc::new(ProtocolServer::new());

    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);

    let (mut pane_a, _) = ClientConn::connect(a_client, Hello {
        version: 1,
        max_message_size: 16 * 1024 * 1024,
        interests: vec![],
        provides: vec![],
    });
    let conn_a = accept_a.join().unwrap();

    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: ServiceId::new("com.pane.nonexistent"),
        expected_version: 1,
    });

    match pane_a.read_control() {
        ControlMessage::InterestDeclined { reason, .. } => {
            assert!(matches!(reason, pane_proto::control::DeclineReason::ServiceUnknown));
        }
        other => panic!("expected InterestDeclined, got {other:?}"),
    }

    drop(pane_a);
    conn_a.wait();
}

#[test]
fn notification_round_trip() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    let (b_client, b_server) = MemoryTransport::pair();
    let accept_b = accept_on_thread(&server, b_server);
    let (mut pane_b, _) = ClientConn::connect(b_client, Hello {
        version: 1,
        max_message_size: 16 * 1024 * 1024,
        interests: vec![],
        provides: vec![ServiceProvision { service: echo_id, version: 1 }],
    });
    let conn_b = accept_b.join().unwrap();

    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);
    let (mut pane_a, _) = ClientConn::connect(a_client, Hello {
        version: 1,
        max_message_size: 16 * 1024 * 1024,
        interests: vec![],
        provides: vec![],
    });
    let conn_a = accept_a.join().unwrap();

    // DeclareInterest
    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });

    let a_session = match pane_a.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted, got {other:?}"),
    };
    let b_session = match pane_b.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted, got {other:?}"),
    };

    pane_a.register_session(a_session);
    pane_b.register_session(b_session);

    // Fire-and-forget notification
    let msg = EchoMessage::Ping("broadcast".into());
    let bytes = postcard::to_allocvec(&msg).unwrap();
    pane_a.send_service(a_session, &ServiceFrame::Notification { payload: bytes });

    let (recv_session, recv_frame) = pane_b.read_service();
    assert_eq!(recv_session, b_session);
    match recv_frame {
        ServiceFrame::Notification { payload } => {
            let msg: EchoMessage = postcard::from_bytes(&payload).unwrap();
            assert_eq!(msg, EchoMessage::Ping("broadcast".into()));
        }
        other => panic!("expected Notification, got {other:?}"),
    }

    drop(pane_a);
    drop(pane_b);
    conn_a.wait();
    conn_b.wait();
}
