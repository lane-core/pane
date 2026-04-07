//! Static service dispatch table.
//!
//! Maps session_id -> type-erased handler fn. Populated during
//! PaneBuilder setup (where Handles<P> bounds are in scope),
//! frozen before the looper runs.
//!
//! Design heritage: labeled coproduct eliminator (the optics agent
//! confirmed this is NOT an optic — service dispatch and MonadicLens
//! are separate concerns with different categorical structures).
//! BeOS BLooper dispatched by message `what` field through a flat
//! switch (src/kits/app/Looper.cpp:547, DispatchMessage). Plan 9
//! devmnt dispatched by Fcall type through mntrpc()
//! (devmnt.c:803). The table here is typed: the closure captures
//! the concrete P::Message type from the Handles<P> bound.
//!
//! The table is a positive structure holding negative values
//! (closures awaiting invocation). Type erasure at this boundary
//! is the correct duploid move: ↑P at serialize (sender), ↓P at
//! deserialize (inside the closure). See session-type consultant's
//! wiring soundness analysis.

use crate::dispatch_ctx::DispatchCtx;
use crate::handles_request::HandlesRequest;
use crate::Messenger;
use pane_proto::{Flow, Handles, Protocol, RequestProtocol, ServiceFrame};
use std::collections::HashMap;
use std::sync::mpsc::SyncSender;

/// Type-erased service notification receiver.
/// Deserializes inner payload as P::Message and calls
/// Handles<P>::receive on the handler.
pub type ServiceReceiver<H> = Box<dyn Fn(&mut H, &Messenger, &[u8]) -> Flow + Send>;

