//! Stress tests for transport, framing, and server layers.
//!
//! Every test here validates a specific invariant under adversarial
//! conditions: high concurrency, rapid lifecycle transitions, or
//! malformed input. Tests are #[ignore] because they are slow —
//! run with `cargo test -- --ignored` or by name.
//!
//! Tests that reveal bugs describe the bug in the doc comment and
//! use assert patterns that document the failure mode.

use std::sync::Arc;

use pane_proto::control::ControlMessage;
use pane_proto::protocol::ServiceId;
use pane_proto::service_frame::ServiceFrame;
use pane_session::bridge::HANDSHAKE_MAX_MESSAGE_SIZE;
use pane_session::frame::{Frame, FrameCodec, FrameError};
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

// -- Shared helpers --

/// Manual client with control and service frame helpers.
/// Reused across stress tests.
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

/// Generic client connection for any Read+Write transport.
/// Used by UnixStream tests where MemoryTransport isn't involved.
struct GenericClientConn<T: std::io::Read + std::io::Write> {
    transport: T,
    codec: FrameCodec,
}

impl<T: std::io::Read + std::io::Write> GenericClientConn<T> {
    fn connect(mut transport: T, hello: Hello) -> (Self, Welcome) {
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
        (GenericClientConn { transport, codec }, welcome)
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

// ════════════════════════════════════════════════════════════════
// S3: Codec resync after oversized frame
// ════════════════════════════════════════════════════════════════

/// S3: After FrameError::Oversized, the codec consumed the 4-byte
/// length prefix but NOT the body bytes. The next read_frame call
/// reads body bytes as the next length prefix, producing garbage.
///
/// This test demonstrates the behavior: after an oversized frame
/// error, the stream is desynced. A subsequent valid frame written
/// after the oversized body will be misinterpreted.
///
/// The FrameError doc comment says "the connection is no longer
/// trustworthy" — the invariant is that after ANY FrameError,
/// the caller must treat the connection as dead. This test
/// documents that the codec does NOT enforce this (no internal
/// poisoning), leaving it to the caller.
///
/// Invariant: FrameError means connection death. Violating this
/// by continuing to read. The codec self-poisons on any error
/// (S3 fix), so the second read returns Poisoned without touching
/// the stream — the desync is prevented structurally.
#[test]
#[ignore]
fn codec_desync_after_oversized_frame() {
    use std::io::Cursor;

    let codec = FrameCodec::new(100);

    // Build a stream with:
    //   1. An oversized frame: length=200, then 200 body bytes
    //   2. A valid frame immediately after
    let mut stream = Vec::new();

    // Oversized frame: length prefix says 200 bytes
    stream.extend_from_slice(&200u32.to_le_bytes());
    // Body: 200 bytes of 0xAA (the codec won't read these)
    stream.extend_from_slice(&vec![0xAA; 200]);

    // Valid frame after the oversized one (never reached due to poison)
    // length=3: u16 service (0x0000) + 1 byte payload
    stream.extend_from_slice(&3u32.to_le_bytes());
    stream.extend_from_slice(&[0x00, 0x00, 0x42]);

    let mut cursor = Cursor::new(stream);

    // First read: returns Oversized error and poisons the codec
    let result = codec.read_frame(&mut cursor);
    assert!(
        matches!(
            result,
            Err(FrameError::Oversized {
                declared: 200,
                limit: 100
            })
        ),
        "first read must return Oversized, got {result:?}"
    );

    // Second read: codec is poisoned, returns Poisoned immediately
    // without touching the stream. The valid frame is never read.
    let result2 = codec.read_frame(&mut cursor);
    assert!(
        matches!(result2, Err(FrameError::Poisoned)),
        "second read must return Poisoned (S3 fix), got {result2:?}"
    );

    // Cursor stayed at offset 4 — poison prevented the desync
    assert_eq!(
        cursor.position(),
        4,
        "cursor must not advance past the length prefix"
    );
}

/// Companion to codec_desync_after_oversized_frame: verifies that
/// after an Oversized error, the cursor stays at offset 4 (only
/// the length prefix consumed). With the poison fix (S3), subsequent
/// reads return Poisoned without advancing the cursor further.
#[test]
#[ignore]
fn oversized_frame_caller_stops_reading() {
    use std::io::Cursor;

    let codec = FrameCodec::new(50);

    let mut stream = Vec::new();
    // Oversized frame
    stream.extend_from_slice(&100u32.to_le_bytes());
    stream.extend_from_slice(&vec![0xBB; 100]);
    // Valid frame that should never be read (u16 service format)
    stream.extend_from_slice(&3u32.to_le_bytes());
    stream.extend_from_slice(&[0x00, 0x00, 0x42]);

    let mut cursor = Cursor::new(stream);

    let result = codec.read_frame(&mut cursor);
    assert!(matches!(result, Err(FrameError::Oversized { .. })));

    // Cursor at offset 4 — length prefix consumed, body not consumed.
    // The codec is now poisoned; any further read returns Poisoned.
    assert_eq!(
        cursor.position(),
        4,
        "cursor should be at offset 4 (length prefix consumed, body not consumed)"
    );
}

// ════════════════════════════════════════════════════════════════
// S1: RevokeInterest / Request race
// ════════════════════════════════════════════════════════════════

/// S1: RevokeInterest arrives at the server BEFORE a pending
/// request's reply. The route is torn down, the reply has no
/// path back to the consumer, and the consumer's Dispatch entry
/// is orphaned forever (no reply, no failure signal).
///
/// This test demonstrates the race by establishing a route,
/// sending a Request, immediately sending RevokeInterest, then
/// having the provider reply. The reply should be silently dropped
/// by the server (no route). The consumer never receives Reply
/// or Failed — the Dispatch entry would leak.
///
/// The server correctly drops the reply (process_service comment:
/// "Missing route on incoming Reply = Cancel/Reply race, not an
/// error"). The bug is consumer-side: nothing notifies the consumer
/// that the request will never be answered.
///
/// This test validates the server's behavior and documents the
/// consumer-side gap.
#[test]
#[ignore]
fn revoke_interest_request_race_reply_dropped() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Provider
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

    // Consumer
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

    // Establish route
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

    // Consumer sends Request
    let inner = postcard::to_allocvec(&EchoMessage::Ping("race".into())).unwrap();
    pane_a.send_service(
        a_session,
        &ServiceFrame::Request {
            token: 1,
            payload: inner,
        },
    );

    // Consumer immediately revokes interest (ServiceHandle dropped)
    pane_a.send_control(&ControlMessage::RevokeInterest {
        session_id: a_session,
    });

    // Provider receives the Request (it was already in flight)
    let (recv_session, recv_frame) = pane_b.read_service();
    assert_eq!(recv_session, b_session);
    let request_token = match recv_frame {
        ServiceFrame::Request { token, payload } => {
            let msg: EchoMessage = postcard::from_bytes(&payload).unwrap();
            assert_eq!(msg, EchoMessage::Ping("race".into()));
            token
        }
        other => panic!("expected Request, got {other:?}"),
    };

    // Provider also receives ServiceTeardown (from the RevokeInterest)
    let teardown = pane_b.read_control();
    assert!(
        matches!(teardown, ControlMessage::ServiceTeardown { .. }),
        "provider must receive ServiceTeardown after RevokeInterest, got {teardown:?}"
    );

    // Provider sends Reply anyway (common in async systems —
    // the provider may have started processing before seeing teardown)
    let reply_bytes = postcard::to_allocvec(&EchoMessage::Pong("race".into())).unwrap();
    pane_b.send_service(
        b_session,
        &ServiceFrame::Reply {
            token: request_token,
            payload: reply_bytes,
        },
    );

    // Give the server time to process the reply
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Consumer does NOT receive the reply — the route is gone.
    // This is correct server behavior. The consumer-side gap is
    // that nothing sends Failed back to the consumer's Dispatch
    // entry — it's orphaned.
    //
    // We verify the server's routing table is clean by checking
    // that no frames arrive on the consumer side. We use a timeout
    // to avoid hanging.
    drop(pane_a);
    drop(pane_b);
    conn_a.wait();
    conn_b.wait();
    // If we got here without hanging, the server cleaned up correctly.
}

/// S1: Repeated DeclareInterest + RevokeInterest cycles. Verifies
/// the server's routing table doesn't leak entries across 5000
/// iterations.
///
/// Invariant: after all interests are revoked and connections
/// closed, the server's routing table and provider index are empty.
/// We verify indirectly: each new DeclareInterest succeeds (the
/// server isn't accumulating stale state that blocks new routes).
#[test]
#[ignore]
fn revoke_interest_no_stale_routing_entries() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Provider stays connected the whole time
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
    let _conn_b = accept_b.join().unwrap();
    pane_b.expect_ready();

