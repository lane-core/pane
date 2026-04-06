//! Wire service 0 envelope — the control protocol.
//!
//! ControlMessage is the outer dispatch enum for service 0, the
//! implicit control channel on every pane connection. Postcard-encoded.
//!
//! The looper demuxes: Lifecycle → Handler methods, connection
//! management → framework-internal. The developer never sees
//! ControlMessage directly — it's plumbing between the bridge
//! thread and the looper's dispatch table.
//!
//! Lifecycle dispatches to Handler methods. DeclareInterest,
//! InterestAccepted/Declined, RevokeInterest, and ServiceTeardown
//! are handled by the server and PaneBuilder. Cancel is defined
//! for wire format stability and will be connected later.
//!
//! Design heritage: Plan 9 9P used a type byte to discriminate
//! message kinds (Tversion=100, Tread=116, etc.) in a flat
//! namespace (intro(5), reference/plan9/man/5/0intro:91-96).
//! BeOS ServerProtocol.h had ~280 flat AS_* opcodes
//! (headers/private/app/ServerProtocol.h:32-373).
//! ControlMessage nests sub-protocols (Lifecycle, service
//! negotiation) as enum variants — the type system groups related
//! messages, an improvement over both flat dispatch tables.

use serde::{Serialize, Deserialize};
use crate::protocols::lifecycle::LifecycleMessage;
use crate::protocol::ServiceId;

/// Wire service 0 envelope. Postcard-encoded.
///
/// The looper demuxes: Lifecycle → Handler methods,
/// connection management → framework-internal.
/// The developer never sees ControlMessage directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ControlMessage {
    /// Lifecycle sub-protocol (every pane).
    Lifecycle(LifecycleMessage),
    /// Service negotiation: client declares interest.
    DeclareInterest {
        service: ServiceId,
        expected_version: u32,
    },
    /// Service negotiation: server accepts.
    InterestAccepted {
        service_uuid: uuid::Uuid,
        session_id: u8,
        version: u32,
    },
    /// Service negotiation: server declines.
    InterestDeclined {
        service_uuid: uuid::Uuid,
        reason: DeclineReason,
    },
    /// Server tears down a service.
    ServiceTeardown {
        session_id: u8,
        reason: TeardownReason,
    },
    /// Client revokes interest in a service.
    RevokeInterest {
        session_id: u8,
    },
    /// Advisory request cancellation (Tflush equivalent).
    Cancel {
        token: u64,
    },
}

/// Why the server declined a service interest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DeclineReason {
    VersionMismatch,
    ServiceUnknown,
}

/// Why the server is tearing down a service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TeardownReason {
    ServiceRevoked,
    ConnectionLost,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_lifecycle() {
        let msg = ControlMessage::Lifecycle(LifecycleMessage::Ready);
        assert!(matches!(msg, ControlMessage::Lifecycle(LifecycleMessage::Ready)));
    }

    #[test]
    fn construct_declare_interest() {
        let msg = ControlMessage::DeclareInterest {
            service: ServiceId::new("com.pane.clipboard"),
            expected_version: 1,
        };
        assert!(matches!(msg, ControlMessage::DeclareInterest { expected_version: 1, .. }));
    }

    #[test]
    fn construct_interest_accepted() {
        let msg = ControlMessage::InterestAccepted {
            service_uuid: uuid::Uuid::nil(),
            session_id: 3,
            version: 1,
        };
        assert!(matches!(msg, ControlMessage::InterestAccepted { session_id: 3, .. }));
    }

    #[test]
    fn construct_interest_declined() {
        let msg = ControlMessage::InterestDeclined {
            service_uuid: uuid::Uuid::nil(),
            reason: DeclineReason::ServiceUnknown,
        };
        assert!(matches!(msg, ControlMessage::InterestDeclined { .. }));
    }

    #[test]
    fn construct_service_teardown() {
        let msg = ControlMessage::ServiceTeardown {
            session_id: 1,
            reason: TeardownReason::ConnectionLost,
        };
        assert!(matches!(msg, ControlMessage::ServiceTeardown { session_id: 1, .. }));
    }

    #[test]
    fn construct_revoke_interest() {
        let msg = ControlMessage::RevokeInterest { session_id: 5 };
        assert!(matches!(msg, ControlMessage::RevokeInterest { session_id: 5 }));
    }

    #[test]
    fn construct_cancel() {
        let msg = ControlMessage::Cancel { token: 42 };
        assert!(matches!(msg, ControlMessage::Cancel { token: 42 }));
    }

    #[test]
    fn roundtrip_lifecycle() {
        let msg = ControlMessage::Lifecycle(LifecycleMessage::Pulse);
        let bytes = postcard::to_allocvec(&msg).expect("serialize");
        let decoded: ControlMessage = postcard::from_bytes(&bytes).expect("deserialize");
        assert!(matches!(decoded, ControlMessage::Lifecycle(LifecycleMessage::Pulse)));
    }

    #[test]
    fn roundtrip_declare_interest() {
        let msg = ControlMessage::DeclareInterest {
            service: ServiceId::new("com.pane.display"),
            expected_version: 2,
        };
        let bytes = postcard::to_allocvec(&msg).expect("serialize");
        let decoded: ControlMessage = postcard::from_bytes(&bytes).expect("deserialize");
        match decoded {
            ControlMessage::DeclareInterest { service, expected_version } => {
                // UUID survives roundtrip; name becomes "<unknown>" (wire format)
                assert_eq!(service.uuid, ServiceId::new("com.pane.display").uuid);
                assert_eq!(expected_version, 2);
            }
            other => panic!("expected DeclareInterest, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_interest_accepted() {
        let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_DNS, b"bob.example.com");
        let msg = ControlMessage::InterestAccepted {
            service_uuid: id,
            session_id: 7,
            version: 3,
        };
        let bytes = postcard::to_allocvec(&msg).expect("serialize");
        let decoded: ControlMessage = postcard::from_bytes(&bytes).expect("deserialize");
        match decoded {
            ControlMessage::InterestAccepted { service_uuid, session_id, version } => {
                assert_eq!(service_uuid, id);
                assert_eq!(session_id, 7);
                assert_eq!(version, 3);
            }
            other => panic!("expected InterestAccepted, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_cancel() {
        let msg = ControlMessage::Cancel { token: 0xDEAD_BEEF };
        let bytes = postcard::to_allocvec(&msg).expect("serialize");
        let decoded: ControlMessage = postcard::from_bytes(&bytes).expect("deserialize");
        match decoded {
            ControlMessage::Cancel { token } => assert_eq!(token, 0xDEAD_BEEF),
            other => panic!("expected Cancel, got {other:?}"),
        }
    }
}
