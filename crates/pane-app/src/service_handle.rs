//! ServiceHandle<P>: a live connection to a service.
//!
//! Bound to a specific Connection and negotiated version at open
//! time — service map changes affect new opens, not existing handles.
//! (Plan 9 fid semantics: bound at open, mount table changes affect
//! new walks only.)
//!
//! Drop sends RevokeInterest (idempotent).
//!
//! Design heritage: Plan 9 fids were the result of walk+open — a
//! bound handle to a specific file on a specific server. BeOS
//! BMessenger targeting a specific BHandler was the equivalent
//! addressing mechanism. ServiceHandle combines both: fid binding
//! semantics (stable after open) with typed protocol constraint
//! (Handles<P> at compile time, which BMessenger lacked).

use crate::Messenger;
use pane_proto::obligation::CancelHandle;
use pane_proto::{Address, Flow, Handler, Handles, Protocol, ServiceFrame};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

/// Monotonic token counter for request/reply correlation.
/// Global — tokens are unique across all ServiceHandles in a process.
///
/// Placeholder: when Reply routing lands, tokens will be sourced
/// from Dispatch (per-looper scope) instead. The global counter
/// is temporary so send_request can produce valid wire frames now.
static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

/// A live connection to a service. Parameterized by protocol.
///
/// Obtained from PaneBuilder::open_service. Protocol-specific
/// methods are added via `impl ServiceHandle<MyProtocol> { ... }`
/// in the protocol's own crate.
///
/// Design heritage: Plan 9 fids were the result of walk+open — a
/// bound handle to a specific file on a specific server. BeOS
/// BMessenger targeting a specific BHandler was the equivalent
/// addressing mechanism. ServiceHandle combines both: fid binding
/// semantics (stable after open) with typed protocol constraint
/// (Handles<P> at compile time, which BMessenger lacked).
#[must_use = "dropping a ServiceHandle revokes the service interest"]
pub struct ServiceHandle<P: Protocol> {
    session_id: u16,
    /// Channel to the connection's write half. Carries (service_id, payload).
    write_tx: Option<std::sync::mpsc::Sender<(u16, Vec<u8>)>>,
    _protocol: PhantomData<P>,
}

impl<P: Protocol> ServiceHandle<P> {
    /// Stub constructor — will be created by PaneBuilder::open_service.
    /// No write channel — sends are silently dropped.
    pub(crate) fn new() -> Self {
        ServiceHandle {
            session_id: 0,
            write_tx: None,
            _protocol: PhantomData,
        }
    }

    /// Full constructor with a write channel and assigned session_id.
    pub(crate) fn with_channel(
        session_id: u16,
        write_tx: std::sync::mpsc::Sender<(u16, Vec<u8>)>,
    ) -> Self {
        ServiceHandle {
            session_id,
            write_tx: Some(write_tx),
            _protocol: PhantomData,
        }
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
        let tag = P::service_id().tag();
        let msg_bytes = postcard::to_allocvec(&msg).expect("service message serialization failed");
        let mut inner_payload = Vec::with_capacity(1 + msg_bytes.len());
        inner_payload.push(tag);
        inner_payload.extend_from_slice(&msg_bytes);

        let token = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
        let frame = ServiceFrame::Request {
            token,
            payload: inner_payload,
        };
        let outer_payload =
            postcard::to_allocvec(&frame).expect("service frame serialization failed");

        if let Some(ref tx) = self.write_tx {
            let _ = tx.send((self.session_id, outer_payload));
        }

        // TODO: install Dispatch entry keyed by token so replies route back.
        // For now, stub callbacks are not installed.
        let _ = (on_reply, on_failed);
        CancelHandle::new(|| {})
    }

    /// Send a fire-and-forget notification through this service binding.
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

    /// The address of the pane providing this service.
    pub fn target_address(&self) -> Address {
        // TODO: return the resolved target address
        Address::local(0)
    }

    /// The session_id assigned by the server for this service binding.
    pub fn session_id(&self) -> u16 {
        self.session_id
    }
}

