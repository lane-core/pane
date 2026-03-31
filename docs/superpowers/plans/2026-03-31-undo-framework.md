# Undo/Redo Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Kit-level undo framework with pluggable policies, optics integration, and sensitive-property exclusion.

**Architecture:** UndoPolicy trait + UndoManager wrapper + RecordingOptic + three built-in policies (Linear, Tree, Coalescing). All in pane-app, handler-local. DynOptic gains `is_undoable()`.

**Tech Stack:** Rust, pane-app, pane-optic (DynOptic extension), std::time::Instant

---

### Task 1: UndoableEdit type and UndoPolicy trait

**Files:**
- Create: `crates/pane-app/src/undo.rs`
- Modify: `crates/pane-app/src/lib.rs` (add `pub mod undo` + re-exports)

- [ ] **Step 1: Write the failing test**

Create `crates/pane-app/tests/undo.rs`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pane-app --test undo 2>&1 | tail -5`
Expected: compilation error — `undo` module doesn't exist.

- [ ] **Step 3: Write the undo module**

Create `crates/pane-app/src/undo.rs`:

```rust
//! Undo/redo framework with pluggable policies.
//!
//! Handler-local state — the undo stack lives in the handler struct,
//! is manipulated within the looper thread, and dies with the pane.
//!
//! Two undo mechanisms:
//! - Property undo (optic-based): RecordingOptic captures old/new on set
//! - Content undo (edit-log): handler records domain-specific edits
//!
//! Both feed into the same UndoPolicy.

use std::time::Instant;

use crate::scripting::AttrValue;

/// A recorded state change. The core data for undo/redo.
#[derive(Debug, Clone, PartialEq)]
pub struct UndoableEdit {
    /// Which property was changed (optic name, or custom label).
    pub property: String,
    /// Previous value (for property undo via optics).
    /// None for content edits where the handler applies the inverse.
    pub old_value: Option<AttrValue>,
    /// New value.
    pub new_value: Option<AttrValue>,
    /// Human-readable description ("Set title", "Insert text").
    pub description: String,
    /// When the edit happened.
    pub timestamp: Instant,
}

/// Pluggable policy for undo history structure and traversal.
pub trait UndoPolicy: Send + 'static {
    /// Record an edit.
    fn record(&mut self, edit: UndoableEdit);

    /// Undo the most recent edit/group.
    fn undo(&mut self) -> Option<UndoableEdit>;

    /// Redo. Behavior is policy-dependent.
    fn redo(&mut self) -> Option<UndoableEdit>;

    fn can_undo(&self) -> bool;
    fn can_redo(&self) -> bool;

    /// Human-readable description of what undo would do.
    fn undo_description(&self) -> Option<&str>;

    /// Human-readable description of what redo would do.
    fn redo_description(&self) -> Option<&str>;

    /// Group multiple edits into a single undo unit.
    fn begin_group(&mut self, description: &str);
    fn end_group(&mut self);

    /// Clear all history.
    fn clear(&mut self);
}

/// Linear undo — standard stack. Redo lost on new edit.
/// The sam/acme model and the default.
pub struct LinearPolicy {
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
    /// Active group (if begin_group was called without end_group).
    active_group: Option<UndoGroup>,
}

/// A single entry or a group of entries.
#[derive(Debug, Clone)]
enum UndoEntry {
    Single(UndoableEdit),
    Group(UndoGroup),
}

#[derive(Debug, Clone)]
struct UndoGroup {
    description: String,
    edits: Vec<UndoableEdit>,
}

impl UndoEntry {
    fn description(&self) -> &str {
        match self {
            UndoEntry::Single(e) => &e.description,
            UndoEntry::Group(g) => &g.description,
        }
    }

    /// Flatten to the edit(s) for undo application.
    /// Groups return their edits in reverse order (last applied = first undone).
    fn into_undo_edit(self) -> UndoableEdit {
        match self {
            UndoEntry::Single(e) => e,
            UndoEntry::Group(g) => {
                // Return a synthetic edit representing the group.
                // The caller can inspect old_value of the first edit
                // in the group for the pre-group state.
                let first = g.edits.first().cloned();
                let last = g.edits.last().cloned();
                UndoableEdit {
                    property: first.as_ref().map(|e| e.property.clone())
                        .unwrap_or_default(),
                    old_value: first.and_then(|e| e.old_value),
                    new_value: last.and_then(|e| e.new_value),
                    description: g.description,
                    timestamp: Instant::now(),
                }
            }
        }
    }
}

