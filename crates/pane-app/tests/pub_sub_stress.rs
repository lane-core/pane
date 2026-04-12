//! Pub/sub stress tests.
//!
//! Exercises the push and long-poll patterns under load: fan-out
//! width, throughput, subscriber churn, mixed modes, and
//! backpressure. All tests use #[ignore] — run with
//! `cargo test --test pub_sub_stress -- --ignored`.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use pane_proto::control::{ControlMessage, TeardownReason};
use pane_proto::protocol::ServiceId;
use pane_proto::service_frame::ServiceFrame;
use pane_session::bridge::HANDSHAKE_MAX_MESSAGE_SIZE;
use pane_session::frame::{Frame, FrameCodec};
use pane_session::handshake::{Hello, Rejection, ServiceProvision, Welcome};
use pane_session::server::ProtocolServer;
use pane_session::transport::MemoryTransport;

use serde::{Deserialize, Serialize};
use std::os::unix::net::UnixStream;

fn topic_service_id() -> ServiceId {
    ServiceId::new("com.pane.stress.topic")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum TopicMessage {
    NextUpdate,
    Update { seq: u64, data: String },
}

// -- Wire helpers --

struct ClientConn {
    transport: MemoryTransport,
    codec: FrameCodec,
}

impl ClientConn {
    fn connect(mut transport: MemoryTransport, hello: Hello) -> (Self, Welcome) {
        let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);
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
) -> thread::JoinHandle<pane_session::server::ConnectionHandle> {
    let server = Arc::clone(server);
    thread::spawn(move || {
        let (r, w) = transport.split();
        server.accept(r, w).expect("accept failed")
    })
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

fn tag() -> u8 {
    topic_service_id().tag()
}

fn serialize_topic(msg: &TopicMessage) -> Vec<u8> {
    let msg_bytes = postcard::to_allocvec(msg).unwrap();
    let mut inner = Vec::with_capacity(1 + msg_bytes.len());
    inner.push(tag());
    inner.extend_from_slice(&msg_bytes);
    inner
}

fn deserialize_topic(payload: &[u8]) -> TopicMessage {
    assert_eq!(payload[0], tag(), "protocol tag mismatch");
    postcard::from_bytes(&payload[1..]).unwrap()
}

/// Connect a subscriber, DeclareInterest, return (conn, sub_session, pub_session).
/// The publisher must call read_control() to consume its InterestAccepted.
fn subscribe(
    server: &Arc<ProtocolServer>,
    publisher: &mut ClientConn,
) -> (ClientConn, u16, u16, pane_session::server::ConnectionHandle) {
    let topic_id = topic_service_id();
    let (sub_client, sub_server) = MemoryTransport::pair();
    let accept = accept_on_thread(server, sub_server);
    let (mut sub, _) = ClientConn::connect(sub_client, make_hello(vec![]));
    let conn = accept.join().unwrap();
    sub.expect_ready();

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

    (sub, sub_session, pub_session, conn)
}

// -- Stress tests --

/// 1 publisher, 50 subscribers, 100 notifications each.
/// Publisher sends to each subscriber per notification (5000 frames).
/// Verify all subscribers receive all 100 in order.
#[test]
#[ignore]
fn stress_push_fan_out() {
    const NUM_SUBS: usize = 50;
    const NUM_MSGS: usize = 100;

    let server = Arc::new(ProtocolServer::new());
    let topic_id = topic_service_id();

    // Publisher
    let (pub_client, pub_server) = MemoryTransport::pair();
    let accept_pub = accept_on_thread(&server, pub_server);
    let (mut publisher, _) = ClientConn::connect(
        pub_client,
        make_hello(vec![ServiceProvision {
            service: topic_id,
            version: 1,
        }]),
    );
    let conn_pub = accept_pub.join().unwrap();
    publisher.expect_ready();

    // Subscribe all
    let mut subs: Vec<(ClientConn, u16, u16, pane_session::server::ConnectionHandle)> =
        Vec::with_capacity(NUM_SUBS);
    for _ in 0..NUM_SUBS {
        subs.push(subscribe(&server, &mut publisher));
    }

    // Collect pub_sessions for fan-out
    let pub_sessions: Vec<u16> = subs.iter().map(|s| s.2).collect();

    // Publisher sends NUM_MSGS notifications to all subscribers
    for seq in 0..NUM_MSGS as u64 {
        let payload = serialize_topic(&TopicMessage::Update {
            seq,
            data: format!("msg-{seq}"),
        });
        for &pub_session in &pub_sessions {
            publisher.send_service(
                pub_session,
                &ServiceFrame::Notification {
                    payload: payload.clone(),
                },
            );
        }
    }

    // Each subscriber reads all messages on its own thread
    let received = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for (mut sub, sub_session, _, conn) in subs {
        let received = Arc::clone(&received);
        handles.push(thread::spawn(move || {
            for expected_seq in 0..NUM_MSGS as u64 {
                let (session, frame) = sub.read_service();
                assert_eq!(session, sub_session);
                match frame {
                    ServiceFrame::Notification { payload } => {
                        let msg = deserialize_topic(&payload);
                        match msg {
                            TopicMessage::Update { seq, .. } => {
                                assert_eq!(seq, expected_seq, "out-of-order delivery");
                                received.fetch_add(1, Ordering::Relaxed);
                            }
                            other => panic!("expected Update, got {other:?}"),
                        }
                    }
                    other => panic!("expected Notification, got {other:?}"),
                }
            }
            drop(sub);
            conn.wait();
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(
        received.load(Ordering::Relaxed),
        NUM_SUBS * NUM_MSGS,
        "not all messages received"
    );

    drop(publisher);
    conn_pub.wait();
}

/// 1 publisher, 20 subscribers, each with a pending NextUpdate.
/// Publisher responds to all 20 in a tight loop. 50 rounds.
/// Verify no lost messages, correct token matching.
#[test]
#[ignore]
fn stress_long_poll_burst() {
    const NUM_SUBS: usize = 20;
    const NUM_ROUNDS: usize = 50;

    let server = Arc::new(ProtocolServer::new());
    let topic_id = topic_service_id();

    // Publisher
    let (pub_client, pub_server) = MemoryTransport::pair();
    let accept_pub = accept_on_thread(&server, pub_server);
    let (mut publisher, _) = ClientConn::connect(
        pub_client,
        make_hello(vec![ServiceProvision {
            service: topic_id,
            version: 1,
        }]),
    );
    let conn_pub = accept_pub.join().unwrap();
    publisher.expect_ready();

    // Subscribe all
    let mut subs = Vec::with_capacity(NUM_SUBS);
    for _ in 0..NUM_SUBS {
        subs.push(subscribe(&server, &mut publisher));
    }

    let received = Arc::new(AtomicUsize::new(0));

    // Spawn subscriber threads — each sends NextUpdate, reads reply, repeats
    let mut sub_handles = Vec::new();
    for (mut sub, sub_session, _, conn) in subs {
        let received = Arc::clone(&received);
        sub_handles.push(thread::spawn(move || {
            for round in 0..NUM_ROUNDS {
                let token = round as u64 + 1;
                sub.send_service(
                    sub_session,
                    &ServiceFrame::Request {
                        token,
                        payload: serialize_topic(&TopicMessage::NextUpdate),
                    },
                );

                let (session, frame) = sub.read_service();
                assert_eq!(session, sub_session);
                match frame {
                    ServiceFrame::Reply {
                        token: t, payload, ..
                    } => {
                        assert_eq!(t, token, "token mismatch");
                        let msg = deserialize_topic(&payload);
                        match msg {
                            TopicMessage::Update { seq, .. } => {
                                assert_eq!(seq, round as u64);
                                received.fetch_add(1, Ordering::Relaxed);
                            }
                            other => panic!("expected Update, got {other:?}"),
                        }
                    }
                    other => panic!("expected Reply, got {other:?}"),
                }
            }
            drop(sub);
            conn.wait();
        }));
    }

    // Publisher loop: read requests, respond immediately
    // Each round: read NUM_SUBS requests, respond to each.
    for round in 0..NUM_ROUNDS {
        // Collect all pending requests for this round
        let mut pending: Vec<(u16, u64)> = Vec::with_capacity(NUM_SUBS);
        for _ in 0..NUM_SUBS {
            let (pub_session, frame) = publisher.read_service();
            match frame {
                ServiceFrame::Request { token, payload } => {
                    let msg = deserialize_topic(&payload);
                    assert_eq!(msg, TopicMessage::NextUpdate);
                    pending.push((pub_session, token));
                }
                other => panic!("expected Request, got {other:?}"),
            }
        }

        // Respond to all in a burst
        for (pub_session, token) in pending {
            publisher.send_service(
                pub_session,
                &ServiceFrame::Reply {
                    token,
                    payload: serialize_topic(&TopicMessage::Update {
                        seq: round as u64,
                        data: format!("round-{round}"),
                    }),
                },
            );
        }
    }

    for h in sub_handles {
        h.join().unwrap();
    }

    assert_eq!(
        received.load(Ordering::Relaxed),
        NUM_SUBS * NUM_ROUNDS,
        "not all replies received"
    );

    drop(publisher);
    conn_pub.wait();
}

/// 1 publisher pushing continuously. 10 long-lived subscribers +
/// 20 transient subscribers that connect, receive 5 messages, and
/// disconnect. Publisher detects each disconnect via ServiceTeardown.
#[test]
#[ignore]
fn stress_subscriber_churn() {
    const LONG_LIVED: usize = 10;
    const TRANSIENT: usize = 20;
    const TRANSIENT_MSG_COUNT: usize = 5;
    const TOTAL_MSGS: usize = 50;

    let server = Arc::new(ProtocolServer::new());
    let topic_id = topic_service_id();

    // Publisher
    let (pub_client, pub_server) = MemoryTransport::pair();
    let accept_pub = accept_on_thread(&server, pub_server);
    let (mut publisher, _) = ClientConn::connect(
        pub_client,
        make_hello(vec![ServiceProvision {
            service: topic_id,
            version: 1,
        }]),
    );
    let conn_pub = accept_pub.join().unwrap();
    publisher.expect_ready();

    // Long-lived subscribers
    let mut long_subs = Vec::with_capacity(LONG_LIVED);
    let mut pub_sessions: Vec<(u16, bool)> = Vec::new(); // (session, alive)
    for _ in 0..LONG_LIVED {
        let (sub, _sub_session, pub_session, conn) = subscribe(&server, &mut publisher);
        pub_sessions.push((pub_session, true));
        long_subs.push((sub, _sub_session, conn));
    }

    // Transient subscribers — connect, subscribe, then spawned to
    // read TRANSIENT_MSG_COUNT and disconnect
    let mut transient_handles = Vec::new();
    let teardowns_received = Arc::new(AtomicUsize::new(0));

    for _ in 0..TRANSIENT {
        let (sub, _sub_session, pub_session, conn) = subscribe(&server, &mut publisher);
        pub_sessions.push((pub_session, true));

        let handle = thread::spawn(move || {
            let mut sub = sub;
            for _ in 0..TRANSIENT_MSG_COUNT {
                let _ = sub.read_service();
            }
            // Disconnect after receiving enough
            drop(sub);
            conn.wait();
        });
        transient_handles.push(handle);
    }

    // Publisher sends TOTAL_MSGS to all alive subscribers.
    // After each send round, check for ServiceTeardown and mark dead.
    for seq in 0..TOTAL_MSGS as u64 {
        let payload = serialize_topic(&TopicMessage::Update {
            seq,
            data: format!("churn-{seq}"),
        });

        for (pub_session, alive) in pub_sessions.iter() {
            if !alive {
                continue;
            }
            // Best-effort send — may fail if subscriber already disconnected
            // and the server has torn down the route. The server will drop
            // frames for dead routes silently.
            publisher.send_service(
                *pub_session,
                &ServiceFrame::Notification {
                    payload: payload.clone(),
                },
            );
        }

        // Drain any ServiceTeardown messages that arrived
        // (non-blocking: we check what's available between sends)
        // Since MemoryTransport blocks on read, we can't truly
        // non-block. Instead we'll collect teardowns after all
        // sends complete.
    }

    // Wait for transient subscribers to finish
    for h in transient_handles {
        h.join().unwrap();
    }

    // Now drain all ServiceTeardown messages from the publisher's
    // control channel. Each transient subscriber generates one.
    for _ in 0..TRANSIENT {
        match publisher.read_control() {
            ControlMessage::ServiceTeardown { reason, .. } => {
                assert!(matches!(reason, TeardownReason::ConnectionLost));
                teardowns_received.fetch_add(1, Ordering::Relaxed);
            }
            other => panic!("expected ServiceTeardown, got {other:?}"),
        }
    }

    assert_eq!(
        teardowns_received.load(Ordering::Relaxed),
        TRANSIENT,
        "not all transient disconnects detected"
    );

    // Long-lived subscribers should have received all TOTAL_MSGS
    let long_received = Arc::new(AtomicUsize::new(0));
    let mut long_handles = Vec::new();
    for (mut sub, sub_session, conn) in long_subs {
        let long_received = Arc::clone(&long_received);
        long_handles.push(thread::spawn(move || {
            for _ in 0..TOTAL_MSGS {
                let (session, frame) = sub.read_service();
                assert_eq!(session, sub_session);
                match frame {
                    ServiceFrame::Notification { .. } => {
                        long_received.fetch_add(1, Ordering::Relaxed);
                    }
                    other => panic!("expected Notification, got {other:?}"),
                }
            }
            drop(sub);
            conn.wait();
        }));
    }

    for h in long_handles {
        h.join().unwrap();
    }

    assert_eq!(
        long_received.load(Ordering::Relaxed),
        LONG_LIVED * TOTAL_MSGS,
        "long-lived subscribers missed messages"
    );

    drop(publisher);
    conn_pub.wait();
}

/// 1 publisher, 10 push subscribers, 10 poll subscribers.
/// Publisher fans out same update to push subs (Notification)
/// and responds to pending poll requests (Reply). 100 rounds.
#[test]
#[ignore]
fn stress_mixed_push_and_poll() {
    const PUSH_SUBS: usize = 10;
    const POLL_SUBS: usize = 10;
    const NUM_ROUNDS: usize = 100;

    let server = Arc::new(ProtocolServer::new());
    let topic_id = topic_service_id();

    // Publisher
    let (pub_client, pub_server) = MemoryTransport::pair();
    let accept_pub = accept_on_thread(&server, pub_server);
    let (mut publisher, _) = ClientConn::connect(
        pub_client,
        make_hello(vec![ServiceProvision {
            service: topic_id,
            version: 1,
        }]),
    );
    let conn_pub = accept_pub.join().unwrap();
    publisher.expect_ready();

    // Push subscribers
    let mut push_subs = Vec::with_capacity(PUSH_SUBS);
    let mut push_pub_sessions = Vec::with_capacity(PUSH_SUBS);
    for _ in 0..PUSH_SUBS {
        let (sub, sub_session, pub_session, conn) = subscribe(&server, &mut publisher);
        push_pub_sessions.push(pub_session);
        push_subs.push((sub, sub_session, conn));
    }

    // Poll subscribers
    let mut poll_subs = Vec::with_capacity(POLL_SUBS);
    for _ in 0..POLL_SUBS {
        let (sub, sub_session, _, conn) = subscribe(&server, &mut publisher);
        poll_subs.push((sub, sub_session, conn));
    }

    let push_received = Arc::new(AtomicUsize::new(0));
    let poll_received = Arc::new(AtomicUsize::new(0));

    // Spawn push subscriber readers
    let mut push_handles = Vec::new();
    for (mut sub, sub_session, conn) in push_subs {
        let push_received = Arc::clone(&push_received);
        push_handles.push(thread::spawn(move || {
            for expected in 0..NUM_ROUNDS as u64 {
                let (session, frame) = sub.read_service();
                assert_eq!(session, sub_session);
                match frame {
                    ServiceFrame::Notification { payload } => {
                        let msg = deserialize_topic(&payload);
                        match msg {
                            TopicMessage::Update { seq, .. } => {
                                assert_eq!(seq, expected);
                                push_received.fetch_add(1, Ordering::Relaxed);
                            }
                            other => panic!("expected Update, got {other:?}"),
                        }
                    }
                    other => panic!("expected Notification, got {other:?}"),
                }
            }
            drop(sub);
            conn.wait();
        }));
    }

    // Spawn poll subscriber threads — send request, read reply, repeat
    let mut poll_handles = Vec::new();
    for (mut sub, sub_session, conn) in poll_subs {
        let poll_received = Arc::clone(&poll_received);
        poll_handles.push(thread::spawn(move || {
            for round in 0..NUM_ROUNDS {
                let token = round as u64 + 1;
                sub.send_service(
                    sub_session,
                    &ServiceFrame::Request {
                        token,
                        payload: serialize_topic(&TopicMessage::NextUpdate),
                    },
                );

                let (session, frame) = sub.read_service();
                assert_eq!(session, sub_session);
                match frame {
                    ServiceFrame::Reply { token: t, payload } => {
                        assert_eq!(t, token);
                        let msg = deserialize_topic(&payload);
                        match msg {
                            TopicMessage::Update { seq, .. } => {
                                assert_eq!(seq, round as u64);
                                poll_received.fetch_add(1, Ordering::Relaxed);
                            }
                            other => panic!("expected Update reply, got {other:?}"),
                        }
                    }
                    other => panic!("expected Reply, got {other:?}"),
                }
            }
            drop(sub);
            conn.wait();
        }));
    }

    // Publisher loop: each round, collect POLL_SUBS requests, then
    // fan out update to push subs and reply to poll subs.
    for round in 0..NUM_ROUNDS {
        // Collect poll requests
        let mut pending_polls: Vec<(u16, u64)> = Vec::with_capacity(POLL_SUBS);
        for _ in 0..POLL_SUBS {
            let (pub_session, frame) = publisher.read_service();
            match frame {
                ServiceFrame::Request { token, .. } => {
                    pending_polls.push((pub_session, token));
                }
                other => panic!("round {round}: expected Request, got {other:?}"),
            }
        }

        let payload = serialize_topic(&TopicMessage::Update {
            seq: round as u64,
            data: format!("mixed-{round}"),
        });

        // Push to notification subscribers
        for &pub_session in &push_pub_sessions {
            publisher.send_service(
                pub_session,
                &ServiceFrame::Notification {
                    payload: payload.clone(),
                },
            );
        }

        // Reply to poll subscribers
        for (pub_session, token) in pending_polls {
            publisher.send_service(
                pub_session,
                &ServiceFrame::Reply {
                    token,
                    payload: payload.clone(),
                },
            );
        }
    }

    for h in push_handles {
        h.join().unwrap();
    }
    for h in poll_handles {
        h.join().unwrap();
    }

    assert_eq!(
        push_received.load(Ordering::Relaxed),
        PUSH_SUBS * NUM_ROUNDS,
        "push subscribers missed messages"
    );
    assert_eq!(
        poll_received.load(Ordering::Relaxed),
        POLL_SUBS * NUM_ROUNDS,
        "poll subscribers missed messages"
    );

    drop(publisher);
    conn_pub.wait();
}

/// 1 publisher, 1 fast subscriber, 1 slow subscriber (reads with delay).
/// Publisher pushes at full speed. Uses MemoryTransport (unbounded channels).
///
/// **Finding:** the ProtocolServer buffers between publisher and
/// subscriber — backpressure does NOT propagate from slow subscriber
/// back to publisher. The publisher writes to its own server
/// connection, and the server forwards to each subscriber's
/// connection independently. These are separate channels.
///
/// This means the backpressure boundary (D7/D9) lives between the
/// server and each subscriber, not between publisher and server.
/// For the publisher to be backpressure-aware per-subscriber, it
/// would need per-subscriber flow control signals — which is
/// exactly what try_send_notification on SubscriberSender would
/// provide once ConnectionSource (C4) replaces the bridge threads.
///
/// This test verifies both subscribers receive all messages despite
/// the speed difference, and that the publisher is NOT stalled.
#[test]
#[ignore]
fn stress_backpressure_slow_subscriber() {
    const NUM_MSGS: usize = 200;
    const SLOW_DELAY_MS: u64 = 5;

    let server = Arc::new(ProtocolServer::new());
    let topic_id = topic_service_id();

    // Publisher
    let (pub_client, pub_server) = MemoryTransport::pair();
    let accept_pub = accept_on_thread(&server, pub_server);
    let (mut publisher, _) = ClientConn::connect(
        pub_client,
        make_hello(vec![ServiceProvision {
            service: topic_id,
            version: 1,
        }]),
    );
    let conn_pub = accept_pub.join().unwrap();
    publisher.expect_ready();

    // Fast subscriber
    let (fast_sub, fast_session, fast_pub_session, fast_conn) = subscribe(&server, &mut publisher);
    let fast_received = Arc::new(AtomicUsize::new(0));
    let fast_done = Arc::new(AtomicBool::new(false));

    let fast_received2 = Arc::clone(&fast_received);
    let fast_done2 = Arc::clone(&fast_done);
    let fast_handle = thread::spawn(move || {
        let mut sub = fast_sub;
        for _ in 0..NUM_MSGS {
            let (session, frame) = sub.read_service();
            assert_eq!(session, fast_session);
            match frame {
                ServiceFrame::Notification { .. } => {
                    fast_received2.fetch_add(1, Ordering::Relaxed);
                }
                other => panic!("fast: expected Notification, got {other:?}"),
            }
        }
        fast_done2.store(true, Ordering::Release);
        drop(sub);
        fast_conn.wait();
    });

    // Slow subscriber — reads with delay
    let (slow_sub, slow_session, slow_pub_session, slow_conn) = subscribe(&server, &mut publisher);
    let slow_received = Arc::new(AtomicUsize::new(0));

    let slow_received2 = Arc::clone(&slow_received);
    let slow_handle = thread::spawn(move || {
        let mut sub = slow_sub;
        for _ in 0..NUM_MSGS {
            thread::sleep(Duration::from_millis(SLOW_DELAY_MS));
            let (session, frame) = sub.read_service();
            assert_eq!(session, slow_session);
            match frame {
                ServiceFrame::Notification { .. } => {
                    slow_received2.fetch_add(1, Ordering::Relaxed);
                }
                other => panic!("slow: expected Notification, got {other:?}"),
            }
        }
        drop(sub);
        slow_conn.wait();
    });

    // Publisher sends to both at full speed.
    // With blocking SyncSender, the slow subscriber will stall
    // the publisher after the channel fills (128 slots).
    let start = std::time::Instant::now();
    for seq in 0..NUM_MSGS as u64 {
        let payload = serialize_topic(&TopicMessage::Update {
            seq,
            data: format!("bp-{seq}"),
        });

        publisher.send_service(
            fast_pub_session,
            &ServiceFrame::Notification {
                payload: payload.clone(),
            },
        );
        publisher.send_service(slow_pub_session, &ServiceFrame::Notification { payload });
    }
    let elapsed = start.elapsed();

    fast_handle.join().unwrap();
    slow_handle.join().unwrap();

    let fast_count = fast_received.load(Ordering::Relaxed);
    let slow_count = slow_received.load(Ordering::Relaxed);

    assert_eq!(fast_count, NUM_MSGS, "fast subscriber missed messages");
    assert_eq!(slow_count, NUM_MSGS, "slow subscriber missed messages");

    // The publisher is NOT stalled — the server buffers between
    // publisher and subscriber connections. Publisher finishes fast
    // because it writes to its own server connection, not directly
    // to subscribers.
    eprintln!(
        "backpressure test: publisher sent {NUM_MSGS} msgs to 2 subs in {elapsed:?} \
         (fast got {fast_count}, slow got {slow_count})"
    );

    // Publisher should finish well before the slow subscriber drains.
    // Slow subscriber takes ~NUM_MSGS * SLOW_DELAY_MS = ~1s.
    // Publisher should finish in milliseconds.
    let slow_floor = Duration::from_millis(SLOW_DELAY_MS * NUM_MSGS as u64 / 4);
    assert!(
        elapsed < slow_floor,
        "publisher was stalled by slow subscriber ({elapsed:?} >= {slow_floor:?}) — \
         backpressure propagated unexpectedly"
    );

    drop(publisher);
    conn_pub.wait();
}

// -- Real socket wire helpers --

/// Wire helper for UnixStream connections. Same interface as ClientConn
/// but backed by real OS sockets with finite buffers.
/// Scaffolded for future full-stack pub/sub stress tests over real sockets.
#[allow(dead_code)]
struct SocketConn {
    stream: UnixStream,
    codec: FrameCodec,
}

#[allow(dead_code)]
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
}

#[allow(dead_code)]
fn accept_unix_on_thread(
    server: &Arc<ProtocolServer>,
    stream: UnixStream,
) -> thread::JoinHandle<pane_session::server::ConnectionHandle> {
    let server = Arc::clone(server);
    thread::spawn(move || {
        let writer = stream.try_clone().expect("UnixStream::try_clone failed");
        server.accept(stream, writer).expect("accept failed")
    })
}

/// Subscribe a SocketConn to the publisher's topic. Returns
/// (subscriber, sub_session, pub_session, conn_handle).
#[allow(dead_code)]
fn subscribe_socket(
    server: &Arc<ProtocolServer>,
    publisher: &mut SocketConn,
) -> (SocketConn, u16, u16, pane_session::server::ConnectionHandle) {
    let topic_id = topic_service_id();
    let (sub_client, sub_server) = UnixStream::pair().unwrap();
    let accept = accept_unix_on_thread(server, sub_server);
    let (mut sub, _) = SocketConn::connect(sub_client, make_hello(vec![]));
    let conn = accept.join().unwrap();
    sub.expect_ready();

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

    (sub, sub_session, pub_session, conn)
}

/// Direct socket backpressure: writer → UnixStream → slow reader.
///
/// No ProtocolServer intermediary — exercises the raw socket
/// backpressure boundary that ConnectionSource will manage.
/// When the reader is slow, OS socket buffers (8KB send + 8KB
/// recv on macOS) fill up and the writer's `write()` blocks.
///
/// This is the backpressure chain that ConnectionSource (C4)
/// converts from blocking stalls into WouldBlock + calloop
/// reregistration. The test establishes the baseline: real
/// sockets DO propagate backpressure, unlike MemoryTransport's
/// unbounded channels.
///
/// Uses pane's FrameCodec wire format — same frames that
/// ConnectionSource will read/write.
#[test]
#[ignore]
fn stress_backpressure_real_sockets() {
    const NUM_MSGS: usize = 1000;
    const SLOW_DELAY_MS: u64 = 5;

    let (mut writer_stream, mut reader_stream) = UnixStream::pair().unwrap();
    let codec_w = FrameCodec::new(16 * 1024 * 1024);
    let codec_r = FrameCodec::permissive(16 * 1024 * 1024);

    let received = Arc::new(AtomicUsize::new(0));
    let received2 = Arc::clone(&received);

    // Slow reader thread: reads frames with a delay.
    let reader_handle = thread::spawn(move || {
        for _ in 0..NUM_MSGS {
            thread::sleep(Duration::from_millis(SLOW_DELAY_MS));
            match codec_r.read_frame(&mut reader_stream) {
                Ok(Frame::Message { service: 1, .. }) => {
                    received2.fetch_add(1, Ordering::Relaxed);
                }
                Ok(other) => panic!("reader: unexpected frame: {other:?}"),
                Err(e) => panic!("reader: read error: {e}"),
            }
        }
    });

    // Writer: pushes frames at full speed on the main thread.
    // Each frame is ~25 bytes on the wire. 1000 frames = ~25KB,
    // which exceeds macOS's 8KB+8KB=16KB unix socket buffer.
    // Writer MUST stall once the buffer fills.
    let start = std::time::Instant::now();
    for seq in 0..NUM_MSGS as u64 {
        let payload = serialize_topic(&TopicMessage::Update {
            seq,
            data: format!("bp-{seq}"),
        });
        let sf = ServiceFrame::Notification { payload };
        let bytes = postcard::to_allocvec(&sf).unwrap();
        codec_w.write_frame(&mut writer_stream, 1, &bytes).unwrap();
    }
    let writer_elapsed = start.elapsed();

    reader_handle.join().unwrap();

    let count = received.load(Ordering::Relaxed);
    assert_eq!(count, NUM_MSGS, "reader missed messages");

    // The writer SHOULD have been stalled by the slow reader.
    // Slow reader takes NUM_MSGS * SLOW_DELAY_MS = 5s.
    // If backpressure works, the writer should take a significant
    // fraction of that — certainly more than 1 second.
    let expected_minimum = Duration::from_millis(SLOW_DELAY_MS * NUM_MSGS as u64 / 4);
    eprintln!(
        "direct socket backpressure: writer sent {NUM_MSGS} frames in \
         {writer_elapsed:?} (reader got {count}, minimum {expected_minimum:?})"
    );

    assert!(
        writer_elapsed > expected_minimum,
        "writer was NOT stalled by slow reader ({writer_elapsed:?} < \
         {expected_minimum:?}) — OS socket backpressure failed to propagate"
    );
}

/// D2 bridge wireup + backpressure: exercises the full
/// connect_unix → NewConnection → ConnectionSource registration
/// → backpressure propagation chain.
///
/// This is the first test that exercises the complete D2 Option A
/// handshake handoff path (bridge.rs connect_unix):
///   1. Bridge runs handshake on blocking UnixStream (CBOR Hello/Welcome)
///   2. Bridge sends NewConnection through the looper's mpsc channel
///   3. Calloop loop registers ConnectionSource on the EventLoop
///   4. Bridge blocks until ack, then returns write_tx
///   5. Sender pushes frames through write_tx at full speed
///   6. ConnectionSource writes to socket → slow peer → backpressure
///
/// The slow peer reads with a 10ms delay per 4KB frame. OS socket
/// buffers (~16KB on macOS) fill within a few frames. Once full,
/// ConnectionSource gets WouldBlock → write queue hits highwater →
/// channel stays full → SyncSender::send blocks → sender is stalled.
///
/// Verifies: all frames delivered, sender was stalled by backpressure,
/// and the D2 handoff machinery (NewConnection → ack) works correctly.
///
/// Design heritage: Plan 9 devmnt.c mntversion → mntattach had no
/// partial-mount code path. A ConnectionSource that exists is a
/// session that speaks the protocol.
#[test]
#[ignore]
fn stress_bridge_wireup_backpressure() {
    use calloop::channel as calloop_channel;
    use pane_app::ConnectionSource;
    use pane_session::bridge::{self, LooperMessage, Registered};
    use std::sync::mpsc;

    const NUM_MSGS: usize = 200;
    const SLOW_DELAY_MS: u64 = 10;
    // 4KB payload per message. 128 (channel cap) * 4KB = 512KB in
    // flight, far exceeding the ~16KB unix socket buffer on macOS.
    // This ensures the socket fills and WouldBlock propagates.
    const PAYLOAD_SIZE: usize = 4096;

    let (client_stream, mut server_stream) = UnixStream::pair().unwrap();

    // Server thread: perform server-side handshake (blocking I/O),
    // send Ready (bootstrap for ConnectionSource readability), then
    // read frames slowly to create backpressure.
    let received = Arc::new(AtomicUsize::new(0));
    let received2 = Arc::clone(&received);
    let server_handle = thread::spawn(move || {
        let codec = FrameCodec::new(HANDSHAKE_MAX_MESSAGE_SIZE);

        // Read Hello (CBOR)
        let frame = codec.read_frame(&mut server_stream).unwrap();
        let payload = match frame {
            Frame::Message {
                service: 0,
                payload,
            } => payload,
            other => panic!("server: expected Hello, got {other:?}"),
        };
        let _hello: Hello = ciborium::de::from_reader(payload.as_slice()).unwrap();

        // Send Welcome (CBOR)
        let decision: Result<Welcome, Rejection> = Ok(Welcome {
            version: 1,
            instance_id: "bridge-wireup-test".into(),
            max_message_size: 16 * 1024 * 1024,
            max_outstanding_requests: 0,
            bindings: vec![],
        });
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(&decision, &mut bytes).unwrap();
        codec.write_frame(&mut server_stream, 0, &bytes).unwrap();

        // Bootstrap: send Ready so ConnectionSource gets a readability
        // wake and can begin draining its write channel.
        let ready =
            ControlMessage::Lifecycle(pane_proto::protocols::lifecycle::LifecycleMessage::Ready);
        let ready_bytes = postcard::to_allocvec(&ready).unwrap();
        let data_codec = FrameCodec::permissive(16 * 1024 * 1024);
        data_codec
            .write_frame(&mut server_stream, 0, &ready_bytes)
            .unwrap();

        // Slow reader: read frames with a delay to create backpressure.
        for _ in 0..NUM_MSGS {
            thread::sleep(Duration::from_millis(SLOW_DELAY_MS));
            match data_codec.read_frame(&mut server_stream) {
                Ok(Frame::Message { service: 1, .. }) => {
                    received2.fetch_add(1, Ordering::Relaxed);
                }
                Ok(other) => panic!("server: unexpected frame: {other:?}"),
                Err(e) => panic!("server: read error: {e}"),
            }
        }
    });

    // -- D2 machinery: calloop loop thread + forwarding thread --

    // The looper's mpsc channel: connect_unix sends NewConnection
    // through looper_tx; the calloop loop consumes from looper_rx.
    let (looper_tx, looper_rx) = mpsc::channel::<LooperMessage>();

    // Calloop channel: the forwarding thread bridges mpsc → calloop.
    let (calloop_tx, calloop_rx) = calloop_channel::channel::<LooperMessage>();

    // Forwarding thread: same pattern as Looper::run — blocks for
    // one message, then drains immediately available.
    let _forwarder = thread::spawn(move || {
        while let Ok(first) = looper_rx.recv() {
            if calloop_tx.send(first).is_err() {
                break;
            }
            while let Ok(msg) = looper_rx.try_recv() {
                if calloop_tx.send(msg).is_err() {
                    return;
                }
            }
        }
    });

    // Calloop loop thread: processes NewConnection (registers
    // ConnectionSource), then pumps the event loop to drive I/O.
    let received3 = Arc::clone(&received);
    let loop_handle = thread::spawn(move || {
        let mut event_loop: calloop::EventLoop<Vec<LooperMessage>> =
            calloop::EventLoop::try_new().expect("event loop creation failed");

        let handle = event_loop.handle();

        // Register the calloop channel. When NewConnection arrives,
        // extract the stream and write_rx, build ConnectionSource,
        // register it, and ack the bridge.
        let loop_handle_inner = handle.clone();
        handle
            .insert_source(
                calloop_rx,
                move |event, _, state: &mut Vec<LooperMessage>| {
                    if let calloop_channel::Event::Msg(msg) = event {
                        match msg {
                            LooperMessage::NewConnection {
                                welcome,
                                stream,
                                write_rx,
                                ack,
                            } => {
                                // D2 step 3: create ConnectionSource and
                                // register on the calloop EventLoop.
                                let source = ConnectionSource::new(
                                    stream,
                                    welcome.max_message_size,
                                    write_rx,
                                    1, // connection_id
                                )
                                .expect("ConnectionSource::new failed");

                                loop_handle_inner
                                    .insert_source(
                                        source,
                                        |msg, _, state: &mut Vec<LooperMessage>| {
                                            state.push(msg);
                                        },
                                    )
                                    .expect("insert_source for ConnectionSource failed");

                                // D2 step 4: ack the bridge thread.
                                let _ = ack.send(Registered);
                            }
                            other => {
                                state.push(other);
                            }
                        }
                    }
                },
            )
            .expect("insert calloop channel failed");

        let mut events = Vec::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(30);

        // Pump until all frames are received or timeout.
        while received3.load(Ordering::Relaxed) < NUM_MSGS {
            if std::time::Instant::now() > deadline {
                panic!(
                    "calloop timeout: server reader got {} of {NUM_MSGS}",
                    received3.load(Ordering::Relaxed),
                );
            }
            let _ = event_loop.dispatch(Some(Duration::from_millis(10)), &mut events);
        }
    });

    // -- D2 step 1-2: bridge handshake + NewConnection handoff --
    //
    // connect_unix performs the handshake on the blocking stream,
    // then sends NewConnection through looper_tx and blocks on ack.
    let hello = Hello {
        version: 1,
        max_message_size: 16 * 1024 * 1024,
        max_outstanding_requests: 0,
        interests: vec![],
        provides: vec![],
    };
    let conn = bridge::connect_unix(client_stream, hello, looper_tx).expect("connect_unix failed");

    assert_eq!(conn.welcome.instance_id, "bridge-wireup-test");

    // -- Sender: push 4KB frames through write_tx at full speed --

    let start = std::time::Instant::now();
    for seq in 0..NUM_MSGS as u64 {
        // 4KB payload: ensures socket buffer fills quickly.
        let data = "X".repeat(PAYLOAD_SIZE);
        let payload = serialize_topic(&TopicMessage::Update { seq, data });
        let sf = ServiceFrame::Notification { payload };
        let bytes = postcard::to_allocvec(&sf).unwrap();
        // SyncSender::send blocks when the channel is full — this
        // is the backpressure signal propagating from the slow peer
        // through ConnectionSource's non-blocking write → channel.
        conn.write_tx.send((1u16, bytes)).unwrap();
    }
    let sender_elapsed = start.elapsed();

    server_handle.join().unwrap();
    loop_handle.join().unwrap();

    let count = received.load(Ordering::Relaxed);
    assert_eq!(count, NUM_MSGS, "not all frames delivered");

    // The sender SHOULD have been stalled by the slow reader.
    // Slow reader takes NUM_MSGS * SLOW_DELAY_MS = 2s.
    // Each frame is ~4KB. Socket buffer (~16KB on macOS) fills
    // within a few frames. The write queue highwater (8) caps
    // buffering between socket and channel, so the bounded mpsc
    // channel (128 slots) stays full. With 200 frames at 10ms/read,
    // the sender should be stalled for at least 250ms.
    let expected_minimum = Duration::from_millis(SLOW_DELAY_MS * NUM_MSGS as u64 / 8);
    eprintln!(
        "bridge wireup backpressure: sender sent {NUM_MSGS} frames in \
         {sender_elapsed:?} (server reader got {count}, minimum {expected_minimum:?})"
    );

    assert!(
        sender_elapsed > expected_minimum,
        "sender was NOT stalled by slow reader ({sender_elapsed:?} < \
         {expected_minimum:?}) — D2 backpressure chain failed"
    );
}

/// ConnectionSource non-blocking write test: exercises the
/// ConnectionSource EventSource implementation with a slow peer
/// to verify WouldBlock handling and write queue management.
///
/// Creates a ConnectionSource wrapping one end of a UnixStream pair.
/// A slow thread reads from the other end. Frames are fed through
/// the write channel (same path as ServiceHandle). Verifies that
/// all frames arrive despite the slow reader, and that the
/// ConnectionSource's non-blocking write logic handles partial
/// writes and WouldBlock correctly.
///
/// The peer sends a Ready frame first to bootstrap readability on
/// the ConnectionSource. This matches real usage: the server sends
/// Ready before data flows. ConnectionSource's before_sleep hook
/// now detects pending writes and generates synthetic writable
/// events, but the bootstrap Ready frame is still sent here to
/// match the real server protocol.
#[test]
#[ignore]
fn stress_connection_source_write_backpressure() {
    use pane_app::ConnectionSource;
    use pane_session::bridge::WRITE_CHANNEL_CAPACITY;
    use std::sync::mpsc;

    const NUM_MSGS: usize = 500;
    const SLOW_DELAY_MS: u64 = 5;

    let (source_stream, mut peer_stream) = UnixStream::pair().unwrap();
    let (write_tx, write_rx) = mpsc::sync_channel(WRITE_CHANNEL_CAPACITY);

    // ConnectionSource sets non-blocking mode on the stream.
    let source = ConnectionSource::new(source_stream, 16 * 1024 * 1024, write_rx, 1).unwrap();

    let received = Arc::new(AtomicUsize::new(0));
    let received2 = Arc::clone(&received);

    // Bootstrap: peer sends a Ready control frame to trigger the
    // first readability event on the ConnectionSource. This is
    // what the server does in real usage after handshake.
    let ready =
        ControlMessage::Lifecycle(pane_proto::protocols::lifecycle::LifecycleMessage::Ready);
    let ready_bytes = postcard::to_allocvec(&ready).unwrap();
    let bootstrap_codec = FrameCodec::new(16 * 1024 * 1024);
    bootstrap_codec
        .write_frame(&mut peer_stream, 0, &ready_bytes)
        .unwrap();

    // Slow reader on the peer end (blocking mode).
    // Reads the frames that ConnectionSource writes to the socket.
    let codec_r = FrameCodec::permissive(16 * 1024 * 1024);
    let reader_handle = thread::spawn(move || {
        for _ in 0..NUM_MSGS {
            thread::sleep(Duration::from_millis(SLOW_DELAY_MS));
            match codec_r.read_frame(&mut peer_stream) {
                Ok(Frame::Message { service: 1, .. }) => {
                    received2.fetch_add(1, Ordering::Relaxed);
                }
                Ok(other) => panic!("reader: unexpected frame: {other:?}"),
                Err(e) => panic!("reader: read error: {e}"),
            }
        }
    });

    // Enqueue frames through the write channel (bounded at
    // WRITE_CHANNEL_CAPACITY=128). The channel itself provides
    // backpressure to the sender when full.
    let sender_handle = thread::spawn(move || {
        for seq in 0..NUM_MSGS as u64 {
            let payload = serialize_topic(&TopicMessage::Update {
                seq,
                data: format!("cs-{seq}"),
            });
            let sf = ServiceFrame::Notification { payload };
            let bytes = postcard::to_allocvec(&sf).unwrap();
            // SyncSender::send blocks when channel is full — this
            // is the backpressure signal to the sender.
            write_tx.send((1u16, bytes)).unwrap();
        }
    });

    // Drive the ConnectionSource manually via calloop.
    // Register it on a calloop EventLoop and pump until all
    // frames are delivered.
    let mut event_loop: calloop::EventLoop<Vec<pane_session::bridge::LooperMessage>> =
        calloop::EventLoop::try_new().unwrap();

    let handle = event_loop.handle();
    handle
        .insert_source(source, |msg, _, state: &mut Vec<_>| {
            state.push(msg);
        })
        .unwrap();

    let mut events = Vec::new();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);

    // Pump until the reader has received all messages or timeout.
    while received.load(Ordering::Relaxed) < NUM_MSGS {
        if std::time::Instant::now() > deadline {
            panic!(
                "timeout: reader got {} of {NUM_MSGS}",
                received.load(Ordering::Relaxed)
            );
        }
        // Short timeout keeps pumping. ConnectionSource reregisters
        // with BOTH interest when writes are pending, so calloop
        // fires process_events on writable readiness to flush the
        // queue. The 10ms timeout bounds latency for write channel
        // drain when the fd has no readability events.
        let _ = event_loop.dispatch(Some(Duration::from_millis(10)), &mut events);
    }

    sender_handle.join().unwrap();
    reader_handle.join().unwrap();

    let count = received.load(Ordering::Relaxed);
    assert_eq!(count, NUM_MSGS, "not all messages delivered");
    eprintln!(
        "ConnectionSource write backpressure: delivered {count} frames \
         through non-blocking write path"
    );
}
