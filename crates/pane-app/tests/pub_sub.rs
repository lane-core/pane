//! Integration test: pub/sub patterns via ProtocolServer.
//!
//! Exercises both provider-side API patterns from
//! decision/provider_side_api:
//!
//! 1. **Push** — provider holds SubscriberSender, pushes
//!    notifications to subscribers. Haiku WatchingService model.
//! 2. **Long-poll** — subscriber sends request, provider holds
//!    ReplyPort, responds when update available. Plan 9 plumber model.
//!
//! Uses the raw ClientConn wire-level helper (same as two_pane_echo)
//! since the high-level Looper integration is not wired for
//! multi-pane yet. The point is to exercise the wire-level patterns
//! that the high-level API will eventually surface.

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

// -- Protocol definition --

fn topic_service_id() -> ServiceId {
    ServiceId::new("com.pane.topic")
}

/// Topic protocol messages flow in both directions:
/// subscriber → publisher (Subscribe/Unsubscribe) and
/// publisher → subscriber (Update).
///
/// In the push model, the publisher sends Update as notifications
/// through its SubscriberSender.
///
/// In the long-poll model, Update is the reply type — the subscriber
/// sends a NextUpdate request and the publisher replies with Update.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum TopicMessage {
    /// subscriber → publisher: request next update (long-poll)
    NextUpdate,
    /// publisher → subscriber: a key-value update
    Update { key: String, value: String },
}

