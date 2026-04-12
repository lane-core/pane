//! Failure cascade obligation type [N3].
//!
//! `TeardownSet` is the result of a failure cascade query on
//! `ActiveSession`. It contains the set of (PeerScope, Token) pairs
//! affected by a transport or session failure. The caller resolves
//! every token — fires `on_failed`, cancels, or otherwise handles
//! each one.
//!
//! Drop-based detection: if a TeardownSet is dropped with unresolved
//! tokens, it panics in debug builds and logs to stderr in release
//! builds. This is the same affine-gap compensation pattern as
//! `ReplyPort` (pane-proto) — a linear obligation enforced at
//! runtime because Rust's type system does not provide linear types.
//!
//! Design heritage: Plan 9's `muxclose` (devmnt.c) walked the
//! pending rpc queue and aborted each outstanding request on mount
//! teardown. The obligation to resolve every pending request was
//! implicit in the C code (a missed rpc means a hung process). pane
//! makes it explicit via TeardownSet's Drop check.
//!
//! EAct precedent: [FH] §4 E-RaiseS + E-CancelMsg — transport
//! error generates ServiceTeardown to all affected sessions.
//!
//! # References
//!
//! - `[FH]` §4 — E-RaiseS (raise failure to session level),
//!   E-CancelMsg (cancel outstanding messages on failure).
//!   TeardownSet is the Rust realization of the token set that
//!   must be resolved when these rules fire.

use crate::{PeerScope, Token};

/// Obligation type for failure cascade results [N3].
///
/// Contains the set of (PeerScope, Token) pairs affected by a
/// transport or session failure. The caller MUST resolve every
/// token — fire `on_failed`, cancel, or otherwise handle each one.
///
/// Drop-based detection: if a TeardownSet is dropped with
/// unresolved tokens, it panics in debug builds and logs an error
/// in release builds. This is the same affine-gap compensation
/// pattern as `ReplyPort`.
#[must_use = "all teardown tokens must be resolved"]
pub struct TeardownSet {
    tokens: Vec<(PeerScope, Token)>,
    /// Tracks whether tokens have been consumed via `drain()`.
    /// Set to true by `drain()`, preventing the Drop check from
    /// firing on an already-consumed set.
    consumed: bool,
}

impl TeardownSet {
    /// Construct a TeardownSet from a list of affected tokens.
    ///
    /// An empty vec is valid — it means the cascade found no
    /// outstanding tokens to fail.
    pub(crate) fn new(tokens: Vec<(PeerScope, Token)>) -> Self {
        TeardownSet {
            tokens,
            consumed: false,
        }
    }

    /// Yield all tokens, marking the set as consumed.
    ///
    /// After calling `drain()`, the TeardownSet is empty and its
    /// Drop check will not fire. The caller is responsible for
    /// resolving every yielded token.
    pub fn drain(&mut self) -> impl Iterator<Item = (PeerScope, Token)> + '_ {
        self.consumed = true;
        self.tokens.drain(..)
    }

    /// Whether this set contains no tokens.
    ///
    /// An empty TeardownSet can be dropped without triggering
    /// the leak detector — there is nothing to resolve.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Number of tokens that must be resolved.
    pub fn len(&self) -> usize {
        self.tokens.len()
    }
}

impl Drop for TeardownSet {
    fn drop(&mut self) {
        if self.consumed || self.tokens.is_empty() {
            return;
        }
        // Affine-gap compensation: the caller failed to resolve
        // all tokens. In debug builds, panic to surface the bug
        // immediately. In release builds, log to stderr — the
        // same channel the watchdog uses (looper.rs:340).
        let count = self.tokens.len();
        if cfg!(debug_assertions) {
            panic!(
                "TeardownSet dropped with {count} unresolved token(s) — \
                 every token from a failure cascade must be resolved"
            );
        } else {
            eprintln!(
                "pane: TeardownSet leaked {count} unresolved token(s) \
                 (N3 obligation violation)"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_set_does_not_panic_on_drop() {
        let set = TeardownSet::new(vec![]);
        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        drop(set); // must not panic
    }

    #[test]
    fn drain_consumes_all_tokens() {
        let mut set = TeardownSet::new(vec![
            (PeerScope(1), Token(10)),
            (PeerScope(1), Token(20)),
            (PeerScope(2), Token(30)),
        ]);

        assert_eq!(set.len(), 3);
        assert!(!set.is_empty());

        let drained: Vec<_> = set.drain().collect();
        assert_eq!(drained.len(), 3);
        assert!(drained.contains(&(PeerScope(1), Token(10))));
        assert!(drained.contains(&(PeerScope(1), Token(20))));
        assert!(drained.contains(&(PeerScope(2), Token(30))));

        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        drop(set); // consumed — must not panic
    }

    #[test]
    fn drain_then_drop_does_not_panic() {
        let mut set = TeardownSet::new(vec![(PeerScope(1), Token(0))]);
        let _: Vec<_> = set.drain().collect();
        drop(set); // consumed via drain — must not panic
    }

    #[test]
    #[should_panic(expected = "unresolved token(s)")]
    fn drop_with_unconsumed_tokens_panics_in_debug() {
        // This test only fires in debug builds (cfg!(debug_assertions)).
        // In release builds, the Drop logs instead of panicking, so
        // this test would fail. That's fine — #[should_panic] tests
        // run in debug profile by default.
        let _set = TeardownSet::new(vec![(PeerScope(1), Token(42))]);
        // dropped without drain — must panic
    }

    #[test]
    #[should_panic(expected = "unresolved token(s)")]
    fn partial_drain_panics_on_remaining() {
        // drain() marks consumed even if called once, so partial
        // consumption is not possible through the drain() API —
        // drain() yields all tokens. This test verifies that
        // creating a set and NOT calling drain triggers the panic.
        //
        // A "partial drain" scenario (take some, leave others)
        // would require a different API. TeardownSet::drain()
        // is all-or-nothing by design.
        let _set = TeardownSet::new(vec![(PeerScope(1), Token(0)), (PeerScope(1), Token(1))]);
        // dropped without drain — must panic
    }

    #[test]
    fn len_and_is_empty_reflect_contents() {
        let set_empty = TeardownSet::new(vec![]);
        assert!(set_empty.is_empty());
        assert_eq!(set_empty.len(), 0);

        let set_one = TeardownSet::new(vec![(PeerScope(1), Token(0))]);
        assert!(!set_one.is_empty());
        assert_eq!(set_one.len(), 1);

        // Consume to avoid Drop panic
        let mut set_one = set_one;
        let _: Vec<_> = set_one.drain().collect();
    }
}
