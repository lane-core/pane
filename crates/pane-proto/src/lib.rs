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
//!   - MonadicLens<S,A> (optic-backed state access)
//!   - PeerAuth + AuthSource (transport-level peer identity)
//!   - Address (resolved pane address for routing)
//!   - ControlMessage (wire service 0 envelope)

pub mod address;
pub mod control;
pub mod message;
pub mod protocol;
pub mod flow;
pub mod handles;
pub mod handler;
pub mod protocols;
pub mod filter;
pub mod monadic_lens;
pub mod obligation;
pub mod peer_auth;
pub mod service_frame;

pub use address::Address;
pub use control::{ControlMessage, DeclineReason, TeardownReason};
pub use flow::Flow;
pub use handles::Handles;
pub use handler::Handler;
pub use message::Message;
pub use protocol::{Protocol, ServiceId};
pub use filter::{MessageFilter, FilterAction};
pub use peer_auth::{PeerAuth, AuthSource};
pub use service_frame::ServiceFrame;
