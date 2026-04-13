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

/// Provider death delivers teardown, not transparent rebind.
///
/// When a provider dies, all sessions bound through it get
/// ServiceTeardown. The server does not silently rebind the
/// consumer to another provider — the handle is invalidated.
///
/// Design heritage: Plan 9 — when a mount point's server dies,
/// all fids bound through it receive Rerror, not transparent
/// rebind to a different server.
#[test]
fn provider_death_sends_teardown_not_rebind() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Pane B: echo provider
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

    // Establish session
    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });
    let a_session = match pane_a.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for A, got {other:?}"),
    };
    let _b_session = match pane_b.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for B, got {other:?}"),
    };

    // Kill the provider
    drop(pane_b);
    conn_b.wait();

    // Consumer should get ServiceTeardown for the bound session
    match pane_a.read_control() {
        ControlMessage::ServiceTeardown {
            session_id,
            reason,
        } => {
            assert_eq!(session_id, a_session, "teardown must reference A's session");
            assert!(
                matches!(reason, pane_proto::control::TeardownReason::ConnectionLost),
                "expected ConnectionLost, got {reason:?}"
            );
        }
        other => panic!("expected ServiceTeardown, got {other:?}"),
    }

    // Connection survives — verify liveness by declaring interest in
    // a service that no longer exists (B is dead, no providers left).
    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
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

/// A new provider does not hijack existing session routes.
///
/// Once a session is bound to a specific provider, a second
/// provider registering the same service does not affect the
/// routing of the existing session. Route table entries are
/// immutable after creation.
///
/// Design heritage: Plan 9 — namec is consulted during walk
/// only, not on every I/O. Once a fid is bound to a qid, the
/// mount table is not re-consulted per read/write.
#[test]
fn new_provider_does_not_affect_existing_handle() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Pane B: first echo provider
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

    // Pane A: consumer — binds session to B
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

    // Establish A→B session
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

    // Pane C: second echo provider — joins AFTER the session exists
    let (c_client, c_server) = MemoryTransport::pair();
    let accept_c = accept_on_thread(&server, c_server);
    let (mut pane_c, _) = ClientConn::connect(
        c_client,
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
    let conn_c = accept_c.join().unwrap();
    pane_c.expect_ready();

    // A sends Request on the existing session — must reach B, not C
    let echo_tag = echo_service_id().tag();
    let ping = EchoMessage::Ping("routed-to-b".into());
    let ping_bytes = postcard::to_allocvec(&ping).unwrap();
    let mut inner = Vec::with_capacity(1 + ping_bytes.len());
    inner.push(echo_tag);
    inner.extend_from_slice(&ping_bytes);

    pane_a.send_service(
        a_session,
        &ServiceFrame::Request {
            token: 99,
            payload: inner,
        },
    );

    // B receives the request
    let (recv_session, recv_frame) = pane_b.read_service();
    assert_eq!(recv_session, b_session);
    match recv_frame {
        ServiceFrame::Request { token, payload } => {
            assert_eq!(token, 99);
            assert_eq!(payload[0], echo_tag);
            let msg: EchoMessage = postcard::from_bytes(&payload[1..]).unwrap();
            assert_eq!(msg, EchoMessage::Ping("routed-to-b".into()));

            // B replies
            let pong = EchoMessage::Pong("from-b".into());
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
        other => panic!("expected Request on B, got {other:?}"),
    }

    // A receives the reply from B
    let (recv_session, recv_frame) = pane_a.read_service();
    assert_eq!(recv_session, a_session);
    match recv_frame {
        ServiceFrame::Reply { token, payload } => {
            assert_eq!(token, 99);
            assert_eq!(payload[0], echo_tag);
            let msg: EchoMessage = postcard::from_bytes(&payload[1..]).unwrap();
            assert_eq!(msg, EchoMessage::Pong("from-b".into()));
        }
        other => panic!("expected Reply on A, got {other:?}"),
    }

    // C never received any frames for A's session — verified
    // structurally: the route table entry for (A, a_session)
    // points to (B, b_session). C has no route entry for this
    // session. The successful B→A round trip above is the proof.

    drop(pane_a);
    drop(pane_b);
    drop(pane_c);
    conn_a.wait();
    conn_b.wait();
    conn_c.wait();
}

/// After a provider dies, new DeclareInterest routes to a survivor.
///
/// When multiple providers serve the same service and one dies,
/// the server's provider_index is cleaned up. A new DeclareInterest
/// finds the surviving provider — the service is still available.
///
/// Design heritage: Plan 9 mount table — after one server in a
/// union mount dies, surviving servers still serve the namespace.
#[test]
fn new_declare_interest_after_provider_death_routes_to_survivor() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Pane B: first echo provider
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

    // Pane C: second echo provider
    let (c_client, c_server) = MemoryTransport::pair();
    let accept_c = accept_on_thread(&server, c_server);
    let (mut pane_c, _) = ClientConn::connect(
        c_client,
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
    let conn_c = accept_c.join().unwrap();
    pane_c.expect_ready();

    // Kill B — provider_index drops B, C survives
    drop(pane_b);
    conn_b.wait();

    // Pane A: consumer — connects after B is dead
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

    // A declares interest — should route to C (the survivor)
    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });
    let a_session = match pane_a.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted (routed to C), got {other:?}"),
    };
    let c_session = match pane_c.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for C, got {other:?}"),
    };
    pane_a.register_session(a_session);
    pane_c.register_session(c_session);

    // Verify routing: A sends Request, C receives it
    let echo_tag = echo_service_id().tag();
    let ping = EchoMessage::Ping("to-survivor".into());
    let ping_bytes = postcard::to_allocvec(&ping).unwrap();
    let mut inner = Vec::with_capacity(1 + ping_bytes.len());
    inner.push(echo_tag);
    inner.extend_from_slice(&ping_bytes);

    pane_a.send_service(
        a_session,
        &ServiceFrame::Request {
            token: 77,
            payload: inner,
        },
    );

    let (recv_session, recv_frame) = pane_c.read_service();
    assert_eq!(recv_session, c_session);
    match recv_frame {
        ServiceFrame::Request { token, payload } => {
            assert_eq!(token, 77);
            assert_eq!(payload[0], echo_tag);
            let msg: EchoMessage = postcard::from_bytes(&payload[1..]).unwrap();
            assert_eq!(msg, EchoMessage::Ping("to-survivor".into()));
        }
        other => panic!("expected Request on C, got {other:?}"),
    }

    drop(pane_a);
    drop(pane_c);
    conn_a.wait();
    conn_c.wait();
}

