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

use crate::Messenger;
use pane_proto::{Flow, Handles, Protocol};
use std::collections::HashMap;

/// Type-erased service notification receiver.
/// Deserializes inner payload as P::Message and calls
/// Handles<P>::receive on the handler.
pub type ServiceReceiver<H> = Box<dyn Fn(&mut H, &Messenger, &[u8]) -> Flow + Send>;

/// Static dispatch table for service frames.
/// Frozen before the looper runs — no dynamic registration.
pub struct ServiceDispatch<H> {
    receivers: HashMap<u16, ServiceReceiver<H>>,
}

impl<H> ServiceDispatch<H> {
    pub fn new() -> Self {
        ServiceDispatch {
            receivers: HashMap::new(),
        }
    }

    /// Register a dispatch entry for a session_id.
    /// Called by PaneBuilder during setup.
    pub fn register(&mut self, session_id: u16, receiver: ServiceReceiver<H>) {
        let prev = self.receivers.insert(session_id, receiver);
        assert!(
            prev.is_none(),
            "duplicate session_id {session_id} in service dispatch"
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

    pub fn is_empty(&self) -> bool {
        self.receivers.is_empty()
    }

    pub fn len(&self) -> usize {
        self.receivers.len()
    }
}

/// Create a type-erased receiver for protocol P.
/// Captures the concrete P::Message type in the closure.
/// The Handles<P> bound is satisfied at the call site
/// (PaneBuilder::open_service where H: Handles<P> is in scope).
pub fn make_service_receiver<H, P>() -> ServiceReceiver<H>
where
    H: Handles<P>,
    P: Protocol,
{
    Box::new(|handler: &mut H, _messenger: &Messenger, payload: &[u8]| {
        let msg: P::Message =
            postcard::from_bytes(payload).expect("service message deserialization failed");
        handler.receive(msg)
    })
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

    #[test]
    fn dispatch_notification_to_handler() {
        let mut dispatch = ServiceDispatch::<TestHandler>::new();
        dispatch.register(3, make_service_receiver::<TestHandler, TestProto>());

        let mut handler = TestHandler { received: vec![] };
        let messenger = crate::Messenger::new();
        let payload = postcard::to_allocvec(&TestMsg::Ping("hello".into())).unwrap();

        let flow = dispatch.dispatch_notification(3, &mut handler, &messenger, &payload);
        assert_eq!(flow, Some(Flow::Continue));
        assert_eq!(handler.received, vec!["hello"]);
    }

    #[test]
    fn dispatch_unknown_session_returns_none() {
        let dispatch = ServiceDispatch::<TestHandler>::new();
        let mut handler = TestHandler { received: vec![] };
        let messenger = crate::Messenger::new();

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
        let messenger = crate::Messenger::new();

        let ping_bytes = postcard::to_allocvec(&TestMsg::Ping("a".into())).unwrap();
        let pong_bytes = postcard::to_allocvec(&OtherMsg::Pong(42)).unwrap();

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
}
