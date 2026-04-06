//! Uniform per-protocol dispatch.
//!
//! Each Handles<P> impl handles messages for one protocol.
//! The looper dispatches P::Message to this method via a fn
//! pointer captured at service registration time.
//!
//! The #[pane::protocol_handler(P)] attribute macro generates the
//! Handles<P>::receive match from named methods. Rust's exhaustive
//! match provides the coverage guarantee.

use crate::flow::Flow;
use crate::obligation::ReplyPort;
use crate::protocol::{Protocol, RequestProtocol};

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

/// A handler that can receive request messages from protocol P
/// and reply through a ReplyPort.
///
/// Separate from `Handles<P>` because:
/// - Notifications deliver `P::Message` only
/// - Requests deliver `P::Message` + `ReplyPort<P::Reply>`
/// - The ReplyPort is an obligation (`#[must_use]`, Drop sends `ReplyFailed`)
///
/// Design heritage: BeOS `BHandler::MessageReceived` handled both
/// notifications and requests in a single method. Replies were sent
/// via `BMessage::SendReply` (`src/kits/app/Message.cpp:588`).
/// Forgetting to reply was a common bug — the sender would hang
/// (`src/kits/app/Messenger.cpp:289`, synchronous `SendMessage`
/// blocks on `receive_port`). pane splits the dispatch surface:
/// `Handles<P>` for notifications, `HandlesRequest<P>` for requests.
/// The `ReplyPort` obligation makes forgotten replies a compile-time
/// warning and a runtime failure signal, never a hang.
pub trait HandlesRequest<P: RequestProtocol> {
    fn receive_request(&mut self, msg: P::Message, reply: ReplyPort<P::Reply>) -> Flow;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Message;
    use crate::protocol::ServiceId;
    use serde::{Deserialize, Serialize};

    // A test protocol
    struct TestProto;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    enum TestMessage {
        Ping,
        Data(String),
    }

    impl Protocol for TestProto {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.proto")
        }
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
        assert_eq!(
            handler.receive(TestMessage::Data("hello".into())),
            Flow::Continue
        );
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
        assert_eq!(
            handler.receive(TestMessage::Data("x".into())),
            Flow::Continue
        );
        assert_eq!(handler.receive(TestMessage::Ping), Flow::Stop);
    }

    #[test]
    fn message_is_clone_and_serialize() {
        // Verify TestMessage satisfies Message bounds
        fn assert_message<T: Message>() {}
        assert_message::<TestMessage>();
    }

    // ── HandlesRequest ─────────────────────────────────────

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    enum RequestReply {
        Pong,
        Echo(String),
    }

    struct RequestProto;

    impl Protocol for RequestProto {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.request")
        }
        type Message = TestMessage;
    }

    impl RequestProtocol for RequestProto {
        type Reply = RequestReply;
    }

    struct RequestHandler {
        received: Vec<TestMessage>,
    }

    impl HandlesRequest<RequestProto> for RequestHandler {
        fn receive_request(&mut self, msg: TestMessage, reply: ReplyPort<RequestReply>) -> Flow {
            self.received.push(msg.clone());
            match msg {
                TestMessage::Ping => reply.reply(RequestReply::Pong),
                TestMessage::Data(s) => reply.reply(RequestReply::Echo(s)),
            }
            Flow::Continue
        }
    }

    #[test]
    fn handles_request_dispatches_and_replies() {
        let mut handler = RequestHandler { received: vec![] };

        let (port, rx) = ReplyPort::channel();
        let flow = handler.receive_request(TestMessage::Ping, port);
        assert_eq!(flow, Flow::Continue);
        assert_eq!(rx.recv().unwrap(), Ok(RequestReply::Pong));

        let (port, rx) = ReplyPort::channel();
        let flow = handler.receive_request(TestMessage::Data("hello".into()), port);
        assert_eq!(flow, Flow::Continue);
        assert_eq!(rx.recv().unwrap(), Ok(RequestReply::Echo("hello".into())));

        assert_eq!(handler.received.len(), 2);
    }

    #[test]
    fn handles_request_dropped_reply_sends_failed() {
        // A handler that forgets to reply — the ReplyPort Drop
        // compensation sends ReplyFailed.
        struct ForgetfulHandler;

        impl HandlesRequest<RequestProto> for ForgetfulHandler {
            fn receive_request(
                &mut self,
                _msg: TestMessage,
                _reply: ReplyPort<RequestReply>,
            ) -> Flow {
                // Deliberately not calling reply.reply(...)
                // Drop fires at end of scope
                Flow::Continue
            }
        }

        let mut handler = ForgetfulHandler;
        let (port, rx) = ReplyPort::channel();
        let flow = handler.receive_request(TestMessage::Ping, port);
        assert_eq!(flow, Flow::Continue);
        assert_eq!(rx.recv().unwrap(), Err(crate::obligation::ReplyFailed));
    }

    #[test]
    fn handles_request_can_return_stop() {
        struct StopHandler;

        impl HandlesRequest<RequestProto> for StopHandler {
            fn receive_request(
                &mut self,
                _msg: TestMessage,
                reply: ReplyPort<RequestReply>,
            ) -> Flow {
                reply.reply(RequestReply::Pong);
                Flow::Stop
            }
        }

        let mut handler = StopHandler;
        let (port, rx) = ReplyPort::channel();
        let flow = handler.receive_request(TestMessage::Ping, port);
        assert_eq!(flow, Flow::Stop);
        assert_eq!(rx.recv().unwrap(), Ok(RequestReply::Pong));
    }

    #[test]
    fn same_handler_handles_both_notifications_and_requests() {
        // A single handler can impl both Handles<P> and
        // HandlesRequest<Q> — different protocols, same pane.
        struct DualHandler;

        impl Handles<TestProto> for DualHandler {
            fn receive(&mut self, _msg: TestMessage) -> Flow {
                Flow::Continue
            }
        }

        impl HandlesRequest<RequestProto> for DualHandler {
            fn receive_request(
                &mut self,
                _msg: TestMessage,
                reply: ReplyPort<RequestReply>,
            ) -> Flow {
                reply.reply(RequestReply::Pong);
                Flow::Continue
            }
        }

        let mut handler = DualHandler;

        // Notification dispatch
        assert_eq!(handler.receive(TestMessage::Ping), Flow::Continue);

        // Request dispatch
        let (port, rx) = ReplyPort::channel();
        assert_eq!(
            handler.receive_request(TestMessage::Ping, port),
            Flow::Continue
        );
        assert_eq!(rx.recv().unwrap(), Ok(RequestReply::Pong));
    }
}