/// Watch a live pane, then disconnect it — watcher receives PaneExited.
///
/// Design heritage: Haiku IsWatched2+3 — add watcher, observed event
/// fires on target death. BRoster::StartWatching registered watchers
/// at the registrar; the registrar broadcast B_SOME_APP_QUIT on app
/// death (src/servers/registrar/TRoster.cpp:1523-1536).
#[test]
fn watch_then_disconnect_delivers_pane_exited() {
    let server = Arc::new(ProtocolServer::new());

    // Pane B: the watch target.
    let (b_client, b_server) = MemoryTransport::pair();
    let accept_b = accept_on_thread(&server, b_server);
    let (mut pane_b, _) = ClientConn::connect(
        b_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_b = accept_b.join().unwrap();
    pane_b.expect_ready();
    let b_address = pane_proto::Address::local(conn_b.conn_id.0);

    // Pane A: the watcher.
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

    // A watches B.
    pane_a.send_control(&ControlMessage::Watch { target: b_address });

    // B disconnects — server should send PaneExited to A.
    drop(pane_b);
    conn_b.wait();

    // A receives PaneExited for B.
    match pane_a.read_control() {
        ControlMessage::PaneExited { address, reason } => {
            assert_eq!(address, b_address);
            assert_eq!(reason, pane_proto::ExitReason::Disconnected);
        }
        other => panic!("expected PaneExited, got {other:?}"),
    }

    drop(pane_a);
    conn_a.wait();
}

/// Watch then unwatch — target disconnect does NOT deliver PaneExited.
///
/// Design heritage: Haiku IsWatched4 — remove watcher, observed event
/// does not fire. BRoster::StopWatching
/// (src/servers/registrar/TRoster.cpp:1538-1558) removed the watcher
/// from the registrar's table.
#[test]
fn unwatch_then_disconnect_no_pane_exited() {
    let server = Arc::new(ProtocolServer::new());

    // Pane B: the watch target.
    let (b_client, b_server) = MemoryTransport::pair();
    let accept_b = accept_on_thread(&server, b_server);
    let (mut pane_b, _) = ClientConn::connect(
        b_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_b = accept_b.join().unwrap();
    pane_b.expect_ready();
    let b_address = pane_proto::Address::local(conn_b.conn_id.0);

    // Pane A: the watcher.
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

    // A watches B, then unwatches.
    pane_a.send_control(&ControlMessage::Watch { target: b_address });
    pane_a.send_control(&ControlMessage::Unwatch { target: b_address });

    // B disconnects — A should NOT receive PaneExited.
    drop(pane_b);
    conn_b.wait();

    // Verify A's next message is NOT PaneExited: send a DeclareInterest
    // for a non-existent service. The server replies with InterestDeclined
    // (ServiceUnknown). If PaneExited were queued, it would arrive first.
    let phantom_id = ServiceId::new("com.pane.phantom");
    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: phantom_id,
        expected_version: 1,
    });
    match pane_a.read_control() {
        ControlMessage::InterestDeclined { reason, .. } => {
            assert!(
                matches!(reason, pane_proto::control::DeclineReason::ServiceUnknown),
                "expected ServiceUnknown (no PaneExited before it), got {reason:?}"
            );
        }
        ControlMessage::PaneExited { .. } => {
            panic!("received PaneExited after Unwatch — Unwatch did not take effect");
        }
        other => panic!("expected InterestDeclined, got {other:?}"),
    }

    drop(pane_a);
    conn_a.wait();
}

