//! Typed setup phase for service registration.
//!
//! PaneBuilder<H> is generic over the handler type to enforce
//! Handles<P> bounds at compile time. Consumed by run_with.
//! Service dispatch routes are established before the event loop
//! starts; request/reply (Dispatch<H>) grows dynamically after.
//!
//! Design heritage: BeOS constructed BWindow + AddChild + handlers
//! before Show()/Run(). Plan 9 assembled namespaces (bind/mount)
//! before exec. PaneBuilder::connect() matches Plan 9's mount(2)
//! blocking semantics and Be's BApplication constructor registration.

use std::collections::HashSet;
use std::marker::PhantomData;

use pane_proto::{Handler, Handles, Protocol, RequestProtocol, ServiceId};

use crate::handles_request::HandlesRequest;
use pane_session::bridge::{self, LooperMessage, WriteMessage};
use pane_session::handshake::ServiceProvision;

use crate::pane::Pane;
use crate::service_dispatch::{make_request_receiver, make_service_receiver, ServiceDispatch};
use crate::service_handle::ServiceHandle;

/// Setup phase for a pane that will use protocol services.
///
/// open_service enforces Handles<P> bounds at compile time for
/// consuming services. serve enforces Handles<P> bounds for
/// providing services. Consumed by run_with — can't open or
/// serve after dispatch begins.
#[must_use = "a PaneBuilder must be consumed by run_with"]
pub struct PaneBuilder<H: Handler> {
    #[allow(dead_code)] // used when run_with passes pane identity to looper
    pane: Pane,
    /// Looper message receiver from the reader thread.
    rx: Option<std::sync::mpsc::Receiver<LooperMessage>>,
    /// Write channel to the writer thread.
    write_tx: Option<std::sync::mpsc::SyncSender<WriteMessage>>,
    /// Messages received during open_service that aren't responses
    /// to our DeclareInterest. Drained by run_with before entering
    /// the main loop.
    buffered_messages: Vec<LooperMessage>,
    registered_services: HashSet<ServiceId>,
    provided_services: Vec<ServiceProvision>,
    service_dispatch: ServiceDispatch<H>,
    _handler: PhantomData<H>,
}

impl<H: Handler> PaneBuilder<H> {
    pub(crate) fn new(pane: Pane) -> Self {
        PaneBuilder {
            pane,
            rx: None,
            write_tx: None,
            buffered_messages: Vec::new(),
            registered_services: HashSet::new(),
            provided_services: Vec::new(),
            service_dispatch: ServiceDispatch::new(),
            _handler: PhantomData,
        }
    }

    /// Advertise that this pane provides protocol P for others.
    ///
    /// Requires H: Handles<P> — compile-time proof the handler
    /// implements the protocol. Populates Hello.provides for the
    /// handshake.
    ///
    /// Panics on duplicate serve for the same ServiceId.
    pub fn serve<P: Protocol>(&mut self)
    where
        H: Handles<P>,
    {
        let id = P::service_id();
        let already_serving = self
            .provided_services
            .iter()
            .any(|p| p.service.uuid == id.uuid);
        assert!(!already_serving, "duplicate serve for {:?}", id);

        self.provided_services.push(ServiceProvision {
            service: id,
            version: 1, // TODO: Protocol::VERSION when versioning lands
        });
    }

    /// The list of ServiceProvisions this builder has accumulated.
    /// Used to populate Hello.provides during connection.
    pub fn provided_services(&self) -> &[ServiceProvision] {
        &self.provided_services
    }

    /// Connect to a server. Performs the handshake with Hello
    /// containing this builder's provided_services. Spawns reader
    /// and writer threads.
    ///
    /// Must be called before open_service. Panics if called twice.
    pub fn connect(
        &mut self,
        transport: impl pane_session::transport::TransportSplit,
    ) -> Result<(), pane_session::transport::ConnectError> {
        assert!(self.rx.is_none(), "already connected");

        let hello = pane_session::handshake::Hello {
            version: 1,
            max_message_size: 16 * 1024 * 1024,
            interests: vec![],
            provides: self.provided_services.clone(),
        };

        let conn = bridge::connect_and_run(hello, transport)?;

        self.rx = Some(conn.rx);
        self.write_tx = Some(conn.write_tx);
        Ok(())
    }

