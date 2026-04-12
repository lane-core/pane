//! Adversarial stress tests for ProtocolServer, routing, and framing.
//!
//! These tests are designed to break the system: find crashes,
//! deadlocks, leaks, and corruption under adversarial conditions.
//! Every test uses `#[ignore]` — run with:
//! `cargo test --test adversarial -- --ignored`
//!
//! Tests that discover bugs document them in the doc comment and
//! use `// BUG:` comments at the assertion site. The goal is to
//! find what breaks, not fix it.

use std::io::Write;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::Duration;

use pane_proto::control::ControlMessage;
use pane_proto::protocol::ServiceId;
use pane_proto::service_frame::ServiceFrame;
use pane_session::bridge::HANDSHAKE_MAX_MESSAGE_SIZE;
use pane_session::frame::{Frame, FrameCodec};
use pane_session::handshake::{Hello, Rejection, ServiceProvision, Welcome};
use pane_session::server::ProtocolServer;

use serde::{Deserialize, Serialize};

// ── Constants ────────────────────────────────────────────────────

// ── Wire helpers ─────────────────────────────────────────────────

/// Wire helper for UnixStream connections. All adversarial tests
/// use real OS sockets, not MemoryTransport.
struct SocketConn {
    stream: UnixStream,
    codec: FrameCodec,
}

impl SocketConn {
    fn connect(mut stream: UnixStream, hello: Hello) -> (Self, Welcome) {
        let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&hello, &mut bytes).unwrap();
        codec.write_frame(&mut stream, 0, &bytes).unwrap();

        let frame = codec.read_frame(&mut stream).unwrap();
        let payload = match frame {
            Frame::Message {
                service: 0,
                payload,
            } => payload,
            other => panic!("unexpected frame: {other:?}"),
        };
        let decision: Result<Welcome, Rejection> =
            ciborium::de::from_reader(payload.as_slice()).unwrap();
        let welcome = decision.expect("rejected");

        let mut codec = codec;
        codec.set_max_message_size(welcome.max_message_size);
        (SocketConn { stream, codec }, welcome)
    }

    fn send_control(&mut self, msg: &ControlMessage) {
        let bytes = postcard::to_allocvec(msg).unwrap();
        self.codec.write_frame(&mut self.stream, 0, &bytes).unwrap();
    }

    fn read_control(&mut self) -> ControlMessage {
        let frame = self.codec.read_frame(&mut self.stream).unwrap();
        match frame {
            Frame::Message {
                service: 0,
                payload,
            } => postcard::from_bytes(&payload).unwrap(),
            other => panic!("expected Control, got {other:?}"),
        }
    }

    /// Read a control message, returning None on transport EOF
    /// (connection closed). Useful when the peer may have been
    /// torn down.
    fn try_read_control(&mut self) -> Option<ControlMessage> {
        match self.codec.read_frame(&mut self.stream) {
            Ok(Frame::Message {
                service: 0,
                payload,
            }) => Some(postcard::from_bytes(&payload).unwrap()),
            Ok(other) => panic!("expected Control, got {other:?}"),
            Err(_) => None,
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
            .write_frame(&mut self.stream, session_id, &bytes)
            .unwrap();
    }

    fn read_service(&mut self) -> (u16, ServiceFrame) {
        let frame = self.codec.read_frame(&mut self.stream).unwrap();
        match frame {
            Frame::Message { service, payload } if service != 0 => {
                let sf: ServiceFrame = postcard::from_bytes(&payload).unwrap();
                (service, sf)
            }
            other => panic!("expected service frame, got {other:?}"),
        }
    }

    /// Read any frame, returning the raw Frame enum. For tests that
    /// need to distinguish control vs service vs EOF without panicking.
    fn try_read_any(&mut self) -> Result<Frame, pane_session::frame::FrameError> {
        self.codec.read_frame(&mut self.stream)
    }
}

fn topic_service_id() -> ServiceId {
    ServiceId::new("com.pane.adversarial.topic")
}

fn make_hello(provides: Vec<ServiceProvision>) -> Hello {
    Hello {
        version: 1,
        max_message_size: 16 * 1024 * 1024,
        max_outstanding_requests: 0,
        interests: vec![],
        provides,
    }
}

fn make_publisher_hello() -> Hello {
    make_hello(vec![ServiceProvision {
        service: topic_service_id(),
        version: 1,
    }])
}

fn accept_unix_on_thread(
    server: &Arc<ProtocolServer>,
    stream: UnixStream,
) -> thread::JoinHandle<pane_session::server::ConnectionHandle> {
    let server = Arc::clone(server);
    thread::spawn(move || {
        let writer = stream.try_clone().expect("try_clone failed");
        server.accept(stream, writer).expect("accept failed")
    })
}

/// Connect and handshake a SocketConn. Returns (conn, conn_handle).
fn connect_socket(
    server: &Arc<ProtocolServer>,
    hello: Hello,
) -> (SocketConn, pane_session::server::ConnectionHandle) {
    let (client, server_sock) = UnixStream::pair().unwrap();
    let accept = accept_unix_on_thread(server, server_sock);
    let (mut conn, _) = SocketConn::connect(client, hello);
    let handle = accept.join().unwrap();
    conn.expect_ready();
    (conn, handle)
}

/// Subscribe a SocketConn to the publisher's topic.
/// Returns (subscriber, sub_session, pub_session, conn_handle).
fn subscribe_socket(
    server: &Arc<ProtocolServer>,
    publisher: &mut SocketConn,
) -> (SocketConn, u16, u16, pane_session::server::ConnectionHandle) {
    let topic_id = topic_service_id();
    let (sub, handle) = connect_socket(server, make_hello(vec![]));
    let mut sub = sub;

    sub.send_control(&ControlMessage::DeclareInterest {
        service: topic_id,
        expected_version: 1,
    });

    let sub_session = match sub.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("subscriber: expected InterestAccepted, got {other:?}"),
    };
    let pub_session = match publisher.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("publisher: expected InterestAccepted, got {other:?}"),
    };

    sub.register_session(sub_session);
    publisher.register_session(pub_session);

    (sub, sub_session, pub_session, handle)
}

/// Wrap a closure in a timeout. Returns the result if the closure
/// completes, panics with "deadlock suspected" if it hangs.
fn with_timeout<T: Send + 'static>(
    name: &str,
    timeout: Duration,
    f: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    let handle = thread::spawn(move || {
        let result = f();
        let _ = tx.send(result);
    });
    match rx.recv_timeout(timeout) {
        Ok(result) => {
            handle.join().unwrap();
            result
        }
        Err(_) => panic!("{name}: deadlock suspected (timed out after {timeout:?})"),
    }
}

// ── Test message types ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Payload {
    seq: u64,
    data: Vec<u8>,
}

fn tag() -> u8 {
    topic_service_id().tag()
}

fn serialize_payload(seq: u64) -> Vec<u8> {
    let msg = Payload {
        seq,
        data: vec![0xAB; 32],
    };
    let msg_bytes = postcard::to_allocvec(&msg).unwrap();
    let mut inner = Vec::with_capacity(1 + msg_bytes.len());
    inner.push(tag());
    inner.extend_from_slice(&msg_bytes);
    inner
}

// ═════════════════════════════════��════════════════════��═════════
// Test 1: Thundering herd
// ════════════════════���═══════════════════════════════════════════

