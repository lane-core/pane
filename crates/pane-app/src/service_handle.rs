//! ServiceHandle<P>: a live connection to a service.
//!
//! Bound to a specific Connection and negotiated version at open
//! time — service map changes affect new opens, not existing handles.
//! (Plan 9 fid semantics: bound at open, mount table changes affect
//! new walks only.)
//!
//! Drop sends RevokeInterest (idempotent).

use std::marker::PhantomData;
use pane_proto::{Address, Flow, Handler, Handles, Protocol};
use pane_proto::obligation::CancelHandle;
use crate::Messenger;

/// A live connection to a service. Parameterized by protocol.
///
/// Obtained from PaneBuilder::open_service. Protocol-specific
/// methods are added via `impl ServiceHandle<MyProtocol> { ... }`
/// in the protocol's own crate.
#[must_use = "dropping a ServiceHandle revokes the service interest"]
pub struct ServiceHandle<P: Protocol> {
    // TODO: service_id, connection_id, session_id, looper_tx
    _protocol: PhantomData<P>,
}

impl<P: Protocol> ServiceHandle<P> {
    /// Stub constructor — will be created by PaneBuilder::open_service.
    pub(crate) fn new() -> Self {
        ServiceHandle { _protocol: PhantomData }
    }

    /// Send a typed request through this service binding.
    /// Installs a one-shot reply callback in the handler's Dispatch.
    ///
    /// The message type is P::Message — protocol-scoped, not
    /// arbitrary serializable. Both sides implement Handles<P>,
    /// so the type agreement is compile-time within a crate
    /// and version-negotiated across processes.
    pub fn send_request<H, R>(
        &self,
        msg: P::Message,
        on_reply: impl FnOnce(&mut H, &Messenger, R) -> Flow + Send + 'static,
        on_failed: impl FnOnce(&mut H, &Messenger) -> Flow + Send + 'static,
    ) -> CancelHandle
    where
        H: Handler + Handles<P> + 'static,
        R: pane_proto::Message,
    {
        // TODO: serialize msg, install Dispatch entry, send via connection
        let _ = (msg, on_reply, on_failed);
        CancelHandle::new(|| {})
    }

    /// The address of the pane providing this service.
    pub fn target_address(&self) -> Address {
        // TODO: return the resolved target address
        Address::local(0)
    }
}

impl<P: Protocol> Drop for ServiceHandle<P> {
    fn drop(&mut self) {
        // TODO: send RevokeInterest via looper_tx
        // let _ = self.looper_tx.send(LooperMessage::RevokeInterest { ... });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pane_proto::protocol::ServiceId;
    use serde::{Serialize, Deserialize};

    // -- Test protocol for exercising type bounds --

    struct TestProto;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum TestMsg {
        Ping,
    }

    impl Protocol for TestProto {
        fn service_id() -> ServiceId { ServiceId::new("com.test.service_handle") }
        type Message = TestMsg;
    }

    // Handler that implements Handles<TestProto>
    struct TestHandler;

    impl Handler for TestHandler {}

    impl Handles<TestProto> for TestHandler {
        fn receive(&mut self, _msg: TestMsg) -> Flow {
            Flow::Continue
        }
    }

    // Reply type satisfying Message bounds
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestReply(u32);

    // -- Tests --

    #[test]
    fn target_address_returns_address() {
        let handle = ServiceHandle::<TestProto>::new();
        let addr = handle.target_address();
        // Stub returns local(0)
        assert!(addr.is_local());
        assert_eq!(addr.pane_id, 0);
    }

    #[test]
    fn send_request_compiles_with_correct_bounds() {
        // This test verifies the type constraints compile and
        // the stub returns a CancelHandle.
        let handle = ServiceHandle::<TestProto>::new();
        let cancel = handle.send_request::<TestHandler, TestReply>(
            TestMsg::Ping,
            |_handler: &mut TestHandler, _messenger: &Messenger, _reply: TestReply| {
                Flow::Continue
            },
            |_handler: &mut TestHandler, _messenger: &Messenger| {
                Flow::Continue
            },
        );
        // CancelHandle is usable — cancel the no-op stub
        cancel.cancel();
    }

    #[test]
    fn send_request_cancel_handle_drop_is_noop() {
        let handle = ServiceHandle::<TestProto>::new();
        let cancel = handle.send_request::<TestHandler, TestReply>(
            TestMsg::Ping,
            |_: &mut TestHandler, _, _: TestReply| Flow::Continue,
            |_: &mut TestHandler, _| Flow::Continue,
        );
        // Drop without cancelling — should not panic
        drop(cancel);
    }
}
