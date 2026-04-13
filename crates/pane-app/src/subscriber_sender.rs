//! SubscriberSender<P>: provider-side handle for pushing
//! notifications to a connected subscriber.
//!
//! Asymmetric from ServiceHandle<P>: SubscriberSender does NOT
//! own the interest lifecycle. The consumer owns that via their
//! ServiceHandle -- when they drop it (RevokeInterest) or the
//! connection dies, the provider receives subscriber_disconnected.
//! SubscriberSender is sending-only (no send_request -- provider
//! pushing requests to consumers is not the pattern).
//!
//! The notification path uses par's `Enqueue<Vec<u8>>` as the
//! in-process channel. `send_notification` serializes and pushes
//! onto the Enqueue; the paired Dequeue is driven as a calloop
//! StreamSource on the Looper thread, which writes frames to the
//! wire via SharedWriter.
//!
//! Design heritage: Haiku's WatchingService
//! (src/servers/registrar/WatchingService.cpp:66-228) stored a
//! BMessenger per client in map<BMessenger, Watcher*>.
//! NotifyWatchers iterated the map, applied a filter predicate,
//! sent to each matching watcher. SubscriberSender is the pane
//! equivalent of that per-client BMessenger -- the provider holds
//! one per subscriber and pushes through it.
//!
//! Plan 9's plumber (reference/plan9/man/4/plumber) multicast by
//! iterating pending Treads from all fids on the same port file.
//! DeclareInterest-as-subscription mirrors open-as-subscribe:
//! the service binding lifecycle IS the subscription lifecycle.

use pane_proto::{Protocol, ServiceFrame};
use pane_session::par;
use std::marker::PhantomData;

/// Provider-side notification sender for a single subscriber.
///
/// Obtained via `Messenger::subscriber_sender()` when the
/// `subscriber_connected` callback fires. Parameterized by
/// protocol for type-safe notification serialization.
///
/// Does NOT implement Drop -> RevokeInterest. The consumer owns
/// the interest lifecycle; this handle becomes invalid when the
/// consumer revokes or the connection dies.
///
/// The inner `Enqueue<Vec<u8>>` is par's session-typed push
/// endpoint. Each `send_notification` call serializes the message
/// and pushes the bytes. `Enqueue::push` is always non-blocking
/// and unbounded -- N1 (non-blocking send) is satisfied by
/// construction. Drop calls `close1()` to prevent par's
/// panic-on-drop.
///
/// # Haiku
///
/// `WatchingService` stored `BMessenger` per client
/// (src/servers/registrar/WatchingService.cpp:66-228).
/// `NotifyWatchers(message, filter)` iterated the map and sent
/// to each matching watcher. SubscriberSender is the per-client
/// BMessenger equivalent.
pub struct SubscriberSender<P: Protocol> {
    session_id: u16,
    /// par Enqueue endpoint. Option because Enqueue::push
    /// consumes self -- we use take/replace to work with &mut self.
    /// None only transiently during push or after Drop.
    enqueue: Option<par::queue::Enqueue<Vec<u8>>>,
    _protocol: PhantomData<P>,
}

impl<P: Protocol> SubscriberSender<P> {
    /// Construct a sender from a pre-created par Enqueue endpoint.
    ///
    /// The Enqueue is created by the Looper during
    /// `dispatch_subscriber_connected` and passed through the
    /// Messenger's subscriber enqueue map. The paired Dequeue is
    /// registered as a calloop StreamSource that drives wire writes.
    pub(crate) fn new(session_id: u16, enqueue: par::queue::Enqueue<Vec<u8>>) -> Self {
        SubscriberSender {
            session_id,
            enqueue: Some(enqueue),
            _protocol: PhantomData,
        }
    }

