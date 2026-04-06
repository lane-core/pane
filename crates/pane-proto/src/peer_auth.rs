//! Peer authentication metadata.
//!
//! Identifies who is on the other end of a connection and how that
//! identity was established. Every accepted connection — local or
//! remote — resolves to a unix uid; the [`AuthSource`] records
//! provenance (kernel credential or certificate).
//!
//! PeerAuth is transport-derived metadata below the session type
//! layer. It does not participate in the par handshake, optics
//! chain, or EAct model.
//!
//! Design heritage: Plan 9 factotum(4)
//! (reference/plan9/man/4/factotum) separated authentication from
//! application protocols — the credential proves a claim, the
//! system operates on the resolved principal (AuthInfo user name).
//! BeOS team_id was kernel-assigned process identity used for
//! ownership checks (cursor->OwningTeam() == fClientTeam,
//! src/servers/app/ServerApp.cpp:1294). PeerAuth combines both:
//! uid (factotum's resolved user) + pid (Be's team_id,
//! src/servers/app/ServerApp.h:71) + provenance (how identity was
//! established, which neither system tracked).
//!
//! # Certificate subject comparison
//!
//! [`AuthSource::Certificate`] compares subject and issuer strings
//! byte-exact. No normalization (case folding, whitespace
//! canonicalization) is performed. Normalization is deferred to
//! Phase 2 when TLS transport is implemented; consumers must not
//! rely on case-insensitive matching.
//!
//! # Design
//!
//! The uid is identity; the certificate is evidence. This is the
//! factotum principle: the credential proves a claim, but the
//! system operates on the resolved principal (uid). Same uid via
//! different transports are distinguishable — Eq and Hash cover
//! the full struct — so connection tracking can differentiate a
//! local kernel-authenticated session from a certificate-backed
//! remote session for the same user.

use std::fmt;
use serde::{Serialize, Deserialize};

/// Identity and provenance of an authenticated peer.
///
/// Constructed by the transport layer after connection acceptance.
/// Fields are public — consumers pattern-match directly.
///
/// # Examples
///
/// ```
/// use pane_proto::peer_auth::{PeerAuth, AuthSource};
///
/// let local = PeerAuth::new(1000, AuthSource::Kernel { pid: 4567 });
///
/// let remote = PeerAuth::new(1000, AuthSource::Certificate {
///     subject: "ada".into(),
///     issuer: "pane-ca".into(),
/// });
///
/// // Same uid, different source — not equal.
/// assert_ne!(local, remote);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PeerAuth {
    pub uid: u32,
    pub source: AuthSource,
}

impl PeerAuth {
    /// Create a new PeerAuth from a resolved uid and its provenance.
    pub fn new(uid: u32, source: AuthSource) -> Self {
        PeerAuth { uid, source }
    }
}

/// How the peer's identity was established.
///
/// Provenance metadata — not the identity itself (that is the uid).
/// `#[non_exhaustive]`: future variants (e.g., token-based auth)
/// will not break downstream match arms.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AuthSource {
    /// Identity established via kernel credentials (SO_PEERCRED or equivalent).
    Kernel {
        /// Process ID of the connecting peer.
        pid: u32,
    },
    /// Identity established via TLS client certificate.
    ///
    /// Subject and issuer are compared byte-exact — no normalization.
    Certificate {
        /// Certificate subject (e.g., common name).
        subject: String,
        /// Certificate issuer.
        issuer: String,
    },
}

impl fmt::Display for PeerAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            AuthSource::Kernel { pid } => {
                write!(f, "uid={} kernel pid={}", self.uid, pid)
            }
            AuthSource::Certificate { subject, issuer } => {
                write!(
                    f,
                    "uid={} cert subject={} issuer={}",
                    self.uid, subject, issuer,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn kernel_auth() -> PeerAuth {
        PeerAuth {
            uid: 1000,
            source: AuthSource::Kernel { pid: 4567 },
        }
    }

    fn cert_auth() -> PeerAuth {
        PeerAuth {
            uid: 1000,
            source: AuthSource::Certificate {
                subject: "ada".into(),
                issuer: "pane-ca".into(),
            },
        }
    }

    // -- Construction --

    #[test]
    fn construct_kernel_variant() {
        let auth = kernel_auth();
        assert_eq!(auth.uid, 1000);
        assert!(matches!(auth.source, AuthSource::Kernel { pid: 4567 }));
    }

    #[test]
    fn construct_certificate_variant() {
        let auth = cert_auth();
        assert_eq!(auth.uid, 1000);
        assert!(matches!(
            auth.source,
            AuthSource::Certificate { ref subject, ref issuer }
            if subject == "ada" && issuer == "pane-ca"
        ));
    }

    // -- Serialization roundtrip --

    #[test]
    fn serialize_roundtrip_kernel() {
        let original = kernel_auth();
        let bytes = postcard::to_allocvec(&original).unwrap();
        let decoded: PeerAuth = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn serialize_roundtrip_certificate() {
        let original = cert_auth();
        let bytes = postcard::to_allocvec(&original).unwrap();
        let decoded: PeerAuth = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    // -- Display --

    #[test]
    fn display_kernel() {
        assert_eq!(kernel_auth().to_string(), "uid=1000 kernel pid=4567");
    }

    #[test]
    fn display_certificate() {
        assert_eq!(
            cert_auth().to_string(),
            "uid=1000 cert subject=ada issuer=pane-ca",
        );
    }

    // -- Eq: same uid, different source are NOT equal --

    #[test]
    fn eq_same_uid_different_source() {
        assert_ne!(kernel_auth(), cert_auth());
    }

    #[test]
    fn eq_identical_values() {
        assert_eq!(kernel_auth(), kernel_auth());
        assert_eq!(cert_auth(), cert_auth());
    }

    // -- Hash: same values hash the same --

    #[test]
    fn hash_consistent() {
        let mut set = HashSet::new();
        set.insert(kernel_auth());
        // Inserting an equal value should not increase size.
        set.insert(kernel_auth());
        assert_eq!(set.len(), 1);

        // Different source for same uid is a distinct entry.
        set.insert(cert_auth());
        assert_eq!(set.len(), 2);
    }

    // -- Clone produces equal value --

    #[test]
    fn clone_equality() {
        let original = cert_auth();
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // Note: #[non_exhaustive] is a compile-time attribute verified by
    // the compiler. Its presence on PeerAuth and AuthSource ensures
    // downstream crates cannot exhaustively construct or match these
    // types without a wildcard arm. This is not runtime-testable but
    // is enforced by the attribute above both type definitions.
}
