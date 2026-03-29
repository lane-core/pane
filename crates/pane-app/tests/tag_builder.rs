use pane_app::{Tag, cmd};
use pane_proto::tag::CommandGroup;

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
    let tag = Tag::new("Editor")
        .command(cmd("save", "Save file").shortcut("Ctrl+S"))
        .command(cmd("close", "Close pane").shortcut("Alt+W"));
    let wire = tag.into_wire();

    assert_eq!(wire.vocabulary.groups.len(), 1);
    assert_eq!(wire.vocabulary.groups[0].label, "Commands");
    assert_eq!(wire.vocabulary.groups[0].commands.len(), 2);

    let save = &wire.vocabulary.groups[0].commands[0];
    assert_eq!(save.name, "save");
    assert_eq!(save.description, "Save file");
    assert_eq!(save.shortcut, Some("Ctrl+S".to_string()));

    let close = &wire.vocabulary.groups[0].commands[1];
    assert_eq!(close.name, "close");
    assert_eq!(close.shortcut, Some("Alt+W".to_string()));
}

#[test]
fn tag_with_explicit_groups() {
    let tag = Tag::new("IDE").groups(vec![
        CommandGroup {
            label: "File".into(),
            commands: vec![
                cmd("save", "Save").build(),
                cmd("close", "Close").build(),
            ],
        },
        CommandGroup {
            label: "Build".into(),
            commands: vec![
                cmd("build", "Build project").build(),
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
fn cmd_no_shortcut() {
    let c = cmd("save", "Save file").build();
    assert!(c.shortcut.is_none());
}

// --- P2-4: Tag/Command builder edge cases ---

#[test]
fn tag_empty_commands() {
    let tag = Tag::new("Empty").commands(vec![]);
    let wire = tag.into_wire();
    assert_eq!(wire.vocabulary.groups.len(), 1);
    assert_eq!(wire.vocabulary.groups[0].commands.len(), 0);
}

#[test]
fn tag_unicode_title() {
    let tag = Tag::new("日本語テスト").short("テスト");
    let wire = tag.into_wire();
    assert_eq!(wire.title.text, "日本語テスト");
    assert_eq!(wire.title.short, Some("テスト".to_string()));
}

// --- Command enabled/disabled ---

#[test]
fn cmd_enabled_default_true() {
    let c = cmd("save", "Save file").build();
    assert!(c.enabled);
}

#[test]
fn cmd_enabled_false() {
    let c = cmd("undo", "Undo").enabled(false).build();
    assert!(!c.enabled);
}

// --- Tag::command() builder ---

#[test]
fn tag_command_builder_chains() {
    let tag = Tag::new("Test")
        .command(cmd("a", "First"))
        .command(cmd("b", "Second"))
        .command(cmd("c", "Third"));
    let wire = tag.into_wire();
    assert_eq!(wire.vocabulary.groups[0].commands.len(), 3);
    assert_eq!(wire.vocabulary.groups[0].commands[0].name, "a");
    assert_eq!(wire.vocabulary.groups[0].commands[2].name, "c");
}
