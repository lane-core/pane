use pane_app::clipboard::{
    Clipboard, ClipboardMetadata, Sensitivity, Locality,
};
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
