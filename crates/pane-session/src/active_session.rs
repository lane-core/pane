//! Post-handshake connection state container.
//!
//! `ActiveSession` consolidates session-layer state that was
//! previously scattered across `LooperCore<H>` fields in pane-app.
//! Everything in ActiveSession is independent of handler type `H`
//! — it belongs in pane-session because it is protocol-layer state,
//! not application-layer state.
//!
//! The type exists as a named product (Iso: scattered fields ↔
//! named product). It was defined before failure cascade methods
//! (C2) because the cascade is a *consumer* of session state, not
//! a contributor to its shape. The cascade policy methods
//! (`cascade_session_failure`, `cascade_connection_failure`)
//! query the correlator's token→session mapping and return
//! `TeardownSet` — the obligation type for N3.
//!
//! Design heritage: Plan 9 `Mnt` struct in devmnt.c
//! (reference/plan9/src/sys/src/9/port/devmnt.c) — per-mount,
//! post-handshake state container holding msize, version string,
//! pending rpc queue, and transport channel. Haiku's `ServerApp`
//! field set (src/servers/app/ServerApp.h) — container-first,
//! teardown-against-container.
//!
//! EAct precedent: handler store σ ([FH] §3.2), defined before
//! failure handling (§4).
//!
//! # References
//!
//! - `[FH]` §3.2 — handler store σ. ActiveSession is the Rust
//!   realization of σ restricted to protocol-layer (H-independent)
//!   fields.
//! - `[FH]` §4 — failure handling defined *after* σ. C2 follows
//!   this ordering.

use std::collections::HashSet;

use crate::correlator::RequestCorrelator;
use crate::teardown::TeardownSet;
use crate::PeerScope;
use crate::Token;

/// Post-handshake connection state [EAct σ].
///
/// Consolidates session-layer state that was previously scattered
/// across `LooperCore<H>` fields. Everything in ActiveSession is
/// independent of handler type H — it belongs in pane-session
/// because it is protocol-layer state, not application-layer state.
///
/// Plan 9 precedent: devmnt.c `Mnt` struct (per-mount, post-handshake
/// state: msize, version, pending rpc queue, transport channel).
/// EAct precedent: handler store σ (§3.2), defined before failure
/// handling (§4).
pub struct ActiveSession {
    /// Token allocation and outstanding request tracking.
    correlator: RequestCorrelator,
    /// Sessions that have been locally revoked (ServiceHandle dropped)
    /// but not yet acknowledged by the peer. Checked in dispatch to
    /// suppress stale frames (H3 invariant).
    ///
    /// Not cleared between batches: a session revoked in batch N
    /// must suppress stale frames that arrive in batch N+1.
    revoked_sessions: HashSet<u16>,
    /// The connection's peer identity.
    primary_connection: PeerScope,
    /// Negotiated maximum message size (from Welcome).
    max_message_size: u32,
    /// Negotiated maximum outstanding requests (from Welcome).
    max_outstanding_requests: u16,
}

impl ActiveSession {
    /// Construct from handshake-negotiated parameters.
    ///
    /// The correlator starts empty (no outstanding requests). The
    /// request cap is set from `max_outstanding_requests` — 0 means
    /// unlimited (no enforcement).
    pub fn new(
        primary_connection: PeerScope,
        max_message_size: u32,
        max_outstanding_requests: u16,
    ) -> Self {
        let mut correlator = RequestCorrelator::new();
        correlator.set_cap(max_outstanding_requests);
        ActiveSession {
            correlator,
            revoked_sessions: HashSet::new(),
            primary_connection,
            max_message_size,
            max_outstanding_requests,
        }
    }

    // --- Correlator delegation ---

    /// Allocate the next monotonic token and increment the
    /// outstanding request count. Delegates to `RequestCorrelator`.
    pub fn allocate_token(&mut self, session_id: u16) -> Token {
        self.correlator.allocate_token(session_id)
    }

    /// Record that one outstanding request has been resolved
    /// (reply, failed, cancel). Delegates to `RequestCorrelator`.
    pub fn record_resolution(&mut self, token: Token) {
        self.correlator.record_resolution(token);
    }

    /// Whether sending another request would exceed the negotiated
    /// cap. Returns false when cap is 0 (unlimited).
    pub fn would_exceed_cap(&self) -> bool {
        self.correlator.would_exceed_cap()
    }