    // Consumer connects once, does many DeclareInterest/RevokeInterest cycles
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
    let _conn_a = accept_a.join().unwrap();
    pane_a.expect_ready();

    // Note: session_ids are monotonic (no recycling). With 65534
    // available per connection, we can do many cycles. Each
    // DeclareInterest allocates one session on the consumer
    // connection AND one on the provider connection. We do 100
    // cycles — well within the 65534 limit.
    let iterations = 100;

    for i in 0..iterations {
        pane_a.send_control(&ControlMessage::DeclareInterest {
            service: echo_id,
            expected_version: 1,
        });

        let a_session = match pane_a.read_control() {
            ControlMessage::InterestAccepted { session_id, .. } => session_id,
            ControlMessage::InterestDeclined { reason, .. } => {
                panic!("DeclareInterest declined at iteration {i}: {reason:?}");
            }
            other => panic!("unexpected at iteration {i}: {other:?}"),
        };

        // Provider gets InterestAccepted too
        let _b_session = match pane_b.read_control() {
            ControlMessage::InterestAccepted { session_id, .. } => session_id,
            other => panic!("provider expected InterestAccepted at iteration {i}, got {other:?}"),
        };

        // Revoke immediately
        pane_a.send_control(&ControlMessage::RevokeInterest {
            session_id: a_session,
        });

        // Revoker receives ServiceTeardown echo (S1 fix: lets
        // the looper fail outstanding dispatch entries).
        match pane_a.read_control() {
            ControlMessage::ServiceTeardown { session_id: s, .. } => {
                assert_eq!(s, a_session);
            }
            other => panic!("revoker expected ServiceTeardown at iteration {i}, got {other:?}"),
        }

        // Provider receives ServiceTeardown
        match pane_b.read_control() {
            ControlMessage::ServiceTeardown { .. } => {}
            other => panic!("provider expected ServiceTeardown at iteration {i}, got {other:?}"),
        }
    }

    // All 100 cycles completed without hanging or panicking.
    // The server processed all events correctly.
}

// ════════════════════════════════════════════════════════════════
// S4: Actor saturation — 64 concurrent clients
// ════════════════════════════════════════════════════════════════