/// 100 subscribers connect simultaneously (barrier-synchronized),
/// all DeclareInterest at the same instant. Verify: no panics, all
/// get InterestAccepted, publisher gets all subscriber_connected
/// callbacks, no leaked connections.
#[test]
#[ignore]
fn thundering_herd() {
    const NUM_SUBS: usize = 100;

    with_timeout("thundering_herd", Duration::from_secs(30), move || {
        let server = Arc::new(ProtocolServer::new());

        // Publisher — use try_clone so we can give one half to the
        // drain thread and keep the other for shutdown control.
        let (pub_conn, conn_pub) = connect_socket(&server, make_publisher_hello());
        let pub_read_stream = pub_conn.stream.try_clone().unwrap();
        let pub_codec = FrameCodec::permissive(16 * 1024 * 1024);
        // Drop the SocketConn on the main thread side — we only
        // need the cloned read stream for the drain thread.
        let pub_stream_for_shutdown = pub_conn.stream;

        // Spawn publisher drain thread BEFORE any subscribers connect.
        // It reads InterestAccepted and ServiceTeardown from the
        // publisher's control channel. Exits on EOF.
        let pub_handle = thread::spawn(move || {
            let mut stream = pub_read_stream;
            let mut pub_accepted = 0u64;
            loop {
                match pub_codec.read_frame(&mut stream) {
                    Ok(Frame::Message {
                        service: 0,
                        payload,
                    }) => {
                        if let Ok(msg) = postcard::from_bytes::<ControlMessage>(&payload) {
                            match msg {
                                ControlMessage::InterestAccepted { .. } => {
                                    pub_accepted += 1;
                                }
                                ControlMessage::ServiceTeardown { .. } => {}
                                _ => {}
                            }
                        }
                    }
                    Ok(_) => {}      // service frames (shouldn't happen)
                    Err(_) => break, // EOF or error
                }
            }
            pub_accepted
        });

        let barrier = Arc::new(Barrier::new(NUM_SUBS));
        let accepted = Arc::new(AtomicUsize::new(0));
        let declined = Arc::new(AtomicUsize::new(0));
        let panicked = Arc::new(AtomicBool::new(false));

        let mut handles = Vec::new();

        for i in 0..NUM_SUBS {
            let server = Arc::clone(&server);
            let barrier = Arc::clone(&barrier);
            let accepted = Arc::clone(&accepted);
            let declined = Arc::clone(&declined);
            let panicked = Arc::clone(&panicked);

            handles.push(thread::spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let (mut sub, handle) = connect_socket(&server, make_hello(vec![]));

                    // All subscribers wait here, then DeclareInterest simultaneously
                    barrier.wait();

                    sub.send_control(&ControlMessage::DeclareInterest {
                        service: topic_service_id(),
                        expected_version: 1,
                    });

                    match sub.read_control() {
                        ControlMessage::InterestAccepted { .. } => {
                            accepted.fetch_add(1, Ordering::Relaxed);
                        }
                        ControlMessage::InterestDeclined { .. } => {
                            declined.fetch_add(1, Ordering::Relaxed);
                        }
                        other => panic!("sub {i}: unexpected control: {other:?}"),
                    }

                    drop(sub);
                    handle.wait();
                }));

                if result.is_err() {
                    panicked.store(true, Ordering::Relaxed);
                    eprintln!("thundering_herd: subscriber {i} panicked");
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // Shutdown(Both) on the publisher's socket. A plain `drop`
        // won't work: try_clone() dup'd the fd, so closing one fd
        // leaves the other open and the server never sees EOF.
        // shutdown() affects the underlying socket, not just the fd.
        let _ = pub_stream_for_shutdown.shutdown(Shutdown::Both);

        let pub_accepted = pub_handle.join().unwrap();
        conn_pub.wait();

        let total_accepted = accepted.load(Ordering::Relaxed);
        let total_declined = declined.load(Ordering::Relaxed);

        assert!(
            !panicked.load(Ordering::Relaxed),
            "thundering_herd: one or more subscribers panicked"
        );
        assert_eq!(
            total_accepted + total_declined,
            NUM_SUBS,
            "not all subscribers resolved: accepted={total_accepted}, declined={total_declined}"
        );
        // All should be accepted (single provider, no capacity limit)
        assert_eq!(
            total_accepted, NUM_SUBS,
            "expected all {NUM_SUBS} accepted, got {total_accepted} accepted, \
             {total_declined} declined"
        );
        assert_eq!(
            pub_accepted, NUM_SUBS as u64,
            "publisher saw {pub_accepted} InterestAccepted, expected {NUM_SUBS}"
        );

        eprintln!(
            "thundering_herd: {total_accepted} accepted, \
             {total_declined} declined, publisher saw {pub_accepted}"
        );
    });
}

// ══════════════════════════════════════════���═════════════════════
// Test 2: Message flood, subscriber never reads
// ═════════════════════════════════════════════════���══════════════

/// Publisher sends 100,000 notifications as fast as possible.
/// Subscriber NEVER reads. Tests: does the server's writer thread
/// fill its queue and tear down the connection cleanly? Does the
/// publisher eventually get backpressure or does something OOM?
///
/// Expected: subscriber's connection is torn down by server queue
/// overflow (D12: queue full -> connection teardown, not frame drop).
/// Publisher's connection stays alive and gets ServiceTeardown.
#[test]
#[ignore]
fn message_flood_no_reader() {
    const NUM_MSGS: usize = 100_000;

    with_timeout(
        "message_flood_no_reader",
        Duration::from_secs(30),
        move || {
            let server = Arc::new(ProtocolServer::new());

            // Publisher
            let (mut publisher, conn_pub) = connect_socket(&server, make_publisher_hello());

            // Subscriber — connects but will never read after handshake
            let (sub, _sub_session, pub_session, _conn_sub) =
                subscribe_socket(&server, &mut publisher);

            // Leak the subscriber's read end — it will never read.
            // The underlying UnixStream stays open but unread.
            let _sub_leak = sub;

            // Publisher floods notifications
            let mut send_errors = 0usize;
            let mut teardown_received = false;

            for seq in 0..NUM_MSGS as u64 {
                let payload = serialize_payload(seq);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    publisher.send_service(
                        pub_session,
                        &ServiceFrame::Notification {
                            payload: payload.clone(),
                        },
                    );
                }));
                if result.is_err() {
                    send_errors += 1;
                    break;
                }

                // Periodically check if we got ServiceTeardown
                // (non-blocking: set read timeout briefly)
                if seq % 1000 == 999 {
                    let _ = publisher
                        .stream
                        .set_read_timeout(Some(Duration::from_millis(1)));
                    if let Some(ControlMessage::ServiceTeardown { .. }) =
                        publisher.try_read_control()
                    {
                        teardown_received = true;
                        eprintln!("message_flood_no_reader: ServiceTeardown at seq {seq}");
                        break;
                    }
                    let _ = publisher.stream.set_read_timeout(None);
                }
            }

            // After the flood, read remaining control messages with
            // a timeout to see if teardown arrives
            if !teardown_received {
                let _ = publisher
                    .stream
                    .set_read_timeout(Some(Duration::from_secs(2)));
                for _ in 0..100 {
                    match publisher.try_read_control() {
                        Some(ControlMessage::ServiceTeardown { session_id, .. })
                            if session_id == pub_session =>
                        {
                            teardown_received = true;
                            break;
                        }
                        Some(_) => continue,
                        None => break,
                    }
                }
            }

            eprintln!(
                "message_flood_no_reader: send_errors={send_errors}, \
                 teardown_received={teardown_received}"
            );

            // Publisher's own connection should still be alive.
            // Verify by sending a control message (DeclareInterest
            // for a non-existent service — we just want to see if
            // the write succeeds).
            let pub_alive = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                publisher.send_control(&ControlMessage::DeclareInterest {
                    service: ServiceId::new("com.pane.adversarial.nonexistent"),
                    expected_version: 1,
                });
            }))
            .is_ok();

            assert!(
                pub_alive,
                "publisher's connection died — expected it to survive subscriber overflow"
            );

            // The subscriber's connection should have been torn down
            // by the server's queue overflow. We expect the publisher
            // to have received ServiceTeardown at some point (either
            // during the flood or after). If the server doesn't have
            // queue overflow detection, this documents the gap.
            //
            // NOTE: Whether teardown_received is true depends on
            // whether the server's D12 overflow detection fires during
            // a 100k message flood. The server's WRITER_CHANNEL_CAPACITY
            // is 4096, plus the OS socket buffer. The subscriber never
            // reads, so the writer thread's mpsc channel should fill up
            // and trigger overflow teardown. But the OS socket buffer
            // can absorb some writes before the writer thread stalls.
            eprintln!(
                "message_flood_no_reader: subscriber teardown observed = {teardown_received}"
            );

            drop(publisher);
            conn_pub.wait();
        },
    );
}

