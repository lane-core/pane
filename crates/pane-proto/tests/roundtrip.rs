use std::num::NonZeroU32;

use proptest::prelude::*;

use pane_proto::*;

// --- Arbitrary strategies ---

fn arb_pane_id() -> impl Strategy<Value = PaneId> {
    (1..=u32::MAX).prop_map(|n| PaneId::new(NonZeroU32::new(n).unwrap()))
}

fn arb_named_color() -> impl Strategy<Value = color::NamedColor> {
    prop_oneof![
        Just(color::NamedColor::Black),
        Just(color::NamedColor::Red),
        Just(color::NamedColor::Green),
        Just(color::NamedColor::Yellow),
        Just(color::NamedColor::Blue),
        Just(color::NamedColor::Magenta),
        Just(color::NamedColor::Cyan),
        Just(color::NamedColor::White),
        Just(color::NamedColor::BrightBlack),
        Just(color::NamedColor::BrightRed),
        Just(color::NamedColor::BrightGreen),
        Just(color::NamedColor::BrightYellow),
        Just(color::NamedColor::BrightBlue),
        Just(color::NamedColor::BrightMagenta),
        Just(color::NamedColor::BrightCyan),
        Just(color::NamedColor::BrightWhite),
    ]
}

fn arb_color() -> impl Strategy<Value = Color> {
    prop_oneof![
        Just(Color::Default),
        arb_named_color().prop_map(Color::Named),
        any::<u8>().prop_map(Color::Indexed),
        (any::<u8>(), any::<u8>(), any::<u8>()).prop_map(|(r, g, b)| Color::Rgb(r, g, b)),
    ]
}

fn arb_cell_attrs() -> impl Strategy<Value = CellAttrs> {
    any::<u8>().prop_map(|bits| CellAttrs::from_bits_truncate(bits))
}

fn arb_cell() -> impl Strategy<Value = Cell> {
    (any::<char>(), arb_color(), arb_color(), arb_cell_attrs())
        .prop_map(|(ch, fg, bg, attrs)| Cell { ch, fg, bg, attrs })
}

fn arb_cell_region() -> impl Strategy<Value = CellRegion> {
    (any::<u16>(), any::<u16>(), 1..=20u16).prop_flat_map(|(col, row, width)| {
        let len = width as usize * 3; // up to 3 rows
        proptest::collection::vec(arb_cell(), 0..=len).prop_map(move |cells| CellRegion {
            col,
            row,
            width,
            cells,
        })
    })
}

fn arb_modifiers() -> impl Strategy<Value = Modifiers> {
    any::<u8>().prop_map(|bits| Modifiers::from_bits_truncate(bits))
}

fn arb_key() -> impl Strategy<Value = Key> {
    prop_oneof![
        any::<char>().prop_map(Key::Char),
        prop_oneof![
            Just(event::NamedKey::Enter),
            Just(event::NamedKey::Tab),
            Just(event::NamedKey::Backspace),
            Just(event::NamedKey::Escape),
            Just(event::NamedKey::Delete),
            Just(event::NamedKey::Home),
            Just(event::NamedKey::End),
            Just(event::NamedKey::PageUp),
            Just(event::NamedKey::PageDown),
            Just(event::NamedKey::Up),
            Just(event::NamedKey::Down),
            Just(event::NamedKey::Left),
            Just(event::NamedKey::Right),
            (1..=12u8).prop_map(event::NamedKey::F),
            Just(event::NamedKey::Insert),
        ]
        .prop_map(Key::Named),
    ]
}

fn arb_key_event() -> impl Strategy<Value = KeyEvent> {
    (arb_key(), arb_modifiers(), prop_oneof![Just(event::KeyState::Press), Just(event::KeyState::Release)])
        .prop_map(|(key, modifiers, state)| KeyEvent { key, modifiers, state })
}

fn arb_mouse_button() -> impl Strategy<Value = MouseButton> {
    prop_oneof![
        Just(MouseButton::Left),
        Just(MouseButton::Middle),
        Just(MouseButton::Right),
        Just(MouseButton::Back),
        Just(MouseButton::Forward),
    ]
}

fn arb_mouse_event_kind() -> impl Strategy<Value = MouseEventKind> {
    prop_oneof![
        arb_mouse_button().prop_map(MouseEventKind::Press),
        arb_mouse_button().prop_map(MouseEventKind::Release),
        Just(MouseEventKind::Move),
        Just(MouseEventKind::ScrollUp),
        Just(MouseEventKind::ScrollDown),
    ]
}

fn arb_mouse_event() -> impl Strategy<Value = MouseEvent> {
    (any::<u16>(), any::<u16>(), arb_mouse_event_kind(), arb_modifiers())
        .prop_map(|(col, row, kind, modifiers)| MouseEvent { col, row, kind, modifiers })
}

fn arb_pane_kind() -> impl Strategy<Value = message::PaneKind> {
    prop_oneof![Just(message::PaneKind::CellGrid), Just(message::PaneKind::Surface)]
}

fn arb_plumb_message() -> impl Strategy<Value = PlumbMessage> {
    (
        ".*",
        ".*",
        ".*",
        ".*",
        proptest::collection::vec((".*", ".*"), 0..=3),
        ".*",
    )
        .prop_map(|(src, dst, wdir, content_type, attrs, data)| PlumbMessage {
            src,
            dst,
            wdir,
            content_type,
            attrs,
            data,
        })
}

