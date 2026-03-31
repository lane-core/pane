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

use std::time::{Duration, Instant};

use crate::scripting::{AttrValue, DynOptic};
use crate::error::ScriptError;

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

/// Convenience wrapper combining an UndoPolicy with save-point tracking.
pub struct UndoManager<P: UndoPolicy = LinearPolicy> {
    policy: P,
    save_point: Option<usize>,
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

/// Wraps another policy, grouping adjacent same-property edits
/// within a timeout into a single undo unit.
///
/// Be's BTextView got this right for typing: consecutive character
/// insertions become one undo step, broken when the cursor moves
/// or a timeout expires.
pub struct CoalescingPolicy<P: UndoPolicy = LinearPolicy> {
    inner: P,
    timeout: Duration,
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
                pending.new_value = edit.new_value;
                pending.timestamp = edit.timestamp;
                return;
            }

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
