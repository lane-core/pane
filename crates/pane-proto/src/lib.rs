//! Protocol vocabulary for pane.
//!
//! Type contracts that all pane crates depend on. No IO, no
//! runtime. Defines:
//!   - Message trait (Clone + Serialize + DeserializeOwned + Send + 'static)
//!   - Protocol trait + ServiceId
//!   - Flow (Continue / Stop)
//!   - Handles<P> (uniform per-protocol dispatch)
//!   - Handler (lifecycle convenience, blanket Handles<Lifecycle>)
//!   - MessageFilter<M> (typed per-protocol filters)
//!   - Property<S,A> (optic-backed state access)

pub mod message;
pub mod protocol;
pub mod flow;
pub mod handles;
pub mod handler;
pub mod protocols;
pub mod filter;
pub mod property;
pub mod monadic_lens;
pub mod obligation;

pub use flow::Flow;
pub use handles::Handles;
pub use handler::Handler;
pub use message::Message;
pub use protocol::{Protocol, ServiceId};
pub use filter::{MessageFilter, FilterAction};