    /// Send a fire-and-forget notification to this subscriber.
    ///
    /// Serializes the message (protocol tag byte + postcard body)
    /// into a ServiceFrame::Notification, then pushes the bytes
    /// onto the par Enqueue. The paired Dequeue (driven by
    /// calloop StreamSource) writes the frame to the wire.
    ///
    /// N1: non-blocking. `Enqueue::push` allocates a oneshot
    /// internally but never blocks. If the Enqueue has been
    /// consumed (subscriber disconnected, Drop already ran),
    /// the send is silently dropped.
    pub fn send_notification(&mut self, msg: P::Message) {
        let tag = P::service_id().tag();
        let msg_bytes = postcard::to_allocvec(&msg).expect("service message serialization failed");
        let mut inner_payload = Vec::with_capacity(1 + msg_bytes.len());
        inner_payload.push(tag);
        inner_payload.extend_from_slice(&msg_bytes);

        let frame = ServiceFrame::Notification {
            payload: inner_payload,
        };
        let outer_payload =
            postcard::to_allocvec(&frame).expect("service frame serialization failed");

        // N1: non-blocking push. Enqueue::push consumes self and
        // returns a new Enqueue for the next push. We use
        // take/replace to work with &mut self.
        if let Some(enqueue) = self.enqueue.take() {
            self.enqueue = Some(enqueue.push(outer_payload));
        }
    }

    /// Fallible variant of `send_notification`.
    ///
    /// With par's Enqueue, push always succeeds (unbounded,
    /// non-blocking). The only failure mode is if the Enqueue
    /// has already been consumed (None). In that case, returns
    /// the original message so the caller retains ownership
    /// (L2 linearity condition).
    pub fn try_send_notification(
        &mut self,
        msg: P::Message,
    ) -> Result<(), (P::Message, pane_session::Backpressure)> {
        if self.enqueue.is_none() {
            return Err((msg, pane_session::Backpressure::ConnectionClosing));
        }
        self.send_notification(msg);
        Ok(())
    }

    /// The session_id identifying this subscriber's service session.
    pub fn session_id(&self) -> u16 {
        self.session_id
    }
}

