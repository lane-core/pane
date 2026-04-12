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
/// Publisher pushes at full speed.
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
    let (fast_sub, fast_session, fast_pub_session, fast_conn) =
        subscribe(&server, &mut publisher);
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
    let (slow_sub, slow_session, slow_pub_session, slow_conn) =
        subscribe(&server, &mut publisher);
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
        publisher.send_service(
            slow_pub_session,
            &ServiceFrame::Notification { payload },
        );
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
