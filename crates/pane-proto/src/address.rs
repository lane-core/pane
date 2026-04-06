//! Pane addressing.
//!
//! A resolved address for a pane — lightweight value type for routing.
//! Like a Plan 9 fid, resolution happens once and the result is stable.
//! How it was resolved (service map, namespace query, received in a
//! message) is not part of the address.
//!
//! Design heritage: Plan 9 fids were client-assigned handles bound
//! at walk/open time. BeOS BMessenger stored (port_id, handler_token,
//! team_id) — a resolved triple you could copy, serialize, and send.
//! Address is the copyable/serializable part; Messenger (which holds
//! a live LooperSender) is the capability part. The split is forced
//! by Rust's ownership — BMessenger could be both because kernel
//! ports were globally addressable.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A resolved address for a pane. Lightweight value type.
/// Copyable, serializable, sendable in protocol messages.
///
/// An Address is a resolved binding — like a Plan 9 fid,
/// resolution happens once and the result is stable.
/// How it was resolved (service map, namespace query, received
/// in a message) is not part of the address.
///
/// # Examples
///
/// ```
/// use pane_proto::Address;
///
/// let local = Address::local(42);
/// assert!(local.is_local());
/// assert_eq!(format!("{local}"), "pane=42@local");
///
/// let remote = Address::remote(42, 7);
/// assert!(!remote.is_local());
/// assert_eq!(format!("{remote}"), "pane=42@server=7");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Address {
    /// The target pane's identity.
    pub pane_id: u64,
    /// Which server hosts this pane. 0 = local server.
    pub server_id: u64,
}

impl Address {
    /// Address a pane on the local server (server_id = 0).
    pub fn local(pane_id: u64) -> Self {
        Address {
            pane_id,
            server_id: 0,
        }
    }

    /// Address a pane on a remote server.
    pub fn remote(pane_id: u64, server_id: u64) -> Self {
        Address { pane_id, server_id }
    }

    /// Whether this address targets the local server.
    pub fn is_local(&self) -> bool {
        self.server_id == 0
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_local() {
            write!(f, "pane={}@local", self.pane_id)
        } else {
            write!(f, "pane={}@server={}", self.pane_id, self.server_id)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // -- Construction --

    #[test]
    fn construct_local() {
        let addr = Address::local(42);
        assert_eq!(addr.pane_id, 42);
        assert_eq!(addr.server_id, 0);
    }

    #[test]
    fn construct_remote() {
        let addr = Address::remote(42, 7);
        assert_eq!(addr.pane_id, 42);
        assert_eq!(addr.server_id, 7);
    }

    // -- is_local --

    #[test]
    fn is_local_true_for_local() {
        assert!(Address::local(1).is_local());
    }

    #[test]
    fn is_local_false_for_remote() {
        assert!(!Address::remote(1, 5).is_local());
    }

    // -- Copy --

    #[test]
    fn address_is_copy() {
        let a = Address::local(42);
        let b = a; // Copy, not move
        let c = a; // a is still usable
        assert_eq!(b, c);
    }

    // -- Serialization roundtrip --

    #[test]
    fn serialize_roundtrip_local() {
        let original = Address::local(42);
        let bytes = postcard::to_allocvec(&original).unwrap();
        let decoded: Address = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    #[test]
    fn serialize_roundtrip_remote() {
        let original = Address::remote(42, 7);
        let bytes = postcard::to_allocvec(&original).unwrap();
        let decoded: Address = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(original, decoded);
    }

    // -- Display --

    #[test]
    fn display_local() {
        assert_eq!(Address::local(42).to_string(), "pane=42@local");
    }

    #[test]
    fn display_remote() {
        assert_eq!(Address::remote(42, 7).to_string(), "pane=42@server=7");
    }

    // -- Eq --

    #[test]
    fn eq_same_address() {
        assert_eq!(Address::local(1), Address::local(1));
        assert_eq!(Address::remote(1, 2), Address::remote(1, 2));
    }

    #[test]
    fn eq_different_pane_id() {
        assert_ne!(Address::local(1), Address::local(2));
    }

    #[test]
    fn eq_different_server_id() {
        assert_ne!(Address::local(1), Address::remote(1, 5));
    }

    // -- Hash: consistent with Eq --

    #[test]
    fn hash_consistent() {
        let mut set = HashSet::new();
        set.insert(Address::local(42));
        // Inserting an equal value should not increase size.
        set.insert(Address::local(42));
        assert_eq!(set.len(), 1);

        // Different address is a distinct entry.
        set.insert(Address::remote(42, 7));
        assert_eq!(set.len(), 2);
    }
}