// ═══════════════════════════���════════════════════════════════════
// Test 3: Rapid connect/disconnect
// ═════════════════════════════════════════════════════��══════════

/// 1000 iterations: connect, DeclareInterest, immediately disconnect.
/// No messages sent. Tests: race conditions in registration/teardown
/// paths. Server's routing table must not leak entries.
///
/// Verify: final routing table is empty after all connections close
/// (indirectly: a new DeclareInterest after the churn still works).
#[test]
#[ignore]
fn rapid_connect_disconnect() {
    const ITERATIONS: usize = 1000;

    with_timeout(
        "rapid_connect_disconnect",
        Duration::from_secs(60),
        move || {
            let server = Arc::new(ProtocolServer::new());

            // Persistent provider
            let (provider, _conn_prov) = connect_socket(&server, make_publisher_hello());

            // Keep a shutdown handle so we can trigger EOF on the drain
            // thread from the main thread. shutdown(Both) affects the
            // underlying socket, so the drain thread's blocking read
            // will return EOF even though it holds a separate fd.
            let provider_shutdown = provider.stream.try_clone().unwrap();

            // Provider drains InterestAccepted/ServiceTeardown on a
            // background thread so its read buffer doesn't block.
            let drain_handle = thread::spawn(move || {
                let mut provider = provider;
                let mut accepted = 0u64;
                let mut teardowns = 0u64;
                loop {
                    match provider.try_read_control() {
                        Some(ControlMessage::InterestAccepted { .. }) => {
                            accepted += 1;
                        }
                        Some(ControlMessage::ServiceTeardown { .. }) => {
                            teardowns += 1;
                        }
                        Some(other) => {
                            eprintln!("rapid_connect_disconnect provider: {other:?}");
                        }
                        None => break,
                    }
                }
                (accepted, teardowns)
            });

            for i in 0..ITERATIONS {
                let (client_sock, server_sock) = UnixStream::pair().unwrap();

                let server_ref = Arc::clone(&server);
                let accept_handle = thread::spawn(move || {
                    let writer = server_sock.try_clone().expect("try_clone");
                    let _ = server_ref.accept(server_sock, writer);
                });

                // Full handshake
                let connect_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let (mut conn, _) = SocketConn::connect(client_sock, make_hello(vec![]));

                    // Read Ready
                    conn.expect_ready();

                    // DeclareInterest
                    conn.send_control(&ControlMessage::DeclareInterest {
                        service: topic_service_id(),
                        expected_version: 1,
                    });

                    // Read response (InterestAccepted or InterestDeclined)
                    let _ = conn.read_control();

                    // Immediately drop — triggers disconnect
                    drop(conn);
                }));

                if connect_result.is_err() {
                    eprintln!("rapid_connect_disconnect: iteration {i} panicked");
                }

                let _ = accept_handle.join();
            }

            // Verify the server still works after 1000 rapid cycles.
            // A new subscriber should successfully DeclareInterest.
            let (client_sock, server_sock) = UnixStream::pair().unwrap();
            let server_ref = Arc::clone(&server);
            let accept_handle = thread::spawn(move || {
                let writer = server_sock.try_clone().expect("try_clone");
                server_ref
                    .accept(server_sock, writer)
                    .expect("final accept")
            });

            let (mut final_conn, _) = SocketConn::connect(client_sock, make_hello(vec![]));
            let final_handle = accept_handle.join().unwrap();
            final_conn.expect_ready();

            final_conn.send_control(&ControlMessage::DeclareInterest {
                service: topic_service_id(),
                expected_version: 1,
            });

            match final_conn.read_control() {
                ControlMessage::InterestAccepted { .. } => {
                    eprintln!("rapid_connect_disconnect: final DeclareInterest accepted");
                }
                other => {
                    panic!("rapid_connect_disconnect: final DeclareInterest failed: {other:?}")
                }
            }

            drop(final_conn);
            final_handle.wait();

            // Shut down the provider's socket to trigger EOF on the
            // drain thread. drop(_conn_prov) only drops the done_rx,
            // it doesn't close the socket.
            let _ = provider_shutdown.shutdown(Shutdown::Both);
            drop(_conn_prov);
            let (accepted, teardowns) = drain_handle.join().unwrap();
            eprintln!(
                "rapid_connect_disconnect: provider saw {accepted} accepted, \
                 {teardowns} teardowns over {ITERATIONS} iterations"
            );
        },
    );
}

// ══════════════════════════════════════════��═════════════════════
// Test 4: Cancel storm
// ══════════════════════════════════════════���═════════════════════