// -- Wire helper (same pattern as two_pane_echo) --

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
) -> std::thread::JoinHandle<pane_session::server::ConnectionHandle> {
    let server = Arc::clone(server);
    std::thread::spawn(move || {
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

// -- Tests --

/// Push pattern: publisher sends notifications to subscriber
/// through the bidirectional service session.
///
/// Wire flow:
///   1. Publisher provides topic service
///   2. Subscriber DeclareInterest → both get InterestAccepted
///   3. Publisher sends Notification(Update) on its session
///   4. Subscriber receives the update
///
/// This is the Haiku WatchingService model — server iterates
/// watcher list and sends to each.
#[test]
fn push_publisher_notifies_subscriber() {
    let server = Arc::new(ProtocolServer::new());
    let topic_id = topic_service_id();

    // Publisher (provider)
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

    // Subscriber (consumer)
    let (sub_client, sub_server) = MemoryTransport::pair();
    let accept_sub = accept_on_thread(&server, sub_server);
    let (mut subscriber, _) = ClientConn::connect(sub_client, make_hello(vec![]));
    let conn_sub = accept_sub.join().unwrap();
    subscriber.expect_ready();

    // Subscriber declares interest in topic
    subscriber.send_control(&ControlMessage::DeclareInterest {
        service: topic_id,
        expected_version: 1,
    });

    // Both sides get InterestAccepted
    let sub_session = match subscriber.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("subscriber: expected InterestAccepted, got {other:?}"),
    };
    let pub_session = match publisher.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("publisher: expected InterestAccepted, got {other:?}"),
    };

    subscriber.register_session(sub_session);
    publisher.register_session(pub_session);

    // Publisher pushes an update notification to subscriber
    let update = TopicMessage::Update {
        key: "weather.temp".into(),
        value: "22C".into(),
    };
    publisher.send_service(
        pub_session,
        &ServiceFrame::Notification {
            payload: serialize_topic(&update),
        },
    );

    // Subscriber receives it
    let (recv_session, recv_frame) = subscriber.read_service();
    assert_eq!(recv_session, sub_session);
    match recv_frame {
        ServiceFrame::Notification { payload } => {
            let msg = deserialize_topic(&payload);
            assert_eq!(
                msg,
                TopicMessage::Update {
                    key: "weather.temp".into(),
                    value: "22C".into(),
                }
            );
        }
        other => panic!("expected Notification, got {other:?}"),
    }

    drop(publisher);
    drop(subscriber);
    conn_pub.wait();
    conn_sub.wait();
}

/// Push pattern with multiple subscribers: publisher fans out
/// to two subscribers independently.
#[test]
fn push_fan_out_to_multiple_subscribers() {
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

    // Subscriber A
    let (a_client, a_server) = MemoryTransport::pair();
    let accept_a = accept_on_thread(&server, a_server);
    let (mut sub_a, _) = ClientConn::connect(a_client, make_hello(vec![]));
    let conn_a = accept_a.join().unwrap();
    sub_a.expect_ready();

    // Subscriber B
    let (b_client, b_server) = MemoryTransport::pair();
    let accept_b = accept_on_thread(&server, b_server);
    let (mut sub_b, _) = ClientConn::connect(b_client, make_hello(vec![]));
    let conn_b = accept_b.join().unwrap();
    sub_b.expect_ready();

    // Both subscribe
    sub_a.send_control(&ControlMessage::DeclareInterest {
        service: topic_id,
        expected_version: 1,
    });
    let a_session = match sub_a.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("A: expected InterestAccepted, got {other:?}"),
    };
    let pub_session_a = match publisher.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("pub(A): expected InterestAccepted, got {other:?}"),
    };

    sub_b.send_control(&ControlMessage::DeclareInterest {
        service: topic_id,
        expected_version: 1,
    });
    let b_session = match sub_b.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("B: expected InterestAccepted, got {other:?}"),
    };
    let pub_session_b = match publisher.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("pub(B): expected InterestAccepted, got {other:?}"),
    };

    sub_a.register_session(a_session);
    sub_b.register_session(b_session);
    publisher.register_session(pub_session_a);
    publisher.register_session(pub_session_b);

    // Publisher fans out — sends same update to both sessions
    let update = TopicMessage::Update {
        key: "weather.wind".into(),
        value: "15kph".into(),
    };
    let payload = serialize_topic(&update);

    publisher.send_service(
        pub_session_a,
        &ServiceFrame::Notification {
            payload: payload.clone(),
        },
    );
    publisher.send_service(
        pub_session_b,
        &ServiceFrame::Notification { payload },
    );

    // Both receive
    let (_, frame_a) = sub_a.read_service();
    let (_, frame_b) = sub_b.read_service();

    let msg_a = match frame_a {
        ServiceFrame::Notification { payload } => deserialize_topic(&payload),
        other => panic!("A: expected Notification, got {other:?}"),
    };
    let msg_b = match frame_b {
        ServiceFrame::Notification { payload } => deserialize_topic(&payload),
        other => panic!("B: expected Notification, got {other:?}"),
    };

    assert_eq!(msg_a, msg_b);
    assert_eq!(
        msg_a,
        TopicMessage::Update {
            key: "weather.wind".into(),
            value: "15kph".into(),
        }
    );

    drop(publisher);
    drop(sub_a);
    drop(sub_b);
    conn_pub.wait();
    conn_a.wait();
    conn_b.wait();
}

