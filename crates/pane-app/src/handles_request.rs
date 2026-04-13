//! Request/reply handler dispatch.
//!
//! Separate from `Handles<P>` (in pane-proto) because request
//! handlers participate in the dispatch lifecycle: they receive
//! a `ReplyPort` obligation and a `DispatchCtx` for issuing
//! outbound requests. Notification handlers need neither.
//!
//! Design heritage: BeOS `BHandler::MessageReceived` handled both
//! notifications and requests in a single method. Forgetting to
//! reply was a common bug (`src/kits/app/Messenger.cpp:201-215`,
//! synchronous `SendMessage` blocks on reply port via
//! `handle_reply` in `src/kits/app/Message.cpp:114-141`). pane
//! splits the dispatch surface: `Handles<P>` for notifications,
//! `HandlesRequest<P>` for requests. The `ReplyPort` obligation
//! makes forgotten replies a compile-time warning and runtime
//! failure signal, never a hang.
//!
//! Crate placement: `HandlesRequest` lives in pane-app (not
//! pane-proto) because it depends on `DispatchCtx`, which wraps
//! `Dispatch<H>` — runtime dispatch infrastructure. This matches
//! Plan 9's separation: 9P message vocabulary (`fcall.h`) lived
//! in libc, while dispatch infrastructure (lib9p's `Srv`) lived
//! in the server framework library
//! (`reference/plan9/man/2/9p`).
//!
//! Theoretical basis: notifications are Cartesian copatterns
//! (duplicable/discardable values). Requests are linear
//! copatterns with a Cartesian extension (`DispatchCtx` carrying
//! the reply wire). The crate boundary matches the
//! Cartesian/linear partition (R4: value/obligation split maps
//! to Cartesian/linear optic split).

use pane_proto::obligation::ReplyPort;
use pane_proto::{Flow, RequestProtocol};

use crate::dispatch_ctx::DispatchCtx;

/// A handler that can receive request messages from protocol P
/// and reply through a `ReplyPort`.
///
/// Separate from `Handles<P>` because:
/// - Notifications deliver `P::Message` only
/// - Requests deliver `P::Message` + `ReplyPort<P::Reply>` +
///   `&mut DispatchCtx<Self>` for outbound requests
/// - The `ReplyPort` is an obligation (`#[must_use]`, Drop sends
///   `ReplyFailed`)
///
/// Session-type protocol fidelity: every Request produces
/// exactly one Reply or Failed (Ferrite, Chen/Balzer/Toninho
/// ECOOP 2022, §4.2). `DispatchCtx<Self>` binds `H = Self` by
/// construction — the handler type parameter cannot diverge.
pub trait HandlesRequest<P: RequestProtocol>: Sized {
    fn receive_request(
        &mut self,
        msg: P::Message,
        reply: ReplyPort<P::Reply>,
        ctx: &mut DispatchCtx<Self>,
    ) -> Flow;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::{Dispatch, PeerScope};
    use pane_proto::obligation::ReplyFailed;
    use pane_proto::protocol::ServiceId;
    use pane_proto::{Handles, Protocol};
    use pane_session::ActiveSession;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    enum TestMessage {
        Ping,
        Data(String),
    }

    struct TestProto;
    impl Protocol for TestProto {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.handles_request")
        }
        type Message = TestMessage;
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    enum RequestReply {
        Pong,
        Echo(String),
    }