impl<P: Protocol> Drop for SubscriberSender<P> {
    fn drop(&mut self) {
        // Clean close prevents par's panic-on-drop on the Dequeue
        // side. close1() signals the StreamSource that no more items
        // will arrive; the stream returns None and calloop removes
        // the source.
        if let Some(enqueue) = self.enqueue.take() {
            enqueue.close1();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pane_proto::{Protocol, ServiceFrame, ServiceId};
    use pane_session::par::Session;
    use serde::{Deserialize, Serialize};

    struct TestProto;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum TestMsg {
        Update(String),
    }

    impl Protocol for TestProto {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.subscriber_sender")
        }
        type Message = TestMsg;
    }

    /// Create a par Enqueue/Dequeue pair for testing.
    /// Returns (enqueue, dequeue) -- the Dequeue must be
    /// consumed to avoid par panic-on-drop.
    fn make_pair() -> (par::queue::Enqueue<Vec<u8>>, par::queue::Dequeue<Vec<u8>>) {
        let mut dequeue_slot = None;
        let enqueue = par::queue::Enqueue::<Vec<u8>>::fork_sync(|dequeue| {
            dequeue_slot = Some(dequeue);
        });
        (enqueue, dequeue_slot.unwrap())
    }

    /// Drain all items from a Dequeue, consuming it.
    fn drain_dequeue(dequeue: par::queue::Dequeue<Vec<u8>>) -> Vec<Vec<u8>> {
        let mut items = Vec::new();
        let rt = futures::executor::LocalPool::new();
        rt.spawner();
        // Use block_on to drive the async pop.
        futures::executor::block_on(async {
            let mut dq = dequeue;
            loop {
                match dq.pop().await {
                    par::queue::Queue::Item(item, next) => {
                        items.push(item);
                        dq = next;
                    }
                    par::queue::Queue::Closed(()) => break,
                }
            }
        });
        items
    }

    #[test]
    fn send_notification_pushes_to_enqueue() {
        let (enqueue, dequeue) = make_pair();
        let mut sender = SubscriberSender::<TestProto>::new(5, enqueue);

        sender.send_notification(TestMsg::Update("hello".into()));
        // Drop sender to close the Enqueue, signaling the Dequeue.
        drop(sender);

        let items = drain_dequeue(dequeue);
        assert_eq!(items.len(), 1);

        // Verify the payload is a valid ServiceFrame::Notification.
        let frame: ServiceFrame =
            postcard::from_bytes(&items[0]).expect("ServiceFrame deserialization failed");
        match frame {
            ServiceFrame::Notification { payload } => {
                let expected_tag = TestProto::service_id().tag();
                assert_eq!(payload[0], expected_tag, "protocol tag must be first byte");
                let msg: TestMsg = postcard::from_bytes(&payload[1..])
                    .expect("inner message deserialization failed");
                assert!(matches!(msg, TestMsg::Update(s) if s == "hello"));
            }
            _ => panic!("expected Notification frame"),
        }
    }

    #[test]
    fn session_id_returns_assigned_value() {
        let (enqueue, dequeue) = make_pair();
        let sender = SubscriberSender::<TestProto>::new(42, enqueue);
        assert_eq!(sender.session_id(), 42);
        // Clean up: drop sender (closes enqueue), consume dequeue.
        drop(sender);
        drain_dequeue(dequeue);
    }

    #[test]
    fn drop_closes_enqueue_cleanly() {
        // Verify that dropping a SubscriberSender closes the Enqueue
        // via close1(), causing the Dequeue to see Closed.
        let (enqueue, dequeue) = make_pair();
        let sender = SubscriberSender::<TestProto>::new(3, enqueue);
        drop(sender);

        // Dequeue should see Closed (not panic).
        let items = drain_dequeue(dequeue);
        assert!(items.is_empty(), "no items should have been pushed");
    }

    #[test]
    fn no_drop_revoke_interest() {
        // Verify that dropping a SubscriberSender does NOT send
        // RevokeInterest. The consumer owns the lifecycle.
        // The Enqueue just closes -- no wire frame is produced.
        let (enqueue, dequeue) = make_pair();
        let sender = SubscriberSender::<TestProto>::new(3, enqueue);
        drop(sender);

        // Dequeue sees Closed, no items.
        let items = drain_dequeue(dequeue);
        assert!(
            items.is_empty(),
            "SubscriberSender must NOT send RevokeInterest on drop"
        );
    }

    // -- try_send_notification --

    #[test]
    fn try_send_notification_succeeds() {
        let (enqueue, dequeue) = make_pair();
        let mut sender = SubscriberSender::<TestProto>::new(5, enqueue);

        let result = sender.try_send_notification(TestMsg::Update("hello".into()));
        assert!(result.is_ok());
        drop(sender);

        let items = drain_dequeue(dequeue);
        assert_eq!(items.len(), 1);

        let frame: ServiceFrame =
            postcard::from_bytes(&items[0]).expect("ServiceFrame deserialization failed");
        assert!(matches!(frame, ServiceFrame::Notification { .. }));
    }

    #[test]
    fn try_send_notification_after_close() {
        // Manually take the enqueue out, simulating a consumed state.
        let (enqueue, dequeue) = make_pair();
        let mut sender = SubscriberSender::<TestProto>::new(5, enqueue);

        // Force the enqueue to None by taking and closing it.
        sender.enqueue.take().unwrap().close1();

        let result = sender.try_send_notification(TestMsg::Update("gone".into()));
        match result {
            Err((_msg, bp)) => {
                assert_eq!(bp, pane_session::Backpressure::ConnectionClosing);
            }
            Ok(()) => panic!("should have failed with ConnectionClosing"),
        }

        // Dequeue was closed, drain it.
        drain_dequeue(dequeue);
    }

    #[test]
    fn multiple_notifications_arrive_in_order() {
        let (enqueue, dequeue) = make_pair();
        let mut sender = SubscriberSender::<TestProto>::new(1, enqueue);

        sender.send_notification(TestMsg::Update("first".into()));
        sender.send_notification(TestMsg::Update("second".into()));
        sender.send_notification(TestMsg::Update("third".into()));
        drop(sender);

        let items = drain_dequeue(dequeue);
        assert_eq!(items.len(), 3);

        // Verify ordering by deserializing each.
        let messages: Vec<String> = items
            .iter()
            .map(|bytes| {
                let frame: ServiceFrame = postcard::from_bytes(bytes).unwrap();
                match frame {
                    ServiceFrame::Notification { payload } => {
                        let msg: TestMsg = postcard::from_bytes(&payload[1..]).unwrap();
                        match msg {
                            TestMsg::Update(s) => s,
                        }
                    }
                    _ => panic!("expected Notification"),
                }
            })
            .collect();
        assert_eq!(messages, vec!["first", "second", "third"]);
    }
}
