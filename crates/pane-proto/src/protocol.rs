//! Protocol trait — links identity, typed messages, and service binding.

use crate::message::Message;

/// Identity of a service in the pane protocol.
///
/// Uses &'static str for compile-time const construction (Protocol
/// trait requires const SERVICE_ID). Serializes as a string on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceId {
    pub name: &'static str,
}

// Note: Copy is fine — ServiceId is just a &'static str wrapper.

impl ServiceId {
    pub const fn new(name: &'static str) -> Self {
        ServiceId { name }
    }
}

// Manual Serialize: serialize the &'static str as a string
impl serde::Serialize for ServiceId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name)
    }
}

// Manual Deserialize: deserialize to &'static str by leaking.
// This is acceptable because ServiceIds are a small, fixed set
// of protocol identifiers — not arbitrary user data.
impl<'de> serde::Deserialize<'de> for ServiceId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s: String = serde::Deserialize::deserialize(deserializer)?;
        // Leak the string to get a 'static reference.
        // In practice, ServiceIds are matched against known constants,
        // so the leaked set is bounded.
        let leaked: &'static str = Box::leak(s.into_boxed_str());
        Ok(ServiceId { name: leaked })
    }
}

/// A protocol relationship between a pane and a service.
pub trait Protocol {
    const SERVICE_ID: ServiceId;
    type Message: Message;
}