impl LinearPolicy {
    pub fn new() -> Self {
        LinearPolicy {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            active_group: None,
        }
    }
}

impl Default for LinearPolicy {
    fn default() -> Self { Self::new() }
}

impl UndoPolicy for LinearPolicy {
    fn record(&mut self, edit: UndoableEdit) {
        // New edit clears the redo stack
        self.redo_stack.clear();

        if let Some(ref mut group) = self.active_group {
            group.edits.push(edit);
        } else {
            self.undo_stack.push(UndoEntry::Single(edit));
        }
    }

    fn undo(&mut self) -> Option<UndoableEdit> {
        let entry = self.undo_stack.pop()?;
        let edit = entry.clone().into_undo_edit();
        self.redo_stack.push(entry);
        Some(edit)
    }

    fn redo(&mut self) -> Option<UndoableEdit> {
        let entry = self.redo_stack.pop()?;
        let edit = entry.clone().into_undo_edit();
        self.undo_stack.push(entry);
        Some(edit)
    }

    fn can_undo(&self) -> bool { !self.undo_stack.is_empty() }
    fn can_redo(&self) -> bool { !self.redo_stack.is_empty() }

    fn undo_description(&self) -> Option<&str> {
        self.undo_stack.last().map(|e| e.description())
    }

    fn redo_description(&self) -> Option<&str> {
        self.redo_stack.last().map(|e| e.description())
    }

    fn begin_group(&mut self, description: &str) {
        self.active_group = Some(UndoGroup {
            description: description.to_string(),
            edits: Vec::new(),
        });
    }

    fn end_group(&mut self) {
        if let Some(group) = self.active_group.take() {
            if !group.edits.is_empty() {
                self.redo_stack.clear();
                self.undo_stack.push(UndoEntry::Group(group));
            }
        }
    }

    fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.active_group = None;
    }
}
```

Add to `crates/pane-app/src/lib.rs` after the scripting module:

```rust
pub mod undo;
```

And in the re-exports section:

```rust
pub use undo::{UndoableEdit, UndoPolicy, LinearPolicy, UndoManager};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pane-app --test undo 2>&1 | tail -5`
Expected: 2 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pane-app/src/undo.rs crates/pane-app/src/lib.rs \
  crates/pane-app/tests/undo.rs
git commit -m "Undo framework: UndoableEdit, UndoPolicy trait, LinearPolicy"
```

---

### Task 2: UndoManager with save-point tracking

**Files:**
- Modify: `crates/pane-app/src/undo.rs`
- Modify: `crates/pane-app/tests/undo.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/pane-app/tests/undo.rs`:

```rust
use pane_app::undo::UndoManager;

#[test]
fn undo_manager_save_point() {
    let mut mgr = UndoManager::new(LinearPolicy::new());
    assert!(mgr.is_saved()); // no edits = at save point

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

    // One undo undoes the whole group
    let edit = mgr.undo().unwrap();
    assert_eq!(edit.description, "paste");
    assert!(!mgr.can_undo());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pane-app --test undo 2>&1 | tail -5`
Expected: compilation error — `UndoManager` not defined.

- [ ] **Step 3: Add UndoManager to undo.rs**

Append to `crates/pane-app/src/undo.rs`:

```rust
/// Convenience wrapper combining an UndoPolicy with save-point tracking.
pub struct UndoManager<P: UndoPolicy = LinearPolicy> {
    policy: P,
    /// Edit count at the last save. None = never saved, treat
    /// initial state as the save point.
    save_point: Option<usize>,
    /// Running edit count (incremented on record, decremented on undo,
    /// incremented on redo).
    edit_count: usize,
}

impl<P: UndoPolicy> UndoManager<P> {
    pub fn new(policy: P) -> Self {
        UndoManager {
            policy,
            save_point: Some(0),
            edit_count: 0,
        }
    }

    pub fn record(&mut self, edit: UndoableEdit) {
        self.policy.record(edit);
        self.edit_count += 1;
    }

    pub fn undo(&mut self) -> Option<UndoableEdit> {
        let edit = self.policy.undo()?;
        self.edit_count = self.edit_count.wrapping_sub(1);
        Some(edit)
    }

    pub fn redo(&mut self) -> Option<UndoableEdit> {
        let edit = self.policy.redo()?;
        self.edit_count += 1;
        Some(edit)
    }

    pub fn mark_saved(&mut self) {
        self.save_point = Some(self.edit_count);
    }

    pub fn is_saved(&self) -> bool {
        self.save_point == Some(self.edit_count)
    }

    pub fn can_undo(&self) -> bool { self.policy.can_undo() }
    pub fn can_redo(&self) -> bool { self.policy.can_redo() }
    pub fn undo_description(&self) -> Option<&str> { self.policy.undo_description() }
    pub fn redo_description(&self) -> Option<&str> { self.policy.redo_description() }

    pub fn begin_group(&mut self, description: &str) {
        self.policy.begin_group(description);
    }

    pub fn end_group(&mut self) {
        self.policy.end_group();
        self.edit_count += 1;
    }

    pub fn clear(&mut self) {
        self.policy.clear();
        self.edit_count = 0;
        self.save_point = None;
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p pane-app --test undo 2>&1 | tail -5`
Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pane-app/src/undo.rs crates/pane-app/tests/undo.rs
git commit -m "UndoManager: save-point tracking over any UndoPolicy"
```

---

### Task 3: DynOptic.is_undoable() extension

**Files:**
- Modify: `crates/pane-app/src/scripting.rs` (DynOptic trait)
- Modify: `crates/pane-app/tests/undo.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/pane-app/tests/undo.rs`:

```rust
use pane_app::scripting::{DynOptic, ScriptError, ValueType, OpKind, SpecifierForm};
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pane-app --test undo sensitive 2>&1 | tail -5`
Expected: compilation error — `is_undoable` not a method on `DynOptic`.

- [ ] **Step 3: Add is_undoable to DynOptic**

In `crates/pane-app/src/scripting.rs`, add to the `DynOptic` trait:

```rust
    /// Whether edits to this property should be recorded for undo.
    /// Override to false for sensitive properties (passwords, tokens).
    /// RecordingOptic checks this before storing old/new values.
    fn is_undoable(&self) -> bool { true }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p pane-app --test undo 2>&1 | tail -5`
Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pane-app/src/scripting.rs crates/pane-app/tests/undo.rs
git commit -m "DynOptic::is_undoable() — sensitive property exclusion for undo"
```

---

### Task 4: RecordingOptic wrapper

**Files:**
- Modify: `crates/pane-app/src/undo.rs`
- Modify: `crates/pane-app/tests/undo.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/pane-app/tests/undo.rs`:

```rust
use pane_app::undo::RecordingOptic;

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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pane-app --test undo recording 2>&1 | tail -5`
Expected: compilation error — `RecordingOptic` not defined.

- [ ] **Step 3: Add RecordingOptic to undo.rs**

Append to `crates/pane-app/src/undo.rs`:

```rust
use crate::scripting::{DynOptic, ScriptError};

/// Wraps a DynOptic to automatically record edits for undo.
///
/// Instruments `set()` to capture (old_value, new_value) before
/// and after mutation. Preserves optic laws — the underlying optic
/// does the actual state manipulation; this wrapper only observes.
///
/// Skips recording for optics where `is_undoable()` returns false
/// (sensitive properties).
pub struct RecordingOptic<'a, P: UndoPolicy> {
    inner: &'a dyn DynOptic,
    undo: &'a mut UndoManager<P>,
}

impl<'a, P: UndoPolicy> RecordingOptic<'a, P> {
    pub fn new(inner: &'a dyn DynOptic, undo: &'a mut UndoManager<P>) -> Self {
        RecordingOptic { inner, undo }
    }

    /// Set a value through the optic, recording the edit for undo.
    pub fn set(
        &mut self,
        state: &mut dyn std::any::Any,
        value: AttrValue,
    ) -> Result<(), ScriptError> {
        if !self.inner.is_undoable() {
            return self.inner.set(state, value);
        }

        let old = self.inner.get(state)?;
        self.inner.set(state, value.clone())?;

        self.undo.record(UndoableEdit {
            property: self.inner.name().to_string(),
            old_value: Some(old),
            new_value: Some(value),
            description: format!("Set {}", self.inner.name()),
            timestamp: Instant::now(),
        });

        Ok(())
    }
}
```

Update the re-export in `lib.rs` to include `RecordingOptic`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p pane-app --test undo 2>&1 | tail -5`
Expected: 7 tests pass.

- [ ] **Step 5: Run full test suite**

Run: `cargo test 2>&1 | grep -E "FAILED|test result:" | head -20`
Expected: all pass, no regressions.

- [ ] **Step 6: Commit**

```bash
git add crates/pane-app/src/undo.rs crates/pane-app/src/lib.rs \
  crates/pane-app/tests/undo.rs
