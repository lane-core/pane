//! Protocol vocabulary for pane.
//!
//! Type contracts that all pane crates depend on. No IO, no
//! runtime. Defines:
//!   - Message trait (Clone + Serialize + DeserializeOwned + Send + 'static)
//!   - Protocol trait + RequestProtocol supertrait + ServiceId
//!   - Flow (Continue / Stop)
//!   - Handles<P> (uniform per-protocol dispatch)
//!   - HandlesRequest<P> (request/reply dispatch with ReplyPort obligation)
//!   - Handler (lifecycle convenience, blanket Handles<Lifecycle>)
//!   - MessageFilter<M> (typed per-protocol filters)
//!   - MonadicLens<S,A> (optic-backed state access)
//!   - PeerAuth + AuthSource (transport-level peer identity)
//!   - Address (resolved pane address for routing)
//!   - ControlMessage (wire service 0 envelope)

pub mod address;
pub mod control;
pub mod filter;
pub mod flow;
pub mod handler;
pub mod handles;
pub mod message;
pub mod monadic_lens;
pub mod obligation;
pub mod peer_auth;
pub mod protocol;
pub mod protocols;
pub mod service_frame;

pub use address::Address;
pub use control::{ControlMessage, DeclineReason, TeardownReason};
pub use filter::{FilterAction, MessageFilter};
pub use flow::Flow;
pub use handler::Handler;
pub use handles::{Handles, HandlesRequest};
pub use message::Message;
pub use peer_auth::{AuthSource, PeerAuth};
pub use protocol::{Protocol, RequestProtocol, ServiceId};
pub use service_frame::ServiceFrame;
