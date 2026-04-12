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
//! are handled by the server and PaneBuilder. Cancel is sent by
//! CancelHandle and handled as advisory no-op server-side (D10).
//!
//! Design heritage: Plan 9 9P used a type byte to discriminate
//! message kinds (Tversion=100, Tread=116, etc.) in a flat
//! namespace (intro(5), reference/plan9/man/5/0intro:91-96).
//! BeOS ServerProtocol.h had ~280 flat AS_* opcodes
//! (headers/private/app/ServerProtocol.h:32-373).
//! ControlMessage nests sub-protocols (Lifecycle, service
//! negotiation) as enum variants — the type system groups related
//! messages, an improvement over both flat dispatch tables.

use crate::address::Address;
use crate::exit_reason::ExitReason;
use crate::protocol::ServiceId;
use crate::protocols::lifecycle::LifecycleMessage;
use serde::{Deserialize, Serialize};

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
        session_id: u16,
        version: u32,
    },
    /// Service negotiation: server declines.
    InterestDeclined {
        service_uuid: uuid::Uuid,
        reason: DeclineReason,
    },
    /// Server tears down a service.
    ServiceTeardown {
        session_id: u16,
        reason: TeardownReason,
    },
    /// Client revokes interest in a service.
    RevokeInterest { session_id: u16 },
    /// Advisory request cancellation (Tflush equivalent).
    Cancel { token: u64 },

    /// Register for death notification of a target pane.
    /// Server-mediated: the server maintains the watch table.
    /// If the target is already dead at registration time,
    /// the server sends PaneExited immediately (no race window).
    ///
    /// Fire-and-forget registration -- no obligation handle.
    /// Cancel via Unwatch. Server cleans up on watcher disconnect.
    ///
    /// # BeOS
    ///
    /// `BRoster::StartWatching`
    /// (src/servers/registrar/TRoster.cpp:1523-1536) registered
    /// watchers at the registrar, which broadcast `B_SOME_APP_QUIT`
    /// on app death. pane's ProtocolServer is the registrar analog.
    Watch { target: Address },

    /// Cancel a prior Watch registration.
    Unwatch { target: Address },

    /// Server -> client: a watched pane has exited.
    /// Delivered on the control channel (service 0).
    /// One-shot: the watch entry is consumed on delivery
    /// (EAct E-InvokeM semantics -- Fowler/Hu S3.3).
    ///
    /// # BeOS
    ///
    /// `B_SOME_APP_QUIT` carried identity fields through
    /// `WatchingService::NotifyWatchers()`
    /// (src/servers/registrar/WatchingService.cpp:204-228).
    PaneExited {
        address: Address,
        reason: ExitReason,
    },
}

/// Why the server declined a service interest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DeclineReason {
    VersionMismatch,
    ServiceUnknown,
    /// The requesting pane provides this service itself.
    SelfProvide,
    /// No session_ids available (65534 limit per connection).
    SessionExhausted,
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
        assert!(matches!(
            msg,
            ControlMessage::Lifecycle(LifecycleMessage::Ready)
        ));
    }

    #[test]
    fn construct_declare_interest() {
        let msg = ControlMessage::DeclareInterest {
            service: ServiceId::new("com.pane.clipboard"),
            expected_version: 1,
        };
        assert!(matches!(
            msg,
            ControlMessage::DeclareInterest {
                expected_version: 1,
                ..
            }
        ));
    }

    #[test]
    fn construct_interest_accepted() {
        let msg = ControlMessage::InterestAccepted {
            service_uuid: uuid::Uuid::nil(),
            session_id: 3,
            version: 1,
        };
        assert!(matches!(
            msg,
            ControlMessage::InterestAccepted { session_id: 3, .. }
        ));
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
        assert!(matches!(
            msg,
            ControlMessage::ServiceTeardown { session_id: 1, .. }
        ));
    }

    #[test]
    fn construct_revoke_interest() {
        let msg = ControlMessage::RevokeInterest { session_id: 5 };
        assert!(matches!(
            msg,
            ControlMessage::RevokeInterest { session_id: 5 }
        ));
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
        assert!(matches!(
            decoded,
            ControlMessage::Lifecycle(LifecycleMessage::Pulse)
        ));
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
            ControlMessage::DeclareInterest {
                service,
                expected_version,
            } => {
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
            ControlMessage::InterestAccepted {
                service_uuid,
                session_id,
                version,
            } => {
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

    #[test]
    fn construct_watch() {
        let msg = ControlMessage::Watch {
            target: crate::Address::local(42),
        };
        assert!(matches!(msg, ControlMessage::Watch { .. }));
    }

    #[test]
    fn construct_unwatch() {
        let msg = ControlMessage::Unwatch {
            target: crate::Address::local(42),
        };
        assert!(matches!(msg, ControlMessage::Unwatch { .. }));
    }

    #[test]
    fn construct_pane_exited() {
        let msg = ControlMessage::PaneExited {
            address: crate::Address::local(42),
            reason: crate::ExitReason::Graceful,
        };
        assert!(matches!(msg, ControlMessage::PaneExited { .. }));
    }

    #[test]
    fn roundtrip_watch() {
        let msg = ControlMessage::Watch {
            target: crate::Address::remote(7, 3),
        };
        let bytes = postcard::to_allocvec(&msg).expect("serialize");
        let decoded: ControlMessage = postcard::from_bytes(&bytes).expect("deserialize");
        match decoded {
            ControlMessage::Watch { target } => {
                assert_eq!(target.pane_id, 7);
                assert_eq!(target.server_id, 3);
            }
            other => panic!("expected Watch, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_pane_exited() {
        let msg = ControlMessage::PaneExited {
            address: crate::Address::local(99),
            reason: crate::ExitReason::Disconnected,
        };
        let bytes = postcard::to_allocvec(&msg).expect("serialize");
        let decoded: ControlMessage = postcard::from_bytes(&bytes).expect("deserialize");
        match decoded {
            ControlMessage::PaneExited { address, reason } => {
                assert_eq!(address.pane_id, 99);
                assert!(address.is_local());
                assert_eq!(reason, crate::ExitReason::Disconnected);
            }
            other => panic!("expected PaneExited, got {other:?}"),
        }
    }
}
