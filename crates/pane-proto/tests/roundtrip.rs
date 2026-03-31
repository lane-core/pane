use proptest::prelude::*;

use pane_proto::*;
use pane_proto::color;
use pane_proto::event;
use pane_proto::attrs::AttrValue;

// --- Arbitrary strategies ---

fn arb_pane_id() -> impl Strategy<Value = PaneId> {
    any::<u128>().prop_map(|n| PaneId::from_uuid(uuid::Uuid::from_u128(n)))
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
    (arb_key(), arb_modifiers(), prop_oneof![Just(event::KeyState::Press), Just(event::KeyState::Release)], proptest::option::of(any::<u64>()))
        .prop_map(|(key, modifiers, state, timestamp)| KeyEvent { key, modifiers, state, timestamp })
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
    (any::<u16>(), any::<u16>(), arb_mouse_event_kind(), arb_modifiers(), proptest::option::of(any::<u64>()))
        .prop_map(|(col, row, kind, modifiers, timestamp)| MouseEvent { col, row, kind, modifiers, timestamp })
}

fn arb_command() -> impl Strategy<Value = pane_proto::tag::Command> {
    (".*", ".*", proptest::option::of(".*"), any::<bool>())
        .prop_map(|(name, description, shortcut, enabled)| pane_proto::tag::Command {
            name, description, shortcut, enabled,
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
        any::<f64>().prop_filter("not NaN", |f| !f.is_nan()).prop_map(AttrValue::Float),
        any::<bool>().prop_map(AttrValue::Bool),
        proptest::collection::vec(any::<u8>(), 0..=16).prop_map(AttrValue::Bytes),
    ]
}

fn arb_completion() -> impl Strategy<Value = pane_proto::tag::Completion> {
    (".*", proptest::option::of(".*"))
        .prop_map(|(text, description)| pane_proto::tag::Completion { text, description })
}

fn arb_command_group() -> impl Strategy<Value = pane_proto::tag::CommandGroup> {
    (".*", proptest::collection::vec(arb_command(), 0..=3))
        .prop_map(|(label, commands)| pane_proto::tag::CommandGroup { label, commands })
}

fn arb_command_vocabulary() -> impl Strategy<Value = pane_proto::tag::CommandVocabulary> {
    proptest::collection::vec(arb_command_group(), 0..=3)
        .prop_map(|groups| pane_proto::tag::CommandVocabulary { groups })
}

fn arb_create_pane_tag() -> impl Strategy<Value = pane_proto::protocol::CreatePaneTag> {
    (arb_pane_title(), arb_command_vocabulary())
        .prop_map(|(title, vocabulary)| pane_proto::protocol::CreatePaneTag { title, vocabulary })
}

fn arb_pane_geometry() -> impl Strategy<Value = pane_proto::PaneGeometry> {
    (any::<u32>(), any::<u32>(), any::<u16>(), any::<u16>())
        .prop_map(|(width, height, cols, rows)| pane_proto::PaneGeometry { width, height, cols, rows })
}

fn arb_client_to_comp() -> impl Strategy<Value = pane_proto::protocol::ClientToComp> {
    use pane_proto::protocol::ClientToComp;
    prop_oneof![
        (arb_pane_id(), proptest::option::of(arb_create_pane_tag())).prop_map(|(pane, tag)| ClientToComp::CreatePane { pane, tag }),
        arb_pane_id().prop_map(|pane| ClientToComp::RequestClose { pane }),
        (arb_pane_id(), arb_pane_title()).prop_map(|(pane, title)| ClientToComp::SetTitle { pane, title }),
        (arb_pane_id(), arb_command_vocabulary()).prop_map(|(pane, vocabulary)| ClientToComp::SetVocabulary { pane, vocabulary }),
        (arb_pane_id(), proptest::collection::vec(any::<u8>(), 0..=32)).prop_map(|(pane, content)| ClientToComp::SetContent { pane, content }),
        (arb_pane_id(), any::<u64>(), proptest::collection::vec(arb_completion(), 0..=3))
            .prop_map(|(pane, token, completions)| ClientToComp::CompletionResponse { pane, token, completions }),
    ]
}

fn arb_comp_to_client() -> impl Strategy<Value = pane_proto::protocol::CompToClient> {
    use pane_proto::protocol::CompToClient;
    prop_oneof![
        (arb_pane_id(), arb_pane_geometry()).prop_map(|(pane, geometry)| CompToClient::PaneCreated { pane, geometry }),
        (arb_pane_id(), arb_pane_geometry()).prop_map(|(pane, geometry)| CompToClient::Resize { pane, geometry }),
        arb_pane_id().prop_map(|pane| CompToClient::Focus { pane }),
        arb_pane_id().prop_map(|pane| CompToClient::Blur { pane }),
        (arb_pane_id(), arb_key_event()).prop_map(|(pane, event)| CompToClient::Key { pane, event }),
        (arb_pane_id(), arb_mouse_event()).prop_map(|(pane, event)| CompToClient::Mouse { pane, event }),
        arb_pane_id().prop_map(|pane| CompToClient::Close { pane }),
        arb_pane_id().prop_map(|pane| CompToClient::CloseAck { pane }),
        arb_pane_id().prop_map(|pane| CompToClient::CommandActivated { pane }),
        arb_pane_id().prop_map(|pane| CompToClient::CommandDismissed { pane }),
        (arb_pane_id(), ".*", ".*").prop_map(|(pane, command, args)| CompToClient::CommandExecuted { pane, command, args }),
        (arb_pane_id(), any::<u64>(), ".*").prop_map(|(pane, token, input)| CompToClient::CompletionRequest { pane, token, input }),
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

    #[test]
    fn roundtrip_pane_geometry(
        width in any::<u32>(),
        height in any::<u32>(),
        cols in any::<u16>(),
        rows in any::<u16>(),
    ) {
        let geom = pane_proto::PaneGeometry { width, height, cols, rows };
        let bytes = serialize(&geom).unwrap();
        let decoded: pane_proto::PaneGeometry = deserialize(&bytes).unwrap();
        prop_assert_eq!(geom, decoded);
    }

    #[test]
    fn roundtrip_client_hello(sig in ".*", ver in any::<u32>()) {
        let hello = pane_proto::ClientHello { signature: sig, version: ver, identity: None };
        let bytes = serialize(&hello).unwrap();
        let decoded: pane_proto::ClientHello = deserialize(&bytes).unwrap();
        prop_assert_eq!(hello, decoded);
    }

    #[test]
    fn roundtrip_server_hello(comp in ".*", ver in any::<u32>()) {
        let hello = pane_proto::ServerHello { compositor: comp, version: ver, instance_id: "test".into() };
        let bytes = serialize(&hello).unwrap();
        let decoded: pane_proto::ServerHello = deserialize(&bytes).unwrap();
        prop_assert_eq!(hello, decoded);
    }

    #[test]
    fn roundtrip_client_to_comp(msg in arb_client_to_comp()) {
        let bytes = serialize(&msg).unwrap();
        let decoded: pane_proto::protocol::ClientToComp = deserialize(&bytes).unwrap();
        prop_assert_eq!(msg, decoded);
    }

    #[test]
    fn roundtrip_comp_to_client(msg in arb_comp_to_client()) {
        let bytes = serialize(&msg).unwrap();
        let decoded: pane_proto::protocol::CompToClient = deserialize(&bytes).unwrap();
        prop_assert_eq!(msg, decoded);
    }
}

#[test]
fn fkey_validation() {
    assert!(event::FKey::try_from(1).is_ok());
    assert!(event::FKey::try_from(24).is_ok());
    assert!(event::FKey::try_from(0).is_err());
    assert!(event::FKey::try_from(25).is_err());
}

#[test]
fn key_event_is_escape() {
    let esc = pane_proto::KeyEvent {
        key: pane_proto::Key::Named(pane_proto::event::NamedKey::Escape),
        modifiers: pane_proto::event::Modifiers::empty(),
        state: pane_proto::event::KeyState::Press,
        timestamp: None,
    };
    assert!(esc.is_escape());

    let not_esc = pane_proto::KeyEvent {
        key: pane_proto::Key::Char('a'),
        modifiers: pane_proto::event::Modifiers::empty(),
        state: pane_proto::event::KeyState::Press,
        timestamp: None,
    };
    assert!(!not_esc.is_escape());

    // Release of Escape is not "is_escape" (only press)
    let esc_release = pane_proto::KeyEvent {
        key: pane_proto::Key::Named(pane_proto::event::NamedKey::Escape),
        modifiers: pane_proto::event::Modifiers::empty(),
        state: pane_proto::event::KeyState::Release,
        timestamp: None,
    };
    assert!(!esc_release.is_escape());
}

#[test]
fn roundtrip_key_event_with_timestamp() {
    let event = pane_proto::KeyEvent {
        key: pane_proto::Key::Char('x'),
        modifiers: pane_proto::event::Modifiers::empty(),
        state: pane_proto::event::KeyState::Press,
        timestamp: Some(1711612800_000_000),
    };
    let bytes = serialize(&event).unwrap();
    let decoded: pane_proto::KeyEvent = deserialize(&bytes).unwrap();
    assert_eq!(event, decoded);
    assert_eq!(decoded.timestamp, Some(1711612800_000_000));
}

#[test]
fn roundtrip_command_enabled_false() {
    let cmd = pane_proto::tag::Command {
        name: "disabled-cmd".into(),
        description: "A disabled command".into(),
        shortcut: None,
        enabled: false,
    };
    let bytes = serialize(&cmd).unwrap();
    let decoded: pane_proto::tag::Command = deserialize(&bytes).unwrap();
    assert_eq!(cmd, decoded);
    assert!(!decoded.enabled);
}
