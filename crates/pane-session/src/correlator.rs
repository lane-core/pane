//! Correlation identifiers for request/reply dispatch.
//!
//! `Token` and `PeerScope` are the key types for the dispatch
//! table: entries are keyed by `(PeerScope, Token)`. They are
//! value types with no behavior — identity is structural equality.
//!
//! `RequestCorrelator` owns the token→session mapping that enables
//! failure cascade: given a session_id, enumerate all outstanding
//! tokens for that session. This mapping was previously implicit
//! in pane-app's `DispatchEntry<H>.session_id` field — extracting
//! it here completes the N1+N2 boundary (protocol-layer state in
//! pane-session, handler-typed state in pane-app).
//!
//! Design heritage: Plan 9's devmnt tracked outstanding requests
//! by tag (u16, reference/plan9/man/5/0intro:91-100). Haiku's
//! port system used int32 message codes for routing
//! (src/system/kernel/port.cpp). pane uses a wider (u64) space
//! to avoid tag recycling concerns on long-lived connections.

use std::collections::HashMap;

/// Scopes dispatch entries by peer. Distinct from the server's
/// ConnectionId — this is looper-internal, the server never sees it.
/// Named PeerScope (not ConnectionId) to avoid confusion with
/// pane-session's server-side ConnectionId.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PeerScope(pub u64);

/// Request token. Unique per Dispatch instance (monotonic counter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token(pub u64);

/// Protocol-layer request correlation and flow control.
///
/// Manages token allocation (monotonic counter) and outstanding
/// request tracking (credit-based cap). The counter is a Fold over
/// key liveness — every allocated token that hasn't been resolved
/// counts against the cap.
///
/// pane-app's `Dispatch<H>` wraps this and adds handler-typed
/// closure storage (`DispatchEntry<H>`). `RequestCorrelator` is
/// generic-parameter-free — it belongs in pane-session because it
/// has no dependency on handler type `H`.
///
/// Realizes [GV10] bounded-buffer encoding (Gay & Vasconcelos 2010)
/// and [FH] E-Send cap precondition (Fowler & Hu §3.2).
pub struct RequestCorrelator {
    next_token: u64,
    outstanding_requests: u64,
    request_cap: u16,
    /// Token→session_id mapping. Every allocated token that hasn't
    /// been resolved appears here. Enables cascade queries:
    /// "which tokens belong to session X?" without scanning the
    /// handler-typed entry map in pane-app.
    token_sessions: HashMap<Token, u16>,
}

impl Default for RequestCorrelator {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestCorrelator {
    /// Starts with token 0, no outstanding requests, cap 0 (unlimited).
    pub fn new() -> Self {
        RequestCorrelator {
            next_token: 0,
            outstanding_requests: 0,
            request_cap: 0,
            token_sessions: HashMap::new(),
        }
    }

    /// Allocate the next monotonic token, record which session it
    /// belongs to, and increment the outstanding request count.
    pub fn allocate_token(&mut self, session_id: u16) -> Token {
        let token = Token(self.next_token);
        self.next_token += 1;
        self.outstanding_requests += 1;
        self.token_sessions.insert(token, session_id);
        token
    }

    /// Record that one outstanding request has been resolved
    /// (reply, failed, cancel). Removes the token from the
    /// session mapping and decrements the outstanding count.
    /// Saturating to prevent underflow if called redundantly.
    pub fn record_resolution(&mut self, token: Token) {
        self.token_sessions.remove(&token);
        self.outstanding_requests = self.outstanding_requests.saturating_sub(1);
    }

    /// Whether sending another request would exceed the negotiated
    /// cap. Returns false when cap is 0 (unlimited).
    pub fn would_exceed_cap(&self) -> bool {
        self.request_cap > 0 && self.outstanding_requests >= self.request_cap as u64
    }

    /// Set the request cap from Welcome negotiation (D9).
    /// 0 = unlimited (no enforcement).
    pub fn set_cap(&mut self, cap: u16) {
        self.request_cap = cap;
    }

    /// Current count of outstanding (unresolved) requests.
    pub fn outstanding_requests(&self) -> u64 {
        self.outstanding_requests
    }

    /// The negotiated cap. 0 = unlimited.
    pub fn request_cap(&self) -> u16 {
        self.request_cap
    }

    /// All outstanding tokens for a given session. Used by
    /// cascade_session_failure to build TeardownSets.
    pub fn tokens_for_session(&self, session_id: u16) -> Vec<Token> {
        self.token_sessions
            .iter()
            .filter(|(_, &sid)| sid == session_id)
            .map(|(&tok, _)| tok)
            .collect()
    }

    /// All outstanding (token, session_id) pairs. Used by
    /// cascade_connection_failure to build a full TeardownSet.
    pub fn all_tokens(&self) -> Vec<(Token, u16)> {
        self.token_sessions
            .iter()
            .map(|(&tok, &sid)| (tok, sid))
            .collect()
    }

