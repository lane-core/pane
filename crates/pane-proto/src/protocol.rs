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

    /// 1-byte protocol tag derived from the UUID.
    ///
    /// Used as a defense-in-depth check at the type erasure
    /// boundary in ServiceDispatch. Both sender and receiver
    /// derive the same tag from the ServiceId established at
    /// DeclareInterest time. Mismatch means a routing bug
    /// delivered the wrong protocol's payload.
    ///
    /// XOR-fold: not injective (collisions possible across 256
    /// values), but catches ~255/256 of misroutes with zero
    /// coordination cost.
    pub fn tag(&self) -> u8 {
        self.uuid.as_bytes().iter().fold(0u8, |acc, b| acc ^ b)
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

/// A protocol that supports request/reply interactions.
///
/// Extends Protocol with a Reply type. Only protocols that impl
/// this supertrait can have requests sent via `send_request`.
/// Notification-only protocols impl only Protocol.
///
/// Design heritage: BeOS BMessage supported both fire-and-forget
/// (`BLooper::PostMessage`, `BMessenger::SendMessage` with no
/// reply — `src/kits/app/Looper.cpp:274`, `src/kits/app/Messenger.cpp:201`)
/// and request/reply (`BMessenger::SendMessage` with reply handler
/// or synchronous reply — `src/kits/app/Messenger.cpp:231`).
/// The distinction was at the call site. pane makes the protocol's
/// reply capability a type-level fact: `RequestProtocol` declares it,
/// `send_request` requires it.
pub trait RequestProtocol: Protocol {
    type Reply: Message;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

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
    fn protocol_tag_deterministic() {
        let a = ServiceId::new("com.pane.lifecycle");
        let b = ServiceId::new("com.pane.lifecycle");
        assert_eq!(a.tag(), b.tag());
    }

    #[test]
    fn different_protocols_usually_different_tags() {
        // XOR-fold is not injective — collisions are possible.
        // With 16 UUID bytes folded to 1, the probability of two
        // independent UUIDs colliding is ~1/256. This test uses
        // a handful of known-different services; if it fails,
        // the XOR-fold accidentally collided on these inputs,
        // which is unlikely but technically allowed.
        let ids: Vec<ServiceId> = vec![
            ServiceId::new("com.pane.lifecycle"),
            ServiceId::new("com.pane.clipboard"),
            ServiceId::new("com.pane.echo"),
            ServiceId::new("com.test.query"),
        ];
        let tags: Vec<u8> = ids.iter().map(|id| id.tag()).collect();
        // At least some should differ. With 4 independent UUIDs,
        // the probability of all 4 colliding is ~(1/256)^3 ≈ 0.
        let unique: std::collections::HashSet<u8> = tags.iter().copied().collect();
        assert!(
            unique.len() > 1,
            "expected distinct tags among different services, got {:?}",
            tags
        );
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

    // ── RequestProtocol ────────────────────────────────────

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum QueryMessage {
        Lookup(String),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum QueryReply {
        Found(String),
        NotFound,
    }

    struct QueryProto;

    impl Protocol for QueryProto {
        fn service_id() -> ServiceId {
            ServiceId::new("com.test.query")
        }
        type Message = QueryMessage;
    }

    impl RequestProtocol for QueryProto {
        type Reply = QueryReply;
    }

    #[test]
    fn request_protocol_extends_protocol() {
        // RequestProtocol is a superset — service_id and Message
        // are inherited from Protocol.
        assert_eq!(QueryProto::service_id().name, "com.test.query");

        // Reply type is accessible through the supertrait.
        fn assert_reply_is_message<P: RequestProtocol>()
        where
            P::Reply: crate::message::Message,
        {
        }
        assert_reply_is_message::<QueryProto>();
    }

    #[test]
    fn notification_only_protocol_does_not_impl_request() {
        // A protocol without RequestProtocol cannot be used
        // where RequestProtocol is required. This is a compile-time
        // property — the test documents the distinction.
        struct NotifyOnly;

        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct Tick;

        impl Protocol for NotifyOnly {
            fn service_id() -> ServiceId {
                ServiceId::new("com.test.notify")
            }
            type Message = Tick;
        }

        // NotifyOnly is a valid Protocol...
        fn accepts_protocol<P: Protocol>() {}
        accepts_protocol::<NotifyOnly>();

        // ...but uncommenting the following would fail to compile:
        //   fn accepts_request<P: RequestProtocol>() {}
        //   accepts_request::<NotifyOnly>(); // ERROR: NotifyOnly doesn't impl RequestProtocol
    }
}