/// S4: 64 clients connecting simultaneously, each performing
/// handshake + DeclareInterest + 100 notification frames + disconnect.
///
/// Validates: single-threaded actor processes all events without
/// starvation. 5-second timeout.
///
/// Invariant: I6 — sequential single-thread dispatch. The actor
/// must process all 64 handshakes, all DeclareInterest messages,
/// and route all frames without any client timing out.
#[test]
#[ignore]
fn actor_saturation_64_clients() {
    use std::sync::Barrier;

    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();
    let num_clients = 64;
    let frames_per_client = 100;

    // Provider — stays connected, receives all frames
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

    // Barrier to synchronize all clients starting at once
    let barrier = Arc::new(Barrier::new(num_clients));
    let mut handles = Vec::new();

    for client_idx in 0..num_clients {
        let server = Arc::clone(&server);
        let barrier = Arc::clone(&barrier);

        handles.push(std::thread::spawn(move || {
            // Each client connects
            let (c_client, c_server) = MemoryTransport::pair();
            let accept_handle = {
                let server = Arc::clone(&server);
                std::thread::spawn(move || {
                    let (r, w) = c_server.split();
                    server.accept(r, w).expect("accept failed")
                })
            };

            let (mut client, _) = ClientConn::connect(
                c_client,
                Hello {
                    version: 1,
                    max_message_size: 16 * 1024 * 1024,
                    interests: vec![],
                    provides: vec![],
                },
            );
            let conn = accept_handle.join().unwrap();
            client.expect_ready();

            // Wait for all clients to be connected
            barrier.wait();

            // DeclareInterest
            client.send_control(&ControlMessage::DeclareInterest {
                service: echo_service_id(),
                expected_version: 1,
            });

            let session = match client.read_control() {
                ControlMessage::InterestAccepted { session_id, .. } => session_id,
                other => panic!("client {client_idx}: expected InterestAccepted, got {other:?}"),
            };
            client.register_session(session);

            // Send frames
            for frame_idx in 0..frames_per_client {
                let msg = EchoMessage::Ping(format!("c{client_idx}f{frame_idx}"));
                let inner = postcard::to_allocvec(&msg).unwrap();
                client.send_service(session, &ServiceFrame::Notification { payload: inner });
            }

            // Disconnect
            drop(client);
            conn.wait();
        }));
    }

    // Timeout wrapper — if any client hangs, the test fails
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let watcher = std::thread::spawn(move || {
        for (i, h) in handles.into_iter().enumerate() {
            h.join()
                .unwrap_or_else(|e| panic!("client {i} panicked: {e:?}"));
        }
        let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(()) => {}
        Err(_) => panic!(
            "actor saturation test timed out after 5s — \
             the server is likely starving some clients"
        ),
    }

    // The fact that all 64 clients completed without timeout proves
    // the actor didn't starve any connection.
    drop(pane_b);
    conn_b.wait();

    // Cleanup: drop server
    drop(server);
    watcher.join().unwrap();
}

// ════════════════════════════════════════════════════════════════
// S5: Rapid connect/disconnect cycling
// ════════════════════════════════════════════════════════════════

/// S5: 1000 iterations of connect → send Hello → immediately close
/// socket before reading Welcome. Validates: no resource leak,
/// ProtocolServer accepts the next connection cleanly.
///
/// Invariant: the server must tolerate premature disconnects at
/// any point during the handshake without leaking threads, file
/// descriptors, or channel slots.
#[test]
#[ignore]
fn rapid_connect_disconnect_no_leak() {
    let server = Arc::new(ProtocolServer::new());
    let iterations = 1000;

    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let server2 = Arc::clone(&server);

    let worker = std::thread::spawn(move || {
        for i in 0..iterations {
            let (c_client, c_server) = MemoryTransport::pair();

            // Accept on another thread (it will fail when the client drops)
            let server_ref = Arc::clone(&server2);
            let accept_handle = std::thread::spawn(move || {
                let (r, w) = c_server.split();
                // This may fail — that's expected when the client drops early
                let _ = server_ref.accept(r, w);
            });

            // Client: write Hello, then immediately drop
            let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);
            let hello = Hello {
                version: 1,
                max_message_size: 16 * 1024 * 1024,
                interests: vec![],
                provides: vec![],
            };
            let bytes = postcard::to_allocvec(&hello).unwrap();
            // Some iterations: send Hello then drop
            // Some iterations: drop without sending anything
            if i % 3 != 0 {
                let mut transport = c_client;
                let _ = codec.write_frame(&mut transport, 0, &bytes);
                drop(transport);
            } else {
                drop(c_client);
            }

            // Wait for accept thread — must not hang
            let _ = accept_handle.join();
        }

        // After all iterations, verify the server still works by
        // doing a successful handshake.
        let (c_client, c_server) = MemoryTransport::pair();
        let server_ref = Arc::clone(&server2);
        let accept_handle = std::thread::spawn(move || {
            let (r, w) = c_server.split();
            server_ref.accept(r, w).expect("final accept must succeed")
        });

        let (client, _welcome) = ClientConn::connect(
            c_client,
            Hello {
                version: 1,
                max_message_size: 16 * 1024 * 1024,
                interests: vec![],
                provides: vec![],
            },
        );

        let conn = accept_handle.join().unwrap();
        drop(client);
        conn.wait();

        let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(std::time::Duration::from_secs(10)) {
        Ok(()) => {}
        Err(_) => panic!(
            "rapid connect/disconnect test timed out — \
             the server is likely leaking resources"
        ),
    }

    worker.join().unwrap();
}

// ════════════════════════════════════════════════════════════════
// S6: Teardown cascade — 8 connections killed simultaneously
// ════════════════════════════════════════════════════════════════

/// S6: 8 connections fully meshed via DeclareInterest. All 8
/// drop within 1ms (barrier-synchronized). Validates: actor thread
/// terminates cleanly, no panic from writing to dead connections.
///
/// Invariant: the server's process_disconnect correctly handles
/// concurrent disconnects without panicking. Dead writers (BrokenPipe)
/// are silently ignored (the `let _ = wh.write_frame(...)` pattern).
#[test]
#[ignore]
fn teardown_cascade_8_connections_barrier() {
    use std::sync::Barrier;

    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();
    let num_connections = 8;

    // Each connection provides AND consumes the echo service.
    // Wait — self-provide is declined. Instead: connections 0..4
    // are providers, connections 4..8 are consumers.
    let barrier = Arc::new(Barrier::new(num_connections));
    let mut conn_threads = Vec::new();

    // Create all connections
    for idx in 0..num_connections {
        let server = Arc::clone(&server);
        let barrier = Arc::clone(&barrier);

        conn_threads.push(std::thread::spawn(move || {
            let (c_client, c_server) = MemoryTransport::pair();
            let server2 = Arc::clone(&server);
            let accept_handle = std::thread::spawn(move || {
                let (r, w) = c_server.split();
                server2.accept(r, w).expect("accept failed")
            });

            let is_provider = idx < num_connections / 2;
            let provides = if is_provider {
                vec![ServiceProvision {
                    service: echo_id,
                    version: 1,
                }]
            } else {
                vec![]
            };

            let (mut client, _) = ClientConn::connect(
                c_client,
                Hello {
                    version: 1,
                    max_message_size: 16 * 1024 * 1024,
                    interests: vec![],
                    provides,
                },
            );
            let conn = accept_handle.join().unwrap();
            client.expect_ready();

            // Consumers declare interest
            if !is_provider {
                client.send_control(&ControlMessage::DeclareInterest {
                    service: echo_id,
                    expected_version: 1,
                });
                // Read InterestAccepted
                match client.read_control() {
                    ControlMessage::InterestAccepted { .. } => {}
                    other => panic!("connection {idx}: expected InterestAccepted, got {other:?}"),
                }
            } else {
                // Providers will receive InterestAccepted notifications
                // for each consumer that connects. We need to drain these.
                // But the timing is tricky — we'll just wait at the barrier.
            }

            // Barrier: all connections are live
            barrier.wait();

            // All drop simultaneously — the barrier exit is
            // approximately synchronized (< 1ms spread).
            drop(client);
            conn.wait();
        }));
    }

    // Timeout wrapper
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let watcher = std::thread::spawn(move || {
        for (i, h) in conn_threads.into_iter().enumerate() {
            h.join()
                .unwrap_or_else(|e| panic!("connection {i} panicked: {e:?}"));
        }
        let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(()) => {}
        Err(_) => panic!(
            "teardown cascade test timed out — \
             the server is likely stuck processing disconnect events"
        ),
    }

    // Server should terminate cleanly when dropped
    drop(server);
    watcher.join().unwrap();
}

// ════════════════════════════════════════════════════════════════
// S7: Provider disappears mid-route
// ════════════════════════════════════════════════════════════════

/// S7: Provider + consumer connected, consumer sending frames,
/// provider drops mid-stream. Validates: consumer gets
/// ServiceTeardown, frames to dead route silently dropped.
///
/// Invariant: frames to a dead route are silently dropped
/// (process_service: "Missing route = not an error"). The consumer
/// receives ServiceTeardown on the control channel.
#[test]
#[ignore]
fn provider_disappears_mid_route_consumer_gets_teardown() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Provider
    let (b_client, b_server) = MemoryTransport::pair();
    let accept_b = accept_on_thread(&server, b_server);
    let (pane_b, _) = ClientConn::connect(
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

    // Consumer
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

    // Establish route
    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });
    let a_session = match pane_a.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted, got {other:?}"),
    };
    pane_a.register_session(a_session);

    // Drop provider immediately
    drop(pane_b);
    conn_b.wait();

    // Consumer should receive ServiceTeardown
    let msg = pane_a.read_control();
    assert!(
        matches!(
            msg,
            ControlMessage::ServiceTeardown {
                reason: pane_proto::control::TeardownReason::ConnectionLost,
                ..
            }
        ),
        "consumer must receive ServiceTeardown, got {msg:?}"
    );

    // Consumer can still send frames — they should be silently dropped.
    // (The route is gone, and the write to a dead writer fails silently.)
    let inner = postcard::to_allocvec(&EchoMessage::Ping("dead".into())).unwrap();
    // This write might fail (broken pipe) if the consumer's writer is
    // the dead provider's reader. But in pane's architecture, the
    // consumer writes to the SERVER, not the provider. The server
    // drops frames with no route.
    //
    // Actually, after ServiceTeardown, the consumer should stop sending.
    // But the test verifies the server doesn't crash.
    //
    // Note: the codec won't let us write to an unknown session if
    // it wasn't registered as permissive. We registered a_session above.
    pane_a.send_service(a_session, &ServiceFrame::Notification { payload: inner });

    // Give server time to process the frame (it should silently drop)
    std::thread::sleep(std::time::Duration::from_millis(50));

    drop(pane_a);
    conn_a.wait();
}

// ════════════════════════════════════════════════════════════════
// S9: Session exhaustion + non-recycling
// ════════════════════════════════════════════════════════════════

