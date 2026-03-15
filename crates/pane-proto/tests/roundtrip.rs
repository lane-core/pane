use std::num::NonZeroU32;

use proptest::prelude::*;

use pane_proto::*;
use pane_proto::color;
use pane_proto::event;
use pane_proto::message;
use pane_proto::attrs::AttrValue;
use pane_proto::server::ServerVerb;

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

fn arb_cell_region() -> impl Strategy<Value = cell::CellRegion> {
    (any::<u16>(), any::<u16>(), 1..=10u16, 1..=5u16).prop_flat_map(|(col, row, width, height)| {
        let len = width as usize * height as usize;
        proptest::collection::vec(arb_cell(), len..=len).prop_map(move |cells| {
            cell::CellRegion::new(col, row, width, height, cells).unwrap()
        })
    })
}

fn arb_modifiers() -> impl Strategy<Value = Modifiers> {
    any::<u8>().prop_map(|bits| Modifiers::from_bits_truncate(bits))
}

fn arb_fkey() -> impl Strategy<Value = event::FKey> {
    (1..=24u8).prop_map(|n| event::FKey::try_from(n).unwrap())
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
            arb_fkey().prop_map(event::NamedKey::F),
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

fn arb_route_message() -> impl Strategy<Value = RouteMessage> {
    (
        ".*",
        ".*",
        ".*",
        ".*",
        proptest::collection::vec((".*", ".*"), 0..=3),
        ".*",
    )
        .prop_map(|(src, dst, wdir, content_type, attrs, data)| RouteMessage {
            src, dst, wdir, content_type, attrs, data,
        })
}

fn arb_attr_value() -> impl Strategy<Value = AttrValue> {
    // Non-recursive for simplicity; nesting tested separately
    prop_oneof![
        ".*".prop_map(AttrValue::String),
        any::<i64>().prop_map(AttrValue::Int),
        any::<bool>().prop_map(AttrValue::Bool),
        proptest::collection::vec(any::<u8>(), 0..=16).prop_map(AttrValue::Bytes),
    ]
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
        (arb_pane_id(), arb_pane_kind())
            .prop_map(|(id, kind)| PaneEvent::Created { id, kind }),
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
            .prop_map(|(id, text)| PaneEvent::TagRoute { id, text }),
        arb_route_message().prop_map(|message| PaneEvent::Route { message }),
    ]
}

fn arb_pane_message<T: std::fmt::Debug + 'static>(
    core_strategy: impl Strategy<Value = T> + 'static,
) -> impl Strategy<Value = PaneMessage<T>> {
    (core_strategy, proptest::collection::vec((".*", arb_attr_value()), 0..=3))
        .prop_map(|(core, attrs)| PaneMessage::with_attrs(core, attrs))
}

// --- Round-trip tests ---

proptest! {
    #[test]
    fn roundtrip_pane_request(msg in arb_pane_message(arb_pane_request())) {
        let bytes = serialize(&msg).unwrap();
        let decoded: PaneMessage<PaneRequest> = deserialize(&bytes).unwrap();
        prop_assert_eq!(msg, decoded);
    }

    #[test]
    fn roundtrip_pane_event(msg in arb_pane_message(arb_pane_event())) {
        let bytes = serialize(&msg).unwrap();
        let decoded: PaneMessage<PaneEvent> = deserialize(&bytes).unwrap();
        prop_assert_eq!(msg, decoded);
    }

    #[test]
    fn roundtrip_cell(cell in arb_cell()) {
        let bytes = serialize(&cell).unwrap();
        let decoded: Cell = deserialize(&bytes).unwrap();
        prop_assert_eq!(cell, decoded);
    }

    #[test]
    fn roundtrip_route_message(msg in arb_route_message()) {
        let bytes = serialize(&msg).unwrap();
        let decoded: RouteMessage = deserialize(&bytes).unwrap();
        prop_assert_eq!(msg, decoded);
    }

    #[test]
    fn roundtrip_attr_value(val in arb_attr_value()) {
        let bytes = serialize(&val).unwrap();
        let decoded: AttrValue = deserialize(&bytes).unwrap();
        prop_assert_eq!(val, decoded);
    }

    #[test]
    fn roundtrip_server_verb_message(
        msg in arb_pane_message(
            prop_oneof![
                Just(ServerVerb::Query),
                Just(ServerVerb::Notify),
                Just(ServerVerb::Command),
            ]
        )
    ) {
        let bytes = serialize(&msg).unwrap();
        let decoded: PaneMessage<ServerVerb> = deserialize(&bytes).unwrap();
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
            match state.apply(req) {
                Ok(new_state) => {
                    if matches!(req, PaneRequest::Create { .. }) {
                        let fake_id = PaneId::new(NonZeroU32::new(
                            (state_pane_count(&new_state) + 1) as u32
                        ).unwrap());
                        state = new_state.activate(fake_id, message::PaneKind::CellGrid)
                            .unwrap_or(new_state);
                    } else {
                        state = new_state;
                    }
                }
                Err(_) => {}
            }
        }
    }
}

