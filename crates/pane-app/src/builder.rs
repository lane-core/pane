//! Typed setup phase for service registration.
//!
//! PaneBuilder<H> is generic over the handler type to enforce
//! Handles<P> bounds at compile time. Consumed by run_with.
//! Service dispatch routes are established before the event loop
//! starts; request/reply (Dispatch<H>) grows dynamically after.
//!
//! Design heritage: BeOS constructed BWindow + AddChild + handlers
//! before Show()/Run(). Plan 9 assembled namespaces (bind/mount)
//! before exec.

use std::collections::HashSet;
use std::marker::PhantomData;

use pane_proto::{Flow, Handler, Handles, Protocol, ServiceId};
use pane_session::handshake::ServiceProvision;

use crate::pane::Pane;
use crate::service_handle::ServiceHandle;

/// Setup phase for a pane that will use protocol services.
///
/// open_service enforces Handles<P> bounds at compile time for
/// consuming services. serve enforces Handles<P> bounds for
/// providing services. Consumed by run_with — can't open or
/// serve after dispatch begins.
#[must_use = "a PaneBuilder must be consumed by run_with"]
pub struct PaneBuilder<H: Handler> {
    pane: Pane,
    registered_services: HashSet<ServiceId>,
    provided_services: Vec<ServiceProvision>,
    _handler: PhantomData<H>,
}

impl<H: Handler> PaneBuilder<H> {
    pub(crate) fn new(pane: Pane) -> Self {
        PaneBuilder {
            pane,
            registered_services: HashSet::new(),
            provided_services: Vec::new(),
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
        let already_serving = self.provided_services.iter()
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

    /// Open a service. Blocks until InterestAccepted/Declined.
    /// Returns None if the service is unavailable.
    /// Panics on duplicate ServiceId.
    ///
    /// The H: Handles<P> bound is checked HERE — this is where
    /// the compile-time verification that the handler can receive
    /// messages from protocol P happens.
    pub fn open_service<P: Protocol>(&mut self) -> Option<ServiceHandle<P>>
    where
        H: Handles<P>,
    {
        let id = P::service_id();
        assert!(
            self.registered_services.insert(id),
            "duplicate open_service for {:?}", id
        );
        // TODO: resolve via service map, send DeclareInterest,
        // block until InterestAccepted/Declined, capture fn pointer
        Some(ServiceHandle::new())
    }

    /// Consume the builder and enter the event loop (headless).
    pub fn run_with(self, _handler: H) -> ! {
        // TODO: looper with catch_unwind, calloop
        let _ = self;
        std::process::exit(0)
    }
}

impl<H: Handler> Drop for PaneBuilder<H> {
    fn drop(&mut self) {
        // Revoke all accepted interests. Idempotent with
        // ServiceHandle<P> Drop.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pane_proto::protocols::lifecycle::LifecycleMessage;
    use serde::{Serialize, Deserialize};

    // A test protocol
    struct TestService;
    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum TestServiceMessage { Ping }

    impl Protocol for TestService {
        fn service_id() -> ServiceId { ServiceId::new("com.test.service") }
        type Message = TestServiceMessage;
    }

    // A second test protocol for multi-service tests
    struct OtherService;
    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum OtherServiceMessage { Pong }

    impl Protocol for OtherService {
        fn service_id() -> ServiceId { ServiceId::new("com.test.other") }
        type Message = OtherServiceMessage;
    }

    // A handler that implements Handles<TestService> and Handles<OtherService>
    struct TestHandler;
    impl Handler for TestHandler {}
    impl Handles<TestService> for TestHandler {
        fn receive(&mut self, _msg: TestServiceMessage) -> Flow {
            Flow::Continue
        }
    }
    impl Handles<OtherService> for TestHandler {
        fn receive(&mut self, _msg: OtherServiceMessage) -> Flow {
            Flow::Continue
        }
    }

    #[test]
    fn open_service_returns_handle() {
        let pane = Pane { id: 1, tag: crate::pane::Tag::new("test") };
        let mut builder = pane.setup::<TestHandler>();
        let handle = builder.open_service::<TestService>();
        assert!(handle.is_some());
    }

    #[test]
    #[should_panic(expected = "duplicate open_service")]
    fn duplicate_open_service_panics() {
        let pane = Pane { id: 1, tag: crate::pane::Tag::new("test") };
        let mut builder = pane.setup::<TestHandler>();
        let _ = builder.open_service::<TestService>();
        let _ = builder.open_service::<TestService>(); // panics
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
        let pane = Pane { id: 1, tag: crate::pane::Tag::new("test") };
        let mut builder = pane.setup::<TestHandler>();
        builder.serve::<TestService>();

        let provisions = builder.provided_services();
        assert_eq!(provisions.len(), 1);
        assert_eq!(provisions[0].service.uuid, TestService::service_id().uuid);
        assert_eq!(provisions[0].version, 1);
    }

    #[test]
    fn serve_multiple_protocols() {
        let pane = Pane { id: 1, tag: crate::pane::Tag::new("test") };
        let mut builder = pane.setup::<TestHandler>();
        builder.serve::<TestService>();
        builder.serve::<OtherService>();

        let provisions = builder.provided_services();
        assert_eq!(provisions.len(), 2);
    }

    #[test]
    #[should_panic(expected = "duplicate serve")]
    fn duplicate_serve_panics() {
        let pane = Pane { id: 1, tag: crate::pane::Tag::new("test") };
        let mut builder = pane.setup::<TestHandler>();
        builder.serve::<TestService>();
        builder.serve::<TestService>(); // panics
    }
}
