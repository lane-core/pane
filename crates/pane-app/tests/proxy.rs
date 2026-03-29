//! PaneHandle tests — the BMessenger equivalent.
//! Tests that the most-used developer API sends correct messages
//! and handles disconnection gracefully.

use std::num::NonZeroU32;
use std::sync::mpsc;

use pane_app::Messenger;
use pane_proto::message::PaneId;
use pane_proto::protocol::ClientToComp;
use pane_proto::tag::{PaneTitle, CommandVocabulary, Completion};

fn pane_id(n: u32) -> PaneId {
    PaneId::new(NonZeroU32::new(n).unwrap())
}

// --- P1-1: Disconnected PaneHandle ---

#[test]
fn pane_handle_send_after_disconnect() {
    let (tx, rx) = mpsc::channel::<ClientToComp>();
    let handle = Messenger::new(pane_id(1), tx);
    drop(rx); // simulate compositor death

    let result = handle.set_title(PaneTitle {
        text: "test".into(),
        short: None,
    });
    assert!(result.is_err(), "should fail when receiver is dropped");
}

// --- P1-2: Send correctness ---

#[test]
fn pane_handle_set_title_sends_correct_message() {
    let (tx, rx) = mpsc::channel();
    let handle = Messenger::new(pane_id(7), tx);

    handle.set_title(PaneTitle {
        text: "Hello".into(),
        short: Some("H".into()),
    }).unwrap();

    let msg = rx.recv().unwrap();
    match msg {
        ClientToComp::SetTitle { pane, title } => {
            assert_eq!(pane, pane_id(7));
            assert_eq!(title.text, "Hello");
            assert_eq!(title.short, Some("H".into()));
        }
        other => panic!("expected SetTitle, got {:?}", other),
    }
}

#[test]
fn pane_handle_set_vocabulary_sends_correct_message() {
    let (tx, rx) = mpsc::channel();
    let handle = Messenger::new(pane_id(3), tx);

    handle.set_vocabulary(CommandVocabulary::default()).unwrap();

    let msg = rx.recv().unwrap();
    assert!(matches!(msg, ClientToComp::SetVocabulary { pane, .. } if pane == pane_id(3)));
}

#[test]
fn pane_handle_set_content_sends_correct_message() {
    let (tx, rx) = mpsc::channel();
    let handle = Messenger::new(pane_id(5), tx);

    handle.set_content(b"hello world").unwrap();

    let msg = rx.recv().unwrap();
    match msg {
        ClientToComp::SetContent { pane, content } => {
            assert_eq!(pane, pane_id(5));
            assert_eq!(content, b"hello world");
        }
        other => panic!("expected SetContent, got {:?}", other),
    }
}

#[test]
fn pane_handle_set_completions_sends_correct_message() {
    let (tx, rx) = mpsc::channel();
    let handle = Messenger::new(pane_id(2), tx);

    handle.set_completions(42, vec![
        Completion { text: "foo".into(), description: Some("a foo".into()) },
    ]).unwrap();

    let msg = rx.recv().unwrap();
    match msg {
        ClientToComp::CompletionResponse { pane, token, completions } => {
            assert_eq!(pane, pane_id(2));
            assert_eq!(token, 42);
            assert_eq!(completions.len(), 1);
            assert_eq!(completions[0].text, "foo");
        }
        other => panic!("expected CompletionResponse, got {:?}", other),
    }
}

#[test]
fn pane_handle_clone_sends_to_same_channel() {
    let (tx, rx) = mpsc::channel();
    let handle1 = Messenger::new(pane_id(1), tx);
    let handle2 = handle1.clone();

    handle1.set_content(b"from 1").unwrap();
    handle2.set_content(b"from 2").unwrap();

    let msg1 = rx.recv().unwrap();
    let msg2 = rx.recv().unwrap();

    // Both arrived on the same channel
    assert!(matches!(msg1, ClientToComp::SetContent { content, .. } if content == b"from 1"));
    assert!(matches!(msg2, ClientToComp::SetContent { content, .. } if content == b"from 2"));
}

// --- P2-3: Identity ---

#[test]
fn pane_handle_id_matches_construction() {
    let (tx, _rx) = mpsc::channel();
    let handle = Messenger::new(pane_id(42), tx);
    assert_eq!(handle.id(), pane_id(42));
}

#[test]
fn pane_handle_clone_has_same_id() {
    let (tx, _rx) = mpsc::channel();
    let handle = Messenger::new(pane_id(99), tx);
    let cloned = handle.clone();
    assert_eq!(handle.id(), cloned.id());
}

#[test]
fn pane_handle_debug_includes_id() {
    let (tx, _rx) = mpsc::channel();
    let handle = Messenger::new(pane_id(7), tx);
    let debug = format!("{:?}", handle);
    assert!(debug.contains("7"), "debug output should include pane ID: {}", debug);
}

// --- Commit 3: Self-delivery tests ---

#[test]
fn send_message_without_looper_returns_error() {
    let (tx, _rx) = mpsc::channel::<ClientToComp>();
    let handle = Messenger::new(pane_id(1), tx);
    // No looper_tx attached — send_message should fail
    let result = handle.send_message(pane_app::Message::Focus);
    assert!(result.is_err(), "send_message without looper should return Disconnected");
}

// Happy-path self-delivery is tested in looper.rs via self_delivery_reaches_handler,
// self_delivery_interleaved_with_comp, and self_delivery_filters_apply, which exercise
// the full LooperMessage pipeline. with_looper() is pub(crate) so we can't construct
// a looper-attached PaneHandle from integration tests directly.

// --- Commit 4: Timer tests ---
// Timer methods require a looper channel (with_looper is pub(crate)), so we can only
// test the error case here. The happy path would require either exposing with_looper
// or testing through the full Pane lifecycle.

#[test]
fn send_delayed_without_looper_returns_error() {
    let (tx, _rx) = mpsc::channel::<ClientToComp>();
    let handle = Messenger::new(pane_id(1), tx);
    let result = handle.send_delayed(pane_app::Message::Focus, std::time::Duration::from_millis(10));
    assert!(result.is_err(), "send_delayed without looper should return Disconnected");
}

#[test]
fn send_periodic_without_looper_returns_error() {
    let (tx, _rx) = mpsc::channel::<ClientToComp>();
    let handle = Messenger::new(pane_id(1), tx);
    let result = handle.send_periodic(pane_app::Message::Focus, std::time::Duration::from_millis(10));
    assert!(result.is_err(), "send_periodic without looper should return Disconnected");
}
