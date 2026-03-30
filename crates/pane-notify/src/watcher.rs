use std::path::Path;
use std::sync::mpsc;
use std::fmt;

use crate::event::{Event, EventKind};

/// Opaque handle to a registered watch. Drop it to unregister.
///
/// # BeOS
///
/// Replaces the `node_ref` passed to `stop_watching()`. RAII: dropping
/// the handle unregisters the watch instead of requiring an explicit call.
#[derive(Debug)]
pub struct WatchHandle {
    pub(crate) _id: u64,
    // Dropping this handle will signal the watcher thread to
    // remove the watch. Implementation-specific.
}

/// Errors from watch registration.
#[derive(Debug)]
pub enum WatchError {
    /// fanotify requires CAP_SYS_ADMIN for FAN_MARK_FILESYSTEM.
    /// Use `watch_path()` for unprivileged watching.
    InsufficientCapabilities,
    /// The path doesn't exist or isn't accessible.
    PathNotFound(std::io::Error),
    /// Kernel interface error.
    Io(std::io::Error),
}

impl fmt::Display for WatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WatchError::InsufficientCapabilities =>
                write!(f, "mount-wide watches require CAP_SYS_ADMIN; use watch_path() for targeted watching"),
            WatchError::PathNotFound(e) =>
                write!(f, "watch path not found: {}", e),
            WatchError::Io(e) =>
                write!(f, "filesystem notification error: {}", e),
        }
    }
}

impl std::error::Error for WatchError {}

impl From<std::io::Error> for WatchError {
    fn from(e: std::io::Error) -> Self {
        WatchError::Io(e)
    }
}

/// Filesystem notification watcher.
///
/// Watches files and directories for changes, delivering events to a
/// channel. The watcher automatically selects fanotify or inotify
/// based on the watch scope.
///
/// # BeOS
///
/// Replaces `watch_node()`. Key changes:
/// - The watcher is an object, not a global function
/// - Events go to a channel, not a `BLooper`
/// - Two watch scopes (mount-wide, targeted) are exposed as separate
///   methods; the watcher selects the kernel interface automatically
///
/// - `watch_mount()` — mount-wide watching via fanotify (requires CAP_SYS_ADMIN)
/// - `watch_path()` — targeted watching via inotify
///
/// # Usage
///
/// ```ignore
/// let (tx, rx) = std::sync::mpsc::channel();
/// let watcher = Watcher::new(tx)?;
///
/// // Watch a specific directory for file creation/deletion
/// let _handle = watcher.watch_path("/etc/pane/route/rules/",
///     EventKind::Create | EventKind::Delete)?;
///
/// // Events arrive on the channel
/// while let Ok(event) = rx.recv() {
///     println!("{}: {:?}", event.path.display(), event.kind);
/// }
/// ```
pub struct Watcher {
    pub(crate) sender: mpsc::Sender<Event>,
    pub(crate) next_id: std::sync::atomic::AtomicU64,
}

impl Watcher {
    /// Create a new watcher that delivers events to the given channel.
    pub fn new(sender: mpsc::Sender<Event>) -> Result<Self, WatchError> {
        Ok(Watcher {
            sender,
            next_id: std::sync::atomic::AtomicU64::new(1),
        })
    }

    /// Watch an entire mount for the specified event kinds.
    /// Uses fanotify with FAN_MARK_FILESYSTEM. Requires CAP_SYS_ADMIN.
    ///
    /// This is the mechanism for pane-store's mount-wide attribute indexing.
    pub fn watch_mount(
        &self,
        _mount_path: impl AsRef<Path>,
        _kinds: &[EventKind],
    ) -> Result<WatchHandle, WatchError> {
        #[cfg(target_os = "linux")]
        {
            self.watch_mount_linux(_mount_path.as_ref(), _kinds)
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.watch_mount_stub(_mount_path.as_ref(), _kinds)
        }
    }

    /// Watch a specific path (file or directory) for the specified event kinds.
    /// Uses inotify. No special capabilities required.
    ///
    /// For directories: watches for creation, deletion, and modification of
    /// entries within the directory (not recursive — watch subdirectories
    /// individually if needed).
    pub fn watch_path(
        &self,
        _path: impl AsRef<Path>,
        _kinds: &[EventKind],
    ) -> Result<WatchHandle, WatchError> {
        #[cfg(target_os = "linux")]
        {
            self.watch_path_linux(_path.as_ref(), _kinds)
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.watch_path_stub(_path.as_ref(), _kinds)
        }
    }

    pub(crate) fn alloc_id(&self) -> u64 {
        self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}
