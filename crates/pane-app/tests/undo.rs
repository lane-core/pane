use pane_app::undo::{UndoableEdit, LinearPolicy, UndoPolicy, UndoManager};
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

#[test]
fn undo_manager_save_point() {
    let mut mgr = UndoManager::new(LinearPolicy::new());
    assert!(mgr.is_saved());

    mgr.record(UndoableEdit {
        property: "x".into(),
        old_value: Some(AttrValue::Int(0)),
        new_value: Some(AttrValue::Int(1)),
        description: "set x".into(),
        timestamp: Instant::now(),
    });

    assert!(!mgr.is_saved());
    mgr.mark_saved();
    assert!(mgr.is_saved());

    mgr.record(UndoableEdit {
        property: "x".into(),
        old_value: Some(AttrValue::Int(1)),
        new_value: Some(AttrValue::Int(2)),
        description: "set x again".into(),
        timestamp: Instant::now(),
    });

    assert!(!mgr.is_saved());
    mgr.undo();
    assert!(mgr.is_saved()); // back to save point
}

#[test]
fn undo_manager_group() {
    let mut mgr = UndoManager::new(LinearPolicy::new());

    mgr.begin_group("paste");
    mgr.record(UndoableEdit {
        property: "a".into(),
        old_value: Some(AttrValue::Int(0)),
        new_value: Some(AttrValue::Int(1)),
        description: "a".into(),
        timestamp: Instant::now(),
    });
    mgr.record(UndoableEdit {
        property: "b".into(),
        old_value: Some(AttrValue::Int(0)),
        new_value: Some(AttrValue::Int(2)),
        description: "b".into(),
        timestamp: Instant::now(),
    });
    mgr.end_group();

    assert_eq!(mgr.undo_description(), Some("paste"));

    let edit = mgr.undo().unwrap();
    assert_eq!(edit.description, "paste");
    assert!(!mgr.can_undo());
}
