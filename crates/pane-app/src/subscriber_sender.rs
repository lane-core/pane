//! SubscriberSender<P>: provider-side handle for pushing
//! notifications to a connected subscriber.
//!
//! Asymmetric from ServiceHandle<P>: SubscriberSender does NOT
//! own the interest lifecycle. The consumer owns that via their
//! ServiceHandle — when they drop it (RevokeInterest) or the
//! connection dies, the provider receives subscriber_disconnected.
//! SubscriberSender is sending-only (no send_request — provider
//! pushing requests to consumers is not the pattern).
//!
//! Design heritage: Haiku's WatchingService
//! (src/servers/registrar/WatchingService.cpp:66-228) stored a
//! BMessenger per client in map<BMessenger, Watcher*>.
//! NotifyWatchers iterated the map, applied a filter predicate,
//! sent to each matching watcher. SubscriberSender is the pane
//! equivalent of that per-client BMessenger — the provider holds
//! one per subscriber and pushes through it.
//!
//! Plan 9's plumber (reference/plan9/man/4/plumber) multicast by
//! iterating pending Treads from all fids on the same port file.
//! DeclareInterest-as-subscription mirrors open-as-subscribe:
//! the service binding lifecycle IS the subscription lifecycle.

use pane_proto::{Protocol, ServiceFrame};
use pane_session::Backpressure;
use std::marker::PhantomData;
use std::sync::mpsc::TrySendError;

/// Provider-side notification sender for a single subscriber.
///
/// Obtained via `Messenger::subscriber_sender()` when the
/// `subscriber_connected` callback fires. Parameterized by
/// protocol for type-safe notification serialization.
///
/// Does NOT implement Drop → RevokeInterest. The consumer owns
/// the interest lifecycle; this handle becomes invalid when the
/// consumer revokes or the connection dies.
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
    write_tx: Option<std::sync::mpsc::SyncSender<(u16, Vec<u8>)>>,
    _protocol: PhantomData<P>,
}

impl<P: Protocol> SubscriberSender<P> {
    /// Construct a sender for the given subscriber session.
    ///
    /// Called by Messenger::subscriber_sender when the handler
    /// receives subscriber_connected.
    pub(crate) fn new(
        session_id: u16,
        write_tx: std::sync::mpsc::SyncSender<(u16, Vec<u8>)>,
    ) -> Self {
        SubscriberSender {
            session_id,
            write_tx: Some(write_tx),
            _protocol: PhantomData,
        }
    }

    /// Send a fire-and-forget notification to this subscriber.
    ///
    /// Same serialization as ServiceHandle::send_notification:
    /// protocol tag byte + postcard message → ServiceFrame::Notification
    /// → write_tx. Best-effort: if the channel is full or closed
    /// (subscriber disconnected), the send is silently dropped.
    pub fn send_notification(&self, msg: P::Message) {
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

        if let Some(ref tx) = self.write_tx {
            let _ = tx.send((self.session_id, outer_payload));
        }
    }

    /// Fallible variant of `send_notification`. Returns the
    /// original message on failure so the caller retains ownership
    /// (L2 linearity condition).
    ///
    /// Same failure modes as ServiceHandle::try_send_notification:
    /// channel full or connection closing.
    pub fn try_send_notification(&self, msg: P::Message) -> Result<(), (P::Message, Backpressure)> {
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

        if let Some(ref tx) = self.write_tx {
            match tx.try_send((self.session_id, outer_payload)) {
                Ok(()) => Ok(()),
                Err(TrySendError::Full(_)) => Err((msg, Backpressure::ChannelFull)),
                Err(TrySendError::Disconnected(_)) => Err((msg, Backpressure::ConnectionClosing)),
            }
        } else {
            Ok(())
        }
    }

    /// The session_id identifying this subscriber's service session.
    pub fn session_id(&self) -> u16 {
        self.session_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pane_proto::{Protocol, ServiceFrame, ServiceId};
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

    #[test]
    fn send_notification_serializes_to_channel() {
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        let sender = SubscriberSender::<TestProto>::new(5, tx);

        sender.send_notification(TestMsg::Update("hello".into()));

        let (session_id, payload) = rx.recv().expect("expected frame on channel");
        assert_eq!(session_id, 5);

        let frame: ServiceFrame =
            postcard::from_bytes(&payload).expect("ServiceFrame deserialization failed");
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
        let (tx, _rx) = std::sync::mpsc::sync_channel(1);
        let sender = SubscriberSender::<TestProto>::new(42, tx);
        assert_eq!(sender.session_id(), 42);
    }

    #[test]
    fn send_on_closed_channel_does_not_panic() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let sender = SubscriberSender::<TestProto>::new(1, tx);
        drop(rx); // close the receiving end
                  // Must not panic — best-effort send.
        sender.send_notification(TestMsg::Update("gone".into()));
    }

    #[test]
    fn no_drop_revoke_interest() {
        // Verify that dropping a SubscriberSender does NOT send
        // RevokeInterest. The consumer owns the lifecycle.
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        let sender = SubscriberSender::<TestProto>::new(3, tx);
        drop(sender);

        // Channel should be empty — no RevokeInterest sent.
        assert!(
            rx.try_recv().is_err(),
            "SubscriberSender must NOT send RevokeInterest on drop"
        );
    }

    // ── try_send_notification ────────────────────────────────

    #[test]
    fn try_send_notification_succeeds() {
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        let sender = SubscriberSender::<TestProto>::new(5, tx);

        let result = sender.try_send_notification(TestMsg::Update("hello".into()));
        assert!(result.is_ok());

        let (session_id, payload) = rx.recv().expect("expected frame");
        assert_eq!(session_id, 5);

        let frame: ServiceFrame =
            postcard::from_bytes(&payload).expect("ServiceFrame deserialization failed");
        assert!(matches!(frame, ServiceFrame::Notification { .. }));
    }

    #[test]
    fn try_send_notification_channel_full() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(1);
        let sender = SubscriberSender::<TestProto>::new(5, tx);

        // Fill the channel
        sender.send_notification(TestMsg::Update("first".into()));

        // Second send hits full channel
        let result = sender.try_send_notification(TestMsg::Update("second".into()));
        match result {
            Err((msg, bp)) => {
                assert!(
                    matches!(msg, TestMsg::Update(s) if s == "second"),
                    "original message returned"
                );
                assert_eq!(bp, Backpressure::ChannelFull);
            }
            Ok(()) => panic!("should have failed with ChannelFull"),
        }
    }

    #[test]
    fn try_send_notification_connection_closing() {
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        let sender = SubscriberSender::<TestProto>::new(5, tx);

        // Close the receiving end
        drop(rx);

        let result = sender.try_send_notification(TestMsg::Update("gone".into()));
        match result {
            Err((_msg, bp)) => {
                assert_eq!(bp, Backpressure::ConnectionClosing);
            }
            Ok(()) => panic!("should have failed with ConnectionClosing"),
        }
    }
}