impl<P: Protocol> Drop for ServiceHandle<P> {
    fn drop(&mut self) {
        if let Some(ref tx) = self.write_tx {
            let msg = pane_proto::control::ControlMessage::RevokeInterest {
                session_id: self.session_id,
            };
            if let Ok(bytes) = postcard::to_allocvec(&msg) {
                // Best-effort: if the channel is closed, the server
                // will clean up via process_disconnect anyway.
                let _ = tx.send((0, bytes));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pane_proto::protocol::ServiceId;
    use serde::{Deserialize, Serialize};

    // -- Test protocol for exercising type bounds --

    struct TestProto;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum TestMsg {
        Ping,
    }

    impl Protocol for TestProto {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.service_handle")
        }
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
            |_handler: &mut TestHandler, _messenger: &Messenger, _reply: TestReply| Flow::Continue,
            |_handler: &mut TestHandler, _messenger: &Messenger| Flow::Continue,
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

    #[test]
    fn send_request_serializes_to_channel() {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = ServiceHandle::<TestProto>::with_channel(5, tx);

        let _cancel = handle.send_request::<TestHandler, TestReply>(
            TestMsg::Ping,
            |_: &mut TestHandler, _, _: TestReply| Flow::Continue,
            |_: &mut TestHandler, _| Flow::Continue,
        );

        let (service_id, payload) = rx.recv().expect("expected frame on channel");
        assert_eq!(service_id, 5);

        // Decode the outer ServiceFrame
        let frame: ServiceFrame =
            postcard::from_bytes(&payload).expect("ServiceFrame deserialization failed");
        match frame {
            ServiceFrame::Request { token, payload } => {
                assert!(token > 0, "token should be nonzero");
                // First byte is the protocol tag
                let expected_tag = TestProto::service_id().tag();
                assert_eq!(payload[0], expected_tag, "protocol tag must be first byte");
                // Decode inner message after the tag
                let msg: TestMsg = postcard::from_bytes(&payload[1..])
                    .expect("inner message deserialization failed");
                assert!(matches!(msg, TestMsg::Ping));
            }
            _ => panic!("expected Request frame"),
        }
    }

    #[test]
    fn send_notification_serializes_to_channel() {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = ServiceHandle::<TestProto>::with_channel(3, tx);

        handle.send_notification(TestMsg::Ping);

        let (service_id, payload) = rx.recv().expect("expected frame on channel");
        assert_eq!(service_id, 3);

        let frame: ServiceFrame =
            postcard::from_bytes(&payload).expect("ServiceFrame deserialization failed");
        match frame {
            ServiceFrame::Notification { payload } => {
                // First byte is the protocol tag
                let expected_tag = TestProto::service_id().tag();
                assert_eq!(payload[0], expected_tag, "protocol tag must be first byte");
                // Decode inner message after the tag
                let msg: TestMsg = postcard::from_bytes(&payload[1..])
                    .expect("inner message deserialization failed");
                assert!(matches!(msg, TestMsg::Ping));
            }
            _ => panic!("expected Notification frame"),
        }
    }

    #[test]
    fn stub_handle_send_request_does_not_panic() {
        // A handle without a write channel (stub) silently drops sends.
        let handle = ServiceHandle::<TestProto>::new();
        let _cancel = handle.send_request::<TestHandler, TestReply>(
            TestMsg::Ping,
            |_: &mut TestHandler, _, _: TestReply| Flow::Continue,
            |_: &mut TestHandler, _| Flow::Continue,
        );
    }

    #[test]
    fn drop_sends_revoke_interest() {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = ServiceHandle::<TestProto>::with_channel(5, tx);

        drop(handle);

        let (service, payload) = rx.recv().expect("expected RevokeInterest frame");
        assert_eq!(service, 0, "RevokeInterest goes on control channel");
        let msg: pane_proto::control::ControlMessage =
            postcard::from_bytes(&payload).expect("deserialize");
        match msg {
            pane_proto::control::ControlMessage::RevokeInterest { session_id } => {
                assert_eq!(session_id, 5);
            }
            other => panic!("expected RevokeInterest, got {other:?}"),
        }
    }

    #[test]
    fn drop_stub_handle_no_panic() {
        // A stub handle (no write channel) should not panic on drop.
        let handle = ServiceHandle::<TestProto>::new();
        drop(handle); // no-op, no panic
    }

    #[test]
    fn session_id_returns_assigned_value() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let handle = ServiceHandle::<TestProto>::with_channel(42, tx);
        assert_eq!(handle.session_id(), 42);
    }
}
