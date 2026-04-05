//! pane-session: IPC adaptation of par's CLL session types.
//!
//! Three layers:
//!   par          — CLL binary channel correctness (complete, Strba)
//!   pane-session — IPC bridge + type vocabulary (this crate)
//!   pane-app     — EAct actor framework (Fowler et al.)
//!
//! This crate provides:
//!   - par re-exports (Send, Recv, Enqueue, Dequeue, Server, etc.)
//!   - Message trait (Clone + Serialize + DeserializeOwned + Send + 'static)
//!   - Protocol trait + ServiceId
//!   - Flow enum (Continue / Stop)
//!   - Handles<P> trait (uniform dispatch, σ entries)
//!   - Handler trait (lifecycle sugar, blanket Handles<Lifecycle>)
//!   - Framework protocols (Lifecycle, Display, Clipboard, Routing)
//!   - MessageFilter<M> (typed per-protocol filters)
//!   - Property<S,A> (optic-backed state access via fp-library)

// Re-export par's CLL types — the session type vocabulary.
pub use par::exchange::{Send, Recv};
pub use par::queue::{Dequeue, Enqueue};
pub use par::server::{Server, Proxy, Connection};
pub use par::{Session, Dual};

pub mod message;
pub mod protocol;
pub mod flow;
pub mod handles;
pub mod handler;
pub mod protocols;
pub mod filter;
pub mod property;

// Convenient re-exports for consumers
pub use flow::Flow;
pub use handles::Handles;
pub use handler::Handler;
pub use message::Message;
pub use protocol::{Protocol, ServiceId};
pub use filter::{MessageFilter, FilterAction};
