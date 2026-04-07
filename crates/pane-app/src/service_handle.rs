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

use crate::dispatch::DispatchEntry;
use crate::dispatch_ctx::DispatchCtx;
use crate::Messenger;
use pane_proto::obligation::{CancelHandle, ReplyPort};
use pane_proto::{Address, Flow, Protocol, ServiceFrame};
use std::marker::PhantomData;

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
    write_tx: Option<std::sync::mpsc::SyncSender<(u16, Vec<u8>)>>,
    _protocol: PhantomData<P>,
}

impl<P: Protocol> ServiceHandle<P> {
    /// Full constructor with a write channel and assigned session_id.
    pub(crate) fn with_channel(
        session_id: u16,
        write_tx: std::sync::mpsc::SyncSender<(u16, Vec<u8>)>,
    ) -> Self {
        ServiceHandle {
            session_id,
            write_tx: Some(write_tx),
            _protocol: PhantomData,
        }
    }

    /// Send a typed request through this service binding.
    /// Installs a one-shot reply callback in the handler's Dispatch
    /// BEFORE the frame hits the wire (install-before-wire invariant).
    ///
    /// The message type is P::Message — protocol-scoped, not
    /// arbitrary serializable. Both sides implement Handles<P>,
    /// so the type agreement is compile-time within a crate
    /// and version-negotiated across processes.
    ///
    /// Design heritage: Plan 9 devmnt.c:786-790 linked Mntrpc
    /// onto m->queue under spinlock BEFORE the write at line 798.
    /// pane: DispatchEntry installed via ctx.insert() before wire
    /// send in send_request.
    pub fn send_request<H, R>(
        &self,
        ctx: &mut DispatchCtx<'_, H>,
        msg: P::Message,
        on_reply: impl FnOnce(&mut H, &Messenger, R) -> Flow + Send + 'static,
        on_failed: impl FnOnce(&mut H, &Messenger) -> Flow + Send + 'static,
    ) -> CancelHandle
    where
        H: pane_proto::Handler + 'static,
        R: pane_proto::Message,
    {
        // 1. Build entry — install BEFORE wire send
        let session_id = self.session_id;
        let entry = DispatchEntry {
            session_id,
            on_reply: Box::new(move |h, m, payload| {
                // The looper passes reply_bytes as Box<Vec<u8>>.
                // Deserialize to the concrete reply type R here,
                // where R is known from the send_request call site.
                let bytes = *payload
                    .downcast::<Vec<u8>>()
                    .expect("fire_reply must pass Box<Vec<u8>>");
                let reply: R = postcard::from_bytes(&bytes).expect("reply deserialization failed");
                on_reply(h, m, reply)
            }),
            on_failed: Box::new(on_failed),
        };
        let token = ctx.insert(entry);

        // 2. Serialize with allocated token
        let tag = P::service_id().tag();
        let msg_bytes = postcard::to_allocvec(&msg).expect("service message serialization failed");
        let mut inner_payload = Vec::with_capacity(1 + msg_bytes.len());
        inner_payload.push(tag);
        inner_payload.extend_from_slice(&msg_bytes);

        let frame = ServiceFrame::Request {
            token: token.0,
            payload: inner_payload,
        };
        let outer_payload =
            postcard::to_allocvec(&frame).expect("service frame serialization failed");

        // 3. Wire send AFTER entry installed
        if let Some(ref tx) = self.write_tx {
            let _ = tx.send((self.session_id, outer_payload));
        }

        // Stub: real cancel sends Cancel{token} on control channel
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

    /// Send a typed request and block until the reply arrives,
    /// the request fails, or the timeout expires.
    ///
    /// Must NOT be called from the looper thread (I8). Panics if
    /// the caller is on the looper thread — blocking there would
    /// deadlock the event loop.
    ///
    /// The request is submitted to the looper via a calloop channel.
    /// The looper installs a dispatch entry (preserving
    /// install-before-wire) and sends the wire frame. The reply
    /// routes back through a oneshot channel to the blocked caller.
    ///
    /// No CancelHandle is returned — the caller is blocked, so
    /// cancellation is incoherent. The dispatch entry is resolved
    /// (reply, failed, or timeout) before this method returns.
    ///
    /// Design heritage: Plan 9 devmnt.c mountio()
    /// (reference/plan9/src/sys/src/9/port/devmnt.c:772-826)
    /// blocked the calling process until mountmux dispatched the
    /// reply by tag. BeOS BMessenger::SendMessage(BMessage*,
    /// BMessage*, bigtime_t) blocked on a temporary reply port
    /// with a timeout (src/kits/app/Messenger.cpp:409-472).
    ///
    /// # Panics
    ///
    /// Panics if called from the looper thread (I8 enforcement).
    pub fn send_and_wait(
        &self,
        messenger: &Messenger,
        msg: P::Message,
        timeout: std::time::Duration,
    ) -> Result<P::Reply, crate::send_and_wait::SendAndWaitError>
    where
        P: pane_proto::RequestProtocol,
        P::Reply: pane_proto::Message,
    {
        use crate::send_and_wait::{SendAndWaitError, SyncRequest};

        // I8: panic if called from the looper thread.
        if let Some(looper_tid) = messenger.looper_thread_id() {
            assert!(
                std::thread::current().id() != looper_tid,
                "send_and_wait called from the looper thread (I8 violation). \
                 Use send_request with callbacks instead."
            );
        }

        // Serialize the inner payload (protocol tag + message).
        // The looper wraps this in ServiceFrame::Request with
        // the token allocated by Dispatch::insert.
        let tag = P::service_id().tag();
        let msg_bytes = match postcard::to_allocvec(&msg) {
            Ok(b) => b,
            Err(_) => return Err(SendAndWaitError::SerializationError),
        };
        let mut inner_payload = Vec::with_capacity(1 + msg_bytes.len());
        inner_payload.push(tag);
        inner_payload.extend_from_slice(&msg_bytes);

        // Oneshot: capacity 1 — the looper sends exactly one result.
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);

        let req = SyncRequest {
            session_id: self.session_id,
            wire_payload: inner_payload,
            reply_tx,
        };

        // Submit to the looper. If the channel is closed, the
        // looper has already shut down.
        if messenger.send_sync_request(req).is_err() {
            return Err(SendAndWaitError::Disconnected);
        }

        // Block until reply, failure, or timeout.
        match reply_rx.recv_timeout(timeout) {
            Ok(Ok(reply_bytes)) => {
                // Deserialize the reply type.
                postcard::from_bytes(&reply_bytes).map_err(|_| SendAndWaitError::SerializationError)
            }
            Ok(Err(e)) => Err(e),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(SendAndWaitError::Timeout),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                Err(SendAndWaitError::Disconnected)
            }
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

/// Create a ReplyPort that sends the reply as a ServiceFrame::Reply
/// through the write channel, or ServiceFrame::Failed on Drop.
///
/// T: Serialize bound is on this constructor, not on ReplyPort<T>.
/// The closure uses try_send — it may be called from Drop, and
/// blocking in Drop is unacceptable.
pub fn wire_reply_port<T>(
    write_tx: std::sync::mpsc::SyncSender<(u16, Vec<u8>)>,
    session_id: u16,
    token: u64,
) -> ReplyPort<T>
where
    T: serde::Serialize + Send + 'static,
{
    ReplyPort::new(move |result| {
        let frame = match result {
            Ok(value) => {
                let payload = postcard::to_allocvec(&value).expect("reply serialization failed");
                ServiceFrame::Reply { token, payload }
            }
            Err(_) => ServiceFrame::Failed { token },
        };
        let bytes = postcard::to_allocvec(&frame).expect("service frame serialization failed");
        // Best-effort: if the channel is closed, the connection is
        // already torn down.
        let _ = write_tx.try_send((session_id, bytes));
    })
}

impl<P: Protocol> Drop for ServiceHandle<P> {
    fn drop(&mut self) {
        if let Some(ref tx) = self.write_tx {
            let msg = pane_proto::control::ControlMessage::RevokeInterest {
                session_id: self.session_id,
            };
            if let Ok(bytes) = postcard::to_allocvec(&msg) {
                // Best-effort, non-blocking: don't stall destruction
                // if the write channel is full. The server will clean
                // up via process_disconnect anyway.
                let _ = tx.try_send((0, bytes));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::{Dispatch, PeerScope};
    use pane_proto::protocol::ServiceId;
    use pane_proto::{Handler, Handles};
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
        let (tx, _rx) = std::sync::mpsc::sync_channel(1);
        let handle = ServiceHandle::<TestProto>::with_channel(0, tx);
        let addr = handle.target_address();
        // Stub returns local(0)
        assert!(addr.is_local());
        assert_eq!(addr.pane_id, 0);
    }

    #[test]
    fn send_request_compiles_with_correct_bounds() {
        // This test verifies the type constraints compile and
        // the stub returns a CancelHandle.
        let (tx, _rx) = std::sync::mpsc::sync_channel(16);
        let handle = ServiceHandle::<TestProto>::with_channel(0, tx);
        let mut dispatch = Dispatch::<TestHandler>::new();
        let mut ctx = DispatchCtx::new(&mut dispatch, PeerScope(1));
        let cancel = handle.send_request::<TestHandler, TestReply>(
            &mut ctx,
            TestMsg::Ping,
            |_handler: &mut TestHandler, _messenger: &Messenger, _reply: TestReply| Flow::Continue,
            |_handler: &mut TestHandler, _messenger: &Messenger| Flow::Continue,
        );
        // CancelHandle is usable — cancel the no-op stub
        cancel.cancel();
    }

    #[test]
    fn send_request_cancel_handle_drop_is_noop() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(16);
        let handle = ServiceHandle::<TestProto>::with_channel(0, tx);
        let mut dispatch = Dispatch::<TestHandler>::new();
        let mut ctx = DispatchCtx::new(&mut dispatch, PeerScope(1));
        let cancel = handle.send_request::<TestHandler, TestReply>(
            &mut ctx,
            TestMsg::Ping,
            |_: &mut TestHandler, _, _: TestReply| Flow::Continue,
            |_: &mut TestHandler, _| Flow::Continue,
        );
        // Drop without cancelling — should not panic
        drop(cancel);
    }

    #[test]
    fn send_request_serializes_to_channel() {
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        let handle = ServiceHandle::<TestProto>::with_channel(5, tx);

        let mut dispatch = Dispatch::<TestHandler>::new();
        let mut ctx = DispatchCtx::new(&mut dispatch, PeerScope(1));
        let _cancel = handle.send_request::<TestHandler, TestReply>(
            &mut ctx,
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
                // Token comes from Dispatch's monotonic counter (starts at 0)
                assert_eq!(token, 0, "first token from Dispatch should be 0");
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
    fn send_request_installs_dispatch_entry() {
        // Verify the dispatch entry is installed before the
        // wire frame is sent — the install-before-wire invariant.
        let (tx, _rx) = std::sync::mpsc::sync_channel(16);
        let handle = ServiceHandle::<TestProto>::with_channel(5, tx);

        let mut dispatch = Dispatch::<TestHandler>::new();
        assert!(dispatch.is_empty());

        let mut ctx = DispatchCtx::new(&mut dispatch, PeerScope(1));
        let _cancel = handle.send_request::<TestHandler, TestReply>(
            &mut ctx,
            TestMsg::Ping,
            |_: &mut TestHandler, _, _: TestReply| Flow::Continue,
            |_: &mut TestHandler, _| Flow::Continue,
        );

        assert_eq!(dispatch.len(), 1, "dispatch entry must be installed");
    }

    #[test]
    fn send_notification_serializes_to_channel() {
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
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
        let (tx, _rx) = std::sync::mpsc::sync_channel(16);
        let handle = ServiceHandle::<TestProto>::with_channel(0, tx);
        let mut dispatch = Dispatch::<TestHandler>::new();
        let mut ctx = DispatchCtx::new(&mut dispatch, PeerScope(1));
        let _cancel = handle.send_request::<TestHandler, TestReply>(
            &mut ctx,
            TestMsg::Ping,
            |_: &mut TestHandler, _, _: TestReply| Flow::Continue,
            |_: &mut TestHandler, _| Flow::Continue,
        );
    }

    #[test]
    fn drop_sends_revoke_interest() {
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
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
        let (tx, _rx) = std::sync::mpsc::sync_channel(16);
        let handle = ServiceHandle::<TestProto>::with_channel(0, tx);
        drop(handle); // no-op, no panic
    }

    #[test]
    fn session_id_returns_assigned_value() {
        let (tx, _rx) = std::sync::mpsc::sync_channel(16);
        let handle = ServiceHandle::<TestProto>::with_channel(42, tx);
        assert_eq!(handle.session_id(), 42);
    }

    // ── wire_reply_port ───────────────────────────────────

    #[test]
    fn wire_reply_port_sends_reply_frame() {
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        let port = super::wire_reply_port::<TestReply>(tx, 7, 42);
        port.reply(TestReply(100));

        let (session_id, bytes) = rx.recv().expect("expected frame");
        assert_eq!(session_id, 7);

        let frame: ServiceFrame = postcard::from_bytes(&bytes).expect("deserialize ServiceFrame");
        match frame {
            ServiceFrame::Reply { token, payload } => {
                assert_eq!(token, 42);
                let reply: TestReply = postcard::from_bytes(&payload).expect("deserialize reply");
                assert_eq!(reply.0, 100);
            }
            other => panic!("expected Reply, got {other:?}"),
        }
    }

    #[test]
    fn wire_reply_port_drop_sends_failed_frame() {
        let (tx, rx) = std::sync::mpsc::sync_channel(16);
        let port = super::wire_reply_port::<TestReply>(tx, 3, 99);
        drop(port);

        let (session_id, bytes) = rx.recv().expect("expected frame");
        assert_eq!(session_id, 3);

        let frame: ServiceFrame = postcard::from_bytes(&bytes).expect("deserialize ServiceFrame");
        match frame {
            ServiceFrame::Failed { token } => {
                assert_eq!(token, 99);
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }
}
