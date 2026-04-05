//! Protocol trait — links identity, typed messages, and service binding.

use crate::message::Message;
use uuid::Uuid;

/// Fixed namespace UUID for pane ServiceId derivation.
/// All pane ServiceIds use UUIDv5 with this namespace.
const PANE_NAMESPACE: Uuid = Uuid::from_bytes([
    0x70, 0x61, 0x6e, 0x65, // "pane"
    0x2d, 0x73, 0x65, 0x72, // "-ser"
    0x76, 0x69, 0x63, 0x65, // "vice"
    0x2d, 0x6e, 0x73, 0x00, // "-ns\0"
]);

/// Identity of a service in the pane protocol.
///
/// The UUID is the machine identity — deterministically derived from
/// the name via UUIDv5. Survives renames and travels across federation
/// boundaries where naming conventions may diverge.
/// The name is the human identity — for pane-fs paths, service maps,
/// and logs.
///
/// Plan 9: analogous to qid.path (stable, machine-comparable)
/// alongside the directory entry name (human-chosen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceId {
    pub uuid: Uuid,
    pub name: &'static str,
}

impl ServiceId {
    /// Derive a ServiceId from a reverse-DNS name.
    /// UUID is deterministically computed via UUIDv5.
    /// Not const fn (SHA-1 is not const-evaluable).
    /// For `const SERVICE_ID` in Protocol impls, use the
    /// `service_id!` proc-macro (future) which computes the
    /// UUID at compile time.
    pub fn new(name: &'static str) -> Self {
        ServiceId {
            uuid: Uuid::new_v5(&PANE_NAMESPACE, name.as_bytes()),
            name,
        }
    }

    /// Explicit UUID for services that have been renamed but must
    /// keep their wire identity.
    pub fn with_uuid(uuid: Uuid, name: &'static str) -> Self {
        ServiceId { uuid, name }
    }
}

impl serde::Serialize for ServiceId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Wire format: UUID bytes only. The name is debugging metadata.
        self.uuid.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for ServiceId {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let uuid = Uuid::deserialize(deserializer)?;
        // On the wire, only the UUID travels. The human name is
        // looked up locally from the known protocol registry.
        // Unknown UUIDs get a placeholder name.
        Ok(ServiceId {
            uuid,
            name: "<unknown>",
        })
    }
}

impl std::fmt::Display for ServiceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.uuid)
    }
}

/// A protocol relationship between a pane and a service.
pub trait Protocol {
    /// Service identity. Not const (UUID requires SHA-1).
    /// Use lazy_static or once_cell for static initialization.
    fn service_id() -> ServiceId;
    type Message: Message;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_id_deterministic() {
        let a = ServiceId::new("com.pane.lifecycle");
        let b = ServiceId::new("com.pane.lifecycle");
        assert_eq!(a, b);
        assert_eq!(a.uuid, b.uuid);
    }

    #[test]
    fn different_names_different_uuids() {
        let a = ServiceId::new("com.pane.lifecycle");
        let b = ServiceId::new("com.pane.clipboard");
        assert_ne!(a.uuid, b.uuid);
    }

    #[test]
    fn serialize_roundtrip() {
        let original = ServiceId::new("com.pane.lifecycle");
        let bytes = postcard::to_allocvec(&original).unwrap();
        let deserialized: ServiceId = postcard::from_bytes(&bytes).unwrap();
        // UUID survives; name becomes "<unknown>" (wire doesn't carry name)
        assert_eq!(original.uuid, deserialized.uuid);
        assert_eq!(deserialized.name, "<unknown>");
    }
}
