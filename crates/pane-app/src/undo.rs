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
