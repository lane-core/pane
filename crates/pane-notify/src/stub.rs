//! Non-Linux stub implementation using polling.
//! Provides basic functionality for development on macOS.
//! NOT for production — Linux fanotify/inotify is the real implementation.

use std::path::Path;
use std::time::Duration;

use crate::event::{Event, EventKind, NodeRef, WatchFlags};
use crate::watcher::{Watcher, WatchHandle, WatchError};

impl Watcher {
    pub(crate) fn watch_mount_stub(
        &self,
        _mount_path: &Path,
        _flags: WatchFlags,
    ) -> Result<WatchHandle, WatchError> {
        eprintln!("pane-notify: mount-wide watching not available on this platform (stub)");
        Ok(WatchHandle { _id: self.alloc_id() })
    }

    pub(crate) fn watch_path_stub(
        &self,
        path: &Path,
        flags: WatchFlags,
    ) -> Result<WatchHandle, WatchError> {
        if !path.exists() {
            return Err(WatchError::PathNotFound(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("{} does not exist", path.display()),
            )));
        }

        let id = self.alloc_id();
        let sender = self.sender.clone();
        let path = path.to_path_buf();

        // Capture initial state BEFORE spawning the thread.
        let initial_entries = list_dir_entries(&path);

        // Stub: poll the directory every 200ms for changes.
        std::thread::spawn(move || {
            let mut last_entries = initial_entries;
            loop {
                std::thread::sleep(Duration::from_millis(200));
                let current_entries = list_dir_entries(&path);

                if flags.contains(WatchFlags::CREATE) {
                    for (name, node) in &current_entries {
                        if !last_entries.iter().any(|(n, _)| n == name) {
                            let _ = sender.send(Event {
                                kind: EventKind::Created {
                                    name: name.into(),
                                    directory: node_ref_for(&path),
                                },
                                path: path.join(name),
                                node: *node,
                            });
                        }
                    }
                }

                if flags.contains(WatchFlags::REMOVE) {
                    for (name, node) in &last_entries {
                        if !current_entries.iter().any(|(n, _)| n == name) {
                            let _ = sender.send(Event {
                                kind: EventKind::Removed {
                                    name: name.into(),
                                    directory: node_ref_for(&path),
                                },
                                path: path.join(name),
                                node: *node,
                            });
                        }
                    }
                }

                // Stat/attr change detection would require caching metadata
                // per entry. Not implemented in the stub — use Linux for that.

                last_entries = current_entries;
            }
        });

        Ok(WatchHandle { _id: id })
    }
}

/// List directory entries with their NodeRef (dev + inode).
fn list_dir_entries(path: &Path) -> Vec<(String, NodeRef)> {
    std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let node = node_ref_from_metadata(&e.path());
            Some((name, node))
        })
        .collect()
}

/// Get a NodeRef from a path's metadata. Falls back to zeros if stat fails.
fn node_ref_for(path: &Path) -> NodeRef {
    node_ref_from_metadata(path)
}

fn node_ref_from_metadata(path: &Path) -> NodeRef {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let Ok(meta) = std::fs::metadata(path) {
            return NodeRef {
                device: meta.dev(),
                inode: meta.ino(),
            };
        }
    }
    NodeRef { device: 0, inode: 0 }
}
