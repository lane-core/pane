use std::num::NonZeroU32;

use proptest::prelude::*;

use pane_proto::*;
use pane_proto::color;
use pane_proto::event;
use pane_proto::attrs::AttrValue;

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

fn arb_command_action() -> impl Strategy<Value = pane_proto::tag::CommandAction> {
    prop_oneof![
        Just(pane_proto::tag::CommandAction::BuiltIn(pane_proto::tag::BuiltIn::Close)),
        Just(pane_proto::tag::CommandAction::BuiltIn(pane_proto::tag::BuiltIn::Copy)),
        ".*".prop_map(pane_proto::tag::CommandAction::Client),
        ".*".prop_map(pane_proto::tag::CommandAction::Route),
    ]
}

fn arb_command() -> impl Strategy<Value = pane_proto::tag::Command> {
    (".*", ".*", proptest::option::of(".*"), arb_command_action())
        .prop_map(|(name, description, shortcut, action)| pane_proto::tag::Command {
            name, description, shortcut, action,
        })
}

fn arb_pane_title() -> impl Strategy<Value = pane_proto::tag::PaneTitle> {
    (".*", proptest::option::of(".*"))
        .prop_map(|(text, short)| pane_proto::tag::PaneTitle { text, short })
}

fn arb_attr_value() -> impl Strategy<Value = AttrValue> {
    prop_oneof![
        ".*".prop_map(AttrValue::String),
        any::<i64>().prop_map(AttrValue::Int),
        any::<bool>().prop_map(AttrValue::Bool),
        proptest::collection::vec(any::<u8>(), 0..=16).prop_map(AttrValue::Bytes),
    ]
}

// --- Round-trip tests for surviving types ---

proptest! {
    #[test]
    fn roundtrip_color(color in arb_color()) {
        let bytes = serialize(&color).unwrap();
        let decoded: Color = deserialize(&bytes).unwrap();
        prop_assert_eq!(color, decoded);
    }

    #[test]
    fn roundtrip_key_event(event in arb_key_event()) {
        let bytes = serialize(&event).unwrap();
        let decoded: KeyEvent = deserialize(&bytes).unwrap();
        prop_assert_eq!(event, decoded);
    }

    #[test]
    fn roundtrip_mouse_event(event in arb_mouse_event()) {
        let bytes = serialize(&event).unwrap();
        let decoded: MouseEvent = deserialize(&bytes).unwrap();
        prop_assert_eq!(event, decoded);
    }

    #[test]
    fn roundtrip_pane_title(title in arb_pane_title()) {
        let bytes = serialize(&title).unwrap();
        let decoded: pane_proto::tag::PaneTitle = deserialize(&bytes).unwrap();
        prop_assert_eq!(title, decoded);
    }

    #[test]
    fn roundtrip_command(cmd in arb_command()) {
        let bytes = serialize(&cmd).unwrap();
        let decoded: pane_proto::tag::Command = deserialize(&bytes).unwrap();
        prop_assert_eq!(cmd, decoded);
    }

    #[test]
    fn roundtrip_pane_id(id in arb_pane_id()) {
        let bytes = serialize(&id).unwrap();
        let decoded: PaneId = deserialize(&bytes).unwrap();
        prop_assert_eq!(id, decoded);
    }

    #[test]
    fn roundtrip_attr_value(val in arb_attr_value()) {
        let bytes = serialize(&val).unwrap();
        let decoded: AttrValue = deserialize(&bytes).unwrap();
        prop_assert_eq!(val, decoded);
    }
}

#[test]
fn fkey_validation() {
    assert!(event::FKey::try_from(1).is_ok());
    assert!(event::FKey::try_from(24).is_ok());
    assert!(event::FKey::try_from(0).is_err());
    assert!(event::FKey::try_from(25).is_err());
}