fn state_pane_count(state: &ProtocolState) -> usize {
    match state {
        ProtocolState::Disconnected => 0,
        ProtocolState::Active { panes, .. } => panes.len(),
    }
}

#[test]
fn state_machine_multi_pane() {
    let state = ProtocolState::Disconnected;
    let state = state.connect().unwrap();

    // Create first pane
    let state = state
        .apply(&PaneRequest::Create {
            name: "shell".into(),
            kind: message::PaneKind::CellGrid,
        })
        .unwrap();
    let id1 = PaneId::new(NonZeroU32::new(1).unwrap());
    let state = state.activate(id1, message::PaneKind::CellGrid).unwrap();

    // Create second pane
    let state = state
        .apply(&PaneRequest::Create {
            name: "editor".into(),
            kind: message::PaneKind::CellGrid,
        })
        .unwrap();
    let id2 = PaneId::new(NonZeroU32::new(2).unwrap());
    let state = state.activate(id2, message::PaneKind::CellGrid).unwrap();

    // Both panes are tracked
    assert_eq!(state_pane_count(&state), 2);

    // Close first pane
    let state = state.apply(&PaneRequest::Close { id: id1 }).unwrap();
    assert_eq!(state_pane_count(&state), 1);

    // Second pane still works
    let state = state
        .apply(&PaneRequest::SetTag {
            id: id2,
            text: "test".into(),
        })
        .unwrap();
    assert_eq!(state_pane_count(&state), 1);
}

#[test]
fn state_machine_connect_errors_when_active() {
    let state = ProtocolState::Disconnected;
    let state = state.connect().unwrap();
    assert!(state.connect().is_err());
}

#[test]
fn state_machine_rejects_write_to_unknown_pane() {
    let state = ProtocolState::Disconnected;
    let state = state.connect().unwrap();

    let id = PaneId::new(NonZeroU32::new(1).unwrap());
    let result = state.apply(&PaneRequest::WriteCells {
        id,
        region: cell::CellRegion::new(0, 0, 1, 1, vec![Cell::default()]).unwrap(),
    });
    assert!(result.is_err());
}

#[test]
fn state_machine_rejects_write_to_surface_pane() {
    let state = ProtocolState::Disconnected;
    let state = state.connect().unwrap();

    let state = state
        .apply(&PaneRequest::Create {
            name: "surf".into(),
            kind: message::PaneKind::Surface,
        })
        .unwrap();
    let id = PaneId::new(NonZeroU32::new(1).unwrap());
    let state = state.activate(id, message::PaneKind::Surface).unwrap();

    let result = state.apply(&PaneRequest::WriteCells {
        id,
        region: cell::CellRegion::new(0, 0, 1, 1, vec![Cell::default()]).unwrap(),
    });
    assert!(result.is_err());
}

#[test]
fn state_machine_activate_without_pending_errors() {
    let state = ProtocolState::Disconnected;
    let state = state.connect().unwrap();

    let id = PaneId::new(NonZeroU32::new(1).unwrap());
    assert!(state.activate(id, message::PaneKind::CellGrid).is_err());
}

#[test]
fn cell_region_validation() {
    // Valid
    let region = cell::CellRegion::new(0, 0, 2, 3, vec![Cell::default(); 6]);
    assert!(region.is_ok());

    // Invalid: wrong count
    let region = cell::CellRegion::new(0, 0, 2, 3, vec![Cell::default(); 5]);
    assert!(region.is_err());

    // Valid: empty
    let region = cell::CellRegion::new(0, 0, 0, 0, vec![]);
    assert!(region.is_ok());
}

#[test]
fn fkey_validation() {
    assert!(event::FKey::try_from(1).is_ok());
    assert!(event::FKey::try_from(24).is_ok());
    assert!(event::FKey::try_from(0).is_err());
    assert!(event::FKey::try_from(25).is_err());
}

#[test]
fn pane_message_attrs() {
    let mut msg = PaneMessage::new(PaneRequest::Create {
        name: "test".into(),
        kind: message::PaneKind::CellGrid,
    });

    assert!(msg.attr("cwd").is_none());

    msg.set_attr("cwd", AttrValue::String("/home/lane".into()));
    assert_eq!(
        msg.attr("cwd").and_then(|v| v.as_str()),
        Some("/home/lane")
    );

    // insert allows duplicates
    msg.insert_attr("ref", AttrValue::String("file1.rs".into()));
    msg.insert_attr("ref", AttrValue::String("file2.rs".into()));
    assert_eq!(msg.attrs_all("ref").count(), 2);
}