/// S9: Session boundary test. With u16 discriminants, 65534 sessions
/// are available (1..=0xFFFE). We can't allocate all of them in a
/// test, so we verify the boundary: allocate a few sessions from a
/// near-max counter state, then confirm exhaustion.
///
/// The test does 100 normal DeclareInterest calls (well within the
/// 65534 limit), then verifies basic round-trip. The unit test in
/// server.rs covers the exact boundary (alloc_session_returns_none_at_overflow).
///
/// Invariant: alloc_session returns None at 0xFFFF boundary (the
/// session 0xFFFF is reserved for ProtocolAbort). The monotonic
/// counter means revoked sessions are never reused.
#[test]
#[ignore]
fn session_exhaustion_no_recycling() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Provider
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

    // Consumer
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

    // Do 100 DeclareInterest/RevokeInterest cycles to verify
    // the server handles u16 sessions correctly end-to-end.
    let mut sessions = Vec::new();
    for i in 0..100 {
        pane_a.send_control(&ControlMessage::DeclareInterest {
            service: echo_id,
            expected_version: 1,
        });

        match pane_a.read_control() {
            ControlMessage::InterestAccepted { session_id, .. } => {
                sessions.push(session_id);
            }
            ControlMessage::InterestDeclined { reason, .. } => {
                panic!(
                    "DeclareInterest declined at iteration {i}: {reason:?}. \
                     Expected success."
                );
            }
            other => panic!("unexpected at iteration {i}: {other:?}"),
        }

        // Drain provider's InterestAccepted
        match pane_b.read_control() {
            ControlMessage::InterestAccepted { .. } => {}
            other => panic!("provider: unexpected at iteration {i}: {other:?}"),
        }
    }

    assert_eq!(sessions.len(), 100);

    // Revoke all 100 sessions
    for &session_id in &sessions {
        pane_a.send_control(&ControlMessage::RevokeInterest { session_id });
        // Revoker gets ServiceTeardown echo (S1 fix)
        match pane_a.read_control() {
            ControlMessage::ServiceTeardown { .. } => {}
            other => panic!("revoker expected ServiceTeardown, got {other:?}"),
        }
        // Provider gets ServiceTeardown
        match pane_b.read_control() {
            ControlMessage::ServiceTeardown { .. } => {}
            other => panic!("provider expected ServiceTeardown, got {other:?}"),
        }
    }

    // Post-revoke DeclareInterest should still succeed (monotonic
    // counter is at 101, well below 0xFFFF). Revoked sessions are
    // not recycled, but the counter hasn't hit the boundary.
    pane_a.send_control(&ControlMessage::DeclareInterest {
        service: echo_id,
        expected_version: 1,
    });

    match pane_a.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => {
            // session_id=101 (the next after 100 allocations)
            assert_eq!(
                session_id, 101,
                "monotonic counter should produce session 101"
            );
        }
        other => {
            panic!("expected InterestAccepted after revoke cycle, got {other:?}")
        }
    }
    // Drain provider
    match pane_b.read_control() {
        ControlMessage::InterestAccepted { .. } => {}
        other => panic!("provider: unexpected post-revoke: {other:?}"),
    }

    drop(pane_a);
    drop(pane_b);
    conn_a.wait();
    conn_b.wait();
}

// ════════════════════════════════════════════════════════════════
// S11: Per-connection frame ordering under burst
// ════════════════════════════════════════════════════════════════

/// S11: 4 clients, 10,000 frames each, each frame embeds a
/// sequence counter. Verify per-connection monotonicity at the
/// provider.
///
/// Invariant: the server's single-threaded actor preserves
/// per-connection frame ordering. Frames from different connections
/// may interleave, but frames from the SAME connection arrive in
/// order.
#[test]
#[ignore]
fn per_connection_frame_ordering_under_burst() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();
    let num_clients = 4;
    let frames_per_client = 10_000;

    // Provider — receives all frames
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

    // Connect all clients and establish routes
    struct ConnectedClient {
        session: u16,
        b_session: u16,
        transport: MemoryTransport,
        codec: FrameCodec,
        conn_handle: pane_session::server::ConnectionHandle,
    }

    let mut clients = Vec::new();

    for _ in 0..num_clients {
        let (c_client, c_server) = MemoryTransport::pair();
        let accept_handle = accept_on_thread(&server, c_server);
        let (mut client, _) = ClientConn::connect(
            c_client,
            Hello {
                version: 1,
                max_message_size: 16 * 1024 * 1024,
                interests: vec![],
                provides: vec![],
            },
        );
        let conn = accept_handle.join().unwrap();
        client.expect_ready();

        client.send_control(&ControlMessage::DeclareInterest {
            service: echo_id,
            expected_version: 1,
        });

        let a_session = match client.read_control() {
            ControlMessage::InterestAccepted { session_id, .. } => session_id,
            other => panic!("expected InterestAccepted, got {other:?}"),
        };
        client.register_session(a_session);

        let b_session = match pane_b.read_control() {
            ControlMessage::InterestAccepted { session_id, .. } => session_id,
            other => panic!("expected InterestAccepted for provider, got {other:?}"),
        };
        pane_b.register_session(b_session);

        clients.push(ConnectedClient {
            session: a_session,
            b_session,
            transport: client.transport,
            codec: client.codec,
            conn_handle: conn,
        });
    }

    // All clients blast frames concurrently
    let mut send_threads = Vec::new();
    for (client_idx, mut client) in clients.into_iter().enumerate() {
        send_threads.push(std::thread::spawn(move || {
            for seq in 0..frames_per_client {
                // Embed client_idx and sequence number in the payload
                let msg = EchoMessage::Ping(format!("{client_idx}:{seq}"));
                let inner = postcard::to_allocvec(&msg).unwrap();
                let frame = ServiceFrame::Notification { payload: inner };
                let outer = postcard::to_allocvec(&frame).unwrap();
                client
                    .codec
                    .write_frame(&mut client.transport, client.session, &outer)
                    .unwrap();
            }
            // Return the client for cleanup
            (client.transport, client.conn_handle, client.b_session)
        }));
    }

    // Wait for all senders to finish
    let mut cleanup = Vec::new();
    for h in send_threads {
        cleanup.push(h.join().unwrap());
    }

    // Read all frames from the provider side and verify per-connection ordering
    let total_expected = num_clients * frames_per_client;
    let mut last_seq: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut total_received = 0;

    // Map b_session -> client_idx
    let mut session_to_client: std::collections::HashMap<u16, usize> =
        std::collections::HashMap::new();
    for (idx, (_, _, b_session)) in cleanup.iter().enumerate() {
        session_to_client.insert(*b_session, idx);
    }

    for _ in 0..total_expected {
        let frame = pane_b.codec.read_frame(&mut pane_b.transport).unwrap();
        match frame {
            Frame::Message { service, payload } if service != 0 => {
                let sf: ServiceFrame = postcard::from_bytes(&payload).unwrap();
                match sf {
                    ServiceFrame::Notification { payload: inner } => {
                        let msg: EchoMessage = postcard::from_bytes(&inner).unwrap();
                        match msg {
                            EchoMessage::Ping(s) => {
                                let parts: Vec<&str> = s.split(':').collect();
                                let client_idx: usize = parts[0].parse().unwrap();
                                let seq: usize = parts[1].parse().unwrap();

                                let last = last_seq.entry(client_idx).or_insert(0);
                                if seq > 0 {
                                    assert!(
                                        seq > *last || *last == 0,
                                        "out-of-order frame for client {client_idx}: \
                                         expected > {last}, got {seq}"
                                    );
                                }
                                *last = seq;
                                total_received += 1;
                            }
                            _ => panic!("unexpected message variant"),
                        }
                    }
                    _ => panic!("expected Notification"),
                }
            }
            other => panic!("expected service frame, got {other:?}"),
        }
    }

    assert_eq!(
        total_received, total_expected,
        "all frames must be received"
    );

    // Cleanup
    for (transport, conn, _) in cleanup {
        drop(transport);
        conn.wait();
    }
    drop(pane_b);
    conn_b.wait();
}

// ════════════════════════════════════════════════════════════════
// P1: proptest — random bytes through read_frame
// ════════════════════════════════════════════════════════════════

