use std::sync::mpsc;

use pane_app::Message;
use pane_proto::event::{KeyEvent, Key, NamedKey, Modifiers, KeyState};
use pane_proto::message::PaneId;
use pane_proto::protocol::{CompToClient, ClientToComp, PaneGeometry};

fn pane_id(n: u32) -> PaneId {
    PaneId::from_uuid(uuid::Uuid::from_u128(n as u128))
}

fn test_geometry() -> PaneGeometry {
    PaneGeometry { width: 800, height: 600, cols: 80, rows: 24 }
}

/// Dummy compositor sender for tests that don't exercise completions.
fn dummy_sender() -> mpsc::Sender<ClientToComp> {
    let (tx, _rx) = mpsc::channel();
    tx
}

#[test]
fn try_from_comp_resize_matching_pane() {
    let msg = CompToClient::Resize {
        pane: pane_id(1),
        geometry: test_geometry(),
    };
    let event = Message::try_from_comp(msg, pane_id(1), &dummy_sender());
    assert!(matches!(event, Some(Message::Resize(_))));
}

#[test]
fn try_from_comp_resize_wrong_pane() {
    let msg = CompToClient::Resize {
        pane: pane_id(1),
        geometry: test_geometry(),
    };
    let event = Message::try_from_comp(msg, pane_id(2), &dummy_sender());
    assert!(event.is_none());
}

#[test]
fn try_from_comp_key_event() {
    let key = KeyEvent {
        key: Key::Named(NamedKey::Escape),
        modifiers: Modifiers::empty(),
        state: KeyState::Press,
        timestamp: None,
    };
    let msg = CompToClient::Key { pane: pane_id(3), event: key.clone() };
    let event = Message::try_from_comp(msg, pane_id(3), &dummy_sender());
    match event {
        Some(Message::Key(k)) => {
            assert!(k.is_escape());
        }
        _ => panic!("expected Key event"),
    }
}

#[test]
fn try_from_comp_close() {
    let msg = CompToClient::Close { pane: pane_id(1) };
    let event = Message::try_from_comp(msg, pane_id(1), &dummy_sender());
    assert!(matches!(event, Some(Message::CloseRequested)));
}

#[test]
fn try_from_comp_command_executed() {
    let msg = CompToClient::CommandExecuted {
        pane: pane_id(1),
        command: "save".into(),
        args: "".into(),
    };
    let event = Message::try_from_comp(msg, pane_id(1), &dummy_sender());
    match event {
        Some(Message::CommandExecuted { command, args }) => {
            assert_eq!(command, "save");
            assert_eq!(args, "");
        }
        _ => panic!("expected CommandExecuted"),
    }
}

#[test]
fn try_from_comp_pane_created_returns_none() {
    let msg = CompToClient::PaneCreated {
        pane: pane_id(1),
        geometry: test_geometry(),
    };
    let event = Message::try_from_comp(msg, pane_id(1), &dummy_sender());
    assert!(event.is_none());
}

// --- P2-2: Complete Message variant coverage ---

#[test]
fn try_from_comp_focus() {
    let msg = CompToClient::Focus { pane: pane_id(1) };
    assert!(matches!(Message::try_from_comp(msg, pane_id(1), &dummy_sender()), Some(Message::Activated)));
}

#[test]
fn try_from_comp_blur() {
    let msg = CompToClient::Blur { pane: pane_id(1) };
    assert!(matches!(Message::try_from_comp(msg, pane_id(1), &dummy_sender()), Some(Message::Deactivated)));
}

#[test]
fn try_from_comp_mouse() {
    use pane_proto::event::{MouseEvent, MouseButton, MouseEventKind, Modifiers};
    let msg = CompToClient::Mouse {
        pane: pane_id(1),
        event: MouseEvent {
            col: 10, row: 5,
            kind: MouseEventKind::Press(MouseButton::Left),
            modifiers: Modifiers::empty(),
            timestamp: None,
        },
    };
    assert!(matches!(Message::try_from_comp(msg, pane_id(1), &dummy_sender()), Some(Message::Mouse(_))));
}

#[test]
fn try_from_comp_command_activated() {
    let msg = CompToClient::CommandActivated { pane: pane_id(1) };
    assert!(matches!(Message::try_from_comp(msg, pane_id(1), &dummy_sender()), Some(Message::CommandActivated)));
}

#[test]
fn try_from_comp_command_dismissed() {
    let msg = CompToClient::CommandDismissed { pane: pane_id(1) };
    assert!(matches!(Message::try_from_comp(msg, pane_id(1), &dummy_sender()), Some(Message::CommandDismissed)));
}

#[test]
fn try_from_comp_completion_request() {
    let (tx, rx) = mpsc::channel();
    let msg = CompToClient::CompletionRequest {
        pane: pane_id(1),
        token: 42,
        input: "hel".into(),
    };
    match Message::try_from_comp(msg, pane_id(1), &tx) {
        Some(Message::CompletionRequest { input, reply }) => {
            assert_eq!(input, "hel");
            // Reply port should send CompletionResponse back through tx
            reply.reply(vec![]);
            let resp = rx.recv().unwrap();
            match resp {
                ClientToComp::CompletionResponse { token, completions, .. } => {
                    assert_eq!(token, 42);
                    assert!(completions.is_empty());
                }
                other => panic!("expected CompletionResponse, got {:?}", other),
            }
        }
        other => panic!("expected CompletionRequest, got {:?}", other),
    }
}

#[test]
fn try_from_comp_all_variants_wrong_pane_returns_none() {
    let s = &dummy_sender();
    let wrong = pane_id(99);
    assert!(Message::try_from_comp(CompToClient::Focus { pane: pane_id(1) }, wrong, s).is_none());
    assert!(Message::try_from_comp(CompToClient::Blur { pane: pane_id(1) }, wrong, s).is_none());
    assert!(Message::try_from_comp(CompToClient::Close { pane: pane_id(1) }, wrong, s).is_none());
    assert!(Message::try_from_comp(CompToClient::CommandActivated { pane: pane_id(1) }, wrong, s).is_none());
    assert!(Message::try_from_comp(CompToClient::CommandDismissed { pane: pane_id(1) }, wrong, s).is_none());
}
