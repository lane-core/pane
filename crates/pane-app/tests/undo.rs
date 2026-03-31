use pane_app::undo::{UndoableEdit, LinearPolicy, UndoPolicy};
use pane_app::scripting::AttrValue;
use std::time::Instant;

#[test]
fn linear_undo_single_edit() {
    let mut policy = LinearPolicy::new();
    assert!(!policy.can_undo());

    policy.record(UndoableEdit {
        property: "title".into(),
        old_value: Some(AttrValue::String("old".into())),
        new_value: Some(AttrValue::String("new".into())),
        description: "Set title".into(),
        timestamp: Instant::now(),
    });

    assert!(policy.can_undo());
    assert!(!policy.can_redo());
    assert_eq!(policy.undo_description(), Some("Set title"));

    let edit = policy.undo().unwrap();
    assert_eq!(edit.property, "title");
    assert_eq!(edit.old_value, Some(AttrValue::String("old".into())));

    assert!(!policy.can_undo());
    assert!(policy.can_redo());
}

#[test]
fn linear_redo_lost_on_new_edit() {
    let mut policy = LinearPolicy::new();

    policy.record(UndoableEdit {
        property: "a".into(),
        old_value: Some(AttrValue::Int(1)),
        new_value: Some(AttrValue::Int(2)),
        description: "edit a".into(),
        timestamp: Instant::now(),
    });

    policy.undo();
    assert!(policy.can_redo());

    // New edit after undo — redo is lost
    policy.record(UndoableEdit {
        property: "b".into(),
        old_value: Some(AttrValue::Int(10)),
        new_value: Some(AttrValue::Int(20)),
        description: "edit b".into(),
        timestamp: Instant::now(),
    });

    assert!(!policy.can_redo());
    assert!(policy.can_undo());
}