    /// Negotiate a service subscription: send DeclareInterest, block
    /// for InterestAccepted/Declined, and return the assigned
    /// session_id + write channel on success. Handles duplicate
    /// rejection, blocking loop, and message buffering.
    ///
    /// Private helper factored from open_service — the caller is
    /// responsible for registering dispatch receivers after this
    /// returns.
    fn open_service_inner<P: Protocol>(
        &mut self,
    ) -> Option<(u16, std::sync::mpsc::SyncSender<WriteMessage>)> {
        let id = P::service_id();
        assert!(
            self.registered_services.insert(id),
            "duplicate open_service for {:?}",
            id
        );

        let rx = self
            .rx
            .as_ref()
            .expect("must call connect() before open_service()");
        let write_tx = self
            .write_tx
            .as_ref()
            .expect("must call connect() before open_service()");

        // Send DeclareInterest on the control channel (service 0)
        let declare = pane_proto::control::ControlMessage::DeclareInterest {
            service: id,
            expected_version: 1,
        };
        let bytes = postcard::to_allocvec(&declare).expect("DeclareInterest serialization failed");
        if write_tx.send((0, bytes)).is_err() {
            return None; // write channel closed
        }

        // Block for response, buffering unrelated messages.
        // This matches Plan 9 mount(2) blocking and Be's
        // BApplication constructor registration pattern.
        loop {
            match rx.recv() {
                Ok(LooperMessage::Control(
                    pane_proto::control::ControlMessage::InterestAccepted {
                        service_uuid,
                        session_id,
                        ..
                    },
                )) if service_uuid == id.uuid => {
                    return Some((session_id, write_tx.clone()));
                }
                Ok(LooperMessage::Control(
                    pane_proto::control::ControlMessage::InterestDeclined { service_uuid, .. },
                )) if service_uuid == id.uuid => {
                    return None;
                }
                Ok(msg) => {
                    // Buffer for later delivery to looper
                    self.buffered_messages.push(msg);
                }
                Err(_) => return None, // channel closed
            }
        }
    }

    /// Open a notification-only service. Blocks until
    /// InterestAccepted/Declined. Returns None if the service
    /// is unavailable. Panics on duplicate ServiceId.
    ///
    /// The H: Handles<P> bound is checked HERE — this is where
    /// the compile-time verification that the handler can receive
    /// messages from protocol P happens.
    pub fn open_service<P: Protocol>(&mut self) -> Option<ServiceHandle<P>>
    where
        H: Handles<P>,
    {
        let (session_id, write_tx) = self.open_service_inner::<P>()?;
        self.service_dispatch
            .register(session_id, make_service_receiver::<H, P>());
        Some(ServiceHandle::with_channel(session_id, write_tx))
    }

    /// Open a service that supports both notifications and requests.
    /// Blocks until InterestAccepted/Declined. Returns None if the
    /// service is unavailable. Panics on duplicate ServiceId.
    ///
    /// Registers both a notification receiver (Handles<P>) and a
    /// request receiver (HandlesRequest<P>) for the session_id.
    /// request_receivers keys are always a subset of receivers keys
    /// when registered through this method.
    pub fn open_service_with_requests<P: RequestProtocol>(&mut self) -> Option<ServiceHandle<P>>
    where
        H: Handles<P> + HandlesRequest<P>,
    {
        let (session_id, write_tx) = self.open_service_inner::<P>()?;
        self.service_dispatch
            .register(session_id, make_service_receiver::<H, P>());
        self.service_dispatch.register_request(
            session_id,
            make_request_receiver::<H, P>(write_tx.clone(), session_id),
        );
        Some(ServiceHandle::with_channel(session_id, write_tx))
    }