git commit -m "RecordingOptic: automatic undo capture with sensitive exclusion"
```

---

### Task 5: CoalescingPolicy wrapper

**Files:**
- Modify: `crates/pane-app/src/undo.rs`
- Modify: `crates/pane-app/tests/undo.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/pane-app/tests/undo.rs`:

```rust
use pane_app::undo::CoalescingPolicy;
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pane-app --test undo coalescing 2>&1 | tail -5`
Expected: compilation error — `CoalescingPolicy` not defined.

- [ ] **Step 3: Implement CoalescingPolicy**

Append to `crates/pane-app/src/undo.rs`:

```rust
/// Wraps another policy, grouping adjacent same-property edits
/// within a timeout into a single undo unit.
///
/// Be's BTextView got this right for typing: consecutive character
/// insertions become one undo step, broken when the cursor moves
/// or a timeout expires.
pub struct CoalescingPolicy<P: UndoPolicy = LinearPolicy> {
    inner: P,
    timeout: Duration,
    /// Pending coalesced edit (not yet flushed to inner policy).
    pending: Option<UndoableEdit>,
}

impl<P: UndoPolicy> CoalescingPolicy<P> {
    pub fn new(inner: P, timeout: Duration) -> Self {
        CoalescingPolicy {
            inner,
            timeout,
            pending: None,
        }
    }

    /// Flush any pending coalesced edit to the inner policy.
    fn flush(&mut self) {
        if let Some(edit) = self.pending.take() {
            self.inner.record(edit);
        }
    }
}

impl<P: UndoPolicy> UndoPolicy for CoalescingPolicy<P> {
    fn record(&mut self, edit: UndoableEdit) {
        if let Some(ref mut pending) = self.pending {
            let same_property = pending.property == edit.property;
            let within_timeout = edit.timestamp
                .duration_since(pending.timestamp) < self.timeout;

            if same_property && within_timeout {
                // Coalesce: keep the original old_value, update new_value
                pending.new_value = edit.new_value;
                pending.timestamp = edit.timestamp;
                return;
            }

            // Different property or timeout — flush and start new
            self.flush();
        }

        self.pending = Some(edit);
    }

    fn undo(&mut self) -> Option<UndoableEdit> {
        self.flush();
        self.inner.undo()
    }

    fn redo(&mut self) -> Option<UndoableEdit> {
        self.inner.redo()
    }

    fn can_undo(&self) -> bool {
        self.pending.is_some() || self.inner.can_undo()
    }

    fn can_redo(&self) -> bool { self.inner.can_redo() }

    fn undo_description(&self) -> Option<&str> {
        if let Some(ref pending) = self.pending {
            Some(&pending.description)
        } else {
            self.inner.undo_description()
        }
    }

    fn redo_description(&self) -> Option<&str> {
        self.inner.redo_description()
    }

    fn begin_group(&mut self, description: &str) {
        self.flush();
        self.inner.begin_group(description);
    }

    fn end_group(&mut self) {
        self.flush();
        self.inner.end_group();
    }

    fn clear(&mut self) {
        self.pending = None;
        self.inner.clear();
    }
}
```

Add `use std::time::Duration;` at the top of undo.rs if not already imported.

Update re-exports in `lib.rs` to include `CoalescingPolicy`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p pane-app --test undo 2>&1 | tail -5`
Expected: 9 tests pass.

- [ ] **Step 5: Full test suite**

Run: `cargo test 2>&1 | grep -E "FAILED|test result:" | head -20`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pane-app/src/undo.rs crates/pane-app/src/lib.rs \
  crates/pane-app/tests/undo.rs
git commit -m "CoalescingPolicy: group adjacent same-property edits by timeout"
```

---

### Task 6: Doc comments, cargo doc, PLAN.md

**Files:**
- Modify: `crates/pane-app/src/undo.rs` (doc comments on module)
- Modify: `PLAN.md` (mark undo items)

- [ ] **Step 1: Run cargo doc**

Run: `cargo doc -p pane-app --no-deps 2>&1 | grep warning`
Expected: zero warnings (fix any that appear).

- [ ] **Step 2: Update PLAN.md**

In the API Tier 2 section or Session-type debt section, add:

```markdown
- [x] **Undo/redo framework** — UndoPolicy trait, LinearPolicy, CoalescingPolicy, UndoManager with save-point, RecordingOptic with sensitive exclusion. Spec: `docs/superpowers/specs/2026-03-31-undo-design.md`.
```

- [ ] **Step 3: Full test suite**

Run: `cargo test 2>&1 | grep -E "FAILED|test result:"` — all green.

- [ ] **Step 4: Commit**

```bash
git add PLAN.md crates/pane-app/src/undo.rs
git commit -m "Undo framework: docs and PLAN.md update"
```