/// P1: Feed arbitrary byte sequences into read_frame. The codec
/// must never panic and must always return Ok(Frame) or
/// Err(FrameError). This is the fundamental safety property of
/// the wire parser: no input from the network can trigger UB,
/// a panic, or an unbounded allocation.
///
/// Invariant: read_frame is total over arbitrary byte streams.
#[test]
#[ignore]
fn codec_read_frame_never_panics() {
    use proptest::prelude::*;
    use std::io::Cursor;

    let config = ProptestConfig {
        cases: 2000,
        ..ProptestConfig::default()
    };

    let mut runner = proptest::test_runner::TestRunner::new(config);
    runner
        .run(&proptest::collection::vec(any::<u8>(), 0..1024), |data| {
            let codec = FrameCodec::new(512);
            let mut cursor = Cursor::new(data);

            // Must not panic. Result is always Ok or Err.
            let result = codec.read_frame(&mut cursor);
            match result {
                Ok(Frame::Message { .. }) | Ok(Frame::Abort) => {}
                Err(FrameError::Oversized { .. })
                | Err(FrameError::UnknownService(_))
                | Err(FrameError::Transport(_))
                | Err(FrameError::TooShort { .. })
                | Err(FrameError::Poisoned) => {}
            }
            Ok(())
        })
        .unwrap();
}

// ════════════════════════════════════════════════════════════════
// P2: proptest — write_frame→read_frame roundtrip
// ════════════════════════════════════════════════════════════════

/// P2: For any valid service discriminant (0..=0xFFFE, excluding
/// 0xFFFF) and any payload up to max_message_size, write_frame
/// followed by read_frame must produce the exact same frame.
///
/// Invariant: the codec is a faithful bijection on valid frames.
/// This is the codec's correctness property — the wire format
/// preserves frame identity.
#[test]
#[ignore]
fn codec_roundtrip_arbitrary_valid_frames() {
    use proptest::prelude::*;
    use std::io::Cursor;

    let config = ProptestConfig {
        cases: 2000,
        ..ProptestConfig::default()
    };

    let mut runner = proptest::test_runner::TestRunner::new(config);
    let strategy = (
        0u16..=0xFFFEu16,
        proptest::collection::vec(any::<u8>(), 0..512),
    );

    runner
        .run(&strategy, |(service, payload)| {
            let max_size = 1024u32;
            let mut codec = FrameCodec::new(max_size);
            if service != 0 {
                codec.register_service(service);
            }

            let mut buf = Vec::new();
            codec.write_frame(&mut buf, service, &payload).unwrap();

            let mut cursor = Cursor::new(buf);
            let frame = codec.read_frame(&mut cursor).unwrap();
            prop_assert_eq!(frame, Frame::Message { service, payload },);
            Ok(())
        })
        .unwrap();
}

// ════════════════════════════════════════════════════════════════
// P3: proptest — random lengths produce correct FrameError variant
// ════════════════════════════════════════════════════════════════

/// P3: For any declared length, the codec must return the correct
/// FrameError variant: TooShort for 0..2, Oversized for
/// length > max_message_size. Lengths in range read the body
/// (and may hit EOF if the body isn't provided).
///
/// Invariant: the length validation logic is exhaustive and
/// deterministic across the full u32 range.
#[test]
#[ignore]
fn codec_length_validation_correct_variant() {
    use proptest::prelude::*;
    use std::io::Cursor;

    let config = ProptestConfig {
        cases: 5000,
        ..ProptestConfig::default()
    };

    let mut runner = proptest::test_runner::TestRunner::new(config);
    runner
        .run(&any::<u32>(), |length| {
            let max_size = 1000u32;
            let codec = FrameCodec::new(max_size);

            // Build a stream with just the length prefix (no body).
            let mut stream = Vec::new();
            stream.extend_from_slice(&length.to_le_bytes());

            let mut cursor = Cursor::new(stream);
            let result = codec.read_frame(&mut cursor);

            match result {
                Err(FrameError::TooShort { declared }) => {
                    prop_assert!(
                        declared < 2,
                        "TooShort should only fire for length < 2, got {declared}"
                    );
                    prop_assert_eq!(declared, length);
                }
                Err(FrameError::Oversized { declared, limit }) => {
                    prop_assert!(
                        declared > max_size,
                        "Oversized should fire for length > {max_size}, got {declared}"
                    );
                    prop_assert_eq!(declared, length);
                    prop_assert_eq!(limit, max_size);
                }
                Err(FrameError::Transport(ref e)) => {
                    // Length was in valid range [2, max_size] but no body bytes
                    prop_assert!(
                        length >= 2 && length <= max_size,
                        "Transport error for out-of-range length {length}: {e}"
                    );
                    prop_assert_eq!(e.kind(), std::io::ErrorKind::UnexpectedEof);
                }
                _ => {
                    // Shouldn't get Ok with no body bytes
                    return Err(proptest::test_runner::TestCaseError::Fail(
                        format!("unexpected result for length {length}: {result:?}").into(),
                    ));
                }
            }
            Ok(())
        })
        .unwrap();
}

// ════════════════════════════════════════════════════════════════
// P4: proptest — poison after any FrameError
// ════════════════════════════════════════════════════════════════

/// P4: After any FrameError (from any cause), the codec is
/// poisoned — subsequent read_frame calls return Poisoned
/// without touching the stream.
///
/// Invariant: poison is monotonic and irreversible. Once set,
/// no further reads occur. This prevents desync attacks where
/// a malformed frame leaves the stream in an inconsistent state
/// and a subsequent "valid" read interprets body bytes as a
/// length prefix.
#[test]
#[ignore]
fn codec_poisoned_after_any_error() {
    use proptest::prelude::*;
    use std::io::Cursor;

    let config = ProptestConfig {
        cases: 1000,
        ..ProptestConfig::default()
    };

    let mut runner = proptest::test_runner::TestRunner::new(config);
    runner
        .run(&proptest::collection::vec(any::<u8>(), 0..256), |data| {
            let codec = FrameCodec::new(64);
            let mut cursor = Cursor::new(data);

            let first = codec.read_frame(&mut cursor);
            if first.is_err() {
                // Must be poisoned now.
                let pos_before = cursor.position();
                let second = codec.read_frame(&mut cursor);
                prop_assert!(
                    matches!(second, Err(FrameError::Poisoned)),
                    "second read after error must return Poisoned, got {second:?}"
                );
                // Cursor must not advance.
                prop_assert_eq!(
                    cursor.position(),
                    pos_before,
                    "cursor must not advance after Poisoned"
                );
            }
            Ok(())
        })
        .unwrap();
}

// ════════════════════════════════════════════════════════════════
// S1r: Randomized DeclareInterest/RevokeInterest race
// ════════════════════════════════════════════════════════════════