/// Publisher sends 500 requests, then immediately cancels all 500.
/// Subscriber replies to each request it receives. Tests: race
/// between Cancel arriving and Reply arriving.
///
/// Verify: no double-dispatch (on_reply AND on_failed for same
/// token), no panic, all tokens resolved exactly once (either
/// reply or cancel).
///
/// NOTE: Cancel is advisory server-side in Phase 1 (server no-op,
/// D10). The server forwards ServiceFrames opaquely. So both
/// Request and Cancel go to the subscriber, and the subscriber's
/// Reply goes back. The race is between the Reply arriving at
/// the publisher and the publisher's local cancel state. This
/// test exercises the wire-level race at the server.
#[test]
#[ignore]
fn cancel_storm() {
    const NUM_REQUESTS: u64 = 500;

    with_timeout("cancel_storm", Duration::from_secs(30), move || {
        let server = Arc::new(ProtocolServer::new());

        // Publisher (provider of the service)
        let (mut publisher, conn_pub) = connect_socket(&server, make_publisher_hello());

        // Subscriber (consumer that sends requests)
        let (sub, sub_session, pub_session, conn_sub) = subscribe_socket(&server, &mut publisher);

        // Keep a shutdown handle so we can force the subscriber's
        // blocking read to fail from the main thread.
        let sub_shutdown = sub.stream.try_clone().unwrap();

        // Subscriber replies to every request on a background thread
        let sub_handle = thread::spawn(move || {
            let mut sub = sub;
            let mut replies_sent = 0u64;
            loop {
                match sub.try_read_any() {
                    Ok(Frame::Message { service, payload }) if service == sub_session => {
                        let sf: ServiceFrame = postcard::from_bytes(&payload).unwrap();
                        match sf {
                            ServiceFrame::Request { token, payload } => {
                                // Reply may fail if connection is shutting down
                                let _ =
                                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                        sub.send_service(
                                            sub_session,
                                            &ServiceFrame::Reply { token, payload },
                                        );
                                    }));
                                replies_sent += 1;
                            }
                            ServiceFrame::Notification { .. } => {}
                            _ => {}
                        }
                    }
                    Ok(Frame::Message { service: 0, .. }) => {
                        // Control message — might be teardown
                    }
                    Ok(_) => {}
                    Err(_) => break, // Connection closed
                }
            }
            drop(sub);
            conn_sub.wait();
            replies_sent
        });

        // Send all requests in a burst
        for token in 1..=NUM_REQUESTS {
            publisher.send_service(
                pub_session,
                &ServiceFrame::Request {
                    token,
                    payload: serialize_payload(token),
                },
            );
        }

        // Immediately cancel all of them
        for token in 1..=NUM_REQUESTS {
            publisher.send_control(&ControlMessage::Cancel { token });
        }

        // Collect responses. Each token should resolve exactly once.
        let mut replied = std::collections::HashSet::new();
        let mut failed = std::collections::HashSet::new();
        let mut double_dispatch = Vec::new();

        // Read with a timeout to avoid hanging
        let _ = publisher
            .stream
            .set_read_timeout(Some(Duration::from_secs(5)));

        loop {
            match publisher.try_read_any() {
                Ok(Frame::Message { service, payload }) if service == pub_session => {
                    let sf: ServiceFrame = postcard::from_bytes(&payload).unwrap();
                    match sf {
                        ServiceFrame::Reply { token, .. } => {
                            if replied.contains(&token) || failed.contains(&token) {
                                double_dispatch.push(token);
                            }
                            replied.insert(token);
                        }
                        ServiceFrame::Failed { token } => {
                            if replied.contains(&token) || failed.contains(&token) {
                                double_dispatch.push(token);
                            }
                            failed.insert(token);
                        }
                        _ => {}
                    }
                }
                Ok(Frame::Message {
                    service: 0,
                    payload,
                }) => {
                    // Control messages (InterestAccepted, ServiceTeardown, etc.)
                    let _msg: Result<ControlMessage, _> = postcard::from_bytes(&payload);
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }

        // Shut down both sides to unblock the subscriber's read.
        // ServiceTeardown only tears down the route, not the
        // connection — need to close the socket to get EOF.
        let _ = publisher.stream.shutdown(Shutdown::Both);
        drop(publisher);
        conn_pub.wait();
        let _ = sub_shutdown.shutdown(Shutdown::Both);

        let replies_sent = sub_handle.join().unwrap();

        assert!(
            double_dispatch.is_empty(),
            "cancel_storm: double-dispatch detected for tokens: {double_dispatch:?}"
        );

        eprintln!(
            "cancel_storm: {}/{NUM_REQUESTS} replied, {}/{NUM_REQUESTS} failed, \
             subscriber sent {replies_sent} replies",
            replied.len(),
            failed.len(),
        );

        // NOTE: In Phase 1, Cancel is a server no-op (D10). All
        // requests should get replies because the subscriber replies
        // to everything and the server just forwards. Cancel doesn't
        // prevent delivery. This documents that behavior.
        assert_eq!(
            replied.len(),
            NUM_REQUESTS as usize,
            "cancel_storm: expected all {NUM_REQUESTS} to get replies \
             (Cancel is advisory/no-op in Phase 1)"
        );
    });
}

// ═══════════════════════════════════════════════════════���════════
// Test 5: Shutdown during traffic
// ═════════════════════════════════════════════���══════════════════

/// Publisher sends notifications in a tight loop. After 1000 messages,
/// subscriber drops its connection mid-stream. Publisher continues
/// sending for another 1000.
///
/// Tests: does the publisher crash when the server tears down the
/// subscriber's route?
///
/// Verify: publisher's connection stays alive, publisher gets
/// ServiceTeardown for the dead subscriber.
#[test]
#[ignore]
fn shutdown_during_traffic() {
    with_timeout(
        "shutdown_during_traffic",
        Duration::from_secs(30),
        move || {
            let server = Arc::new(ProtocolServer::new());

            let (mut publisher, conn_pub) = connect_socket(&server, make_publisher_hello());

            let (sub, _sub_session, pub_session, conn_sub) =
                subscribe_socket(&server, &mut publisher);

            // Send first 1000 messages
            for seq in 0..1000u64 {
                publisher.send_service(
                    pub_session,
                    &ServiceFrame::Notification {
                        payload: serialize_payload(seq),
                    },
                );
            }

            // Kill subscriber mid-stream
            drop(sub);
            conn_sub.wait();

            // Continue sending 1000 more — these go to a dead route.
            // The server should silently drop them (no route), not crash.
            let mut post_kill_send_errors = 0usize;
            for seq in 1000..2000u64 {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    publisher.send_service(
                        pub_session,
                        &ServiceFrame::Notification {
                            payload: serialize_payload(seq),
                        },
                    );
                }));
                if result.is_err() {
                    post_kill_send_errors += 1;
                    break;
                }
            }

            // Check for ServiceTeardown
            let _ = publisher
                .stream
                .set_read_timeout(Some(Duration::from_secs(2)));
            let mut got_teardown = false;
            loop {
                match publisher.try_read_control() {
                    Some(ControlMessage::ServiceTeardown { session_id, .. })
                        if session_id == pub_session =>
                    {
                        got_teardown = true;
                        break;
                    }
                    Some(_) => continue,
                    None => break,
                }
            }

            assert_eq!(
                post_kill_send_errors, 0,
                "shutdown_during_traffic: publisher panicked sending to dead route"
            );
            assert!(
                got_teardown,
                "shutdown_during_traffic: publisher never received ServiceTeardown \
                 for killed subscriber"
            );

            // Publisher should still be alive
            let _ = publisher.stream.set_read_timeout(None);
            let pub_alive = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                publisher.send_control(&ControlMessage::DeclareInterest {
                    service: ServiceId::new("com.pane.adversarial.probe"),
                    expected_version: 1,
                });
            }))
            .is_ok();

            assert!(
                pub_alive,
                "shutdown_during_traffic: publisher connection died after subscriber kill"
            );

            drop(publisher);
            conn_pub.wait();
            eprintln!("shutdown_during_traffic: passed");
        },
    );
}

// ══════════════════════════════════════════���═════════════════════
// Test 6: Half-close socket
// ══════════════════════════════════════════════���═════════════════