    /// Set the request cap. 0 = unlimited.
    /// Normally set once during construction from Welcome, but
    /// exposed for renegotiation scenarios.
    pub fn set_cap(&mut self, cap: u16) {
        self.correlator.set_cap(cap);
    }

    /// Current count of outstanding (unresolved) requests.
    pub fn outstanding_requests(&self) -> u64 {
        self.correlator.outstanding_requests()
    }

    /// The negotiated request cap. 0 = unlimited.
    pub fn request_cap(&self) -> u16 {
        self.correlator.request_cap()
    }

    /// Reset the correlator's outstanding count to zero. Used
    /// during destruction sequence after entries have been dropped.
    pub fn clear_correlator(&mut self) {
        self.correlator.clear();
    }

    // --- Session lifecycle ---

    /// Mark a session as locally revoked. The session's
    /// ServiceHandle has been dropped; the wire RevokeInterest
    /// frame is sent separately in batch phase 4. Frames arriving
    /// for revoked sessions are silently dropped (H3).
    pub fn revoke_session(&mut self, session_id: u16) {
        self.revoked_sessions.insert(session_id);
    }

    /// Whether a session has been locally revoked (H3 check).
    /// Used in dispatch to suppress stale frames for sessions
    /// whose ServiceHandle has been dropped.
    pub fn is_revoked(&self, session_id: u16) -> bool {
        self.revoked_sessions.contains(&session_id)
    }

    // --- Accessors ---

    /// The connection's peer identity.
    pub fn primary_connection(&self) -> PeerScope {
        self.primary_connection
    }

    /// Negotiated maximum message size (from Welcome).
    pub fn max_message_size(&self) -> u32 {
        self.max_message_size
    }

    /// Negotiated maximum outstanding requests (from Welcome).
    /// 0 = unlimited. This is the raw negotiated value; the
    /// correlator's `request_cap()` reflects the same value but
    /// lives on the correlator for enforcement.
    pub fn max_outstanding_requests(&self) -> u16 {
        self.max_outstanding_requests
    }

    /// The set of locally revoked sessions. Exposed for batch
    /// dispatch (phase 5 H3 suppression across batch boundaries).
    pub fn revoked_sessions(&self) -> &HashSet<u16> {
        &self.revoked_sessions
    }

    // --- Failure cascade policy [N3] ---

    /// N3: Enumerate all tokens that should be failed due to a
    /// session-level failure (e.g., ServiceTeardown received).
    ///
    /// Returns a `TeardownSet` containing all outstanding tokens
    /// for the given session, paired with `primary_connection`.
    /// The caller must fire `on_failed` for every token in the set.
    ///
    /// After this call, the affected tokens are removed from the
    /// correlator — they are about to be failed by the caller,
    /// so they count as resolved from the correlator's perspective.
    ///
    /// Reference: [FH] §4 (E-RaiseS — raise failure to session).
    pub fn cascade_session_failure(&mut self, session_id: u16) -> TeardownSet {
        let peer = self.primary_connection;
        let tokens = self.correlator.tokens_for_session(session_id);
        let entries: Vec<_> = tokens.iter().map(|&tok| (peer, tok)).collect();
        // Remove from correlator — caller resolves the TeardownSet.
        for &tok in &tokens {
            self.correlator.record_resolution(tok);
        }
        TeardownSet::new(entries)
    }

