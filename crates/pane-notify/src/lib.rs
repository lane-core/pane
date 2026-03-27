//! Filesystem notification for the pane desktop environment.
//!
//! Abstracts over fanotify (mount-wide) and inotify (targeted) based on
//! watch scope. Consumers request watches by intent; pane-notify selects
//! the kernel interface.
//!
//! Two delivery modes:
//! - **Channel**: events sent to a `std::sync::mpsc::Sender` — for looper-based
//!   servers and pane-app clients.
//! - **Calloop**: events delivered as a calloop `EventSource` — for the compositor
//!   only (feature-gated behind `calloop`).
//!
//! On non-Linux platforms (macOS dev), a polling stub is provided for
//! basic testing. The real implementation requires Linux 5.1+ (fanotify
//! with FAN_REPORT_FID) and Linux 2.6.13+ (inotify).

mod event;
mod watcher;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(not(target_os = "linux"))]
mod stub;

pub use event::{Event, EventKind};
pub use watcher::{Watcher, WatchHandle, WatchError};