/// Connect via UnixStream::pair(). After handshake, shut down the
/// WRITE half of the subscriber's socket while leaving READ open.
/// Then send a notification from publisher.
///
/// Tests: what happens when the server writes to a half-closed
/// socket?
///
/// Verify: server detects the error, tears down the connection,
/// no panic.
#[test]
#[ignore]
fn half_close_socket() {
    with_timeout("half_close_socket", Duration::from_secs(10), move || {
        let server = Arc::new(ProtocolServer::new());

        let (mut publisher, conn_pub) = connect_socket(&server, make_publisher_hello());

        let (sub, _sub_session, pub_session, _conn_sub) = subscribe_socket(&server, &mut publisher);

        // Half-close: shut down the subscriber's write side.
        // The server's reader thread will get EOF when reading from
        // this connection. The read side stays open so the subscriber
        // could theoretically still receive data.
        sub.stream.shutdown(Shutdown::Write).unwrap();

        // Give the server a moment to detect the EOF on the reader
        thread::sleep(Duration::from_millis(100));

        // Publisher sends a notification to the (half-dead) subscriber
        let send_ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            publisher.send_service(
                pub_session,
                &ServiceFrame::Notification {
                    payload: serialize_payload(0),
                },
            );
        }))
        .is_ok();

        assert!(
            send_ok,
            "half_close_socket: publisher panicked sending to half-closed subscriber"
        );

        // Publisher should eventually get ServiceTeardown
        let _ = publisher
            .stream
            .set_read_timeout(Some(Duration::from_secs(3)));
        let mut got_teardown = false;
        loop {
            match publisher.try_read_control() {
                Some(ControlMessage::ServiceTeardown { session_id, .. })
                    if session_id == pub_session =>
                {
                    got_teardown = true;
                    break;
                }
                Some(_) => continue,
                None => break,
            }
        }

        assert!(
            got_teardown,
            "half_close_socket: publisher never received ServiceTeardown \
             for half-closed subscriber"
        );

        eprintln!("half_close_socket: passed — server detected half-close");

        drop(sub);
        drop(publisher);
        conn_pub.wait();
    });
}

// ══════════════════════════════════════════════════���═════════════
// Test 7: Malformed frames
// ══════════════════════════════════════════════════���═════════════

/// Connect, complete handshake normally, then send raw garbage bytes
/// on the wire. Tests: does the server handle malformed input without
/// crashing?
///
/// Patterns tested:
/// 1. Random bytes (not a valid frame)
/// 2. Truncated frame header (only 2 of 4 length bytes)
/// 3. Frame claiming 1GB length
/// 4. Valid length but corrupted payload
///
/// Verify: server disconnects the bad client, other clients are
/// unaffected.
#[test]
#[ignore]
fn malformed_frames() {
    with_timeout("malformed_frames", Duration::from_secs(15), move || {
        let server = Arc::new(ProtocolServer::new());

        // Good client (canary — should survive all the bad clients)
        let (mut good_pub, conn_good) = connect_socket(&server, make_publisher_hello());

        // Pattern 1: Random garbage after handshake
        {
            let (client_sock, server_sock) = UnixStream::pair().unwrap();
            let server_ref = Arc::clone(&server);
            let accept = thread::spawn(move || {
                let writer = server_sock.try_clone().expect("try_clone");
                let _ = server_ref.accept(server_sock, writer);
            });

            // Complete handshake
            let (mut bad, _) = SocketConn::connect(client_sock, make_hello(vec![]));
            // Read Ready
            bad.expect_ready();

            // Send raw garbage
            let _ = bad.stream.write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE]);
            let _ = bad.stream.flush();
            thread::sleep(Duration::from_millis(50));

            drop(bad);
            let _ = accept.join();
        }

        // Pattern 2: Truncated length prefix
        {
            let (client_sock, server_sock) = UnixStream::pair().unwrap();
            let server_ref = Arc::clone(&server);
            let accept = thread::spawn(move || {
                let writer = server_sock.try_clone().expect("try_clone");
                let _ = server_ref.accept(server_sock, writer);
            });

            let (mut bad, _) = SocketConn::connect(client_sock, make_hello(vec![]));
            bad.expect_ready();

            // Only 2 of 4 bytes for the length prefix, then close
            let _ = bad.stream.write_all(&[0x03, 0x00]);
            let _ = bad.stream.flush();

            drop(bad);
            let _ = accept.join();
        }

        // Pattern 3: Frame claiming 1GB length
        {
            let (client_sock, server_sock) = UnixStream::pair().unwrap();
            let server_ref = Arc::clone(&server);
            let accept = thread::spawn(move || {
                let writer = server_sock.try_clone().expect("try_clone");
                let _ = server_ref.accept(server_sock, writer);
            });

            let (mut bad, _) = SocketConn::connect(client_sock, make_hello(vec![]));
            bad.expect_ready();

            // Length prefix: 1GB = 0x40000000
            let _ = bad.stream.write_all(&0x40000000u32.to_le_bytes());
            let _ = bad.stream.flush();
            thread::sleep(Duration::from_millis(50));

            drop(bad);
            let _ = accept.join();
        }

        // Pattern 4: Valid length, corrupted payload
        {
            let (client_sock, server_sock) = UnixStream::pair().unwrap();
            let server_ref = Arc::clone(&server);
            let accept = thread::spawn(move || {
                let writer = server_sock.try_clone().expect("try_clone");
                let _ = server_ref.accept(server_sock, writer);
            });

            let (mut bad, _) = SocketConn::connect(client_sock, make_hello(vec![]));
            bad.expect_ready();

            // Valid frame header: length=10, service=0 (control)
            // but payload is garbage (not valid postcard)
            let _ = bad.stream.write_all(&10u32.to_le_bytes());
            let _ = bad.stream.write_all(&0u16.to_le_bytes()); // service 0
            let _ = bad.stream.write_all(&[0xFF; 8]); // garbage payload
            let _ = bad.stream.flush();
            thread::sleep(Duration::from_millis(50));

            drop(bad);
            let _ = accept.join();
        }

        // Verify the good client is still alive by subscribing a new
        // consumer and exchanging a message.
        let (mut sub, handle) = connect_socket(&server, make_hello(vec![]));
        sub.send_control(&ControlMessage::DeclareInterest {
            service: topic_service_id(),
            expected_version: 1,
        });

        let sub_session = match sub.read_control() {
            ControlMessage::InterestAccepted { session_id, .. } => session_id,
            other => panic!("malformed_frames: canary subscribe failed: {other:?}"),
        };

        // Drain the publisher's accumulated InterestAccepted/ServiceTeardown
        let _ = good_pub
            .stream
            .set_read_timeout(Some(Duration::from_millis(500)));
        let mut pub_session = None;
        loop {
            match good_pub.try_read_control() {
                Some(ControlMessage::InterestAccepted { session_id, .. }) => {
                    good_pub.register_session(session_id);
                    pub_session = Some(session_id);
                }
                Some(ControlMessage::ServiceTeardown { .. }) => continue,
                _ => break,
            }
        }
        let _ = good_pub.stream.set_read_timeout(None);

        let pub_session =
            pub_session.expect("malformed_frames: publisher never got InterestAccepted for canary");

        sub.register_session(sub_session);
        good_pub.send_service(
            pub_session,
            &ServiceFrame::Notification {
                payload: serialize_payload(42),
            },
        );

        let (sess, frame) = sub.read_service();
        assert_eq!(sess, sub_session);
        match frame {
            ServiceFrame::Notification { .. } => {
                eprintln!("malformed_frames: canary received notification — server survived");
            }
            other => panic!("malformed_frames: canary got {other:?}"),
        }

        drop(sub);
        handle.wait();
        drop(good_pub);
        conn_good.wait();
    });
}

// ════════════════════════════════════════════════════════════════
// Test 8: Max connections
// ══════════════════════════════════════════��═════════════════════

