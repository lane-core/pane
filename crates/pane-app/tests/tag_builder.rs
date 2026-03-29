use pane_app::{Tag, cmd, Builtin};
use pane_proto::tag::{CommandAction, CommandGroup};

#[test]
fn tag_minimal() {
    let tag = Tag::new("Hello");
    let wire = tag.into_wire();
    assert_eq!(wire.title.text, "Hello");
    assert!(wire.title.short.is_none());
    assert!(wire.vocabulary.groups.is_empty());
}

#[test]
fn tag_with_short_title() {
    let tag = Tag::new("Weather: San Francisco").short("SF");
    let wire = tag.into_wire();
    assert_eq!(wire.title.text, "Weather: San Francisco");
    assert_eq!(wire.title.short, Some("SF".to_string()));
}

#[test]
fn tag_with_commands() {
    let tag = Tag::new("Editor").commands(vec![
        cmd("save", "Save file")
            .shortcut("Ctrl+S")
            .client("save"),
        cmd("close", "Close pane")
            .shortcut("Alt+W")
            .built_in(Builtin::Close),
    ]);
    let wire = tag.into_wire();

    assert_eq!(wire.vocabulary.groups.len(), 1);
    assert_eq!(wire.vocabulary.groups[0].label, "Commands");
    assert_eq!(wire.vocabulary.groups[0].commands.len(), 2);

    let save = &wire.vocabulary.groups[0].commands[0];
    assert_eq!(save.name, "save");
    assert_eq!(save.description, "Save file");
    assert_eq!(save.shortcut, Some("Ctrl+S".to_string()));
    assert!(matches!(save.action, CommandAction::Client(ref s) if s == "save"));

    let close = &wire.vocabulary.groups[0].commands[1];
    assert_eq!(close.name, "close");
    assert!(matches!(close.action, CommandAction::Builtin(Builtin::Close)));
}

#[test]
fn tag_with_explicit_groups() {
    let tag = Tag::new("IDE").groups(vec![
        CommandGroup {
            label: "File".into(),
            commands: vec![
                cmd("save", "Save").client("save"),
                cmd("close", "Close").built_in(Builtin::Close),
            ],
        },
        CommandGroup {
            label: "Build".into(),
            commands: vec![
                cmd("build", "Build project").client("build"),
            ],
        },
    ]);
    let wire = tag.into_wire();

    assert_eq!(wire.vocabulary.groups.len(), 2);
    assert_eq!(wire.vocabulary.groups[0].label, "File");
    assert_eq!(wire.vocabulary.groups[0].commands.len(), 2);
    assert_eq!(wire.vocabulary.groups[1].label, "Build");
    assert_eq!(wire.vocabulary.groups[1].commands.len(), 1);
}

#[test]
fn cmd_route_action() {
    let c = cmd("open", "Open file").route("edit $file");
    assert!(matches!(c.action, CommandAction::Route(ref s) if s == "edit $file"));
}

// --- P2-4: Tag/Command builder edge cases ---

#[test]
fn tag_empty_commands() {
    let tag = Tag::new("Empty").commands(vec![]);
    let wire = tag.into_wire();
    // Empty commands should produce one group with zero commands
    assert_eq!(wire.vocabulary.groups.len(), 1);
    assert_eq!(wire.vocabulary.groups[0].commands.len(), 0);
}

#[test]
fn cmd_no_shortcut() {
    let c = cmd("save", "Save file").client("save");
    assert!(c.shortcut.is_none());
}

#[test]
fn tag_unicode_title() {
    let tag = Tag::new("日本語テスト").short("テスト");
    let wire = tag.into_wire();
    assert_eq!(wire.title.text, "日本語テスト");
    assert_eq!(wire.title.short, Some("テスト".to_string()));
}

// --- Commit 1: Command enabled/disabled ---

#[test]
fn cmd_enabled_default_true() {
    let c = cmd("save", "Save file").client("save");
    assert!(c.enabled);
}

#[test]
fn cmd_enabled_false() {
    let c = cmd("undo", "Undo").enabled(false).client("undo");
    assert!(!c.enabled);
}
