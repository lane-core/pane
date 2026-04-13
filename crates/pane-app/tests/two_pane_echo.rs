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
use pane_session::handshake::{Hello, RejectReason, Rejection, ServiceProvision, Welcome};
use pane_session::server::{AcceptError, ProtocolServer};
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
    fn connect(transport: MemoryTransport, hello: Hello) -> (Self, Welcome) {
        match Self::try_connect(transport, hello) {
            Ok(pair) => pair,
            Err(rejection) => panic!("rejected: {rejection:?}"),
        }
    }

    /// Handshake that returns the server's decision without panicking
    /// on rejection. Used by version_mismatch_rejected to observe the
    /// Rejection payload.
    fn try_connect(
        mut transport: MemoryTransport,
        hello: Hello,
    ) -> Result<(Self, Welcome), Rejection> {
        let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);

        // Handshake uses CBOR (D11)
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&hello, &mut bytes).unwrap();
        codec.write_frame(&mut transport, 0, &bytes).unwrap();

        let frame = codec.read_frame(&mut transport).unwrap();
        let payload = match frame {
            Frame::Message {
                service: 0,
                payload,
            } => payload,
            other => panic!("unexpected frame: {other:?}"),
        };
        let decision: Result<Welcome, Rejection> =
            ciborium::de::from_reader(payload.as_slice()).unwrap();

        match decision {
            Ok(welcome) => {
                let mut codec = codec;
                codec.set_max_message_size(welcome.max_message_size);
                Ok((ClientConn { transport, codec }, welcome))
            }
            Err(rejection) => Err(rejection),
        }
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
            max_outstanding_requests: 0,
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
            max_outstanding_requests: 0,
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
            max_outstanding_requests: 0,
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
            max_outstanding_requests: 0,
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

/// Self-provide rejection — a pane declaring interest in a service
/// it provides itself should be declined with `SelfProvide`.
///
/// Design heritage: Plan 9 walk(5) on own exports returns ELOOP —
/// a process cannot walk into a name it exported itself
/// (reference/plan9/man/5/walk, T20 from analysis/plan9_test_heritage).
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
            max_outstanding_requests: 0,
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
        ControlMessage::InterestDeclined {
            service_uuid,
            reason,
        } => {
            assert_eq!(service_uuid, echo_id.uuid);
            assert!(
                matches!(reason, pane_proto::control::DeclineReason::SelfProvide),
                "expected SelfProvide, got {reason:?}"
            );
        }
        other => panic!("expected InterestDeclined for self-provide, got {other:?}"),
    }

    // Connection stays alive after rejection — verify by sending
    // another DeclareInterest and receiving a response.
    let phantom_id = ServiceId::new("com.pane.phantom");
    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: phantom_id,
        expected_version: 1,
    });
    match pane_a.read_control() {
        ControlMessage::InterestDeclined { reason, .. } => {
            assert!(
                matches!(reason, pane_proto::control::DeclineReason::ServiceUnknown),
                "liveness check: expected ServiceUnknown, got {reason:?}"
            );
        }
        other => panic!("liveness check: expected InterestDeclined, got {other:?}"),
    }

    drop(pane_a);
    conn_a.wait();
}