/// Type-erased service request receiver.
/// Deserializes inner payload as P::Message, constructs a
/// ReplyPort<P::Reply>, and calls HandlesRequest<P>::receive_request
/// on the handler. The closure captures write_tx and session_id at
/// construction time — reply routing is wired at registration, not
/// at dispatch.
///
/// 5 params: handler, messenger, inner payload (tag byte + message
/// bytes), token for reply correlation, dispatch context for
/// outbound requests.
pub type RequestReceiver<H> =
    Box<dyn Fn(&mut H, &Messenger, &[u8], u64, &mut DispatchCtx<'_, H>) -> Flow + Send>;

/// Static dispatch table for service frames.
/// Frozen before the looper runs — no dynamic registration.
///
/// Two tables: notification receivers (keyed by session_id) and
/// request receivers (keyed by session_id). When registered via
/// PaneBuilder::open_service_with_requests, request_receivers keys
/// are always a subset of receivers keys.
pub struct ServiceDispatch<H> {
    receivers: HashMap<u16, ServiceReceiver<H>>,
    request_receivers: HashMap<u16, RequestReceiver<H>>,
}

impl<H> Default for ServiceDispatch<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H> ServiceDispatch<H> {
    pub fn new() -> Self {
        ServiceDispatch {
            receivers: HashMap::new(),
            request_receivers: HashMap::new(),
        }
    }

    /// Register a notification dispatch entry for a session_id.
    /// Called by PaneBuilder during setup.
    pub fn register(&mut self, session_id: u16, receiver: ServiceReceiver<H>) {
        let prev = self.receivers.insert(session_id, receiver);
        assert!(
            prev.is_none(),
            "duplicate session_id {session_id} in service dispatch"
        );
    }

    /// Register a request dispatch entry for a session_id.
    /// Called by PaneBuilder during setup.
    pub fn register_request(&mut self, session_id: u16, receiver: RequestReceiver<H>) {
        let prev = self.request_receivers.insert(session_id, receiver);
        assert!(
            prev.is_none(),
            "duplicate request session_id {session_id} in service dispatch"
        );
    }

    /// Dispatch a Notification payload to the handler for this session_id.
    /// Returns None if the session_id is not registered (unknown service frame).
    pub fn dispatch_notification(
        &self,
        session_id: u16,
        handler: &mut H,
        messenger: &Messenger,
        payload: &[u8],
    ) -> Option<Flow> {
        let receiver = self.receivers.get(&session_id)?;
        Some(receiver(handler, messenger, payload))
    }

    /// Dispatch a Request payload to the handler for this session_id.
    ///
    /// Total: every Request produces exactly one Reply or Failed on
    /// the wire. If no request receiver is registered for this
    /// session_id, sends Failed via write_tx so the consumer's
    /// Dispatch entry isn't orphaned, then returns Flow::Continue.
    ///
    /// The write_tx parameter is ONLY for the fallback path (no
    /// receiver registered). Per-protocol RequestReceiver closures
    /// use their captured write_tx clones from registration time —
    /// these are two different failure domains.
    ///
    /// Design heritage: session types require protocol fidelity —
    /// every Request produces exactly one Reply or Failed (Ferrite,
    /// Chen/Balzer/Toninho ECOOP 2022, §4.2). Returning Flow (not
    /// Option<Flow>) encodes totality at the type level.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_request(
        &self,
        session_id: u16,
        handler: &mut H,
        messenger: &Messenger,
        payload: &[u8],
        token: u64,
        write_tx: &SyncSender<(u16, Vec<u8>)>,
        ctx: &mut DispatchCtx<'_, H>,
    ) -> Flow {
        match self.request_receivers.get(&session_id) {
            Some(receiver) => receiver(handler, messenger, payload, token, ctx),
            None => {
                // No request receiver registered — send Failed so the
                // consumer's Dispatch entry isn't orphaned.
                send_failed(write_tx, session_id, token);
                Flow::Continue
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.receivers.is_empty() && self.request_receivers.is_empty()
    }

    /// Number of notification receivers.
    pub fn len(&self) -> usize {
        self.receivers.len()
    }

    /// Number of request receivers.
    pub fn request_len(&self) -> usize {
        self.request_receivers.len()
    }
}

/// Create a type-erased receiver for protocol P.
/// Captures the concrete P::Message type in the closure.
/// The Handles<P> bound is satisfied at the call site
/// (PaneBuilder::open_service where H: Handles<P> is in scope).
///
/// The closure checks the protocol tag byte (S8) before
/// deserializing the inner payload. Mismatch means a routing
/// bug — the frame is dropped, not panicked.
pub fn make_service_receiver<H, P>() -> ServiceReceiver<H>
where
    H: Handles<P>,
    P: Protocol,
{
    let expected_tag = P::service_id().tag();
    Box::new(
        move |handler: &mut H, _messenger: &Messenger, payload: &[u8]| {
            // Protocol tag check (S8): verify the payload's tag matches
            // the protocol registered for this session_id.
            if payload.is_empty() || payload[0] != expected_tag {
                // Tag mismatch — routing delivered the wrong protocol's
                // payload to this closure. Drop the frame.
                return Flow::Continue;
            }
            let msg: P::Message = postcard::from_bytes(&payload[1..])
                .expect("service message deserialization failed");
            handler.receive(msg)
        },
    )
}

/// Create a type-erased request receiver for protocol P.
/// Captures write_tx and session_id for reply routing, and
/// the concrete P::Message + P::Reply types in the closure.
///
/// The closure:
/// 1. Checks the protocol tag byte (S8) — mismatch sends
///    ServiceFrame::Failed via write_tx and returns Flow::Continue.
///    A tag mismatch is a routing bug; the consumer's Dispatch entry
///    would be orphaned if we silently dropped.
/// 2. Deserializes P::Message — failure sends ServiceFrame::Failed
///    and returns Flow::Continue. A malformed payload on one request
///    must not kill the pane.
/// 3. Constructs ReplyPort<P::Reply> via wire_reply_port.
/// 4. Calls handler.receive_request(msg, reply_port).
///
/// Invariant: every Request with a token produces exactly one
/// Reply or Failed on the wire.
pub fn make_request_receiver<H, P>(
    write_tx: SyncSender<(u16, Vec<u8>)>,
    session_id: u16,
) -> RequestReceiver<H>
where
    H: HandlesRequest<P>,
    P: RequestProtocol,
{
    let expected_tag = P::service_id().tag();
    Box::new(
        move |handler: &mut H,
              _messenger: &Messenger,
              payload: &[u8],
              token: u64,
              ctx: &mut DispatchCtx<'_, H>| {
            // Protocol tag check (S8): verify the payload's tag matches
            // the protocol registered for this session_id.
            if payload.is_empty() || payload[0] != expected_tag {
                // Tag mismatch — send Failed so the consumer's Dispatch
                // entry doesn't become orphaned.
                send_failed(&write_tx, session_id, token);
                return Flow::Continue;
            }
            let msg: P::Message = match postcard::from_bytes(&payload[1..]) {
                Ok(m) => m,
                Err(_) => {
                    // Deserialization failure — send Failed, don't panic.
                    send_failed(&write_tx, session_id, token);
                    return Flow::Continue;
                }
            };
            let reply_port = crate::service_handle::wire_reply_port::<P::Reply>(
                write_tx.clone(),
                session_id,
                token,
            );
            handler.receive_request(msg, reply_port, ctx)
        },
    )
}

/// Send a ServiceFrame::Failed for a given token. Best-effort:
/// if the channel is full or closed, the connection is already
/// torn down.
pub(crate) fn send_failed(write_tx: &SyncSender<(u16, Vec<u8>)>, session_id: u16, token: u64) {
    let frame = ServiceFrame::Failed { token };
    if let Ok(bytes) = postcard::to_allocvec(&frame) {
        let _ = write_tx.try_send((session_id, bytes));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pane_proto::{Flow, Handler, Handles, Protocol, ServiceId};
    use serde::{Deserialize, Serialize};

    struct TestProto;
    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum TestMsg {
        Ping(String),
    }

    impl Protocol for TestProto {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.dispatch")
        }
        type Message = TestMsg;
    }

    struct TestHandler {
        received: Vec<String>,
    }
    impl Handler for TestHandler {}
    impl Handles<TestProto> for TestHandler {
        fn receive(&mut self, msg: TestMsg) -> Flow {
            match msg {
                TestMsg::Ping(s) => self.received.push(s),
            }
            Flow::Continue
        }
    }

    /// Build a tagged payload: protocol tag byte + postcard-serialized message.
    fn tagged_payload<P: Protocol>(msg: &P::Message) -> Vec<u8> {
        let tag = P::service_id().tag();
        let msg_bytes = postcard::to_allocvec(msg).unwrap();
        let mut payload = Vec::with_capacity(1 + msg_bytes.len());
        payload.push(tag);
        payload.extend_from_slice(&msg_bytes);
        payload
    }

    #[test]
    fn dispatch_notification_to_handler() {
        let mut dispatch = ServiceDispatch::<TestHandler>::new();
        dispatch.register(3, make_service_receiver::<TestHandler, TestProto>());

        let mut handler = TestHandler { received: vec![] };
        let messenger = crate::Messenger::stub();
        let payload = tagged_payload::<TestProto>(&TestMsg::Ping("hello".into()));

        let flow = dispatch.dispatch_notification(3, &mut handler, &messenger, &payload);
        assert_eq!(flow, Some(Flow::Continue));
        assert_eq!(handler.received, vec!["hello"]);
    }

    #[test]
    fn dispatch_unknown_session_returns_none() {
        let dispatch = ServiceDispatch::<TestHandler>::new();
        let mut handler = TestHandler { received: vec![] };
        let messenger = crate::Messenger::stub();

        let flow = dispatch.dispatch_notification(99, &mut handler, &messenger, &[]);
        assert!(flow.is_none());
    }

    #[test]
    fn multiple_protocols_dispatch_independently() {
        struct OtherProto;
        #[derive(Debug, Clone, Serialize, Deserialize)]
        enum OtherMsg {
            Pong(u32),
        }
        impl Protocol for OtherProto {
            fn service_id() -> ServiceId {
                ServiceId::new("com.test.other.dispatch")
            }
            type Message = OtherMsg;
        }

        struct MultiHandler {
            pings: Vec<String>,
            pongs: Vec<u32>,
        }
        impl Handler for MultiHandler {}
        impl Handles<TestProto> for MultiHandler {
            fn receive(&mut self, msg: TestMsg) -> Flow {
                match msg {
                    TestMsg::Ping(s) => self.pings.push(s),
                }
                Flow::Continue
            }
        }
        impl Handles<OtherProto> for MultiHandler {
            fn receive(&mut self, msg: OtherMsg) -> Flow {
                match msg {
                    OtherMsg::Pong(n) => self.pongs.push(n),
                }
                Flow::Continue
            }
        }

        let mut dispatch = ServiceDispatch::<MultiHandler>::new();
        dispatch.register(1, make_service_receiver::<MultiHandler, TestProto>());
        dispatch.register(2, make_service_receiver::<MultiHandler, OtherProto>());

        let mut handler = MultiHandler {
            pings: vec![],
            pongs: vec![],
        };
        let messenger = crate::Messenger::stub();

        let ping_bytes = tagged_payload::<TestProto>(&TestMsg::Ping("a".into()));
        let pong_bytes = tagged_payload::<OtherProto>(&OtherMsg::Pong(42));

        dispatch.dispatch_notification(1, &mut handler, &messenger, &ping_bytes);
        dispatch.dispatch_notification(2, &mut handler, &messenger, &pong_bytes);

        assert_eq!(handler.pings, vec!["a"]);
        assert_eq!(handler.pongs, vec![42]);
    }

    #[test]
    #[should_panic(expected = "duplicate session_id")]
    fn duplicate_session_id_panics() {
        let mut dispatch = ServiceDispatch::<TestHandler>::new();
        dispatch.register(1, make_service_receiver::<TestHandler, TestProto>());
        dispatch.register(1, make_service_receiver::<TestHandler, TestProto>());
    }

    #[test]
    fn wrong_protocol_tag_drops_frame() {
        let mut dispatch = ServiceDispatch::<TestHandler>::new();
        dispatch.register(3, make_service_receiver::<TestHandler, TestProto>());

        let mut handler = TestHandler { received: vec![] };
        let messenger = crate::Messenger::stub();

        // Build a payload with the wrong tag byte (flipped).
        let correct_tag = TestProto::service_id().tag();
        let wrong_tag = !correct_tag;
        let msg_bytes = postcard::to_allocvec(&TestMsg::Ping("stealth".into())).unwrap();
        let mut payload = Vec::with_capacity(1 + msg_bytes.len());
        payload.push(wrong_tag);
        payload.extend_from_slice(&msg_bytes);

        let flow = dispatch.dispatch_notification(3, &mut handler, &messenger, &payload);
        assert_eq!(flow, Some(Flow::Continue));
        assert!(
            handler.received.is_empty(),
            "handler must NOT receive a frame with a wrong protocol tag"
        );
    }

    #[test]
    fn empty_payload_drops_frame() {
        let mut dispatch = ServiceDispatch::<TestHandler>::new();
        dispatch.register(3, make_service_receiver::<TestHandler, TestProto>());

        let mut handler = TestHandler { received: vec![] };
        let messenger = crate::Messenger::stub();

        let flow = dispatch.dispatch_notification(3, &mut handler, &messenger, &[]);
        assert_eq!(flow, Some(Flow::Continue));
        assert!(handler.received.is_empty());
    }

    // ── RequestReceiver tests ────────────────────────────────

    use crate::dispatch::{Dispatch, PeerScope};
    use crate::dispatch_ctx::DispatchCtx;
    use crate::handles_request::HandlesRequest;
    use pane_proto::obligation::ReplyPort;
    use pane_proto::{RequestProtocol, ServiceFrame};
    use std::sync::mpsc::sync_channel;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    enum TestReply {
        Ack(String),
    }

    struct TestRequestProto;

    impl Protocol for TestRequestProto {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.request.dispatch")
        }
        type Message = TestMsg;
    }

    impl RequestProtocol for TestRequestProto {
        type Reply = TestReply;
    }

    struct RequestHandler {
        received: Vec<String>,
    }
    impl Handler for RequestHandler {}
    impl Handles<TestRequestProto> for RequestHandler {
        fn receive(&mut self, msg: TestMsg) -> Flow {
            match msg {
                TestMsg::Ping(s) => self.received.push(s),
            }
            Flow::Continue
        }
    }
    impl HandlesRequest<TestRequestProto> for RequestHandler {
        fn receive_request(
            &mut self,
            msg: TestMsg,
            reply: ReplyPort<TestReply>,
            _ctx: &mut DispatchCtx<Self>,
        ) -> Flow {
            match msg {
                TestMsg::Ping(s) => {
                    let echo = s.clone();
                    self.received.push(s);
                    reply.reply(TestReply::Ack(echo));
                }
            }
            Flow::Continue
        }
    }

    #[test]
    fn request_tag_mismatch_sends_failed() {
        let (write_tx, write_rx) = sync_channel(16);
        let receiver = make_request_receiver::<RequestHandler, TestRequestProto>(write_tx, 5);

        let mut handler = RequestHandler { received: vec![] };
        let messenger = crate::Messenger::stub();

        // Wrong tag byte
        let correct_tag = TestRequestProto::service_id().tag();
        let wrong_tag = !correct_tag;
        let msg_bytes = postcard::to_allocvec(&TestMsg::Ping("x".into())).unwrap();
        let mut payload = vec![wrong_tag];
        payload.extend_from_slice(&msg_bytes);

        let mut dispatch = Dispatch::new();
        let mut ctx = DispatchCtx::new(&mut dispatch, PeerScope(1));
        let flow = receiver(&mut handler, &messenger, &payload, 42, &mut ctx);
        assert_eq!(flow, Flow::Continue);
        assert!(handler.received.is_empty());

        // Verify Failed was sent on the wire
        let (sid, bytes) = write_rx.recv().expect("expected Failed frame");
        assert_eq!(sid, 5);
        let frame: ServiceFrame = postcard::from_bytes(&bytes).unwrap();
        assert!(matches!(frame, ServiceFrame::Failed { token: 42 }));
    }

    #[test]
    fn request_deserialization_failure_sends_failed() {
        let (write_tx, write_rx) = sync_channel(16);
        let receiver = make_request_receiver::<RequestHandler, TestRequestProto>(write_tx, 5);

        let mut handler = RequestHandler { received: vec![] };
        let messenger = crate::Messenger::stub();

        // Correct tag, garbage payload
        let correct_tag = TestRequestProto::service_id().tag();
        let payload = vec![correct_tag, 0xFF, 0xFF, 0xFF, 0xFF];

        let mut dispatch = Dispatch::new();
        let mut ctx = DispatchCtx::new(&mut dispatch, PeerScope(1));
        let flow = receiver(&mut handler, &messenger, &payload, 99, &mut ctx);
        assert_eq!(flow, Flow::Continue);
        assert!(handler.received.is_empty());

        let (sid, bytes) = write_rx.recv().expect("expected Failed frame");
        assert_eq!(sid, 5);
        let frame: ServiceFrame = postcard::from_bytes(&bytes).unwrap();
        assert!(matches!(frame, ServiceFrame::Failed { token: 99 }));
    }

    #[test]
    fn request_dispatches_to_handler() {
        let (write_tx, write_rx) = sync_channel(16);
        let receiver = make_request_receiver::<RequestHandler, TestRequestProto>(write_tx, 7);

        let mut handler = RequestHandler { received: vec![] };
        let messenger = crate::Messenger::stub();

        let payload = tagged_payload::<TestRequestProto>(&TestMsg::Ping("hello".into()));

        let mut dispatch = Dispatch::new();
        let mut ctx = DispatchCtx::new(&mut dispatch, PeerScope(1));
        let flow = receiver(&mut handler, &messenger, &payload, 10, &mut ctx);
        assert_eq!(flow, Flow::Continue);
        assert_eq!(handler.received, vec!["hello"]);

        // Verify Reply was sent on the wire
        let (sid, bytes) = write_rx.recv().expect("expected Reply frame");
        assert_eq!(sid, 7);
        let frame: ServiceFrame = postcard::from_bytes(&bytes).unwrap();
        match frame {
            ServiceFrame::Reply { token, payload } => {
                assert_eq!(token, 10);
                let reply: TestReply = postcard::from_bytes(&payload).unwrap();
                assert_eq!(reply, TestReply::Ack("hello".into()));
            }
            other => panic!("expected Reply, got {other:?}"),
        }
    }

    #[test]
    fn request_handler_panic_sends_failed_via_drop() {
        // When the handler panics, the ReplyPort is dropped during
        // unwind, which fires the Drop compensation (Failed frame).
        struct PanicHandler;
        impl Handler for PanicHandler {}
        impl Handles<TestRequestProto> for PanicHandler {
            fn receive(&mut self, _msg: TestMsg) -> Flow {
                Flow::Continue
            }
        }
        impl HandlesRequest<TestRequestProto> for PanicHandler {
            fn receive_request(
                &mut self,
                _msg: TestMsg,
                _reply: ReplyPort<TestReply>,
                _ctx: &mut DispatchCtx<Self>,
            ) -> Flow {
                panic!("handler crashed");
            }
        }

        let (write_tx, write_rx) = sync_channel(16);
        let receiver = make_request_receiver::<PanicHandler, TestRequestProto>(write_tx, 3);

        let mut handler = PanicHandler;
        let messenger = crate::Messenger::stub();
        let payload = tagged_payload::<TestRequestProto>(&TestMsg::Ping("boom".into()));

        let mut dispatch = Dispatch::new();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut ctx = DispatchCtx::new(&mut dispatch, PeerScope(1));
            receiver(&mut handler, &messenger, &payload, 55, &mut ctx)
        }));
        assert!(result.is_err());

        // ReplyPort Drop fired during unwind — verify Failed was sent
        let (sid, bytes) = write_rx.recv().expect("expected Failed frame from Drop");
        assert_eq!(sid, 3);
        let frame: ServiceFrame = postcard::from_bytes(&bytes).unwrap();
        assert!(matches!(frame, ServiceFrame::Failed { token: 55 }));
    }

    #[test]
    fn multiple_request_protocols_dispatch_independently() {
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        enum OtherReqMsg {
            Query(u32),
        }
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
        enum OtherReply {
            Answer(u32),
        }

        struct OtherRequestProto;
        impl Protocol for OtherRequestProto {
            fn service_id() -> ServiceId {
                ServiceId::new("com.test.other.request.dispatch")
            }
            type Message = OtherReqMsg;
        }
        impl RequestProtocol for OtherRequestProto {
            type Reply = OtherReply;
        }

        struct MultiReqHandler {
            pings: Vec<String>,
            queries: Vec<u32>,
        }
        impl Handler for MultiReqHandler {}
        impl Handles<TestRequestProto> for MultiReqHandler {
            fn receive(&mut self, _msg: TestMsg) -> Flow {
                Flow::Continue
            }
        }
        impl HandlesRequest<TestRequestProto> for MultiReqHandler {
            fn receive_request(
                &mut self,
                msg: TestMsg,
                reply: ReplyPort<TestReply>,
                _ctx: &mut DispatchCtx<Self>,
            ) -> Flow {
                match msg {
                    TestMsg::Ping(s) => {
                        self.pings.push(s.clone());
                        reply.reply(TestReply::Ack(s));
                    }
                }
                Flow::Continue
            }
        }
        impl Handles<OtherRequestProto> for MultiReqHandler {
            fn receive(&mut self, _msg: OtherReqMsg) -> Flow {
                Flow::Continue
            }
        }
        impl HandlesRequest<OtherRequestProto> for MultiReqHandler {
            fn receive_request(
                &mut self,
                msg: OtherReqMsg,
                reply: ReplyPort<OtherReply>,
                _ctx: &mut DispatchCtx<Self>,
            ) -> Flow {
                match msg {
                    OtherReqMsg::Query(n) => {
                        self.queries.push(n);
                        reply.reply(OtherReply::Answer(n * 2));
                    }
                }
                Flow::Continue
            }
        }

        let (write_tx, write_rx) = sync_channel(16);
        let mut service_dispatch = ServiceDispatch::<MultiReqHandler>::new();
        service_dispatch.register_request(
            1,
            make_request_receiver::<MultiReqHandler, TestRequestProto>(write_tx.clone(), 1),
        );
        service_dispatch.register_request(
            2,
            make_request_receiver::<MultiReqHandler, OtherRequestProto>(write_tx, 2),
        );

        let mut handler = MultiReqHandler {
            pings: vec![],
            queries: vec![],
        };
        let messenger = crate::Messenger::stub();

        let ping_payload = tagged_payload::<TestRequestProto>(&TestMsg::Ping("a".into()));
        let query_payload = {
            let tag = OtherRequestProto::service_id().tag();
            let msg_bytes = postcard::to_allocvec(&OtherReqMsg::Query(21)).unwrap();
            let mut p = Vec::with_capacity(1 + msg_bytes.len());
            p.push(tag);
            p.extend_from_slice(&msg_bytes);
            p
        };

        // Fallback write_tx — only used if dispatch finds no receiver.
        // Here both session_ids are registered, so this is never used.
        let (fallback_tx, _fallback_rx) = sync_channel(16);
        let mut dispatch_table = Dispatch::new();
        let mut ctx = DispatchCtx::new(&mut dispatch_table, PeerScope(1));
        service_dispatch.dispatch_request(
            1,
            &mut handler,
            &messenger,
            &ping_payload,
            100,
            &fallback_tx,
            &mut ctx,
        );
        let mut ctx = DispatchCtx::new(&mut dispatch_table, PeerScope(1));
        service_dispatch.dispatch_request(
            2,
            &mut handler,
            &messenger,
            &query_payload,
            200,
            &fallback_tx,
            &mut ctx,
        );

        assert_eq!(handler.pings, vec!["a"]);
        assert_eq!(handler.queries, vec![21]);

        // Verify both replies
        let mut replies: Vec<(u16, ServiceFrame)> = Vec::new();
        while let Ok((sid, bytes)) = write_rx.try_recv() {
            replies.push((sid, postcard::from_bytes(&bytes).unwrap()));
        }
        assert_eq!(replies.len(), 2);
    }

    #[test]
    fn request_unknown_session_sends_failed() {
        let service_dispatch = ServiceDispatch::<RequestHandler>::new();
        let mut handler = RequestHandler { received: vec![] };
        let messenger = crate::Messenger::stub();

        let (fallback_tx, fallback_rx) = sync_channel(16);
        let mut dispatch_table = Dispatch::new();
        let mut ctx = DispatchCtx::new(&mut dispatch_table, PeerScope(1));
        let flow = service_dispatch.dispatch_request(
            99,
            &mut handler,
            &messenger,
            &[],
            1,
            &fallback_tx,
            &mut ctx,
        );
        assert_eq!(flow, Flow::Continue);

        // No receiver registered — Failed sent via fallback write_tx
        let (sid, bytes) = fallback_rx.recv().expect("expected Failed frame");
        assert_eq!(sid, 99);
        let frame: ServiceFrame = postcard::from_bytes(&bytes).unwrap();
        assert!(matches!(frame, ServiceFrame::Failed { token: 1 }));
    }

    #[test]
    #[should_panic(expected = "duplicate request session_id")]
    fn duplicate_request_session_id_panics() {
        let (tx1, _) = sync_channel(1);
        let (tx2, _) = sync_channel(1);
        let mut dispatch = ServiceDispatch::<RequestHandler>::new();
        dispatch.register_request(
            1,
            make_request_receiver::<RequestHandler, TestRequestProto>(tx1, 1),
        );
        dispatch.register_request(
            1,
            make_request_receiver::<RequestHandler, TestRequestProto>(tx2, 1),
        );
    }

    #[test]
    fn is_empty_checks_both_tables() {
        let mut dispatch = ServiceDispatch::<RequestHandler>::new();
        assert!(dispatch.is_empty());

        let (tx, _) = sync_channel(1);
        dispatch.register_request(
            1,
            make_request_receiver::<RequestHandler, TestRequestProto>(tx, 1),
        );
        assert!(!dispatch.is_empty());
    }

    #[test]
    fn request_empty_payload_sends_failed() {
        let (write_tx, write_rx) = sync_channel(16);
        let receiver = make_request_receiver::<RequestHandler, TestRequestProto>(write_tx, 5);

        let mut handler = RequestHandler { received: vec![] };
        let messenger = crate::Messenger::stub();

        let mut dispatch = Dispatch::new();
        let mut ctx = DispatchCtx::new(&mut dispatch, PeerScope(1));
        let flow = receiver(&mut handler, &messenger, &[], 77, &mut ctx);
        assert_eq!(flow, Flow::Continue);
        assert!(handler.received.is_empty());

        let (sid, bytes) = write_rx.recv().expect("expected Failed frame");
        assert_eq!(sid, 5);
        let frame: ServiceFrame = postcard::from_bytes(&bytes).unwrap();
        assert!(matches!(frame, ServiceFrame::Failed { token: 77 }));
    }
}
