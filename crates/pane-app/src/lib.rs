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

pub mod builder;
pub mod dispatch;
pub mod dispatch_ctx;
pub mod exit_reason;
pub mod handles_request;
pub mod looper_core;
pub mod messenger;
pub mod pane;
pub mod service_dispatch;
pub mod service_handle;

pub use builder::PaneBuilder;
pub use dispatch_ctx::DispatchCtx;
pub use exit_reason::ExitReason;
pub use handles_request::HandlesRequest;
pub use messenger::Messenger;
pub use pane::Pane;
pub use service_handle::{wire_reply_port, ServiceHandle};

// Re-export pane-proto types for convenience
pub use pane_proto::{
    Address, FilterAction, Flow, Handler, Handles, Message, MessageFilter, Protocol,
    RequestProtocol, ServiceId,
};