/// Service-unknown rejection — declaring interest in a service
/// nobody provides yields `InterestDeclined { ServiceUnknown }`.
///
/// Design heritage: Plan 9 walk(5) on a non-existent name returns
/// ENOENT (reference/plan9/man/5/walk, T21 from
/// analysis/plan9_test_heritage).
#[test]
fn declare_interest_no_provider_declined() {
    let server = Arc::new(ProtocolServer::new());
    let nonexistent_id = ServiceId::new("com.pane.nonexistent");

    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);

    let (mut pane_a, _) = ClientConn::connect(
        a_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_a = accept_a.join().unwrap();
    pane_a.expect_ready();

    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: nonexistent_id,
        expected_version: 1,
    });

    match pane_a.read_control() {
        ControlMessage::InterestDeclined {
            service_uuid,
            reason,
        } => {
            assert_eq!(service_uuid, nonexistent_id.uuid);
            assert!(
                matches!(reason, pane_proto::control::DeclineReason::ServiceUnknown),
                "expected ServiceUnknown, got {reason:?}"
            );
        }
        other => panic!("expected InterestDeclined, got {other:?}"),
    }

    // Connection stays alive after rejection — verify by sending
    // another DeclareInterest and receiving a response.
    let phantom_id = ServiceId::new("com.pane.phantom");
    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: phantom_id,
        expected_version: 1,
    });
    match pane_a.read_control() {
        ControlMessage::InterestDeclined { reason, .. } => {
            assert!(
                matches!(reason, pane_proto::control::DeclineReason::ServiceUnknown),
                "liveness check: expected ServiceUnknown, got {reason:?}"
            );
        }
        other => panic!("liveness check: expected InterestDeclined, got {other:?}"),
    }

    drop(pane_a);
    conn_a.wait();
}

