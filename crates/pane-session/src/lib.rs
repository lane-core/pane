//! pane-session: par-backed session channels for IPC.
//!
//! par (Michal Strba) provides CLL session types with a complete
//! in-process runtime (oneshot channels, duality, fork_sync).
//! pane-session bridges par's runtime to IPC transports:
//!
//!   Handler ←→ par (oneshot) ←→ Bridge thread ←→ Transport ←→ wire
//!
//! The handler uses par's native API directly — par::exchange::Send::send(),
//! par::exchange::Recv::recv(). The bridge thread serializes values
//! (postcard) between par's oneshot channels and the Transport.
//!
//! No Chan<S,T> wrapper — par IS the session channel. pane-session
//! provides the Transport trait and protocol-specific bridges.

pub use par;

pub mod transport;
pub mod bridge;
pub mod handshake;

pub use transport::Transport;
