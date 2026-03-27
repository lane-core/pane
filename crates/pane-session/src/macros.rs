//! Macros for ergonomic N-ary branching over nested binary Select/Branch.
//!
//! The session type primitives are binary: `Select<L, R>` and `Branch<L, R>`.
//! N-ary choices are encoded as right-nested binary: `Select<A, Select<B, C>>`.
//! These macros flatten the nesting for readability.

/// Construct an N-ary select type from a flat list of continuations.
///
/// ```ignore
/// // These are equivalent:
/// type Negotiation = choice![Accept, Fallback, Reject];
/// type Negotiation = Select<Accept, Select<Fallback, Reject>>;
/// ```
#[macro_export]
macro_rules! choice {
    ($a:ty, $b:ty) => {
        $crate::Select<$a, $b>
    };
    ($a:ty, $($rest:ty),+ $(,)?) => {
        $crate::Select<$a, $crate::choice![$($rest),+]>
    };
}

/// Receive an N-ary branch selection with flat, labeled arms.
///
/// Expands nested `offer()` calls into a flat match. Each arm binds
/// the continuation channel to the given name. Labels are for
/// documentation — they appear in the source but don't affect behavior.
///
/// ```ignore
/// // 2-way (same as manual offer + match):
/// offer!(chan, {
///     accepted(c) => { /* c: Chan<AcceptPath, T> */ },
///     rejected(c) => { /* c: Chan<RejectPath, T> */ },
/// })
///
/// // 3-way (flattens nested Select<A, Select<B, C>>):
/// offer!(chan, {
///     accepted(c) => { /* ... */ },
///     fallback(c) => { /* ... */ },
///     rejected(c) => { /* ... */ },
/// })
///
/// // 4-way:
/// offer!(chan, {
///     accepted(c) => { /* ... */ },
///     fallback(c) => { /* ... */ },
///     version_mismatch(c) => { /* ... */ },
///     rejected(c) => { /* ... */ },
/// })
/// ```
///
/// The result type is `Result<R, SessionError>` where R is the common
/// return type of all arms.
#[macro_export]
macro_rules! offer {
    // Base case: exactly 2 arms
    ($chan:expr, {
        $label_a:ident($a:ident) => $body_a:expr,
        $label_b:ident($b:ident) => $body_b:expr $(,)?
    }) => {
        match $chan.offer()? {
            $crate::Offer::Left($a) => $body_a,
            $crate::Offer::Right($b) => $body_b,
        }
    };
    // Recursive case: 3+ arms — peel off the first, recurse on the rest
    ($chan:expr, {
        $label_a:ident($a:ident) => $body_a:expr,
        $($rest:tt)+
    }) => {
        match $chan.offer()? {
            $crate::Offer::Left($a) => $body_a,
            $crate::Offer::Right(__rest) => {
                $crate::offer!(__rest, { $($rest)+ })
            },
        }
    };
}
