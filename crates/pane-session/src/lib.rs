//! pane-session: IPC session channels backed by par's CLL formalism.
//!
//! par (Michal Strba) provides the session type vocabulary and
//! duality checking. par is in-process (oneshot channels, panics on
//! disconnect). pane-session provides the IPC adaptation:
//!
//!   - Transport trait (unix, tcp, tls, memory)
//!   - Serialization via postcard
//!   - Chan<S> — session-typed channel over a Transport
//!   - Handshake types (Hello, Welcome, ControlMessage)
//!   - Wire framing ([length][service][payload])
//!
//! par's types are re-exported for protocol design and duality:
//! use par::exchange::{Send, Recv} to define protocol types, then
//! run them over pane-session's Chan + Transport.

// Re-export par for protocol design
pub use par;

pub mod transport;
pub mod chan;
pub mod handshake;

pub use chan::Chan;
pub use transport::Transport;