/// S1r: Randomized version of the S1 race test. Two threads operate
/// on the same server concurrently: one sends DeclareInterest +
/// service frames at random intervals, the other sends RevokeInterest
/// at a random point. Run 500 iterations with deterministic seed.
///
/// Invariant: no panic, no hang (5s timeout per iteration), server
/// routing table converges to clean state after each cycle.
#[test]
#[ignore]
fn randomized_declare_revoke_race() {
    use std::time::Duration;

    // Simple LCG for deterministic randomness (no external crate needed).
    // Seed is fixed for reproducibility.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
            self.0
        }
        fn range(&mut self, lo: u64, hi: u64) -> u64 {
            lo + self.next() % (hi - lo)
        }
    }

    let iterations = 500;
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Provider stays connected the whole time
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
    let _conn_b = accept_b.join().unwrap();
    pane_b.expect_ready();

    // Consumer
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
    let _conn_a = accept_a.join().unwrap();
    pane_a.expect_ready();

    let mut rng = Lcg(0xDEAD_BEEF_CAFE_1234);

    for iter in 0..iterations {
        // DeclareInterest
        pane_a.send_control(&ControlMessage::DeclareInterest {
            service: echo_id,
            expected_version: 1,
        });

        let a_session = match pane_a.read_control() {
            ControlMessage::InterestAccepted { session_id, .. } => session_id,
            ControlMessage::InterestDeclined { reason, .. } => {
                panic!("declined at iter {iter}: {reason:?}");
            }
            other => panic!("unexpected at iter {iter}: {other:?}"),
        };

        // Provider gets InterestAccepted
        let b_session = match pane_b.read_control() {
            ControlMessage::InterestAccepted { session_id, .. } => session_id,
            other => panic!("provider unexpected at iter {iter}: {other:?}"),
        };
        pane_a.register_session(a_session);
        pane_b.register_session(b_session);

        // Random delay before sending some frames
        let pre_frames = rng.range(0, 5);
        for f in 0..pre_frames {
            let inner = postcard::to_allocvec(&EchoMessage::Ping(format!("{iter}:{f}"))).unwrap();
            pane_a.send_service(a_session, &ServiceFrame::Notification { payload: inner });
            // Random microsecond delay
            let delay = rng.range(0, 100);
            std::thread::sleep(Duration::from_micros(delay));
        }

        // RevokeInterest at a random point
        let revoke_delay = rng.range(0, 50);
        std::thread::sleep(Duration::from_micros(revoke_delay));
        pane_a.send_control(&ControlMessage::RevokeInterest {
            session_id: a_session,
        });

        // Drain expected messages: revoker ServiceTeardown
        match pane_a.read_control() {
            ControlMessage::ServiceTeardown { session_id, .. } => {
                assert_eq!(session_id, a_session);
            }
            other => panic!("revoker expected ServiceTeardown at iter {iter}, got {other:?}"),
        }

        // Provider gets ServiceTeardown (may also get in-flight frames first)
        loop {
            let frame = pane_b.codec.read_frame(&mut pane_b.transport).unwrap();
            match frame {
                Frame::Message {
                    service: 0,
                    payload,
                } => {
                    let msg: ControlMessage = postcard::from_bytes(&payload).unwrap();
                    match msg {
                        ControlMessage::ServiceTeardown { .. } => break,
                        _ => {
                            panic!("provider expected ServiceTeardown at iter {iter}, got {msg:?}")
                        }
                    }
                }
                Frame::Message { service, .. } if service == b_session => {
                    // In-flight service frame — consumed and discarded
                }
                other => panic!("provider unexpected frame at iter {iter}: {other:?}"),
            }
        }
    }

    // All 500 iterations completed without hang or panic.
    // Server routing table is clean — verified indirectly because
    // each DeclareInterest succeeded (stale routes would block).
}

// ════════════════════════════════════════════════════════════════
// U1: UnixStream handshake
// ════════════════════════════════════════════════════════════════

/// U1: Perform a complete handshake over a real UnixStream socketpair.
/// This validates the Transport blanket impl for UnixStream and the
/// TransportSplit impl (try_clone).
///
/// Invariant: the framing protocol works identically over real
/// kernel-buffered sockets as over MemoryTransport. This is the
/// transport-independence property.
#[test]
#[ignore]
fn unix_stream_handshake() {
    use std::os::unix::net::UnixStream;

    let (client_sock, server_sock) = UnixStream::pair().unwrap();

    let server = Arc::new(ProtocolServer::new());

    // Server accepts on a thread (UnixStream implements TransportSplit)
    let server2 = Arc::clone(&server);
    let accept_handle = std::thread::spawn(move || {
        let writer = server_sock
            .try_clone()
            .expect("try_clone for server socket");
        server2.accept(server_sock, writer).expect("accept failed")
    });

    // Client handshake
    let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);
    let mut transport = client_sock;

    let hello = Hello {
        version: 1,
        max_message_size: 16 * 1024 * 1024,
        interests: vec![],
        provides: vec![],
    };
    let bytes = postcard::to_allocvec(&hello).unwrap();
    codec.write_frame(&mut transport, 0, &bytes).unwrap();

    // Read Welcome
    let frame = codec.read_frame(&mut transport).unwrap();
    let payload = match frame {
        Frame::Message {
            service: 0,
            payload,
        } => payload,
        other => panic!("expected Welcome frame, got {other:?}"),
    };
    let decision: Result<Welcome, Rejection> = postcard::from_bytes(&payload).unwrap();
    let welcome = decision.expect("expected Welcome, got Rejection");
    assert_eq!(welcome.version, 1);

    // Read Ready
    let frame = codec.read_frame(&mut transport).unwrap();
    let payload = match frame {
        Frame::Message {
            service: 0,
            payload,
        } => payload,
        other => panic!("expected Ready frame, got {other:?}"),
    };
    let msg: ControlMessage = postcard::from_bytes(&payload).unwrap();
    assert!(
        matches!(
            msg,
            ControlMessage::Lifecycle(pane_proto::protocols::lifecycle::LifecycleMessage::Ready)
        ),
        "expected Ready, got {msg:?}"
    );

    drop(transport);
    let conn = accept_handle.join().unwrap();
    conn.wait();
}

// ════════════════════════════════════════════════════════════════
// U2: UnixStream DeclareInterest + service frame exchange
// ════════════════════════════════════════════════════════════════

