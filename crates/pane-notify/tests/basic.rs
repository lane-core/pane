use std::sync::mpsc;
use std::time::Duration;

use pane_notify::{Watcher, EventKind, WatchFlags};

#[test]
fn watch_path_detects_file_creation() {
    let dir = tempfile::tempdir().unwrap();
    let (tx, rx) = mpsc::channel();

    let watcher = Watcher::new(tx).unwrap();
    let _handle = watcher.watch_path(dir.path(), WatchFlags::CREATE).unwrap();

    // Create a file in the watched directory
    std::fs::write(dir.path().join("test.toml"), "content").unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2));

    #[cfg(not(target_os = "linux"))]
    {
        let event = event.expect("should have received create event");
        assert!(matches!(event.kind, EventKind::Created { .. }));
        assert!(event.path.ends_with("test.toml"));
        assert_ne!(event.node.inode, 0, "should have a real inode");
    }

    #[cfg(target_os = "linux")]
    {
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
    let _handle = watcher.watch_path(dir.path(), WatchFlags::REMOVE).unwrap();

    // Delete the file
    std::fs::remove_file(&file_path).unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2));

    #[cfg(not(target_os = "linux"))]
    {
        let event = event.expect("should have received remove event");
        assert!(matches!(event.kind, EventKind::Removed { .. }));
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

    let result = watcher.watch_path("/nonexistent/path/that/does/not/exist", WatchFlags::CREATE);
    assert!(result.is_err());
}

#[test]
fn created_event_has_directory_node_ref() {
    let dir = tempfile::tempdir().unwrap();
    let (tx, rx) = mpsc::channel();

    let watcher = Watcher::new(tx).unwrap();
    let _handle = watcher.watch_path(dir.path(), WatchFlags::CREATE).unwrap();

    std::fs::write(dir.path().join("new-file.txt"), "data").unwrap();

    let event = rx.recv_timeout(Duration::from_secs(2));

    #[cfg(not(target_os = "linux"))]
    {
        let event = event.expect("should have received event");
        if let EventKind::Created { name, directory } = &event.kind {
            assert_eq!(name, "new-file.txt");
            assert_ne!(directory.inode, 0, "directory should have real inode");
        } else {
            panic!("expected Created, got {:?}", event.kind);
        }
    }

    #[cfg(target_os = "linux")]
    {
        let _ = event;
    }
}
