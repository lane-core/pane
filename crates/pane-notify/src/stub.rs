//! Non-Linux stub implementation using polling.
//! Provides basic functionality for development on macOS.
//! NOT for production — Linux fanotify/inotify is the real implementation.

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use crate::event::{Event, EventKind};
use crate::watcher::{Watcher, WatchHandle, WatchError};

impl Watcher {
    pub(crate) fn watch_mount_stub(
        &self,
        _mount_path: &Path,
        _kinds: &[EventKind],
    ) -> Result<WatchHandle, WatchError> {
        // On non-Linux, mount-wide watching is not available.
        // Return a no-op handle. Real implementation uses fanotify.
        eprintln!("pane-notify: mount-wide watching not available on this platform (stub)");
        Ok(WatchHandle { _id: self.alloc_id() })
    }

    pub(crate) fn watch_path_stub(
        &self,
        path: &Path,
        kinds: &[EventKind],
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
        let kinds = kinds.to_vec();

        // Capture the initial state BEFORE spawning the thread,
        // so any file created after watch_path returns will be detected.
        let initial_entries = list_dir_entries(&path);

        // Stub: poll the directory every 200ms for changes.
        // This is crude but sufficient for macOS development.
        std::thread::spawn(move || {
            let mut last_entries = initial_entries;
            loop {
                std::thread::sleep(Duration::from_millis(200));
                let current_entries = list_dir_entries(&path);

                // Detect new files
                if kinds.contains(&EventKind::Create) {
                    for entry in &current_entries {
                        if !last_entries.contains(entry) {
                            let _ = sender.send(Event {
                                kind: EventKind::Create,
                                path: path.join(entry),
                            });
                        }
                    }
                }

                // Detect deleted files
                if kinds.contains(&EventKind::Delete) {
                    for entry in &last_entries {
                        if !current_entries.contains(entry) {
                            let _ = sender.send(Event {
                                kind: EventKind::Delete,
                                path: path.join(entry),
                            });
                        }
                    }
                }

                last_entries = current_entries;
            }
        });

        Ok(WatchHandle { _id: id })
    }
}

fn list_dir_entries(path: &Path) -> Vec<String> {
    std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect()
}
