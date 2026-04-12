//! Non-blocking send contract for the looper thread.
//!
//! All sends callable from the looper thread go through
//! [`NonBlockingSend`]. The trait constrains the send path so
//! blocking sends are unrepresentable in code that takes
//! `impl NonBlockingSend` — the type system enforces N1 (all
//! looper-thread sends are non-blocking).
//!
//! Two expected implementors:
//! - `SharedWriter` (looper-thread direct writes, the 90% path)
//! - A bounded-channel wrapper (cross-thread sends via mpsc)
//!
//! Design heritage: BeOS BDirectMessageTarget bypassed the port for
//! same-process sends — message went directly to BMessageQueue
//! (src/kits/app/MessageQueue.cpp), port got a zero-byte poke to
//! wake the looper. Plan 9 mountio wrote directly to the kernel
//! pipe (reference/plan9/src/sys/src/9/port/devmnt.c:803) — one
//! buffer, no intermediate channels. Both systems ensured the
//! send path never blocked the dispatching thread.
//!
//! # References
//!
//! - `[FH]` §3.2 — E-Send: async message append to session queue.
//!   The rule is a premise of the operational semantics (Preservation
//!   Theorem 4, Progress Theorem 5). If the send blocks, the actor
//!   is no longer idle after E-Suspend, and Progress does not hold.

use crate::Backpressure;

/// Non-blocking frame send contract \[N1\].
///
/// All sends callable from the looper thread go through this
/// trait. Implementors return immediately — blocking the looper
/// thread violates I2 (handlers must not block) and creates
/// deadlock risk (Bug 2: bidirectional buffer deadlock).
///
/// Realizes `[FH]` E-Send: "async message append to session
/// queue" (§3.2). The rule is a premise of the operational
/// semantics — if violated, safety theorems (Preservation,
/// Progress) do not hold.
pub trait NonBlockingSend {
    /// Attempt to send a framed message for the given service.
    ///
    /// Returns `Ok(())` on success, `Err(Backpressure)` if the
    /// send cannot complete without blocking. The caller handles
    /// backpressure — either cap-and-abort (`send_request`) or
    /// return the error to the caller (`try_send_request`).
    fn try_send_frame(&self, service: u16, payload: &[u8]) -> Result<(), Backpressure>;
}
