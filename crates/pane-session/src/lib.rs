//! Session-typed IPC channels for pane.
//!
//! Uses par (Michal Strba) for session type primitives and duality.
//! pane-session bridges par's in-process channels to IPC transports
//! via serialization (postcard) and a bridge thread per connection.
//!
//! The handler uses par's native Send/Recv API. The bridge thread
//! translates between par's oneshot channels and the Transport.

pub use par;

pub mod bridge;
pub mod frame;
pub mod handshake;
pub mod peer_cred;
pub mod server;
pub mod transport;

pub use transport::Transport;