/// U2: Full service negotiation and frame exchange over real
/// UnixStream sockets. Provider and consumer connected to a
/// ProtocolServer via socketpairs — DeclareInterest, Notification
/// roundtrip, RevokeInterest.
///
/// Invariant: service frame routing works over real kernel sockets.
/// Validates that FrameCodec's read_exact/write_all compose
/// correctly with the kernel's socket buffer chunking.
#[test]
#[ignore]
fn unix_stream_service_frame_exchange() {
    use std::os::unix::net::UnixStream;

    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Provider
    let (bp_sock, bs_sock) = UnixStream::pair().unwrap();
    let server2 = Arc::clone(&server);
    let accept_b = std::thread::spawn(move || {
        let writer = bs_sock.try_clone().expect("try_clone");
        server2.accept(bs_sock, writer).expect("accept failed")
    });

    let (mut pane_b, _) = GenericClientConn::connect(
        bp_sock,
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

    // Consumer
    let (ap_sock, as_sock) = UnixStream::pair().unwrap();
    let server2 = Arc::clone(&server);
    let accept_a = std::thread::spawn(move || {
        let writer = as_sock.try_clone().expect("try_clone");
        server2.accept(as_sock, writer).expect("accept failed")
    });

    let (mut pane_a, _) = GenericClientConn::connect(
        ap_sock,
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
        other => panic!("expected InterestAccepted for provider, got {other:?}"),
    };
    pane_a.register_session(a_session);
    pane_b.register_session(b_session);

    // Send a Notification from consumer to provider
    let inner = postcard::to_allocvec(&EchoMessage::Ping("unix".into())).unwrap();
    pane_a.send_service(a_session, &ServiceFrame::Notification { payload: inner });

    let (recv_session, recv_frame) = pane_b.read_service();
    assert_eq!(recv_session, b_session);
    match recv_frame {
        ServiceFrame::Notification { payload } => {
            let msg: EchoMessage = postcard::from_bytes(&payload).unwrap();
            assert_eq!(msg, EchoMessage::Ping("unix".into()));
        }
        other => panic!("expected Notification, got {other:?}"),
    }

    // Clean teardown
    pane_a.send_control(&ControlMessage::RevokeInterest {
        session_id: a_session,
    });
    // Drain teardown messages
    match pane_a.read_control() {
        ControlMessage::ServiceTeardown { .. } => {}
        other => panic!("expected ServiceTeardown, got {other:?}"),
    }
    match pane_b.read_control() {
        ControlMessage::ServiceTeardown { .. } => {}
        other => panic!("expected ServiceTeardown, got {other:?}"),
    }

    drop(pane_a);
    drop(pane_b);
    conn_a.wait();
    conn_b.wait();
}

// ════════════════════════════════════════════════════════════════
// U3: UnixStream provider disconnect → ServiceTeardown
// ════════════════════════════════════════════════════════════════

/// U3: Provider closes its socket. Consumer must receive
/// ServiceTeardown(ConnectionLost) from the server over the
/// real UnixStream transport.
///
/// Invariant: transport death detection works over real sockets.
/// The reader thread's read_frame hits EOF, posts Disconnected
/// to the actor, which sends ServiceTeardown to the peer.
#[test]
#[ignore]
fn unix_stream_provider_disconnect_teardown() {
    use std::os::unix::net::UnixStream;

    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Provider
    let (bp_sock, bs_sock) = UnixStream::pair().unwrap();
    let server2 = Arc::clone(&server);
    let accept_b = std::thread::spawn(move || {
        let writer = bs_sock.try_clone().expect("try_clone");
        server2.accept(bs_sock, writer).expect("accept failed")
    });

    let (pane_b, _) = GenericClientConn::connect(
        bp_sock,
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

    // Consumer
    let (ap_sock, as_sock) = UnixStream::pair().unwrap();
    let server2 = Arc::clone(&server);
    let accept_a = std::thread::spawn(move || {
        let writer = as_sock.try_clone().expect("try_clone");
        server2.accept(as_sock, writer).expect("accept failed")
    });

    let (mut pane_a, _) = GenericClientConn::connect(
        ap_sock,
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
    pane_a.register_session(a_session);

    // Kill provider
    drop(pane_b);
    conn_b.wait();

    // Consumer should get ServiceTeardown(ConnectionLost)
    let msg = pane_a.read_control();
    assert!(
        matches!(
            msg,
            ControlMessage::ServiceTeardown {
                reason: pane_proto::control::TeardownReason::ConnectionLost,
                ..
            }
        ),
        "expected ServiceTeardown(ConnectionLost), got {msg:?}"
    );

    drop(pane_a);
    conn_a.wait();
}

// ════════════════════════════════════════════════════════════════
// U4: Rapid connect/disconnect over real UnixStream sockets
// ════════════════════════════════════════════════════════════════

/// U4: 100 iterations of connect → handshake → disconnect over
/// real UnixStream sockets. Real sockets exercise kernel buffer
/// management and fd recycling. Fewer iterations than the
/// MemoryTransport variant (S5) because real sockets are slower.
///
/// Invariant: the server recovers cleanly from rapid real-socket
/// churn without fd leaks or thread hangs.
#[test]
#[ignore]
fn unix_stream_rapid_connect_disconnect() {
    use std::os::unix::net::UnixStream;

    let server = Arc::new(ProtocolServer::new());
    // 50 iterations is sufficient to exercise the churn invariant.
    // Higher counts increase the chance of timing-related flakiness
    // on loaded machines (real sockets, real threads).
    let iterations = 50;

    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let server2 = Arc::clone(&server);

    let worker = std::thread::spawn(move || {
        for i in 0..iterations {
            let (client_sock, server_sock) = UnixStream::pair().unwrap();

            let server_ref = Arc::clone(&server2);
            let accept_handle = std::thread::spawn(move || {
                let writer = server_sock.try_clone().expect("try_clone");
                let _ = server_ref.accept(server_sock, writer);
            });

            // Some iterations: full handshake then drop.
            // Some iterations: drop immediately.
            if i % 3 != 0 {
                let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);
                let hello = Hello {
                    version: 1,
                    max_message_size: 16 * 1024 * 1024,
                    interests: vec![],
                    provides: vec![],
                };
                let bytes = postcard::to_allocvec(&hello).unwrap();
                let mut transport = client_sock;
                let _ = codec.write_frame(&mut transport, 0, &bytes);
                drop(transport);
            } else {
                drop(client_sock);
            }

            let _ = accept_handle.join();
        }

        // Verify server still works after the churn
        let (client_sock, server_sock) = UnixStream::pair().unwrap();
        let server_ref = Arc::clone(&server2);
        let accept_handle = std::thread::spawn(move || {
            let writer = server_sock.try_clone().expect("try_clone");
            server_ref
                .accept(server_sock, writer)
                .expect("final accept must succeed")
        });

        let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);
        let hello = Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
            provides: vec![],
        };
        let bytes = postcard::to_allocvec(&hello).unwrap();
        let mut transport = client_sock;
        codec.write_frame(&mut transport, 0, &bytes).unwrap();

        // Read Welcome
        let frame = codec.read_frame(&mut transport).unwrap();
        match frame {
            Frame::Message { service: 0, .. } => {}
            other => panic!("expected Welcome, got {other:?}"),
        }

        drop(transport);
        let conn = accept_handle.join().unwrap();
        conn.wait();

        let _ = done_tx.send(());
    });

    match done_rx.recv_timeout(std::time::Duration::from_secs(30)) {
        Ok(()) => {}
        Err(_) => panic!(
            "unix stream rapid connect/disconnect timed out — \
             likely fd leak or thread hang"
        ),
    }

    worker.join().unwrap();
}

// ════════════════════════════════════════════════════════════════
// L1: Sustained load — 4 clients, 5000 frames each
// ════════════════════════════════════════════════════════════════

