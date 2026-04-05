//! pane-proto: protocol vocabulary for the pane framework.
//!
//! The type contracts that all pane crates depend on. No IO, no
//! runtime, no par. Pure type definitions:
//!
//!   - Message trait (Clone + Serialize + DeserializeOwned + Send + 'static)
//!   - Protocol trait + ServiceId
//!   - Flow enum (Continue / Stop)
//!   - Handles<P> trait (uniform dispatch, σ entries)
//!   - Handler trait (lifecycle sugar, blanket Handles<Lifecycle>)
//!   - MessageFilter<M> (typed per-protocol filters)
//!   - Property<S,A> (optic-backed state access via fp-library)
//!
//! Three layers:
//!   pane-proto   — type contracts (this crate)
//!   pane-session — par-backed session channels, transport, wire framing
//!   pane-app     — EAct actor framework

pub mod message;
pub mod protocol;
pub mod flow;
pub mod handles;
pub mod handler;
pub mod protocols;
pub mod filter;
pub mod property;

// Convenient re-exports
pub use flow::Flow;
pub use handles::Handles;
pub use handler::Handler;
pub use message::Message;
pub use protocol::{Protocol, ServiceId};
pub use filter::{MessageFilter, FilterAction};