fn arb_pane_request() -> impl Strategy<Value = PaneRequest> {
    prop_oneof![
        (".*", arb_pane_kind())
            .prop_map(|(name, kind)| PaneRequest::Create { name, kind }),
        arb_pane_id().prop_map(|id| PaneRequest::Close { id }),
        (arb_pane_id(), arb_cell_region())
            .prop_map(|(id, region)| PaneRequest::WriteCells { id, region }),
        (arb_pane_id(), any::<i32>())
            .prop_map(|(id, delta)| PaneRequest::Scroll { id, delta }),
        (arb_pane_id(), ".*")
            .prop_map(|(id, text)| PaneRequest::SetTag { id, text }),
        (arb_pane_id(), any::<bool>())
            .prop_map(|(id, dirty)| PaneRequest::SetDirty { id, dirty }),
        (arb_pane_id(), any::<u16>(), any::<u16>())
            .prop_map(|(id, cols, rows)| PaneRequest::RequestGeometry { id, cols, rows }),
    ]
}

fn arb_pane_event() -> impl Strategy<Value = PaneEvent> {
    prop_oneof![
        arb_pane_id().prop_map(|id| PaneEvent::Created { id }),
        (arb_pane_id(), arb_key_event())
            .prop_map(|(id, event)| PaneEvent::Key { id, event }),
        (arb_pane_id(), arb_mouse_event())
            .prop_map(|(id, event)| PaneEvent::Mouse { id, event }),
        (arb_pane_id(), any::<u16>(), any::<u16>())
            .prop_map(|(id, cols, rows)| PaneEvent::Resize { id, cols, rows }),
        (arb_pane_id(), any::<bool>())
            .prop_map(|(id, focused)| PaneEvent::Focus { id, focused }),
        arb_pane_id().prop_map(|id| PaneEvent::CloseRequested { id }),
        (arb_pane_id(), ".*")
            .prop_map(|(id, text)| PaneEvent::TagExecute { id, text }),
        (arb_pane_id(), ".*")
            .prop_map(|(id, text)| PaneEvent::TagPlumb { id, text }),
        arb_plumb_message().prop_map(|message| PaneEvent::Plumb { message }),
    ]
}

// --- Round-trip tests ---

proptest! {
    #[test]
    fn roundtrip_pane_request(req in arb_pane_request()) {
        let bytes = serialize(&req).unwrap();
        let decoded: PaneRequest = deserialize(&bytes).unwrap();
        prop_assert_eq!(req, decoded);
    }

    #[test]
    fn roundtrip_pane_event(evt in arb_pane_event()) {
        let bytes = serialize(&evt).unwrap();
        let decoded: PaneEvent = deserialize(&bytes).unwrap();
        prop_assert_eq!(evt, decoded);
    }

    #[test]
    fn roundtrip_cell(cell in arb_cell()) {
        let bytes = serialize(&cell).unwrap();
        let decoded: Cell = deserialize(&bytes).unwrap();
        prop_assert_eq!(cell, decoded);
    }

    #[test]
    fn roundtrip_plumb_message(msg in arb_plumb_message()) {
        let bytes = serialize(&msg).unwrap();
        let decoded: PlumbMessage = deserialize(&bytes).unwrap();
        prop_assert_eq!(msg, decoded);
    }
}

// --- State machine tests ---

proptest! {
    #[test]
    fn state_machine_no_panics(requests in proptest::collection::vec(arb_pane_request(), 0..50)) {
        let mut state = ProtocolState::Disconnected;
        state = state.connect().unwrap();

        for req in &requests {
            // apply may return Ok or Err, but must never panic
            match state.apply(req) {
                Ok(new_state) => {
                    // If Create succeeded, simulate compositor response
                    if matches!(req, PaneRequest::Create { .. }) {
                        let fake_id = PaneId::new(NonZeroU32::new(1).unwrap());
                        state = new_state.activate(fake_id).unwrap_or(new_state);
                    } else {
                        state = new_state;
                    }
                }
                Err(_) => {
                    // State unchanged on error — this is correct
                }
            }
        }
    }
}

#[test]
fn state_machine_create_then_close() {
    let state = ProtocolState::Disconnected;
    let state = state.connect().unwrap();
    assert_eq!(state, ProtocolState::Connected);

    // Create is valid when Connected
    let state = state
        .apply(&PaneRequest::Create {
            name: "test".into(),
            kind: message::PaneKind::CellGrid,
        })
        .unwrap();

    // Simulate compositor assigning an id
    let id = PaneId::new(NonZeroU32::new(42).unwrap());
    let state = state.activate(id).unwrap();
    assert_eq!(state, ProtocolState::Active { pane_id: id });

    // Close returns to Connected
    let state = state.apply(&PaneRequest::Close { id }).unwrap();
    assert_eq!(state, ProtocolState::Connected);
}

#[test]
fn state_machine_rejects_write_before_create() {
    let state = ProtocolState::Disconnected;
    let state = state.connect().unwrap();

    let id = PaneId::new(NonZeroU32::new(1).unwrap());
    let result = state.apply(&PaneRequest::WriteCells {
        id,
        region: CellRegion {
            col: 0,
            row: 0,
            width: 1,
            cells: vec![],
        },
    });

    assert!(result.is_err());
}

// Need to re-export the inner modules for test access
use pane_proto::color;
use pane_proto::event;
use pane_proto::message;
