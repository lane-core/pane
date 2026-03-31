use pane_app::clipboard::{
    Clipboard, ClipboardMetadata, Sensitivity, Locality,
};
use pane_app::clipboard::ClipboardWriteLock;
use std::time::Duration;

#[test]
fn clipboard_system_default() {
    let clip = Clipboard::system();
    assert_eq!(clip.name(), "system");
}

#[test]
fn clipboard_named() {
    let clip = Clipboard::named("kill-ring");
    assert_eq!(clip.name(), "kill-ring");
}

#[test]
fn metadata_normal() {
    let meta = ClipboardMetadata {
        content_type: "text/plain".into(),
        sensitivity: Sensitivity::Normal,
        locality: Locality::Any,
    };
    assert!(matches!(meta.sensitivity, Sensitivity::Normal));
    assert!(matches!(meta.locality, Locality::Any));
}

#[test]
fn metadata_secret_with_ttl() {
    let meta = ClipboardMetadata {
        content_type: "text/plain".into(),
        sensitivity: Sensitivity::Secret { ttl: Duration::from_secs(30) },
        locality: Locality::Local,
    };
    if let Sensitivity::Secret { ttl } = meta.sensitivity {
        assert_eq!(ttl, Duration::from_secs(30));
    } else {
        panic!("expected Secret");
    }
    assert!(matches!(meta.locality, Locality::Local));
}

#[test]
fn write_lock_commit_consumes() {
    let (tx, _rx) = std::sync::mpsc::channel();
    let lock = ClipboardWriteLock::new_for_test("system".into(), tx);

    lock.commit(
        b"hello".to_vec(),
        ClipboardMetadata {
            content_type: "text/plain".into(),
            sensitivity: Sensitivity::Normal,
            locality: Locality::Any,
        },
    );
    // lock is consumed — using it again would be a compile error.
}

#[test]
fn write_lock_revert_consumes() {
    let (tx, _rx) = std::sync::mpsc::channel();
    let lock = ClipboardWriteLock::new_for_test("system".into(), tx);
    lock.revert();
}

#[test]
fn write_lock_drop_reverts() {
    let (tx, rx) = std::sync::mpsc::channel();
    {
        let _lock = ClipboardWriteLock::new_for_test("system".into(), tx);
        // dropped without commit — should send revert
    }
    let msg = rx.try_recv();
    assert!(msg.is_ok(), "drop should send revert");
}
