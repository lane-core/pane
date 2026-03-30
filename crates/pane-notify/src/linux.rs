//! Linux implementation using fanotify and inotify.
//!
//! fanotify: mount-wide watching with FAN_MARK_FILESYSTEM + FAN_ATTRIB
//! inotify: targeted file/directory watching
//!
//! Both deliver events to the consumer's channel.

use std::path::Path;

use crate::event::WatchFlags;
use crate::watcher::{Watcher, WatchHandle, WatchError};

impl Watcher {
    pub(crate) fn watch_mount_linux(
        &self,
        mount_path: &Path,
        flags: WatchFlags,
    ) -> Result<WatchHandle, WatchError> {
        // TODO: Phase 3 implementation
        // 1. Open fanotify fd: fanotify_init(FAN_CLASS_NOTIF | FAN_REPORT_FID, O_RDONLY)
        // 2. Mark the mount: fanotify_mark(fd, FAN_MARK_ADD | FAN_MARK_FILESYSTEM, mask, AT_FDCWD, mount_path)
        //    where mask maps WatchFlags to FAN_ATTRIB, FAN_CREATE | FAN_DELETE, etc.
        // 3. Spawn a reader thread that reads fanotify_event_metadata from the fd
        // 4. Resolve file handles (FAN_REPORT_FID) to paths via /proc/self/fd
        // 5. Construct Event with NodeRef, EventKind, path
        //
        // Requires CAP_SYS_ADMIN for FAN_MARK_FILESYSTEM.

        let _ = (mount_path, flags);
        todo!("fanotify implementation — Linux only, Phase 3")
    }

    pub(crate) fn watch_path_linux(
        &self,
        path: &Path,
        flags: WatchFlags,
    ) -> Result<WatchHandle, WatchError> {
        // TODO: Phase 3 implementation
        // 1. Open inotify fd: inotify_init1(IN_NONBLOCK | IN_CLOEXEC)
        // 2. Add watch: inotify_add_watch(fd, path, mask)
        //    where mask maps WatchFlags to IN_CREATE, IN_DELETE, IN_MODIFY, IN_ATTRIB,
        //    IN_MOVED_FROM, IN_MOVED_TO
        // 3. Spawn a reader thread that reads inotify_event from the fd
        // 4. Construct EventKind with cookie (for moves), StatFields (inferred from
        //    IN_MODIFY → SIZE|MODIFICATION_TIME, IN_ATTRIB → CHANGE_TIME)
        // 5. Construct NodeRef from stat(2) on the path
        // 6. Send Event to the channel

        let _ = (path, flags);
        todo!("inotify implementation — Linux only, Phase 3")
    }
}