    /// Reset outstanding count and token mapping to zero. Used
    /// during destruction sequence after entries have been dropped.
    pub fn clear(&mut self) {
        self.outstanding_requests = 0;
        self.token_sessions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_monotonicity() {
        let mut cor = RequestCorrelator::new();
        let t0 = cor.allocate_token(1);
        let t1 = cor.allocate_token(1);
        let t2 = cor.allocate_token(2);
        assert_eq!(t0, Token(0));
        assert_eq!(t1, Token(1));
        assert_eq!(t2, Token(2));
        assert_ne!(t0, t1);
    }

    #[test]
    fn counter_increments_on_allocate() {
        let mut cor = RequestCorrelator::new();
        assert_eq!(cor.outstanding_requests(), 0);
        cor.allocate_token(1);
        assert_eq!(cor.outstanding_requests(), 1);
        cor.allocate_token(1);
        assert_eq!(cor.outstanding_requests(), 2);
    }

    #[test]
    fn counter_decrements_on_resolution() {
        let mut cor = RequestCorrelator::new();
        let t0 = cor.allocate_token(1);
        let t1 = cor.allocate_token(1);
        assert_eq!(cor.outstanding_requests(), 2);
        cor.record_resolution(t0);
        assert_eq!(cor.outstanding_requests(), 1);
        cor.record_resolution(t1);
        assert_eq!(cor.outstanding_requests(), 0);
    }

    #[test]
    fn counter_saturates_at_zero() {
        let mut cor = RequestCorrelator::new();
        // Resolving a token that was never allocated — saturates
        cor.record_resolution(Token(99));
        assert_eq!(cor.outstanding_requests(), 0);
    }

    #[test]
    fn counter_resets_on_clear() {
        let mut cor = RequestCorrelator::new();
        cor.allocate_token(1);
        cor.allocate_token(2);
        assert_eq!(cor.outstanding_requests(), 2);
        cor.clear();
        assert_eq!(cor.outstanding_requests(), 0);
    }

    #[test]
    fn unlimited_cap_never_exceeds() {
        let mut cor = RequestCorrelator::new();
        assert_eq!(cor.request_cap(), 0);
        assert!(!cor.would_exceed_cap());
        cor.allocate_token(1);
        assert!(!cor.would_exceed_cap());
    }

    #[test]
    fn cap_enforced() {
        let mut cor = RequestCorrelator::new();
        cor.set_cap(2);
        // 0 of 2
        assert!(!cor.would_exceed_cap());
        cor.allocate_token(1);
        // 1 of 2
        assert!(!cor.would_exceed_cap());
        cor.allocate_token(1);
        // 2 of 2 — at cap
        assert!(cor.would_exceed_cap());
    }

    #[test]
    fn cap_clears_after_resolution() {
        let mut cor = RequestCorrelator::new();
        cor.set_cap(1);
        let t0 = cor.allocate_token(1);
        assert!(cor.would_exceed_cap());
        cor.record_resolution(t0);
        assert!(!cor.would_exceed_cap());
    }

    // --- Token→session mapping ---

    #[test]
    fn tokens_for_session_returns_correct_subset() {
        let mut cor = RequestCorrelator::new();
        let t0 = cor.allocate_token(1);
        let _t1 = cor.allocate_token(2);
        let t2 = cor.allocate_token(1);
        let _t3 = cor.allocate_token(3);

        let mut session_1 = cor.tokens_for_session(1);
        session_1.sort_by_key(|t| t.0);
        assert_eq!(session_1, vec![t0, t2]);

        let session_2 = cor.tokens_for_session(2);
        assert_eq!(session_2.len(), 1);
        assert_eq!(session_2[0], Token(1));
    }

    #[test]
    fn tokens_for_session_returns_empty_for_unknown() {
        let mut cor = RequestCorrelator::new();
        cor.allocate_token(1);
        assert!(cor.tokens_for_session(99).is_empty());
    }

    #[test]
    fn all_tokens_returns_all_outstanding() {
        let mut cor = RequestCorrelator::new();
        cor.allocate_token(1);
        cor.allocate_token(2);
        cor.allocate_token(1);

        let mut all = cor.all_tokens();
        all.sort_by_key(|(t, _)| t.0);
        assert_eq!(all, vec![(Token(0), 1), (Token(1), 2), (Token(2), 1)]);
    }

    #[test]
    fn record_resolution_removes_from_token_sessions() {
        let mut cor = RequestCorrelator::new();
        let t0 = cor.allocate_token(1);
        let t1 = cor.allocate_token(1);
        assert_eq!(cor.tokens_for_session(1).len(), 2);

        cor.record_resolution(t0);
        let remaining = cor.tokens_for_session(1);
        assert_eq!(remaining, vec![t1]);
    }

    #[test]
    fn clear_removes_from_token_sessions() {
        let mut cor = RequestCorrelator::new();
        cor.allocate_token(1);
        cor.allocate_token(2);
        assert_eq!(cor.all_tokens().len(), 2);

        cor.clear();
        assert!(cor.all_tokens().is_empty());
        assert!(cor.tokens_for_session(1).is_empty());
    }
}