/// Open connections until something breaks or a reasonable limit
/// is hit (try 500). Each connection does a full handshake.
///
/// Tests: resource exhaustion (file descriptors, memory, thread count).
///
/// Verify: either all 500 connect successfully, or connections
/// beyond a limit are cleanly rejected (not panicked).
#[test]
#[ignore]
fn max_connections() {
    const TARGET: usize = 500;

    with_timeout("max_connections", Duration::from_secs(60), move || {
        let server = Arc::new(ProtocolServer::new());
        let mut conns: Vec<(SocketConn, pane_session::server::ConnectionHandle)> = Vec::new();
        let mut failed_at = None;

        for i in 0..TARGET {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let (client_sock, server_sock) = UnixStream::pair().unwrap();

                let server_ref = Arc::clone(&server);
                let accept = thread::spawn(move || {
                    let writer = server_sock.try_clone().expect("try_clone");
                    server_ref.accept(server_sock, writer).expect("accept")
                });

                let (mut conn, _) = SocketConn::connect(client_sock, make_hello(vec![]));
                let handle = accept.join().unwrap();
                conn.expect_ready();
                (conn, handle)
            }));

            match result {
                Ok(pair) => conns.push(pair),
                Err(e) => {
                    eprintln!("max_connections: failed at connection {i}: {e:?}");
                    failed_at = Some(i);
                    break;
                }
            }
        }

        let num_connected = conns.len();
        eprintln!("max_connections: successfully connected {num_connected}/{TARGET}");

        // Clean up all connections
        for (conn, handle) in conns {
            drop(conn);
            handle.wait();
        }

        // Server should still accept after mass disconnect
        let (probe, probe_handle) = connect_socket(&server, make_hello(vec![]));
        drop(probe);
        probe_handle.wait();

        if let Some(n) = failed_at {
            eprintln!(
                "max_connections: resource limit hit at {n} — \
                 check fd limits and thread counts"
            );
        } else {
            eprintln!("max_connections: all {TARGET} connections succeeded");
        }

        // Whether we hit a limit or not, the test passes if nothing panicked
        // and the server survived.
    });
}

// ═══════════════════════════════════��════════════════════════════
// Test 9: Dispatch stall under load
// ══════════════���═════════════════════════════════════════════════

/// Set up a route where the subscriber takes 100ms to process every
/// 10th message. Publisher sends 100 messages rapidly.
///
/// Tests: does the server stall when a downstream connection is slow?
/// With D12 (per-connection writer threads), the slow subscriber
/// should not affect the server's actor thread.
///
/// Verify: messages before and after the stall are delivered correctly.
/// The publisher's sends complete without blocking on the slow sub.
#[test]
#[ignore]
fn dispatch_stall_under_load() {
    const NUM_MSGS: usize = 100;

    with_timeout(
        "dispatch_stall_under_load",
        Duration::from_secs(30),
        move || {
            let server = Arc::new(ProtocolServer::new());

            let (mut publisher, conn_pub) = connect_socket(&server, make_publisher_hello());

            let (sub, sub_session, pub_session, conn_sub) =
                subscribe_socket(&server, &mut publisher);

            // Publisher sends all messages in a burst
            let start = std::time::Instant::now();
            for seq in 0..NUM_MSGS as u64 {
                publisher.send_service(
                    pub_session,
                    &ServiceFrame::Notification {
                        payload: serialize_payload(seq),
                    },
                );
            }
            let send_elapsed = start.elapsed();

            // Subscriber reads with artificial delays
            let sub_handle = thread::spawn(move || {
                let mut sub = sub;
                let mut received = 0usize;
                for i in 0..NUM_MSGS {
                    match sub.try_read_any() {
                        Ok(Frame::Message { service, .. }) if service == sub_session => {
                            received += 1;
                            // Stall on every 10th message
                            if i % 10 == 9 {
                                thread::sleep(Duration::from_millis(100));
                            }
                        }
                        Ok(Frame::Message { service: 0, .. }) => {
                            // Control message — skip
                        }
                        Ok(other) => {
                            eprintln!("dispatch_stall: unexpected frame: {other:?}");
                        }
                        Err(e) => {
                            eprintln!("dispatch_stall: read error at msg {i}: {e}");
                            break;
                        }
                    }
                }
                drop(sub);
                conn_sub.wait();
                received
            });

            let received = sub_handle.join().unwrap();

            eprintln!(
                "dispatch_stall_under_load: publisher sent {NUM_MSGS} in {send_elapsed:?}, \
                 subscriber received {received}"
            );

            // Publisher sends should have completed quickly — D12 means
            // the writer thread absorbs the slow subscriber without
            // stalling the actor.
            assert!(
                send_elapsed < Duration::from_secs(2),
                "dispatch_stall: publisher sends took {send_elapsed:?} — \
                 slow subscriber may be stalling the actor"
            );

            assert_eq!(
                received, NUM_MSGS,
                "dispatch_stall: subscriber only received {received}/{NUM_MSGS}"
            );

            drop(publisher);
            conn_pub.wait();
        },
    );
}

// ══════════════════════════════════════════════��═════════════════
// Test 10: Concurrent teardown and send
// ════════════════��═════════════════════════════��═════════════════

