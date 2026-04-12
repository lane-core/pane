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
        }
    }

    /// Allocate the next monotonic token and increment the
    /// outstanding request count.
    pub fn allocate_token(&mut self) -> Token {
        let token = Token(self.next_token);
        self.next_token += 1;
        self.outstanding_requests += 1;
        token
    }

    /// Record that one outstanding request has been resolved
    /// (reply, failed, cancel). Saturating to prevent underflow
    /// if called redundantly.
    pub fn record_resolution(&mut self) {
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

    /// Reset outstanding count to zero. Used during destruction
    /// sequence after entries have been dropped.
    pub fn clear(&mut self) {
        self.outstanding_requests = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_monotonicity() {
        let mut cor = RequestCorrelator::new();
        let t0 = cor.allocate_token();
        let t1 = cor.allocate_token();
        let t2 = cor.allocate_token();
        assert_eq!(t0, Token(0));
        assert_eq!(t1, Token(1));
        assert_eq!(t2, Token(2));
        assert_ne!(t0, t1);
    }

    #[test]
    fn counter_increments_on_allocate() {
        let mut cor = RequestCorrelator::new();
        assert_eq!(cor.outstanding_requests(), 0);
        cor.allocate_token();
        assert_eq!(cor.outstanding_requests(), 1);
        cor.allocate_token();
        assert_eq!(cor.outstanding_requests(), 2);
    }

    #[test]
    fn counter_decrements_on_resolution() {
        let mut cor = RequestCorrelator::new();
        cor.allocate_token();
        cor.allocate_token();
        assert_eq!(cor.outstanding_requests(), 2);
        cor.record_resolution();
        assert_eq!(cor.outstanding_requests(), 1);
        cor.record_resolution();
        assert_eq!(cor.outstanding_requests(), 0);
    }

    #[test]
    fn counter_saturates_at_zero() {
        let mut cor = RequestCorrelator::new();
        cor.record_resolution();
        assert_eq!(cor.outstanding_requests(), 0);
    }

    #[test]
    fn counter_resets_on_clear() {
        let mut cor = RequestCorrelator::new();
        cor.allocate_token();
        cor.allocate_token();
        assert_eq!(cor.outstanding_requests(), 2);
        cor.clear();
        assert_eq!(cor.outstanding_requests(), 0);
    }

    #[test]
    fn unlimited_cap_never_exceeds() {
        let mut cor = RequestCorrelator::new();
        assert_eq!(cor.request_cap(), 0);
        assert!(!cor.would_exceed_cap());
        cor.allocate_token();
        assert!(!cor.would_exceed_cap());
    }

    #[test]
    fn cap_enforced() {
        let mut cor = RequestCorrelator::new();
        cor.set_cap(2);
        // 0 of 2
        assert!(!cor.would_exceed_cap());
        cor.allocate_token();
        // 1 of 2
        assert!(!cor.would_exceed_cap());
        cor.allocate_token();
        // 2 of 2 — at cap
        assert!(cor.would_exceed_cap());
    }

    #[test]
    fn cap_clears_after_resolution() {
        let mut cor = RequestCorrelator::new();
        cor.set_cap(1);
        cor.allocate_token();
        assert!(cor.would_exceed_cap());
        cor.record_resolution();
        assert!(!cor.would_exceed_cap());
    }
}
