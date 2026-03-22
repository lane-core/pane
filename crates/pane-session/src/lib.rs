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

pub mod types;
pub mod dual;
pub mod error;
pub mod transport;

pub use types::{Send, Recv, End, Chan};
pub use dual::Dual;
pub use error::SessionError;
