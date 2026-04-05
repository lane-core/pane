//! ServiceHandle<P>: a live connection to a service.
//!
//! Bound to a specific Connection and negotiated version at open
//! time — service map changes affect new opens, not existing handles.
//! (Plan 9 fid semantics: bound at open, mount table changes affect
//! new walks only.)
//!
//! Drop sends RevokeInterest (idempotent).

use std::marker::PhantomData;
use pane_session::Protocol;

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
}

impl<P: Protocol> Drop for ServiceHandle<P> {
    fn drop(&mut self) {
        // TODO: send RevokeInterest via looper_tx
        // let _ = self.looper_tx.send(LooperMessage::RevokeInterest { ... });
    }
}
