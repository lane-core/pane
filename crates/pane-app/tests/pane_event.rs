use std::num::NonZeroU32;

use pane_app::PaneMessage;
use pane_proto::event::{KeyEvent, Key, NamedKey, Modifiers, KeyState};
use pane_proto::message::PaneId;
use pane_proto::protocol::{CompToClient, PaneGeometry};

fn pane_id(n: u32) -> PaneId {
    PaneId::new(NonZeroU32::new(n).unwrap())
}

fn test_geometry() -> PaneGeometry {
    PaneGeometry { width: 800, height: 600, cols: 80, rows: 24 }
}

#[test]
fn from_comp_resize_matching_pane() {
    let msg = CompToClient::Resize {
        pane: pane_id(1),
        geometry: test_geometry(),
    };
    let event = PaneMessage::from_comp(msg, pane_id(1));
    assert!(matches!(event, Some(PaneMessage::Resize(_))));
}

#[test]
fn from_comp_resize_wrong_pane() {
    let msg = CompToClient::Resize {
        pane: pane_id(1),
        geometry: test_geometry(),
    };
    let event = PaneMessage::from_comp(msg, pane_id(2));
    assert!(event.is_none());
}

#[test]
fn from_comp_key_event() {
    let key = KeyEvent {
        key: Key::Named(NamedKey::Escape),
        modifiers: Modifiers::empty(),
        state: KeyState::Press,
        timestamp: None,
    };
    let msg = CompToClient::Key { pane: pane_id(3), event: key.clone() };
    let event = PaneMessage::from_comp(msg, pane_id(3));
    match event {
        Some(PaneMessage::Key(k)) => {
            assert!(k.is_escape());
        }
        _ => panic!("expected Key event"),
    }
}

#[test]
fn from_comp_close() {
    let msg = CompToClient::Close { pane: pane_id(1) };
    let event = PaneMessage::from_comp(msg, pane_id(1));
    assert!(matches!(event, Some(PaneMessage::Close)));
}

#[test]
fn from_comp_command_executed() {
    let msg = CompToClient::CommandExecuted {
        pane: pane_id(1),
        command: "save".into(),
        args: "".into(),
    };
    let event = PaneMessage::from_comp(msg, pane_id(1));
    match event {
        Some(PaneMessage::CommandExecuted { command, args }) => {
            assert_eq!(command, "save");
            assert_eq!(args, "");
        }
        _ => panic!("expected CommandExecuted"),
    }
}

#[test]
fn from_comp_pane_created_returns_none() {
    let msg = CompToClient::PaneCreated {
        pane: pane_id(1),
        geometry: test_geometry(),
    };
    let event = PaneMessage::from_comp(msg, pane_id(1));
    assert!(event.is_none());
}

// --- P2-2: Complete PaneMessage variant coverage ---

#[test]
fn from_comp_focus() {
    let msg = CompToClient::Focus { pane: pane_id(1) };
    assert!(matches!(PaneMessage::from_comp(msg, pane_id(1)), Some(PaneMessage::Focus)));
}

#[test]
fn from_comp_blur() {
    let msg = CompToClient::Blur { pane: pane_id(1) };
    assert!(matches!(PaneMessage::from_comp(msg, pane_id(1)), Some(PaneMessage::Blur)));
}

#[test]
fn from_comp_mouse() {
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
    assert!(matches!(PaneMessage::from_comp(msg, pane_id(1)), Some(PaneMessage::Mouse(_))));
}

#[test]
fn from_comp_command_activated() {
    let msg = CompToClient::CommandActivated { pane: pane_id(1) };
    assert!(matches!(PaneMessage::from_comp(msg, pane_id(1)), Some(PaneMessage::CommandActivated)));
}

#[test]
fn from_comp_command_dismissed() {
    let msg = CompToClient::CommandDismissed { pane: pane_id(1) };
    assert!(matches!(PaneMessage::from_comp(msg, pane_id(1)), Some(PaneMessage::CommandDismissed)));
}

#[test]
fn from_comp_completion_request() {
    let msg = CompToClient::CompletionRequest {
        pane: pane_id(1),
        token: 42,
        input: "hel".into(),
    };
    match PaneMessage::from_comp(msg, pane_id(1)) {
        Some(PaneMessage::CompletionRequest { token, input }) => {
            assert_eq!(token, 42);
            assert_eq!(input, "hel");
        }
        other => panic!("expected CompletionRequest, got {:?}", other),
    }
}

#[test]
fn from_comp_all_variants_wrong_pane_returns_none() {
    let wrong = pane_id(99);
    assert!(PaneMessage::from_comp(CompToClient::Focus { pane: pane_id(1) }, wrong).is_none());
    assert!(PaneMessage::from_comp(CompToClient::Blur { pane: pane_id(1) }, wrong).is_none());
    assert!(PaneMessage::from_comp(CompToClient::Close { pane: pane_id(1) }, wrong).is_none());
    assert!(PaneMessage::from_comp(CompToClient::CommandActivated { pane: pane_id(1) }, wrong).is_none());
    assert!(PaneMessage::from_comp(CompToClient::CommandDismissed { pane: pane_id(1) }, wrong).is_none());
}
