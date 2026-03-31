use pane_app::undo::{UndoableEdit, LinearPolicy, UndoPolicy, UndoManager, CoalescingPolicy};
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

use pane_app::scripting::{DynOptic, ValueType, OpKind, SpecifierForm};
use pane_app::ScriptError;
use pane_app::undo::RecordingOptic;
use std::any::Any;

struct SensitiveField;

impl DynOptic for SensitiveField {
    fn name(&self) -> &str { "password" }
    fn get(&self, _state: &dyn Any) -> Result<AttrValue, ScriptError> {
        Ok(AttrValue::String("secret".into()))
    }
    fn set(&self, _state: &mut dyn Any, _value: AttrValue) -> Result<(), ScriptError> {
        Ok(())
    }
    fn is_writable(&self) -> bool { true }
    fn count(&self, _state: &dyn Any) -> Result<usize, ScriptError> { Ok(1) }
    fn value_type(&self) -> ValueType { ValueType::String }
    fn operations(&self) -> &'static [OpKind] { &[OpKind::Get, OpKind::Set] }
    fn specifier_forms(&self) -> &'static [SpecifierForm] { &[SpecifierForm::Direct] }
    fn is_undoable(&self) -> bool { false }
}

#[test]
fn sensitive_optic_is_not_undoable() {
    let field = SensitiveField;
    assert!(!field.is_undoable());
}

/// A simple DynOptic that reads/writes a String field.
struct TitleOptic;

impl DynOptic for TitleOptic {
    fn name(&self) -> &str { "title" }
    fn get(&self, state: &dyn Any) -> Result<AttrValue, ScriptError> {
        let s = state.downcast_ref::<String>().unwrap();
        Ok(AttrValue::String(s.clone()))
    }
    fn set(&self, state: &mut dyn Any, value: AttrValue) -> Result<(), ScriptError> {
        let s = state.downcast_mut::<String>().unwrap();
        if let AttrValue::String(v) = value {
            *s = v;
        }
        Ok(())
    }
    fn is_writable(&self) -> bool { true }
    fn count(&self, _state: &dyn Any) -> Result<usize, ScriptError> { Ok(1) }
    fn value_type(&self) -> ValueType { ValueType::String }
    fn operations(&self) -> &'static [OpKind] { &[OpKind::Get, OpKind::Set] }
    fn specifier_forms(&self) -> &'static [SpecifierForm] { &[SpecifierForm::Direct] }
}

#[test]
fn recording_optic_captures_edits() {
    let mut mgr = UndoManager::new(LinearPolicy::new());
    let optic = TitleOptic;
    let mut state = String::from("hello");

    {
        let mut rec = RecordingOptic::new(&optic, &mut mgr);
        rec.set(&mut state, AttrValue::String("world".into())).unwrap();
    }

    assert_eq!(state, "world");
    assert!(mgr.can_undo());
    assert_eq!(mgr.undo_description(), Some("Set title"));

    let edit = mgr.undo().unwrap();
    assert_eq!(edit.old_value, Some(AttrValue::String("hello".into())));
    assert_eq!(edit.new_value, Some(AttrValue::String("world".into())));
}

#[test]
fn recording_optic_skips_sensitive() {
    let mut mgr = UndoManager::new(LinearPolicy::new());
    let optic = SensitiveField;
    let mut state = String::from("old_password");

    {
        let mut rec = RecordingOptic::new(&optic, &mut mgr);
        rec.set(&mut state, AttrValue::String("new_password".into())).unwrap();
    }

    // Edit happened but was not recorded
    assert!(!mgr.can_undo());
}

use std::time::Duration;

#[test]
fn coalescing_groups_same_property() {
    let mut policy = CoalescingPolicy::new(
        LinearPolicy::new(),
        Duration::from_secs(2),
    );

    let now = Instant::now();

    policy.record(UndoableEdit {
        property: "text".into(),
        old_value: Some(AttrValue::String("h".into())),
        new_value: Some(AttrValue::String("he".into())),
        description: "typing".into(),
        timestamp: now,
    });

    policy.record(UndoableEdit {
        property: "text".into(),
        old_value: Some(AttrValue::String("he".into())),
        new_value: Some(AttrValue::String("hel".into())),
        description: "typing".into(),
        timestamp: now,
    });

    policy.record(UndoableEdit {
        property: "text".into(),
        old_value: Some(AttrValue::String("hel".into())),
        new_value: Some(AttrValue::String("hell".into())),
        description: "typing".into(),
        timestamp: now,
    });

    // Three edits coalesced into one — single undo gets back to "h"
    assert!(policy.can_undo());
    let edit = policy.undo().unwrap();
    assert_eq!(edit.old_value, Some(AttrValue::String("h".into())));
    assert_eq!(edit.new_value, Some(AttrValue::String("hell".into())));
    assert!(!policy.can_undo());
}

#[test]
fn coalescing_breaks_on_different_property() {
    let mut policy = CoalescingPolicy::new(
        LinearPolicy::new(),
        Duration::from_secs(2),
    );

    let now = Instant::now();

    policy.record(UndoableEdit {
        property: "text".into(),
        old_value: Some(AttrValue::String("a".into())),
        new_value: Some(AttrValue::String("ab".into())),
        description: "typing".into(),
        timestamp: now,
    });

    // Different property breaks coalescing
    policy.record(UndoableEdit {
        property: "title".into(),
        old_value: Some(AttrValue::String("old".into())),
        new_value: Some(AttrValue::String("new".into())),
        description: "set title".into(),
        timestamp: now,
    });

    // Two separate undo steps
    assert!(policy.can_undo());
    policy.undo(); // undoes title change
    assert!(policy.can_undo());
    policy.undo(); // undoes text change
    assert!(!policy.can_undo());
}
