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
//!
//! # BeOS
//!
//! Descends from `BNodeMonitor` (`watch_node()` / `stop_watching()` +
//! `B_NODE_MONITOR` messages). Key changes:
//! - Events go to a channel instead of a `BLooper` — consumers are
//!   not required to be loopers
//! - Watching is by intent (mount-wide vs. targeted path) instead of
//!   by `node_ref` — pane-notify automatically selects the right
//!   kernel interface (fanotify or inotify)
//! - RAII: dropping a [`WatchHandle`] unregisters the watch instead
//!   of requiring an explicit `stop_watching()` call
//!
//! Haiku's `BNodeMonitor` implementation and the Haiku Book's
//! [NodeMonitor documentation](reference/haiku-book/storage/NodeMonitor.dox)
//! informed the design of the event kinds and watch semantics.

mod event;
mod watcher;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(not(target_os = "linux"))]
mod stub;

pub use event::{Event, EventKind, NodeRef, StatFields, WatchFlags, AttrCause};
pub use watcher::{Watcher, WatchHandle, WatchError};