    /// Consume the builder and enter the event loop.
    ///
    /// Drains buffered messages (received during open_service),
    /// then enters the calloop-backed Looper. Returns the exit reason.
    pub fn run_with(mut self, handler: H) -> crate::exit_reason::ExitReason {
        use crate::dispatch::PeerScope;
        use crate::looper::Looper;
        use crate::looper_core::{DispatchOutcome, LooperCore};

        let rx = self
            .rx
            .take()
            .expect("must call connect() before run_with()");
        let service_dispatch = std::mem::take(&mut self.service_dispatch);
        let buffered = std::mem::take(&mut self.buffered_messages);

        let write_tx_for_looper = self
            .write_tx
            .as_ref()
            .expect("must call connect() before run_with()")
            .clone();

        let (exit_tx, _exit_rx) = std::sync::mpsc::channel();

        let mut core = LooperCore::with_service_dispatch(
            handler,
            PeerScope(1),
            write_tx_for_looper,
            exit_tx,
            service_dispatch,
        );

        // Drain buffered messages from setup phase
        for msg in buffered {
            let outcome = match msg {
                LooperMessage::Control(pane_proto::control::ControlMessage::Lifecycle(lm)) => {
                    core.dispatch_lifecycle(lm)
                }
                LooperMessage::Service {
                    session_id,
                    payload,
                } => core.dispatch_service(session_id, &payload),
                LooperMessage::Control(_) => {
                    // Non-lifecycle control messages during setup —
                    // framework-internal, skip.
                    DispatchOutcome::Continue
                }
            };
            if let DispatchOutcome::Exit(reason) = outcome {
                core.shutdown();
                return reason;
            }
        }

        // Enter the calloop-backed event loop
        let looper = Looper::new(core);
        looper.run(rx)
    }
}

impl<H: Handler> Drop for PaneBuilder<H> {
    fn drop(&mut self) {
        // ServiceHandle<P> Drop sends RevokeInterest for each
        // open service. Server's process_disconnect is the backstop
        // if the connection closes before RevokeInterest is sent.
        // No additional cleanup needed here.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pane_proto::protocols::lifecycle::LifecycleMessage;
    use serde::{Deserialize, Serialize};

    // A test protocol
    struct TestService;
    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum TestServiceMessage {
        Ping,
    }

    impl Protocol for TestService {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.service")
        }
        type Message = TestServiceMessage;
    }

    // A second test protocol for multi-service tests
    struct OtherService;
    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum OtherServiceMessage {
        Pong,
    }

    impl Protocol for OtherService {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.other")
        }
        type Message = OtherServiceMessage;
    }

    // A handler that implements Handles<TestService> and Handles<OtherService>
    struct TestHandler;
    impl Handler for TestHandler {}
    impl Handles<TestService> for TestHandler {
        fn receive(&mut self, _msg: TestServiceMessage) -> pane_proto::Flow {
            pane_proto::Flow::Continue
        }
    }
    impl Handles<OtherService> for TestHandler {
        fn receive(&mut self, _msg: OtherServiceMessage) -> pane_proto::Flow {
            pane_proto::Flow::Continue
        }
    }

    #[test]
    fn setup_requires_handles_bound() {
        // This test is a compile-time check: if TestHandler doesn't
        // implement Handles<TestService>, builder.open_service::<TestService>()
        // won't compile. The fact that this module compiles is the test.
        //
        // A handler WITHOUT Handles<TestService>:
        struct NoServiceHandler;
        impl Handler for NoServiceHandler {}
        // builder.open_service::<TestService>() would not compile for NoServiceHandler
        // because NoServiceHandler does not implement Handles<TestService>.
    }

    #[test]
    fn serve_populates_provided_services() {
        let pane = Pane::new(crate::pane::Tag::new("test"));
        let mut builder = pane.setup::<TestHandler>();
        builder.serve::<TestService>();

        let provisions = builder.provided_services();
        assert_eq!(provisions.len(), 1);
        assert_eq!(provisions[0].service.uuid, TestService::service_id().uuid);
        assert_eq!(provisions[0].version, 1);
    }

    #[test]
    fn serve_multiple_protocols() {
        let pane = Pane::new(crate::pane::Tag::new("test"));
        let mut builder = pane.setup::<TestHandler>();
        builder.serve::<TestService>();
        builder.serve::<OtherService>();

        let provisions = builder.provided_services();
        assert_eq!(provisions.len(), 2);
    }

    #[test]
    #[should_panic(expected = "duplicate serve")]
    fn duplicate_serve_panics() {
        let pane = Pane::new(crate::pane::Tag::new("test"));
        let mut builder = pane.setup::<TestHandler>();
        builder.serve::<TestService>();
        builder.serve::<TestService>(); // panics
    }

    #[test]
    #[should_panic(expected = "must call connect()")]
    fn open_service_without_connect_panics() {
        let pane = Pane::new(crate::pane::Tag::new("test"));
        let mut builder = pane.setup::<TestHandler>();
        let _ = builder.open_service::<TestService>(); // panics
    }
}
