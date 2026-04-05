//! Handles<P>: uniform dispatch trait.
//!
//! Each Handles<P> impl is one entry in EAct's handler store σ
//! (under pane's one-service-per-protocol constraint). The looper
//! dispatches P::Message to this method via a monomorphized fn
//! pointer captured at service registration.
//!
//! The #[pane::protocol_handler(P)] attribute macro generates the
//! Handles<P>::receive match from named methods. Rust's exhaustive
//! match IS the coverage guarantee.

use crate::flow::Flow;
use crate::protocol::Protocol;

/// A handler that can receive value messages from protocol P.
///
/// The macro generates two dispatch surfaces:
/// 1. Value dispatch — this receive method, matching on P::Message
///    variants (Clone-safe values).
/// 2. Obligation dispatch — separate typed callbacks for obligation
///    handles (not through this trait).
///
/// Both dispatch to the same &mut self, same looper thread (I6/I7).
pub trait Handles<P: Protocol> {
    fn receive(&mut self, msg: P::Message) -> Flow;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Message;
    use crate::protocol::ServiceId;
    use serde::{Serialize, Deserialize};

    // A test protocol
    struct TestProto;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    enum TestMessage {
        Ping,
        Data(String),
    }

    impl Protocol for TestProto {
        fn service_id() -> ServiceId { ServiceId::new("com.test.proto") }
        type Message = TestMessage;
    }

    // A handler that tracks received messages
    struct TestHandler {
        received: Vec<TestMessage>,
    }

    impl Handles<TestProto> for TestHandler {
        fn receive(&mut self, msg: TestMessage) -> Flow {
            self.received.push(msg);
            Flow::Continue
        }
    }

    #[test]
    fn handles_dispatches_messages() {
        let mut handler = TestHandler { received: vec![] };

        assert_eq!(handler.receive(TestMessage::Ping), Flow::Continue);
        assert_eq!(handler.receive(TestMessage::Data("hello".into())), Flow::Continue);
        assert_eq!(handler.received.len(), 2);
        assert_eq!(handler.received[0], TestMessage::Ping);
    }

    #[test]
    fn handles_can_return_stop() {
        struct StopOnPing;

        impl Handles<TestProto> for StopOnPing {
            fn receive(&mut self, msg: TestMessage) -> Flow {
                match msg {
                    TestMessage::Ping => Flow::Stop,
                    _ => Flow::Continue,
                }
            }
        }

        let mut handler = StopOnPing;
        assert_eq!(handler.receive(TestMessage::Data("x".into())), Flow::Continue);
        assert_eq!(handler.receive(TestMessage::Ping), Flow::Stop);
    }

    #[test]
    fn message_is_clone_and_serialize() {
        // Verify TestMessage satisfies Message bounds
        fn assert_message<T: Message>() {}
        assert_message::<TestMessage>();
    }
}
