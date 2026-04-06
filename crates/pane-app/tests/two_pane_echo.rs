//! Integration test: two-pane service routing via ProtocolServer.
//!
//! Proof that the full service registration and inter-pane messaging
//! stack works end-to-end. Uses the single-threaded actor server.

use std::sync::Arc;

use pane_proto::control::ControlMessage;
use pane_proto::protocol::ServiceId;
use pane_proto::service_frame::ServiceFrame;
use pane_session::bridge::HANDSHAKE_MAX_MESSAGE_SIZE;
use pane_session::frame::{Frame, FrameCodec};
use pane_session::handshake::{Hello, Rejection, ServiceProvision, Welcome};
use pane_session::server::ProtocolServer;
use pane_session::transport::MemoryTransport;

use serde::{Deserialize, Serialize};

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

    fn send_control(&mut self, msg: &ControlMessage) {
        let bytes = postcard::to_allocvec(msg).unwrap();
        self.codec
            .write_frame(&mut self.transport, 0, &bytes)
            .unwrap();
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

    /// Read Ready from the server. Asserts the first active-phase
    /// message is Lifecycle::Ready — the server sends this after
    /// Welcome to signal the active phase has begun.
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

    fn send_service(&mut self, session_id: u16, frame: &ServiceFrame) {
        let bytes = postcard::to_allocvec(frame).unwrap();
        self.codec
            .write_frame(&mut self.transport, session_id, &bytes)
            .unwrap();
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

    let (mut pane_b, _) = ClientConn::connect(
        b_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
            provides: vec![ServiceProvision {
                service: echo_id,
                version: 1,
            }],
        },
    );
    let conn_b = accept_b.join().unwrap();
    pane_b.expect_ready();

    // -- Pane A: echo consumer --
    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);

    let (mut pane_a, _) = ClientConn::connect(
        a_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_a = accept_a.join().unwrap();
    pane_a.expect_ready();

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
    // Inner payloads carry a protocol tag byte (S8) before the
    // postcard-serialized message. The server forwards opaquely.
    let echo_tag = echo_service_id().tag();

    let ping = EchoMessage::Ping("hello".into());
    let ping_bytes = postcard::to_allocvec(&ping).unwrap();
    let mut inner = Vec::with_capacity(1 + ping_bytes.len());
    inner.push(echo_tag);
    inner.extend_from_slice(&ping_bytes);

    pane_a.send_service(
        a_session,
        &ServiceFrame::Request {
            token: 42,
            payload: inner,
        },
    );

    let (recv_session, recv_frame) = pane_b.read_service();
    assert_eq!(recv_session, b_session);
    match recv_frame {
        ServiceFrame::Request { token, payload } => {
            assert_eq!(token, 42);
            // Strip tag byte before deserializing
            assert_eq!(payload[0], echo_tag, "protocol tag must match");
            let msg: EchoMessage = postcard::from_bytes(&payload[1..]).unwrap();
            assert_eq!(msg, EchoMessage::Ping("hello".into()));

            let pong = EchoMessage::Pong("hello".into());
            let pong_bytes = postcard::to_allocvec(&pong).unwrap();
            let mut reply_inner = Vec::with_capacity(1 + pong_bytes.len());
            reply_inner.push(echo_tag);
            reply_inner.extend_from_slice(&pong_bytes);

            pane_b.send_service(
                b_session,
                &ServiceFrame::Reply {
                    token,
                    payload: reply_inner,
                },
            );
        }
        other => panic!("expected Request, got {other:?}"),
    }

    let (recv_session, recv_frame) = pane_a.read_service();
    assert_eq!(recv_session, a_session);
    match recv_frame {
        ServiceFrame::Reply { token, payload } => {
            assert_eq!(token, 42);
            assert_eq!(payload[0], echo_tag, "protocol tag must match");
            let msg: EchoMessage = postcard::from_bytes(&payload[1..]).unwrap();
            assert_eq!(msg, EchoMessage::Pong("hello".into()));
        }
        other => panic!("expected Reply, got {other:?}"),
    }

    drop(pane_a);
    drop(pane_b);
    conn_a.wait();
    conn_b.wait();
}