    struct RequestProto;
    impl Protocol for RequestProto {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.handles_request.req")
        }
        type Message = TestMessage;
    }
    impl RequestProtocol for RequestProto {
        type Reply = RequestReply;
    }

    // ── HandlesRequest ─────────────────────────────────────

    struct RequestHandler {
        received: Vec<TestMessage>,
    }

    impl HandlesRequest<RequestProto> for RequestHandler {
        fn receive_request(
            &mut self,
            msg: TestMessage,
            reply: ReplyPort<RequestReply>,
            _ctx: &mut DispatchCtx<Self>,
        ) -> Flow {
            self.received.push(msg.clone());
            match msg {
                TestMessage::Ping => reply.reply(RequestReply::Pong),
                TestMessage::Data(s) => reply.reply(RequestReply::Echo(s)),
            }
            Flow::Continue
        }
    }

    fn make_ctx<'a>(
        dispatch: &'a mut Dispatch<RequestHandler>,
        session: &'a mut ActiveSession,
    ) -> DispatchCtx<'a, RequestHandler> {
        DispatchCtx::new(dispatch, session, PeerScope(1))
    }

    #[test]
    fn handles_request_dispatches_and_replies() {
        let mut handler = RequestHandler { received: vec![] };
        let mut dispatch = Dispatch::new();
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);

        let (port, rx) = ReplyPort::channel();
        let mut ctx = make_ctx(&mut dispatch, &mut session);
        let flow = handler.receive_request(TestMessage::Ping, port, &mut ctx);
        assert_eq!(flow, Flow::Continue);
        assert_eq!(rx.recv().unwrap(), Ok(RequestReply::Pong));

        let (port, rx) = ReplyPort::channel();
        let mut ctx = make_ctx(&mut dispatch, &mut session);
        let flow = handler.receive_request(TestMessage::Data("hello".into()), port, &mut ctx);
        assert_eq!(flow, Flow::Continue);
        assert_eq!(rx.recv().unwrap(), Ok(RequestReply::Echo("hello".into())));

        assert_eq!(handler.received.len(), 2);
    }

    #[test]
    fn handles_request_dropped_reply_sends_failed() {
        struct ForgetfulHandler;

        impl HandlesRequest<RequestProto> for ForgetfulHandler {
            fn receive_request(
                &mut self,
                _msg: TestMessage,
                _reply: ReplyPort<RequestReply>,
                _ctx: &mut DispatchCtx<Self>,
            ) -> Flow {
                // Deliberately not calling reply.reply(...)
                // Drop fires at end of scope
                Flow::Continue
            }
        }

        let mut handler = ForgetfulHandler;
        let mut dispatch = Dispatch::new();
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        let (port, rx) = ReplyPort::channel();
        let mut ctx = DispatchCtx::new(&mut dispatch, &mut session, PeerScope(1));
        let flow = handler.receive_request(TestMessage::Ping, port, &mut ctx);
        assert_eq!(flow, Flow::Continue);
        assert_eq!(rx.recv().unwrap(), Err(ReplyFailed));
    }

    #[test]
    fn handles_request_can_return_stop() {
        struct StopHandler;

        impl HandlesRequest<RequestProto> for StopHandler {
            fn receive_request(
                &mut self,
                _msg: TestMessage,
                reply: ReplyPort<RequestReply>,
                _ctx: &mut DispatchCtx<Self>,
            ) -> Flow {
                reply.reply(RequestReply::Pong);
                Flow::Stop
            }
        }

        let mut handler = StopHandler;
        let mut dispatch = Dispatch::new();
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        let (port, rx) = ReplyPort::channel();
        let mut ctx = DispatchCtx::new(&mut dispatch, &mut session, PeerScope(1));
        let flow = handler.receive_request(TestMessage::Ping, port, &mut ctx);
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
                _ctx: &mut DispatchCtx<Self>,
            ) -> Flow {
                reply.reply(RequestReply::Pong);
                Flow::Continue
            }
        }

        let mut handler = DualHandler;

        // Notification dispatch
        assert_eq!(handler.receive(TestMessage::Ping), Flow::Continue);

        // Request dispatch
        let mut dispatch = Dispatch::new();
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        let (port, rx) = ReplyPort::channel();
        let mut ctx = DispatchCtx::new(&mut dispatch, &mut session, PeerScope(1));
        assert_eq!(
            handler.receive_request(TestMessage::Ping, port, &mut ctx),
            Flow::Continue
        );
        assert_eq!(rx.recv().unwrap(), Ok(RequestReply::Pong));
    }
}