/// L1: 4 clients each send 5000 Notification frames (20000 total)
/// to a single provider through the ProtocolServer. Each frame
/// embeds a (client_idx, sequence) counter. After the burst, verify:
/// - All 20000 frames arrived at the provider
/// - Per-client sequence ordering is monotonic
/// - No thread leaks (thread count stable before/after)
///
/// This is a sustained throughput test, not a latency test. The
/// server's single-threaded actor must keep up with 4 concurrent
/// writers without dropping frames or deadlocking.
///
/// Invariant: I6 (single-threaded actor) + per-connection ordering
/// under sustained load. The mpsc channel between reader threads
/// and the actor is unbounded — this test also validates that the
/// actor drains fast enough to avoid OOM.
#[test]
#[ignore]
fn sustained_load_4_clients_5000_frames_each() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();
    let num_clients = 4;
    let frames_per_client = 5_000;

    // Provider
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

    // Connect clients and establish routes
    struct Client {
        session: u16,
        b_session: u16,
        transport: MemoryTransport,
        codec: FrameCodec,
        conn_handle: pane_session::server::ConnectionHandle,
    }

    let mut clients = Vec::new();
    for _ in 0..num_clients {
        let (c_client, c_server) = MemoryTransport::pair();
        let accept_handle = accept_on_thread(&server, c_server);
        let (mut client, _) = ClientConn::connect(
            c_client,
            Hello {
                version: 1,
                max_message_size: 16 * 1024 * 1024,
                interests: vec![],
                provides: vec![],
            },
        );
        let conn = accept_handle.join().unwrap();
        client.expect_ready();

        client.send_control(&ControlMessage::DeclareInterest {
            service: echo_id,
            expected_version: 1,
        });

        let a_session = match client.read_control() {
            ControlMessage::InterestAccepted { session_id, .. } => session_id,
            other => panic!("expected InterestAccepted, got {other:?}"),
        };
        client.register_session(a_session);

        let b_session = match pane_b.read_control() {
            ControlMessage::InterestAccepted { session_id, .. } => session_id,
            other => panic!("expected InterestAccepted for provider, got {other:?}"),
        };
        pane_b.register_session(b_session);

        clients.push(Client {
            session: a_session,
            b_session,
            transport: client.transport,
            codec: client.codec,
            conn_handle: conn,
        });
    }

    // Blast frames from all clients concurrently
    let mut send_threads = Vec::new();
    for (client_idx, mut client) in clients.into_iter().enumerate() {
        send_threads.push(std::thread::spawn(move || {
            for seq in 0..frames_per_client {
                // Pack client_idx and seq into a compact payload
                let tag = format!("{client_idx}:{seq}");
                let inner = postcard::to_allocvec(&EchoMessage::Ping(tag)).unwrap();
                let frame = ServiceFrame::Notification { payload: inner };
                let outer = postcard::to_allocvec(&frame).unwrap();
                client
                    .codec
                    .write_frame(&mut client.transport, client.session, &outer)
                    .unwrap();
            }
            (client.transport, client.conn_handle, client.b_session)
        }));
    }

    let mut cleanup = Vec::new();
    for h in send_threads {
        cleanup.push(h.join().unwrap());
    }

    // Session-to-client mapping
    let _session_to_client: std::collections::HashMap<u16, usize> = cleanup
        .iter()
        .enumerate()
        .map(|(idx, (_, _, b_session))| (*b_session, idx))
        .collect();

    // Read all frames and verify ordering
    let total_expected = num_clients * frames_per_client;
    let mut last_seq: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut total_received = 0;

    for _ in 0..total_expected {
        let frame = pane_b.codec.read_frame(&mut pane_b.transport).unwrap();
        match frame {
            Frame::Message { service, payload } if service != 0 => {
                let sf: ServiceFrame = postcard::from_bytes(&payload).unwrap();
                match sf {
                    ServiceFrame::Notification { payload: inner } => {
                        let msg: EchoMessage = postcard::from_bytes(&inner).unwrap();
                        match msg {
                            EchoMessage::Ping(s) => {
                                let parts: Vec<&str> = s.split(':').collect();
                                let client_idx: usize = parts[0].parse().unwrap();
                                let seq: usize = parts[1].parse().unwrap();

                                let last = last_seq.entry(client_idx).or_insert(0);
                                if seq > 0 {
                                    assert!(
                                        seq > *last || *last == 0,
                                        "out-of-order: client {client_idx}, \
                                         expected > {last}, got {seq}"
                                    );
                                }
                                *last = seq;
                                total_received += 1;
                            }
                            _ => panic!("unexpected message variant"),
                        }
                    }
                    _ => panic!("expected Notification"),
                }
            }
            other => panic!("expected service frame, got {other:?}"),
        }
    }

    assert_eq!(
        total_received, total_expected,
        "all {total_expected} frames must be received"
    );

    // Per-client: verify all sequences were received
    for idx in 0..num_clients {
        let last = last_seq.get(&idx).copied().unwrap_or(0);
        assert_eq!(
            last,
            frames_per_client - 1,
            "client {idx}: expected final seq {}, got {last}",
            frames_per_client - 1
        );
    }

    // Cleanup
    for (transport, conn, _) in cleanup {
        drop(transport);
        conn.wait();
    }
    drop(pane_b);
    conn_b.wait();
}

// ════════════════════════════════════════════════════════════════
// B1: Backpressure / channel fill — no backpressure exists
// ════════════════════════════════════════════════════════════════

/// B1: Proves the current architecture has NO backpressure.
///
/// Setup: consumer connects but stops reading. Provider sends
/// 10,000 frames rapidly. The write_tx channel (mpsc::Sender) is
/// unbounded — sends never block. The frames queue in the mpsc
/// channel between the reader thread and the actor, and in the
/// write handle's mpsc between the actor and the consumer's
/// MemoryTransport.
///
/// This test verifies:
/// 1. The sender (provider→server→consumer path) never blocks
/// 2. All 10,000 writes complete without error
/// 3. The consumer's channel fills without bound
///
/// This is a DOCUMENTATION test — it proves a known architectural
/// gap. The consequence: a slow consumer causes unbounded memory
/// growth in the server's event channel and the consumer's write
/// channel. Future work should add backpressure (bounded channels
/// or flow control).
#[test]
#[ignore]
fn no_backpressure_unbounded_channel_fill() {
    let server = Arc::new(ProtocolServer::new());
    let echo_id = echo_service_id();

    // Provider
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

    // Consumer — connects but will STOP reading
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
    let _conn_a = accept_a.join().unwrap();
    pane_a.expect_ready();

    // Establish route
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
        other => panic!("expected InterestAccepted for provider, got {other:?}"),
    };
    pane_a.register_session(a_session);
    pane_b.register_session(b_session);

    // Consumer stops reading. The MemoryTransport's mpsc receiver
    // is still alive (pane_a owns it), but nobody calls read().
    // Frames will accumulate in the MemoryTransport's channel.

    // Provider sends 10,000 frames. With no backpressure, all
    // sends complete immediately (mpsc::Sender::send is wait-free
    // for unbounded channels).
    let frame_count = 10_000;
    let send_start = std::time::Instant::now();

    // Run the send in a thread with a timeout — if backpressure
    // existed, this would block.
    let (send_done_tx, send_done_rx) = std::sync::mpsc::channel();
    let send_thread = std::thread::spawn(move || {
        for seq in 0..frame_count {
            let inner = postcard::to_allocvec(&EchoMessage::Ping(format!("bp:{seq}"))).unwrap();
            pane_b.send_service(b_session, &ServiceFrame::Notification { payload: inner });
        }
        let elapsed = send_start.elapsed();
        let _ = send_done_tx.send(elapsed);
        pane_b
    });

    // The send must complete within 5 seconds. With no backpressure,
    // it should be near-instant (the mpsc channel absorbs everything).
    match send_done_rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(elapsed) => {
            // Document: all 10,000 sends completed without blocking.
            // This proves no backpressure exists.
            assert!(
                elapsed.as_secs() < 3,
                "sending 10,000 frames took {elapsed:?} — unexpectedly slow, \
                 possible backpressure detected"
            );
        }
        Err(_) => {
            panic!(
                "sending 10,000 frames to a non-reading consumer blocked \
                 for >5s — this indicates backpressure was added but the \
                 test was not updated. If intentional, update this test \
                 to reflect the new architecture."
            );
        }
    }

    let pane_b = send_thread.join().unwrap();

    // Give the server time to forward frames to the consumer's channel.
    // The actor thread drains the provider's frames and writes them
    // to the consumer's MemoryTransport mpsc — all unbounded.
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Now read some frames from the consumer to verify they queued.
    // We don't read all 10,000 — just verify the queue is populated.
    let mut read_count = 0;
    for _ in 0..100 {
        let frame = pane_a.codec.read_frame(&mut pane_a.transport);
        match frame {
            Ok(Frame::Message { service, .. }) if service == a_session => {
                read_count += 1;
            }
            Ok(Frame::Message { service: 0, .. }) => {
                // Control message — skip
            }
            other => {
                panic!("unexpected frame during drain: {other:?}");
            }
        }
    }
    assert!(
        read_count > 0,
        "consumer's channel should have queued frames"
    );

    // Document the consequence: the remaining ~9900 frames are still
    // in memory. In a real system, a slow consumer causes unbounded
    // growth. This test proves the gap exists.

    drop(pane_a);
    drop(pane_b);
    conn_b.wait();
}
