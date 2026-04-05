//! Protocol trait — links identity, typed messages, and service binding.

use crate::message::Message;

/// Identity of a service in the pane protocol.
///
/// The UUID is the machine identity — deterministically derived from
/// the name via UUIDv5. The name is the human identity — for pane-fs
/// paths, service maps, and logs.
///
/// Plan 9: analogous to qid.path (stable, machine-comparable)
/// alongside the directory entry name (human-chosen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceId {
    // TODO: pub uuid: Uuid — add uuid dependency when needed
    pub name: &'static str,
}

impl ServiceId {
    pub const fn new(name: &'static str) -> Self {
        ServiceId { name }
    }
}

/// A protocol relationship between a pane and a service.
///
/// Links identity (ServiceId) and typed messages into a single
/// type-level definition. Every service — lifecycle, display,
/// clipboard, routing, application-defined — is a Protocol.
pub trait Protocol {
    /// Service identity.
    const SERVICE_ID: ServiceId;
    /// The typed events this protocol produces. Must be Message
    /// (Clone + Serialize + DeserializeOwned + Send + 'static).
    type Message: Message;
}