/// Old gap #1: connection drop → ServiceTeardown delivery to the peer.
/// Verifies that when one pane disconnects, the other receives
/// ServiceTeardown on the wire (not just server-side cleanup).
#[test]
fn connection_drop_delivers_service_teardown_to_peer() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Pane B: provider
    let (b_client, b_server) = MemoryTransport::pair();
    let accept_b = accept_on_thread(&server, b_server);
    let (mut pane_b, _) = ClientConn::connect(
        b_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
            provides: vec![ServiceProvision {
                service: echo_id,
                version: 1,
            }],
        },
    );
    let conn_b = accept_b.join().unwrap();
    pane_b.expect_ready();

    // Pane A: consumer
    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);
    let (mut pane_a, _) = ClientConn::connect(
        a_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_a = accept_a.join().unwrap();
    pane_a.expect_ready();

    // Establish service binding
    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });
    let _a_session = match pane_a.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for A, got {other:?}"),
    };
    let _b_session = match pane_b.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for B, got {other:?}"),
    };

    // Drop pane A — server should send ServiceTeardown to pane B
    drop(pane_a);
    conn_a.wait();

    // Pane B should receive ServiceTeardown
    let msg = pane_b.read_control();
    match msg {
        ControlMessage::ServiceTeardown { reason, .. } => {
            assert!(matches!(
                reason,
                pane_proto::control::TeardownReason::ConnectionLost
            ));
        }
        other => panic!("expected ServiceTeardown, got {other:?}"),
    }

    drop(pane_b);
    conn_b.wait();
}

/// Old gap #2: self-provide rejection — a pane declaring interest
/// in a service it provides itself should be declined.
#[test]
fn self_provide_interest_declined() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Pane A provides echo AND tries to consume echo
    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);
    let (mut pane_a, _) = ClientConn::connect(
        a_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
            provides: vec![ServiceProvision {
                service: echo_id,
                version: 1,
            }],
        },
    );
    let conn_a = accept_a.join().unwrap();
    pane_a.expect_ready();

    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });

    match pane_a.read_control() {
        ControlMessage::InterestDeclined { .. } => {
            // Correct: self-provide is declined
        }
        other => panic!("expected InterestDeclined for self-provide, got {other:?}"),
    }

    drop(pane_a);
    conn_a.wait();
}

#[test]
fn declare_interest_no_provider_declined() {
    let server = Arc::new(ProtocolServer::new());

    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);

    let (mut pane_a, _) = ClientConn::connect(
        a_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_a = accept_a.join().unwrap();
    pane_a.expect_ready();

    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: ServiceId::new("com.pane.nonexistent"),
        expected_version: 1,
    });

    match pane_a.read_control() {
        ControlMessage::InterestDeclined { reason, .. } => {
            assert!(matches!(
                reason,
                pane_proto::control::DeclineReason::ServiceUnknown
            ));
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
    let (mut pane_b, _) = ClientConn::connect(
        b_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
            provides: vec![ServiceProvision {
                service: echo_id,
                version: 1,
            }],
        },
    );
    let conn_b = accept_b.join().unwrap();
    pane_b.expect_ready();

    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);
    let (mut pane_a, _) = ClientConn::connect(
        a_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_a = accept_a.join().unwrap();
    pane_a.expect_ready();

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

    // Fire-and-forget notification — inner payload carries tag (S8).
    let echo_tag = echo_service_id().tag();
    let msg = EchoMessage::Ping("broadcast".into());
    let msg_bytes = postcard::to_allocvec(&msg).unwrap();
    let mut inner = Vec::with_capacity(1 + msg_bytes.len());
    inner.push(echo_tag);
    inner.extend_from_slice(&msg_bytes);
    pane_a.send_service(a_session, &ServiceFrame::Notification { payload: inner });

    let (recv_session, recv_frame) = pane_b.read_service();
    assert_eq!(recv_session, b_session);
    match recv_frame {
        ServiceFrame::Notification { payload } => {
            assert_eq!(payload[0], echo_tag, "protocol tag must match");
            let msg: EchoMessage = postcard::from_bytes(&payload[1..]).unwrap();
            assert_eq!(msg, EchoMessage::Ping("broadcast".into()));
        }
        other => panic!("expected Notification, got {other:?}"),
    }

    drop(pane_a);
    drop(pane_b);
    conn_a.wait();
    conn_b.wait();
}
