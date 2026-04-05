//! Actor framework for pane.
//!
//! Runtime layer: composes pane-proto's type contracts into a
//! working actor system with single-threaded dispatch.
//!
//! Provides: Pane, PaneBuilder, Dispatch (request/reply),
//! Messenger, ServiceHandle, ExitReason.
//!
//! Design heritage: BeOS BLooper (sequential dispatch, one thread
//! per actor). Formalized by Fowler et al.'s EAct.

pub mod exit_reason;
pub mod messenger;
pub mod service_handle;
pub mod dispatch;
pub mod pane;
pub mod builder;
pub mod looper_core;

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