/// Watch a target that never existed — immediate PaneExited.
///
/// Design heritage: Haiku IsWatched5 — SendNotices to an empty
/// watcher table is safe, and watching a dead target should resolve
/// immediately. Plan 9 — watching a dead name resolves immediately
/// rather than blocking.
#[test]
fn watch_unknown_target_immediate_pane_exited() {
    let server = Arc::new(ProtocolServer::new());

    // Pane A: the watcher.
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

    // Watch a target that was never connected.
    let ghost_address = pane_proto::Address::local(9999);
    pane_a.send_control(&ControlMessage::Watch {
        target: ghost_address,
    });

    // Should get immediate PaneExited — no blocking.
    match pane_a.read_control() {
        ControlMessage::PaneExited { address, reason } => {
            assert_eq!(address, ghost_address);
            assert_eq!(reason, pane_proto::ExitReason::Disconnected);
        }
        other => panic!("expected immediate PaneExited for unknown target, got {other:?}"),
    }

    drop(pane_a);
    conn_a.wait();
}

/// Watch self then disconnect — server handles gracefully without panic.
///
/// Edge case: when A watches itself and then disconnects, the server's
/// process_disconnect tries to send PaneExited to A (the watcher), but
/// A's transport is being torn down. The enqueue either fails (pushing
/// to overflow_teardown, which is a no-op on an already-disconnecting
/// connection) or succeeds into a dead writer thread. Neither panics.
#[test]
fn watch_self_then_disconnect() {
    let server = Arc::new(ProtocolServer::new());

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

    // A watches itself.
    let a_address = pane_proto::Address::local(conn_a.conn_id.0);
    pane_a.send_control(&ControlMessage::Watch { target: a_address });

    // A disconnects — server must not panic during process_disconnect
    // when it tries to deliver PaneExited to the dying connection.
    drop(pane_a);
    conn_a.wait();

    // If we reach here, the server handled watch-self teardown
    // without panicking. Verify the server is still functional
    // by connecting a new pane.
    let (b_client, b_server) = MemoryTransport::pair();
    let accept_b = accept_on_thread(&server, b_server);
    let (mut pane_b, _) = ClientConn::connect(
        b_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_b = accept_b.join().unwrap();
    pane_b.expect_ready();

    drop(pane_b);
    conn_b.wait();
}

/// Full disconnect cleanup: no ghost routes, stale providers, or orphaned
/// watch table entries survive after all connections close.
///
/// Lifecycle: connect, provide, declare interest, watch, then disconnect
/// both panes in sequence. After cleanup, verify a new provider/consumer
/// pair can bind the same service and complete a full round-trip.
///
/// Design heritage: Plan 9 — freefidpool walks the entire fid pool on
/// close; closepgrp walks the mount table. Property: zero residual
/// references after close (reference/plan9/src/sys/src/cmd/exportfs/).
#[test]
fn disconnect_cleans_all_state() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // -- Phase 1: establish a full session with watches --

    // Pane B: echo provider
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

    // A declares interest in echo → gets session
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

    // A watches B
    let b_address = pane_proto::Address::local(conn_b.conn_id.0);
    pane_a.send_control(&ControlMessage::Watch { target: b_address });

    // -- Phase 2: disconnect A, then B --

    // Drop A's transport — server processes disconnect, cleans A's
    // routing_table entries, watch registrations, conn_addresses.
    // B should receive ServiceTeardown (A revoked interest via disconnect).
    drop(pane_a);
    conn_a.wait();

    match pane_b.read_control() {
        ControlMessage::ServiceTeardown { reason, .. } => {
            assert!(
                matches!(reason, pane_proto::control::TeardownReason::ConnectionLost),
                "expected ConnectionLost, got {reason:?}"
            );
        }
        other => panic!("expected ServiceTeardown after A disconnect, got {other:?}"),
    }

    // Drop B's transport — server processes disconnect, cleans B's
    // provider_index entry, watch_table, conn_addresses.
    drop(pane_b);
    conn_b.wait();

    // -- Phase 3: verify no ghost state --

    // Pane D: new consumer — DeclareInterest for echo should get
    // ServiceUnknown because B (the provider) is dead and cleaned up.
    let (d_client, d_server) = MemoryTransport::pair();
    let accept_d = accept_on_thread(&server, d_server);
    let (mut pane_d, _) = ClientConn::connect(
        d_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_d = accept_d.join().unwrap();
    pane_d.expect_ready();

    pane_d.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });
    match pane_d.read_control() {
        ControlMessage::InterestDeclined { reason, .. } => {
            assert!(
                matches!(reason, pane_proto::control::DeclineReason::ServiceUnknown),
                "ghost provider: expected ServiceUnknown, got {reason:?}"
            );
        }
        other => panic!("ghost provider check: expected InterestDeclined, got {other:?}"),
    }

    drop(pane_d);
    conn_d.wait();

    // -- Phase 4: new provider/consumer round-trip on same service --

    // Pane E: new echo provider
    let (e_client, e_server) = MemoryTransport::pair();
    let accept_e = accept_on_thread(&server, e_server);
    let (mut pane_e, _) = ClientConn::connect(
        e_client,
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
    let conn_e = accept_e.join().unwrap();
    pane_e.expect_ready();

    // Pane F: new consumer
    let (f_client, f_server) = MemoryTransport::pair();
    let accept_f = accept_on_thread(&server, f_server);
    let (mut pane_f, _) = ClientConn::connect(
        f_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_f = accept_f.join().unwrap();
    pane_f.expect_ready();

    // F declares interest → routes to E (not ghost B)
    pane_f.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });
    let f_session = match pane_f.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for F, got {other:?}"),
    };
    let e_session = match pane_e.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for E, got {other:?}"),
    };
    pane_f.register_session(f_session);
    pane_e.register_session(e_session);

    // Full round-trip: F sends Request, E receives and replies, F gets Reply.
    let echo_tag = echo_service_id().tag();
    let ping = EchoMessage::Ping("post-cleanup".into());
    let ping_bytes = postcard::to_allocvec(&ping).unwrap();
    let mut inner = Vec::with_capacity(1 + ping_bytes.len());
    inner.push(echo_tag);
    inner.extend_from_slice(&ping_bytes);

    pane_f.send_service(
        f_session,
        &ServiceFrame::Request {
            token: 1,
            payload: inner,
        },
    );

    let (recv_session, recv_frame) = pane_e.read_service();
    assert_eq!(recv_session, e_session);
    match recv_frame {
        ServiceFrame::Request { token, payload } => {
            assert_eq!(token, 1);
            assert_eq!(payload[0], echo_tag);
            let msg: EchoMessage = postcard::from_bytes(&payload[1..]).unwrap();
            assert_eq!(msg, EchoMessage::Ping("post-cleanup".into()));

            let pong = EchoMessage::Pong("post-cleanup".into());
            let pong_bytes = postcard::to_allocvec(&pong).unwrap();
            let mut reply_inner = Vec::with_capacity(1 + pong_bytes.len());
            reply_inner.push(echo_tag);
            reply_inner.extend_from_slice(&pong_bytes);

            pane_e.send_service(
                e_session,
                &ServiceFrame::Reply {
                    token,
                    payload: reply_inner,
                },
            );
        }
        other => panic!("expected Request on E, got {other:?}"),
    }

    let (recv_session, recv_frame) = pane_f.read_service();
    assert_eq!(recv_session, f_session);
    match recv_frame {
        ServiceFrame::Reply { token, payload } => {
            assert_eq!(token, 1);
            assert_eq!(payload[0], echo_tag);
            let msg: EchoMessage = postcard::from_bytes(&payload[1..]).unwrap();
            assert_eq!(msg, EchoMessage::Pong("post-cleanup".into()));
        }
        other => panic!("expected Reply on F, got {other:?}"),
    }

    drop(pane_e);
    drop(pane_f);
    conn_e.wait();
    conn_f.wait();
}

