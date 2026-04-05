//! pane-app: the EAct actor framework.
//!
//! This is the runtime layer — it composes pane-proto's type
//! contracts into a working actor system:
//!
//!   pane-proto — type contracts (Message, Protocol, Handles<P>, Handler, Flow)
//!   pane-app     — runtime (Messenger, Pane, PaneBuilder, Dispatch, looper)
//!
//! EAct correspondence:
//!   Actor         = a Pane running its Handler on the looper thread
//!   Handler store σ = Handles<P> fn pointers + Dispatch<H> entries
//!   E-Suspend     = send_request installs Dispatch entry
//!   E-React       = reply arrives, entry consumed, callback fires
//!   E-Reset       = handler returns Flow (back to idle)
//!   E-Raise       = panic → catch_unwind (actor annihilated)

pub mod exit_reason;
pub mod messenger;
pub mod service_handle;
pub mod dispatch;
pub mod pane;
pub mod builder;

pub use exit_reason::ExitReason;
pub use messenger::Messenger;
pub use service_handle::ServiceHandle;
pub use pane::Pane;
pub use builder::PaneBuilder;

// Re-export pane-proto types for convenience
pub use pane_proto::{
    Flow, Handler, Handles, Message, Protocol, ServiceId,
    MessageFilter, FilterAction,
};