    /// N3: Enumerate all tokens that should be failed due to a
    /// connection-level failure (transport death).
    ///
    /// Returns a `TeardownSet` containing ALL outstanding tokens
    /// across all sessions on this connection, paired with
    /// `primary_connection`. The caller must fire `on_failed` for
    /// every token in the set.
    ///
    /// After this call, the correlator is cleared — all tokens
    /// are about to be failed, so the outstanding count resets
    /// to zero.
    ///
    /// Reference: [FH] §4 (E-CancelMsg — cancel outstanding
    /// messages on connection failure).
    pub fn cascade_connection_failure(&mut self) -> TeardownSet {
        let peer = self.primary_connection;
        let all = self.correlator.all_tokens();
        let entries: Vec<_> = all.iter().map(|&(tok, _session_id)| (peer, tok)).collect();
        // Bulk clear — more efficient than per-token resolution.
        self.correlator.clear();
        TeardownSet::new(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_with_negotiated_params() {
        let session = ActiveSession::new(PeerScope(1), 16 * 1024 * 1024, 128);
        assert_eq!(session.primary_connection(), PeerScope(1));
        assert_eq!(session.max_message_size(), 16 * 1024 * 1024);
        assert_eq!(session.request_cap(), 128);
        assert_eq!(session.outstanding_requests(), 0);
        assert!(session.revoked_sessions().is_empty());
    }

    #[test]
    fn construction_unlimited_cap() {
        let session = ActiveSession::new(PeerScope(42), 8192, 0);
        assert_eq!(session.request_cap(), 0);
        assert!(!session.would_exceed_cap());
    }

    #[test]
    fn correlator_delegation_allocate_and_resolve() {
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        assert_eq!(session.outstanding_requests(), 0);

        let t0 = session.allocate_token(1);
        assert_eq!(t0, Token(0));
        assert_eq!(session.outstanding_requests(), 1);

        let t1 = session.allocate_token(1);
        assert_eq!(t1, Token(1));
        assert_eq!(session.outstanding_requests(), 2);

        session.record_resolution(t1);
        assert_eq!(session.outstanding_requests(), 1);

        session.record_resolution(t0);
        assert_eq!(session.outstanding_requests(), 0);
    }

    #[test]
    fn correlator_delegation_cap_enforcement() {
        let mut session = ActiveSession::new(PeerScope(1), 8192, 2);
        assert!(!session.would_exceed_cap());

        session.allocate_token(1);
        assert!(!session.would_exceed_cap());

        let t1 = session.allocate_token(1);
        // 2 of 2 — at cap
        assert!(session.would_exceed_cap());

        session.record_resolution(t1);
        assert!(!session.would_exceed_cap());
    }

    #[test]
    fn session_revocation() {
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);

        assert!(!session.is_revoked(5));
        session.revoke_session(5);
        assert!(session.is_revoked(5));

        // Idempotent — revoking twice doesn't panic
        session.revoke_session(5);
        assert!(session.is_revoked(5));

        // Other sessions unaffected
        assert!(!session.is_revoked(6));
    }

    #[test]
    fn revoked_sessions_accessor() {
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        session.revoke_session(10);
        session.revoke_session(20);

        let revoked = session.revoked_sessions();
        assert_eq!(revoked.len(), 2);
        assert!(revoked.contains(&10));
        assert!(revoked.contains(&20));
    }

    #[test]
    fn clear_correlator_resets_outstanding() {
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        session.allocate_token(1);
        session.allocate_token(2);
        session.allocate_token(1);
        assert_eq!(session.outstanding_requests(), 3);

        session.clear_correlator();
        assert_eq!(session.outstanding_requests(), 0);
    }

    #[test]
    fn set_cap_after_construction() {
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        assert_eq!(session.request_cap(), 0);
        assert!(!session.would_exceed_cap());

        session.set_cap(1);
        assert_eq!(session.request_cap(), 1);

        session.allocate_token(1);
        assert!(session.would_exceed_cap());
    }

    // --- Failure cascade policy [N3] ---

    #[test]
    fn cascade_session_failure_returns_session_tokens() {
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        let t0 = session.allocate_token(5);
        let _t1 = session.allocate_token(6);
        let t2 = session.allocate_token(5);
        assert_eq!(session.outstanding_requests(), 3);

        let mut set = session.cascade_session_failure(5);
        assert_eq!(set.len(), 2);

        let mut drained: Vec<_> = set.drain().collect();
        drained.sort_by_key(|&(_, tok)| tok.0);
        assert_eq!(drained, vec![(PeerScope(1), t0), (PeerScope(1), t2)]);

        // Cascaded tokens removed from correlator
        assert_eq!(session.outstanding_requests(), 1);
    }

    #[test]
    fn cascade_connection_failure_returns_all_tokens() {
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        let t0 = session.allocate_token(1);
        let t1 = session.allocate_token(2);
        let t2 = session.allocate_token(1);
        assert_eq!(session.outstanding_requests(), 3);

        let mut set = session.cascade_connection_failure();
        assert_eq!(set.len(), 3);

        let mut drained: Vec<_> = set.drain().collect();
        drained.sort_by_key(|&(_, tok)| tok.0);
        assert_eq!(
            drained,
            vec![(PeerScope(1), t0), (PeerScope(1), t1), (PeerScope(1), t2),]
        );

        // Correlator fully cleared
        assert_eq!(session.outstanding_requests(), 0);
    }

    #[test]
    fn cascade_returns_allocated_tokens() {
        // After allocating tokens across sessions, cascade returns
        // the correct subset (session) or full set (connection).
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        let t0 = session.allocate_token(1);
        let t1 = session.allocate_token(2);
        assert_eq!(session.outstanding_requests(), 2);

        // Session cascade returns only session 1's token
        let mut set = session.cascade_session_failure(1);
        assert_eq!(set.len(), 1);
        let drained: Vec<_> = set.drain().collect();
        assert_eq!(drained, vec![(PeerScope(1), t0)]);
        assert_eq!(session.outstanding_requests(), 1);

        // Connection cascade returns remaining token
        let mut set = session.cascade_connection_failure();
        assert_eq!(set.len(), 1);
        let drained: Vec<_> = set.drain().collect();
        assert_eq!(drained, vec![(PeerScope(1), t1)]);
        assert_eq!(session.outstanding_requests(), 0);
    }

    #[test]
    fn cascade_session_failure_isolates_sessions() {
        // Cascading one session does not affect other sessions' tokens.
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        session.allocate_token(10);
        session.allocate_token(20);
        session.allocate_token(10);
        session.allocate_token(30);
        assert_eq!(session.outstanding_requests(), 4);

        let mut set = session.cascade_session_failure(10);
        assert_eq!(set.len(), 2);
        let _: Vec<_> = set.drain().collect();

        // Sessions 20 and 30 still have their tokens
        assert_eq!(session.outstanding_requests(), 2);

        let mut set = session.cascade_session_failure(20);
        assert_eq!(set.len(), 1);
        let _: Vec<_> = set.drain().collect();
        assert_eq!(session.outstanding_requests(), 1);

        let mut set = session.cascade_session_failure(30);
        assert_eq!(set.len(), 1);
        let _: Vec<_> = set.drain().collect();
        assert_eq!(session.outstanding_requests(), 0);
    }

    #[test]
    fn cascade_session_failure_decrements_outstanding() {
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        session.allocate_token(1);
        session.allocate_token(1);
        session.allocate_token(1);
        session.allocate_token(2);
        assert_eq!(session.outstanding_requests(), 4);

        let mut set = session.cascade_session_failure(1);
        assert_eq!(set.len(), 3);
        let _: Vec<_> = set.drain().collect();

        // Exactly 3 decremented — 1 remains for session 2
        assert_eq!(session.outstanding_requests(), 1);
    }

    #[test]
    fn cascade_connection_failure_clears_all() {
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        session.allocate_token(1);
        session.allocate_token(2);
        session.allocate_token(3);
        session.allocate_token(1);
        assert_eq!(session.outstanding_requests(), 4);

        let mut set = session.cascade_connection_failure();
        assert_eq!(set.len(), 4);
        let _: Vec<_> = set.drain().collect();

        assert_eq!(session.outstanding_requests(), 0);

        // No tokens left for any session
        let mut set = session.cascade_session_failure(1);
        assert!(set.is_empty());
        let _: Vec<_> = set.drain().collect();
    }

    #[test]
    fn double_cascade_same_session_second_empty() {
        // First cascade resolves all tokens for the session.
        // Second cascade on same session returns empty — tokens
        // were already removed.
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        session.allocate_token(5);
        session.allocate_token(5);

        let mut first = session.cascade_session_failure(5);
        assert_eq!(first.len(), 2);
        let _: Vec<_> = first.drain().collect();

        let mut second = session.cascade_session_failure(5);
        assert!(second.is_empty());
        let _: Vec<_> = second.drain().collect();
    }

    #[test]
    fn cascade_empty_session_returns_empty() {
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        session.allocate_token(1);

        // Session 99 has no tokens — cascade returns empty set
        let set = session.cascade_session_failure(99);
        assert!(set.is_empty());
        assert_eq!(session.outstanding_requests(), 1);
    }

    #[test]
    fn cascade_connection_failure_no_tokens_returns_empty() {
        let mut session = ActiveSession::new(PeerScope(1), 8192, 0);
        let set = session.cascade_connection_failure();
        assert!(set.is_empty());
        assert_eq!(session.outstanding_requests(), 0);
    }
}