/// Two threads: Thread A sends notifications through publisher
/// continuously. Thread B periodically disconnects and reconnects
/// the subscriber (every 50ms).
///
/// Tests: race between server routing a frame to a subscriber and
/// the subscriber's connection being torn down.
///
/// Verify: no panic, no corrupted state. Publisher may get
/// ServiceTeardown but its own connection stays alive.
#[test]
#[ignore]
fn concurrent_teardown_and_send() {
    const DURATION_MS: u64 = 3000;
    const RECONNECT_INTERVAL_MS: u64 = 50;

    with_timeout(
        "concurrent_teardown_and_send",
        Duration::from_secs(30),
        move || {
            let server = Arc::new(ProtocolServer::new());

            let (pub_client, pub_server) = UnixStream::pair().unwrap();
            let server_ref = Arc::clone(&server);
            let accept_pub = thread::spawn(move || {
                let writer = pub_server.try_clone().expect("try_clone");
                server_ref.accept(pub_server, writer).expect("accept pub")
            });

            let (mut publisher, _) = SocketConn::connect(pub_client, make_publisher_hello());
            let conn_pub = accept_pub.join().unwrap();
            publisher.expect_ready();

            // Initial subscribe
            let (sub, _sub_session, pub_session, conn_sub) =
                subscribe_socket(&server, &mut publisher);

            let stop = Arc::new(AtomicBool::new(false));
            let send_count = Arc::new(AtomicUsize::new(0));
            let send_errors = Arc::new(AtomicUsize::new(0));

            // Thread A: continuous sends through publisher.
            // We need shared access to the publisher stream, but
            // SocketConn is not Send. Instead, we use the raw stream.
            let pub_stream = publisher.stream.try_clone().unwrap();
            let pub_codec = FrameCodec::new(16 * 1024 * 1024);
            // Register the service on this codec
            // (write_frame doesn't validate service, but we want clean frames)

            let stop_a = Arc::clone(&stop);
            let send_count_a = Arc::clone(&send_count);
            let send_errors_a = Arc::clone(&send_errors);
            let sender_handle = thread::spawn(move || {
                let mut stream = pub_stream;
                let mut seq = 0u64;
                while !stop_a.load(Ordering::Relaxed) {
                    let payload = serialize_payload(seq);
                    let sf = ServiceFrame::Notification { payload };
                    let bytes = postcard::to_allocvec(&sf).unwrap();
                    match pub_codec.write_frame(&mut stream, pub_session, &bytes) {
                        Ok(()) => {
                            send_count_a.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => {
                            send_errors_a.fetch_add(1, Ordering::Relaxed);
                            break;
                        }
                    }
                    seq += 1;
                }
            });

            // Thread B: periodic disconnect/reconnect of subscriber
            let server_b = Arc::clone(&server);
            let stop_b = Arc::clone(&stop);
            let churn_handle = thread::spawn(move || {
                let mut current_sub: Option<(SocketConn, pane_session::server::ConnectionHandle)> =
                    Some((sub, conn_sub));
                let mut reconnects = 0u64;

                while !stop_b.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(RECONNECT_INTERVAL_MS));

                    // Drop current subscriber
                    if let Some((s, h)) = current_sub.take() {
                        drop(s);
                        h.wait();
                    }

                    // Reconnect
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let (client_sock, server_sock) = UnixStream::pair().unwrap();
                        let server_ref = Arc::clone(&server_b);
                        let accept = thread::spawn(move || {
                            let writer = server_sock.try_clone().expect("try_clone");
                            server_ref.accept(server_sock, writer).expect("accept")
                        });

                        let (mut conn, _) = SocketConn::connect(client_sock, make_hello(vec![]));
                        let handle = accept.join().unwrap();
                        conn.expect_ready();

                        conn.send_control(&ControlMessage::DeclareInterest {
                            service: topic_service_id(),
                            expected_version: 1,
                        });

                        let _ = conn.read_control();
                        (conn, handle)
                    }));

                    match result {
                        Ok((conn, handle)) => {
                            current_sub = Some((conn, handle));
                            reconnects += 1;
                        }
                        Err(_) => {
                            eprintln!("concurrent_teardown: reconnect panicked");
                        }
                    }
                }

                if let Some((s, h)) = current_sub.take() {
                    drop(s);
                    h.wait();
                }
                reconnects
            });

            // Let it run
            thread::sleep(Duration::from_millis(DURATION_MS));
            stop.store(true, Ordering::Relaxed);

            sender_handle.join().unwrap();

            // The publisher needs to drain its control messages (InterestAccepted,
            // ServiceTeardown from churn). Set a timeout so we don't hang.
            let _ = publisher
                .stream
                .set_read_timeout(Some(Duration::from_millis(500)));
            while publisher.try_read_control().is_some() {}

            let reconnects = churn_handle.join().unwrap();

            let total_sent = send_count.load(Ordering::Relaxed);
            let total_errors = send_errors.load(Ordering::Relaxed);

            eprintln!(
                "concurrent_teardown_and_send: sent={total_sent}, errors={total_errors}, \
                 reconnects={reconnects}"
            );

            // Publisher should not have had write errors — its own
            // connection should survive subscriber churn.
            assert_eq!(
                total_errors, 0,
                "concurrent_teardown: publisher had {total_errors} send errors — \
                 subscriber churn should not affect publisher's connection"
            );

            drop(publisher);
            conn_pub.wait();
        },
    );
}

// ════════════════════════════════════���═══════════════════════════
// Test 11: Backpressure cascade isolation
// ═══════════════════��══════════════════════════���═════════════════

