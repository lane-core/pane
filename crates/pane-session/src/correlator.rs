//! Correlation identifiers for request/reply dispatch.
//!
//! `Token` and `PeerScope` are the key types for the dispatch
//! table: entries are keyed by `(PeerScope, Token)`. They are
//! value types with no behavior — identity is structural equality.
//!
//! Design heritage: Plan 9's devmnt tracked outstanding requests
//! by tag (u16, reference/plan9/man/5/0intro:91-100). Haiku's
//! port system used int32 message codes for routing
//! (src/system/kernel/port.cpp). pane uses a wider (u64) space
//! to avoid tag recycling concerns on long-lived connections.

/// Scopes dispatch entries by peer. Distinct from the server's
/// ConnectionId — this is looper-internal, the server never sees it.
/// Named PeerScope (not ConnectionId) to avoid confusion with
/// pane-session's server-side ConnectionId.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerScope(pub u64);

/// Request token. Unique per Dispatch instance (monotonic counter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token(pub u64);