/// Rapid full-mesh connect-disconnect, then verify server is fully functional.
///
/// Four panes form a mesh (two providers, two consumers, watches between
/// them), then all disconnect in rapid succession. After cleanup, a new
/// provider/consumer pair completes a full round-trip — proving the server
/// has no residual state from the torn-down mesh.
///
/// Design heritage: Plan 9 — after full mesh teardown, new mounts work.
/// The server's state machine returns to an equivalent of fresh-start.
#[test]
fn post_disconnect_new_connections_fully_functional() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // -- Phase 1: build a 4-pane mesh --

    // Pane A: echo provider #1
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

    // Pane B: echo provider #2
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

    // Pane C: consumer #1
    let (c_client, c_server) = MemoryTransport::pair();
    let accept_c = accept_on_thread(&server, c_server);
    let (mut pane_c, _) = ClientConn::connect(
        c_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_c = accept_c.join().unwrap();
    pane_c.expect_ready();

    // Pane D: consumer #2
    let (d_client, d_server) = MemoryTransport::pair();
    let accept_d = accept_on_thread(&server, d_server);
    let (mut pane_d, _) = ClientConn::connect(
        d_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_d = accept_d.join().unwrap();
    pane_d.expect_ready();

    // C declares interest → gets session (routed to A or B)
    pane_c.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });
    let _c_session = match pane_c.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for C, got {other:?}"),
    };
    // Provider (A or B) also receives InterestAccepted — drain it.
    // The server sends to whichever provider was selected; we don't
    // know which, so try A first, fall back to B.
    //
    // Because the server picks the first provider in the index, and
    // A connected first, A gets the session for C.
    let _a_session_for_c = match pane_a.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for A (from C), got {other:?}"),
    };

    // D declares interest → gets session
    pane_d.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });
    let _d_session = match pane_d.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for D, got {other:?}"),
    };
    // Provider receives InterestAccepted — A again (first in index).
    let _a_session_for_d = match pane_a.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for A (from D), got {other:?}"),
    };

    // C watches A, D watches B
    let a_address = pane_proto::Address::local(conn_a.conn_id.0);
    let b_address = pane_proto::Address::local(conn_b.conn_id.0);
    pane_c.send_control(&ControlMessage::Watch { target: a_address });
    pane_d.send_control(&ControlMessage::Watch { target: b_address });

    // -- Phase 2: disconnect all four in rapid succession --
    // Drop transports, then wait on all handles. The waits ensure
    // the actor has processed every disconnect event.
    drop(pane_a);
    drop(pane_b);
    drop(pane_c);
    drop(pane_d);
    conn_a.wait();
    conn_b.wait();
    conn_c.wait();
    conn_d.wait();

    // -- Phase 3: verify server is fully functional --

    // Pane E: new echo provider
    let (e_client, e_server) = MemoryTransport::pair();
    let accept_e = accept_on_thread(&server, e_server);
    let (mut pane_e, _) = ClientConn::connect(
        e_client,
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
    let conn_e = accept_e.join().unwrap();
    pane_e.expect_ready();

    // Pane F: new consumer
    let (f_client, f_server) = MemoryTransport::pair();
    let accept_f = accept_on_thread(&server, f_server);
    let (mut pane_f, _) = ClientConn::connect(
        f_client,
        Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            interests: vec![],
            provides: vec![],
        },
    );
    let conn_f = accept_f.join().unwrap();
    pane_f.expect_ready();

    // F declares interest → routes to E (no ghost A/B)
    pane_f.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });
    let f_session = match pane_f.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for F, got {other:?}"),
    };
    let e_session = match pane_e.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted for E, got {other:?}"),
    };
    pane_f.register_session(f_session);
    pane_e.register_session(e_session);

    // Full round-trip proves no residual routing state interferes.
    let echo_tag = echo_service_id().tag();
    let ping = EchoMessage::Ping("post-mesh-teardown".into());
    let ping_bytes = postcard::to_allocvec(&ping).unwrap();
    let mut inner = Vec::with_capacity(1 + ping_bytes.len());
    inner.push(echo_tag);
    inner.extend_from_slice(&ping_bytes);

    pane_f.send_service(
        f_session,
        &ServiceFrame::Request {
            token: 2,
            payload: inner,
        },
    );

    let (recv_session, recv_frame) = pane_e.read_service();
    assert_eq!(recv_session, e_session);
    match recv_frame {
        ServiceFrame::Request { token, payload } => {
            assert_eq!(token, 2);
            assert_eq!(payload[0], echo_tag);
            let msg: EchoMessage = postcard::from_bytes(&payload[1..]).unwrap();
            assert_eq!(msg, EchoMessage::Ping("post-mesh-teardown".into()));

            let pong = EchoMessage::Pong("post-mesh-teardown".into());
            let pong_bytes = postcard::to_allocvec(&pong).unwrap();
            let mut reply_inner = Vec::with_capacity(1 + pong_bytes.len());
            reply_inner.push(echo_tag);
            reply_inner.extend_from_slice(&pong_bytes);

            pane_e.send_service(
                e_session,
                &ServiceFrame::Reply {
                    token,
                    payload: reply_inner,
                },
            );
        }
        other => panic!("expected Request on E, got {other:?}"),
    }

    let (recv_session, recv_frame) = pane_f.read_service();
    assert_eq!(recv_session, f_session);
    match recv_frame {
        ServiceFrame::Reply { token, payload } => {
            assert_eq!(token, 2);
            assert_eq!(payload[0], echo_tag);
            let msg: EchoMessage = postcard::from_bytes(&payload[1..]).unwrap();
            assert_eq!(msg, EchoMessage::Pong("post-mesh-teardown".into()));
        }
        other => panic!("expected Reply on F, got {other:?}"),
    }

    drop(pane_e);
    drop(pane_f);
    conn_e.wait();
    conn_f.wait();
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
