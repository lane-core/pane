//! pane-session: par-backed session channels for IPC.
//!
//! par (Michal Strba) provides the CLL session type vocabulary:
//! Send/Recv (exchange), Enqueue/Dequeue (streaming), Server/Proxy
//! (lifecycle), Session trait (duality), Dual type alias.
//!
//! pane-session provides Chan<S, T> — par's session types over a
//! Transport trait with postcard serialization. Par's types are used
//! directly as phantom state parameters on Chan. PhantomData<S> is
//! zero-size regardless of S's internal structure.

pub use par;

pub mod transport;
pub mod chan;
pub mod handshake;
pub mod bridge;

pub use chan::Chan;
pub use transport::Transport;