/// 1 publisher, 3 subscribers (A=fast, B=slow, C=fast). Publisher
/// sends 10,000 notifications to all three. B reads with 50ms delay.
///
/// Tests: does B's slowness affect A and C? With D12 (per-connection
/// writer threads), A and C should be unaffected.
///
/// Verify: A and C receive all messages promptly. B is eventually
/// torn down (server queue overflow) or severely behind.
#[test]
#[ignore]
fn backpressure_cascade_isolation() {
    const NUM_MSGS: usize = 10_000;

    with_timeout(
        "backpressure_cascade_isolation",
        Duration::from_secs(60),
        move || {
            let server = Arc::new(ProtocolServer::new());

            let (mut publisher, conn_pub) = connect_socket(&server, make_publisher_hello());

            // Subscribe three clients. Keep shutdown handles so we can
            // force-close connections from the main thread — blocking
            // reads on the subscriber threads won't exit on their own
            // after ServiceTeardown (that tears down the route, not
            // the connection).
            let (sub_a, sub_a_sess, pub_a_sess, conn_a) = subscribe_socket(&server, &mut publisher);
            let (sub_b, sub_b_sess, pub_b_sess, conn_b) = subscribe_socket(&server, &mut publisher);
            let sub_b_shutdown = sub_b.stream.try_clone().unwrap();
            let (sub_c, sub_c_sess, pub_c_sess, conn_c) = subscribe_socket(&server, &mut publisher);

            // Subscriber A: fast reader
            let a_handle = thread::spawn(move || {
                let mut sub = sub_a;
                let mut count = 0usize;
                let start = std::time::Instant::now();
                loop {
                    match sub.try_read_any() {
                        Ok(Frame::Message { service, .. }) if service == sub_a_sess => {
                            count += 1;
                            if count >= NUM_MSGS {
                                break;
                            }
                        }
                        Ok(Frame::Message { service: 0, .. }) => {} // control
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
                let elapsed = start.elapsed();
                drop(sub);
                conn_a.wait();
                (count, elapsed)
            });

            // Subscriber B: slow reader (50ms per message)
            let b_handle = thread::spawn(move || {
                let mut sub = sub_b;
                let mut count = 0usize;
                loop {
                    match sub.try_read_any() {
                        Ok(Frame::Message { service, .. }) if service == sub_b_sess => {
                            count += 1;
                            thread::sleep(Duration::from_millis(50));
                        }
                        Ok(Frame::Message { service: 0, .. }) => {}
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
                drop(sub);
                conn_b.wait();
                count
            });

            // Subscriber C: fast reader
            let c_handle = thread::spawn(move || {
                let mut sub = sub_c;
                let mut count = 0usize;
                let start = std::time::Instant::now();
                loop {
                    match sub.try_read_any() {
                        Ok(Frame::Message { service, .. }) if service == sub_c_sess => {
                            count += 1;
                            if count >= NUM_MSGS {
                                break;
                            }
                        }
                        Ok(Frame::Message { service: 0, .. }) => {}
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
                let elapsed = start.elapsed();
                drop(sub);
                conn_c.wait();
                (count, elapsed)
            });

            // Publisher sends to all three per message.
            // Fan-out: for each message, send to each subscriber's session.
            for seq in 0..NUM_MSGS as u64 {
                let payload = serialize_payload(seq);
                for &sess in &[pub_a_sess, pub_b_sess, pub_c_sess] {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        publisher.send_service(
                            sess,
                            &ServiceFrame::Notification {
                                payload: payload.clone(),
                            },
                        );
                    }));
                    if result.is_err() {
                        // Publisher write failed — likely B's overflow
                        // caused teardown. Skip B's session from now on.
                        // (In practice we just continue and let sends fail.)
                    }
                }
            }

            // Wait for fast readers with a generous timeout
            let _ = publisher
                .stream
                .set_read_timeout(Some(Duration::from_millis(500)));
            // Drain publisher's control messages
            while publisher.try_read_control().is_some() {}

            // Drop publisher to signal route teardown. Fast readers A
            // and C should already have exited (they break at NUM_MSGS).
            // Slow reader B needs its socket force-closed — ServiceTeardown
            // tears down the route but B's connection stays alive.
            drop(publisher);
            conn_pub.wait();
            let _ = sub_b_shutdown.shutdown(Shutdown::Both);

            let (a_count, a_elapsed) = a_handle.join().unwrap();
            let b_count = b_handle.join().unwrap();
            let (c_count, c_elapsed) = c_handle.join().unwrap();

            eprintln!(
                "backpressure_cascade_isolation:\n  \
                 A (fast): {a_count}/{NUM_MSGS} in {a_elapsed:?}\n  \
                 B (slow): {b_count}/{NUM_MSGS}\n  \
                 C (fast): {c_count}/{NUM_MSGS} in {c_elapsed:?}"
            );

            // Fast readers should get all messages
            assert_eq!(
                a_count, NUM_MSGS,
                "backpressure_cascade: A (fast) missed messages: {a_count}/{NUM_MSGS}"
            );
            assert_eq!(
                c_count, NUM_MSGS,
                "backpressure_cascade: C (fast) missed messages: {c_count}/{NUM_MSGS}"
            );

            // B should have received far fewer than the fast readers.
            // At 50ms/msg, it can read ~20/sec. 10000 msgs at 50ms each
            // would take 500 seconds. The test runs much shorter than
            // that, so B should be cut off by overflow or by publisher
            // EOF. The exact count depends on OS buffer sizes + server
            // channel capacity.
            eprintln!("backpressure_cascade: B read {b_count} msgs before disconnection");

            // D12 isolation check: A and C should finish in reasonable time.
            // If B's slowness cascaded, A and C would take minutes.
            assert!(
                a_elapsed < Duration::from_secs(10),
                "backpressure_cascade: A took {a_elapsed:?} — \
                 slow subscriber may be cascading"
            );
            assert!(
                c_elapsed < Duration::from_secs(10),
                "backpressure_cascade: C took {c_elapsed:?} — \
                 slow subscriber may be cascading"
            );
        },
    );
}

// ═══════════════════════════════════════════════════════════��════
// Test 12: Zero-length and boundary messages
// ═══════════════════════════════════════════════���════════════════

/// Send messages at boundary sizes: 0 bytes payload, 1 byte,
/// max_message_size - 1, max_message_size, max_message_size + 1
/// (should fail).
///
/// Tests: edge cases in frame codec.
///
/// Verify: valid sizes succeed, oversized is rejected without crash.
#[test]
#[ignore]
fn zero_length_and_boundary_messages() {
    with_timeout(
        "zero_length_and_boundary_messages",
        Duration::from_secs(15),
        move || {
            let server = Arc::new(ProtocolServer::new());

            // Use a small max_message_size for faster testing
            let small_max: u32 = 4096;
            let hello = Hello {
                version: 1,
                max_message_size: small_max,
                max_outstanding_requests: 0,
                interests: vec![],
                provides: vec![ServiceProvision {
                    service: topic_service_id(),
                    version: 1,
                }],
            };

            let (mut publisher, conn_pub) = connect_socket(&server, hello);

            let (sub, sub_session, pub_session, conn_sub) =
                subscribe_socket(&server, &mut publisher);
            let sub_shutdown = sub.stream.try_clone().unwrap();

            // Spawn subscriber reader
            let sub_handle = thread::spawn(move || {
                let mut sub = sub;
                let mut received = Vec::new();
                loop {
                    match sub.try_read_any() {
                        Ok(Frame::Message { service, payload }) if service == sub_session => {
                            received.push(payload.len());
                        }
                        Ok(Frame::Message { service: 0, .. }) => {} // control
                        Ok(_) => {}
                        Err(_) => break,
                    }
                }
                drop(sub);
                conn_sub.wait();
                received
            });

            // Test boundary sizes. The frame wire format is:
            // [length: u32 LE][service: u16 LE][payload]
            // where length = 2 + payload.len().
            // max_message_size applies to the length field.
            // So max payload = max_message_size - 2.
            //
            // ServiceFrame::Notification { payload } is then postcard-
            // encoded, adding more overhead. We test at the raw frame
            // level for precision.

            // Size 1: empty payload (0 bytes)
            {
                let sf = ServiceFrame::Notification { payload: vec![] };
                let bytes = postcard::to_allocvec(&sf).unwrap();
                publisher.send_service(pub_session, &sf);
                eprintln!("boundary: empty payload → {} wire bytes", bytes.len());
            }

            // Size 2: 1-byte payload
            {
                let sf = ServiceFrame::Notification {
                    payload: vec![0x42],
                };
                publisher.send_service(pub_session, &sf);
            }

            // Size 3: large payload near max_message_size.
            // The wire frame length = 2 (service) + postcard_bytes.
            // We want the total frame body (service + payload) to be
            // just under max_message_size. postcard overhead for
            // Notification{payload: Vec<u8>} is ~2-5 bytes for the
            // enum tag + length prefix. So we try a payload of
            // max_message_size - 20 bytes to be safe.
            {
                let big_payload = vec![0xBB; (small_max as usize).saturating_sub(20)];
                let sf = ServiceFrame::Notification {
                    payload: big_payload,
                };
                let bytes = postcard::to_allocvec(&sf).unwrap();
                let wire_len = 2 + bytes.len(); // service + payload
                eprintln!("boundary: big payload → wire_len={wire_len}, max={small_max}");

                if wire_len <= small_max as usize {
                    publisher.send_service(pub_session, &sf);
                } else {
                    eprintln!("boundary: big payload exceeds max, skipping send");
                }
            }

            // Size 4: oversized — raw bytes on the wire that claim to
            // be larger than max. This goes to the server's reader thread,
            // which should reject it and tear down the connection.
            // We send this on a SEPARATE connection so the publisher survives.
            {
                let (client_sock, server_sock) = UnixStream::pair().unwrap();
                let server_ref = Arc::clone(&server);
                let accept = thread::spawn(move || {
                    let writer = server_sock.try_clone().expect("try_clone");
                    let _ = server_ref.accept(server_sock, writer);
                });

                let (mut bad, _) = SocketConn::connect(
                    client_sock,
                    Hello {
                        version: 1,
                        max_message_size: small_max,
                        max_outstanding_requests: 0,
                        interests: vec![],
                        provides: vec![],
                    },
                );
                bad.expect_ready();

                // Write a frame claiming length = max + 100
                let oversized_len = small_max + 100;
                let _ = bad.stream.write_all(&oversized_len.to_le_bytes());
                // Don't bother writing the body — the server should
                // reject on length alone.
                let _ = bad.stream.flush();
                thread::sleep(Duration::from_millis(50));

                drop(bad);
                let _ = accept.join();
            }

            // Signal subscriber to stop: drop publisher (triggers route
            // teardown), then force-close subscriber socket so its
            // blocking read returns EOF.
            drop(publisher);
            conn_pub.wait();
            let _ = sub_shutdown.shutdown(Shutdown::Both);

            let received_sizes = sub_handle.join().unwrap();
            eprintln!(
                "zero_length_and_boundary_messages: subscriber received {} frames, \
                 sizes: {:?}",
                received_sizes.len(),
                received_sizes
            );

            // We should have received at least the first 2 messages
            // (empty and 1-byte). The big one depends on whether it
            // fit within max_message_size.
            assert!(
                received_sizes.len() >= 2,
                "boundary: expected at least 2 messages, got {}",
                received_sizes.len()
            );
        },
    );
}
