//! Linux implementation using fanotify and inotify.
//!
//! fanotify: mount-wide watching with FAN_MARK_FILESYSTEM + FAN_ATTRIB
//! inotify: targeted file/directory watching
//!
//! Both deliver events to the consumer's channel.

use std::path::Path;

use crate::event::EventKind;
use crate::watcher::{Watcher, WatchHandle, WatchError};

impl Watcher {
    pub(crate) fn watch_mount_linux(
        &self,
        mount_path: &Path,
        kinds: &[EventKind],
    ) -> Result<WatchHandle, WatchError> {
        // TODO: Phase 3 implementation
        // 1. Open fanotify fd: fanotify_init(FAN_CLASS_NOTIF | FAN_REPORT_FID, O_RDONLY)
        // 2. Mark the mount: fanotify_mark(fd, FAN_MARK_ADD | FAN_MARK_FILESYSTEM, mask, AT_FDCWD, mount_path)
        //    where mask = FAN_ATTRIB for xattr changes, FAN_CREATE | FAN_DELETE for file ops
        // 3. Spawn a reader thread that reads fanotify_event_metadata from the fd
        // 4. Resolve file handles (FAN_REPORT_FID) to paths via /proc/self/fd
        // 5. Send Event to the channel
        //
        // Requires CAP_SYS_ADMIN for FAN_MARK_FILESYSTEM.
        // If the capability check fails, return WatchError::InsufficientCapabilities.

        let _ = (mount_path, kinds);
        todo!("fanotify implementation — Linux only, Phase 3")
    }

    pub(crate) fn watch_path_linux(
        &self,
        path: &Path,
        kinds: &[EventKind],
    ) -> Result<WatchHandle, WatchError> {
        // TODO: Phase 3 implementation
        // 1. Open inotify fd: inotify_init1(IN_NONBLOCK | IN_CLOEXEC)
        // 2. Add watch: inotify_add_watch(fd, path, mask)
        //    where mask maps EventKind to IN_CREATE, IN_DELETE, IN_MODIFY, IN_ATTRIB, etc.
        // 3. Spawn a reader thread that reads inotify_event from the fd
        // 4. Resolve watch descriptor + name to full path
        // 5. Send Event to the channel
        //
        // No special capabilities required.

        let _ = (path, kinds);
        todo!("inotify implementation — Linux only, Phase 3")
    }
}
