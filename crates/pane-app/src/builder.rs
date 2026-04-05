//! PaneBuilder<H>: typed setup phase for service registration.
//!
//! Generic over H to enforce Handles<P> bounds at compile time.
//! Consumed by run_with — the builder pattern where the terminal
//! method both builds and enters the event loop.
//!
//! EAct: restricts σ construction to a pre-loop phase. The dynamic
//! part of σ (Dispatch<H> entries from send_request) still grows
//! after run_with.
//!
//! Plan 9: namespace construction (bind/mount) before exec.

use std::collections::HashSet;
use std::marker::PhantomData;

use pane_proto::{Flow, Handler, Handles, Protocol, ServiceId};

use crate::pane::Pane;
use crate::service_handle::ServiceHandle;

/// Setup phase for a pane that will use protocol services.
///
/// open_service enforces Handles<P> bounds at compile time.
/// Consumed by run_with — can't open services after dispatch begins.
#[must_use = "a PaneBuilder must be consumed by run_with"]
pub struct PaneBuilder<H: Handler> {
    pane: Pane,
    registered_services: HashSet<ServiceId>,
    _handler: PhantomData<H>,
}

impl<H: Handler> PaneBuilder<H> {
    pub(crate) fn new(pane: Pane) -> Self {
        PaneBuilder {
            pane,
            registered_services: HashSet::new(),
            _handler: PhantomData,
        }
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

    // A handler that implements Handles<TestService>
    struct TestHandler;
    impl Handler for TestHandler {}
    impl Handles<TestService> for TestHandler {
        fn receive(&mut self, _msg: TestServiceMessage) -> Flow {
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
}