/// Service-unknown after provider disconnect — a provider connects,
/// registers service S, then disconnects. A subsequent consumer
/// declaring interest in S gets `InterestDeclined { ServiceUnknown }`
/// because `process_disconnect` cleans up the provider_index.
///
/// Design heritage: Plan 9 freefidpool after mount close — the
/// exported name should resolve to nothing once the exporter's
/// fids are freed (T23 from analysis/plan9_test_heritage).
#[test]
fn service_unknown_after_provider_disconnect() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Pane B: provider — connects, registers echo, then disconnects.
    let (b_client, b_server) = MemoryTransport::pair();
    let accept_b = accept_on_thread(&server, b_server);
    let (mut pane_b, _) = ClientConn::connect(
        b_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![ServiceProvision {
                service: echo_id,
                version: 1,
            }],
        },
    );
    let conn_b = accept_b.join().unwrap();
    pane_b.expect_ready();

    // Drop the provider — reader thread detects EOF, posts
    // Disconnected, actor calls process_disconnect which removes
    // echo_id from the provider_index.
    drop(pane_b);
    conn_b.wait();

    // Pane A: consumer — connects after provider is gone.
    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);
    let (mut pane_a, _) = ClientConn::connect(
        a_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_a = accept_a.join().unwrap();
    pane_a.expect_ready();

    // Provider is gone — should get ServiceUnknown.
    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });

    match pane_a.read_control() {
        ControlMessage::InterestDeclined {
            service_uuid,
            reason,
        } => {
            assert_eq!(service_uuid, echo_id.uuid);
            assert!(
                matches!(reason, pane_proto::control::DeclineReason::ServiceUnknown),
                "expected ServiceUnknown after provider disconnect, got {reason:?}"
            );
        }
        other => panic!("expected InterestDeclined after provider disconnect, got {other:?}"),
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
            max_outstanding_requests: 0,
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
            max_outstanding_requests: 0,
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

/// Version mismatch — client sends an unknown protocol version and
/// the server rejects with VersionMismatch before spawning any
/// reader/writer threads.
///
/// Design heritage: Plan 9 Tversion discipline — the server rejects
/// unknown versions rather than silently accepting
/// (reference/plan9/man/5/version:19-48).
#[test]
fn version_mismatch_rejected() {
    let server = Arc::new(ProtocolServer::new());
    let (client_transport, server_transport) = MemoryTransport::pair();

    // Accept runs on a thread — returns Err(Rejected) for bad version.
    let server_clone = Arc::clone(&server);
    let accept_handle = std::thread::spawn(move || {
        let (r, w) = server_transport.split();
        server_clone.accept(r, w)
    });

    // Client sends bad version
    let result = ClientConn::try_connect(
        client_transport,
        Hello {
            version: 999,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
    );

    // Client should get Rejection
    let rejection = match result {
        Err(r) => r,
        Ok(_) => panic!("expected rejection, got Ok"),
    };
    assert!(
        matches!(rejection.reason, RejectReason::VersionMismatch),
        "expected VersionMismatch, got {:?}",
        rejection.reason
    );
    assert!(
        rejection.message.as_ref().unwrap().contains("version 1"),
        "rejection message should mention supported version"
    );

    // Server accept should return Err(Rejected)
    let accept_result = accept_handle.join().unwrap();
    match accept_result {
        Err(AcceptError::Rejected(RejectReason::VersionMismatch)) => {}
        Err(other) => panic!("expected AcceptError::Rejected(VersionMismatch), got {other}"),
        Ok(_) => panic!("expected accept to fail, got Ok"),
    }
}

/// Second Hello on service 0 during the active phase is silently
/// dropped — the server's actor loop deserializes service-0 payloads
/// as postcard ControlMessage, so a CBOR Hello fails deserialization
/// and is ignored. The connection stays alive.
///
/// This tests the session subtyping invariant from the other side:
/// not "can I extend the handshake type?" but "does a stale
/// handshake message in the wrong phase cause damage?" The answer
/// is no — the format split (CBOR handshake / postcard data plane)
/// provides a natural firewall.
///
/// Design heritage: Plan 9 version(5) — a second Tversion after
/// the session is established resets the connection
/// (reference/plan9/man/5/version:19-48). pane is more lenient:
/// the CBOR/postcard format mismatch means the message is simply
/// unrecognizable to the data-plane parser, so it is dropped
/// rather than triggering a reset.
#[test]
fn second_hello_mid_session_rejected() {
    let server = Arc::new(ProtocolServer::new());

    let (client_transport, server_transport) = MemoryTransport::pair();
    let accept_handle = accept_on_thread(&server, server_transport);

    let (mut pane, _welcome) = ClientConn::connect(
        client_transport,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn = accept_handle.join().unwrap();
    pane.expect_ready();

    // Send a CBOR-serialized Hello on service 0 — this is what a
    // buggy client might do if it re-ran the handshake sequence.
    let rogue_hello = Hello {
        version: 1,
        max_message_size: 16 * 1024 * 1024,
        max_outstanding_requests: 0,
        interests: vec![],
        provides: vec![],
    };
    let mut cbor_bytes = Vec::new();
    ciborium::ser::into_writer(&rogue_hello, &mut cbor_bytes).unwrap();
    pane.codec
        .write_frame(&mut pane.transport, 0, &cbor_bytes)
        .unwrap();

    // Connection is still alive — verify by sending a valid
    // DeclareInterest and getting a response.
    let phantom_id = ServiceId::new("com.pane.phantom");
    pane.send_control(&ControlMessage::DeclareInterest {
        service: phantom_id,
        expected_version: 1,
    });
    match pane.read_control() {
        ControlMessage::InterestDeclined { reason, .. } => {
            assert!(
                matches!(reason, pane_proto::control::DeclineReason::ServiceUnknown),
                "liveness check: expected ServiceUnknown, got {reason:?}"
            );
        }
        other => {
            panic!("liveness check after rogue Hello: expected InterestDeclined, got {other:?}")
        }
    }

    drop(pane);
    conn.wait();
}

/// Regression guard: version 1 still produces Ok(Welcome).
/// Ensures version validation doesn't accidentally reject valid clients.
#[test]
fn version_one_accepted() {
    let server = Arc::new(ProtocolServer::new());
    let (client_transport, server_transport) = MemoryTransport::pair();

    let accept_handle = accept_on_thread(&server, server_transport);

    let result = ClientConn::try_connect(
        client_transport,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
    );

    let (pane, welcome) = match result {
        Ok(pair) => pair,
        Err(r) => panic!("version 1 should be accepted, got rejection: {r:?}"),
    };
    assert_eq!(welcome.version, 1);
    let conn = accept_handle.join().unwrap();

    drop(pane);
    conn.wait();
}
