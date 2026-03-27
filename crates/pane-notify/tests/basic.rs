use std::sync::mpsc;
use std::time::Duration;

use pane_notify::{Watcher, Event, EventKind};

#[test]
fn watch_path_detects_file_creation() {
    let dir = tempfile::tempdir().unwrap();
    let (tx, rx) = mpsc::channel();

    let watcher = Watcher::new(tx).unwrap();
    let _handle = watcher.watch_path(dir.path(), &[EventKind::Create]).unwrap();

    // Create a file in the watched directory
    std::fs::write(dir.path().join("test.toml"), "content").unwrap();

    // On the stub (macOS), the polling interval is 500ms.
    // On Linux with inotify, this would be near-instant.
    let event = rx.recv_timeout(Duration::from_secs(2));

    // The stub should detect the new file
    #[cfg(not(target_os = "linux"))]
    {
        let event = event.expect("should have received create event");
        assert_eq!(event.kind, EventKind::Create);
        assert!(event.path.ends_with("test.toml"));
    }

    // On Linux, the inotify impl will be tested separately
    #[cfg(target_os = "linux")]
    {
        // TODO: test with real inotify
        let _ = event;
    }
}

#[test]
fn watch_path_detects_file_deletion() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("to-delete.toml");
    std::fs::write(&file_path, "content").unwrap();

    let (tx, rx) = mpsc::channel();
    let watcher = Watcher::new(tx).unwrap();
    let _handle = watcher.watch_path(dir.path(), &[EventKind::Delete]).unwrap();

    // Delete the file
    std::fs::remove_file(&file_path).unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2));

    #[cfg(not(target_os = "linux"))]
    {
        let event = event.expect("should have received delete event");
        assert_eq!(event.kind, EventKind::Delete);
        assert!(event.path.ends_with("to-delete.toml"));
    }

    #[cfg(target_os = "linux")]
    {
        let _ = event;
    }
}

#[test]
fn watch_nonexistent_path_returns_error() {
    let (tx, _rx) = mpsc::channel();
    let watcher = Watcher::new(tx).unwrap();

    let result = watcher.watch_path("/nonexistent/path/that/does/not/exist", &[EventKind::Create]);
    assert!(result.is_err());
}
