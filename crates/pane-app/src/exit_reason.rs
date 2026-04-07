//! ExitReason: re-exported from pane-proto.
//!
//! ExitReason lives in pane-proto because it's wire-transmitted
//! in PaneExited notifications. pane-app re-exports it for
//! backward compatibility.

pub use pane_proto::ExitReason;
