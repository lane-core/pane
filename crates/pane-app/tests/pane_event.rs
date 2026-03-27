use std::num::NonZeroU32;

use pane_app::PaneEvent;
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
    let event = PaneEvent::from_comp(&msg, pane_id(1));
    assert!(matches!(event, Some(PaneEvent::Resize(_))));
}

#[test]
fn from_comp_resize_wrong_pane() {
    let msg = CompToClient::Resize {
        pane: pane_id(1),
        geometry: test_geometry(),
    };
    let event = PaneEvent::from_comp(&msg, pane_id(2));
    assert!(event.is_none());
}

#[test]
fn from_comp_key_event() {
    let key = KeyEvent {
        key: Key::Named(NamedKey::Escape),
        modifiers: Modifiers::empty(),
        state: KeyState::Press,
    };
    let msg = CompToClient::Key { pane: pane_id(3), event: key.clone() };
    let event = PaneEvent::from_comp(&msg, pane_id(3));
    match event {
        Some(PaneEvent::Key(k)) => {
            assert!(k.is_escape());
        }
        _ => panic!("expected Key event"),
    }
}

#[test]
fn from_comp_close() {
    let msg = CompToClient::Close { pane: pane_id(1) };
    let event = PaneEvent::from_comp(&msg, pane_id(1));
    assert!(matches!(event, Some(PaneEvent::Close)));
}

#[test]
fn from_comp_command_executed() {
    let msg = CompToClient::CommandExecuted {
        pane: pane_id(1),
        command: "save".into(),
        args: "".into(),
    };
    let event = PaneEvent::from_comp(&msg, pane_id(1));
    match event {
        Some(PaneEvent::CommandExecuted { command, args }) => {
            assert_eq!(command, "save");
            assert_eq!(args, "");
        }
        _ => panic!("expected CommandExecuted"),
    }
}

#[test]
fn from_comp_pane_created_returns_none() {
    // PaneCreated is handled internally by the kit, not forwarded
    let msg = CompToClient::PaneCreated {
        pane: pane_id(1),
        geometry: test_geometry(),
    };
    let event = PaneEvent::from_comp(&msg, pane_id(1));
    assert!(event.is_none());
}
