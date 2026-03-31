//! Session-typed channels for the pane desktop environment.
//!
//! Provides `Chan<S, T>` — a channel whose type tracks the protocol state.
//! The session type `S` describes what messages can be sent and received,
//! in what order, with what choices. The transport `T` handles serialization
//! and delivery (unix sockets for production, in-memory for testing).
//!
//! Crash-safe: `recv()` returns `Err(SessionError::Disconnected)`, not a panic.
//! This is the property that par cannot provide and the reason for the custom
//! implementation.
//!
//! Theoretical basis: Caires-Pfenning/Wadler correspondence between linear
//! logic and session types. Formal primitives verified in Lean/Agda.
//!
//! # BeOS
//!
//! No BeOS equivalent. BeOS used untyped kernel ports — any thread could
//! send any `BMessage` to any port at any time. Session types replace
//! that with compile-time protocol verification: the type system
//! guarantees message ordering, branch coverage, and deadlock freedom.
//!
//! # Plan 9
//!
//! 9P provides protocol structure through convention and the `intro(5)`
//! specification: clients and servers agree on message ordering (attach
//! before walk, walk before open, open before read). Errors are caught
//! at runtime when a server receives an unexpected message. Session
//! types move this structure into the type system — 9P's `intro(5)`
//! rules become compile-time constraints. The transport trait mirrors
//! 9P's transport independence: the same protocol runs over unix
//! sockets, TCP, or in-memory channels.

pub mod types;
pub mod dual;
pub mod error;
pub mod framing;
pub mod transport;
pub mod calloop;
#[macro_use]
pub mod macros;

pub use types::{Send, Recv, Select, Branch, Offer, End, Chan};
pub use dual::Dual;
pub use error::SessionError;