/// Long-poll pattern: subscriber sends Request(NextUpdate),
/// publisher replies with Update when ready.
///
/// This is the Plan 9 plumber model — subscriber blocks on read,
/// server responds when data available. In pane terms: subscriber
/// sends a request, provider holds the ReplyPort, responds later.
///
/// At the wire level this is a standard Request/Reply exchange.
/// The "long" part is that the publisher holds the request and
/// responds asynchronously — which the protocol already supports.
#[test]
fn long_poll_request_reply() {
    let server = Arc::new(ProtocolServer::new());
    let topic_id = topic_service_id();

    // Publisher (provider)
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

    // Subscriber
    let (sub_client, sub_server) = MemoryTransport::pair();
    let accept_sub = accept_on_thread(&server, sub_server);
    let (mut subscriber, _) = ClientConn::connect(sub_client, make_hello(vec![]));
    let conn_sub = accept_sub.join().unwrap();
    subscriber.expect_ready();

    // Establish binding
    subscriber.send_control(&ControlMessage::DeclareInterest {
        service: topic_id,
        expected_version: 1,
    });
    let sub_session = match subscriber.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted, got {other:?}"),
    };
    let pub_session = match publisher.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted, got {other:?}"),
    };
    subscriber.register_session(sub_session);
    publisher.register_session(pub_session);

    // Subscriber sends NextUpdate request (token 1)
    let next = TopicMessage::NextUpdate;
    subscriber.send_service(
        sub_session,
        &ServiceFrame::Request {
            token: 1,
            payload: serialize_topic(&next),
        },
    );

    // Publisher receives the request
    let (recv_session, recv_frame) = publisher.read_service();
    assert_eq!(recv_session, pub_session);
    let token = match recv_frame {
        ServiceFrame::Request { token, payload } => {
            let msg = deserialize_topic(&payload);
            assert_eq!(msg, TopicMessage::NextUpdate);
            token
        }
        other => panic!("expected Request, got {other:?}"),
    };

    // Publisher holds the request, then replies with an update
    let update = TopicMessage::Update {
        key: "weather.humidity".into(),
        value: "65%".into(),
    };
    publisher.send_service(
        pub_session,
        &ServiceFrame::Reply {
            token,
            payload: serialize_topic(&update),
        },
    );

    // Subscriber receives the reply
    let (recv_session, recv_frame) = subscriber.read_service();
    assert_eq!(recv_session, sub_session);
    match recv_frame {
        ServiceFrame::Reply { token: t, payload } => {
            assert_eq!(t, 1);
            let msg = deserialize_topic(&payload);
            assert_eq!(
                msg,
                TopicMessage::Update {
                    key: "weather.humidity".into(),
                    value: "65%".into(),
                }
            );
        }
        other => panic!("expected Reply, got {other:?}"),
    }

    // Subscriber immediately re-polls (second round)
    subscriber.send_service(
        sub_session,
        &ServiceFrame::Request {
            token: 2,
            payload: serialize_topic(&TopicMessage::NextUpdate),
        },
    );

    let (_, req2) = publisher.read_service();
    let token2 = match req2 {
        ServiceFrame::Request { token, .. } => token,
        other => panic!("expected second Request, got {other:?}"),
    };

    let update2 = TopicMessage::Update {
        key: "weather.humidity".into(),
        value: "70%".into(),
    };
    publisher.send_service(
        pub_session,
        &ServiceFrame::Reply {
            token: token2,
            payload: serialize_topic(&update2),
        },
    );

    let (_, reply2) = subscriber.read_service();
    match reply2 {
        ServiceFrame::Reply { payload, .. } => {
            let msg = deserialize_topic(&payload);
            assert_eq!(
                msg,
                TopicMessage::Update {
                    key: "weather.humidity".into(),
                    value: "70%".into(),
                }
            );
        }
        other => panic!("expected second Reply, got {other:?}"),
    }

    drop(publisher);
    drop(subscriber);
    conn_pub.wait();
    conn_sub.wait();
}

/// Subscriber disconnect: publisher detects via ServiceTeardown.
///
/// When subscriber drops its connection, the server sends
/// ServiceTeardown to the publisher. This is the cleanup signal
/// for removing the subscriber from the push list.
#[test]
fn subscriber_disconnect_sends_teardown_to_publisher() {
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

    // Subscriber
    let (sub_client, sub_server) = MemoryTransport::pair();
    let accept_sub = accept_on_thread(&server, sub_server);
    let (mut subscriber, _) = ClientConn::connect(sub_client, make_hello(vec![]));
    let conn_sub = accept_sub.join().unwrap();
    subscriber.expect_ready();

    // Establish binding
    subscriber.send_control(&ControlMessage::DeclareInterest {
        service: topic_id,
        expected_version: 1,
    });
    let _sub_session = match subscriber.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted, got {other:?}"),
    };
    let _pub_session = match publisher.read_control() {
        ControlMessage::InterestAccepted { session_id, .. } => session_id,
        other => panic!("expected InterestAccepted, got {other:?}"),
    };

    // Subscriber disconnects
    drop(subscriber);
    conn_sub.wait();

    // Publisher should receive ServiceTeardown
    match publisher.read_control() {
        ControlMessage::ServiceTeardown { reason, .. } => {
            assert!(matches!(
                reason,
                pane_proto::control::TeardownReason::ConnectionLost
            ));
        }
        other => panic!("expected ServiceTeardown, got {other:?}"),
    }

    drop(publisher);
    conn_pub.wait();
}
