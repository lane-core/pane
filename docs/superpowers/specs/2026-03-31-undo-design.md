# Undo/Redo Design

Kit-level undo framework with pluggable policies, optics integration,
and declarative security for sensitive state. Designed with input from
be-systems-engineer, plan9-systems-engineer, and session-type-consultant.

---

## Architecture

Undo is **handler-local state**, not a system service. The undo stack
lives in the handler struct, is manipulated within the looper thread,
and dies with the pane. No protocol messages, no separate server, no
filesystem projection of history.

External tools interact with undo through:
- **ctl commands**: `echo undo > /pane/{id}/ctl`
- **observable attributes**: `can-undo`, `can-redo`, `undo-description`
- **pane-store indexing**: session managers can query "which panes have
  unsaved undo state?"

### Why not a service

- Undo is intrinsically local to the editing session (Plan 9 engineer).
- Undo histories are large, change rapidly, and are rarely needed by
  external tools (Plan 9 engineer).
- Adding undo to the connectivity graph creates deadlock potential for
  zero benefit (session-type consultant, DLfActRiS analysis).
- Be had no system-level undo. The gap was real, but the fix is a kit
  framework, not a system service (Be engineer).

### Two undo mechanisms

1. **Property undo** (optic-based): a `RecordingOptic` wrapper captures
   `(old_value, new_value)` on every `DynOptic::set`. Automatic
   undo-for-free on any scriptable property. Sound because optic laws
   (PutGet) guarantee the inverse is well-defined (Clarke et al.
   Proposition 2.3).

2. **Content undo** (edit-log-based): for domain-specific operations
   where edits are richer than get/set pairs (insert at position,
   delete range, transpose, indent block). Uses the `Edit` trait
   directly.

Both mechanisms feed into the same `UndoPolicy`, which manages
grouping, traversal, and history structure.

---

## Kit API

### UndoPolicy trait

The pluggable policy that determines how edits are grouped and how
undo traversal works:

```rust
pub trait UndoPolicy: Send + 'static {
    /// Record an edit.
    fn record(&mut self, edit: UndoableEdit);

    /// Undo the most recent edit/group. Returns the edit for
    /// the handler to apply the inverse.
    fn undo(&mut self) -> Option<UndoableEdit>;

    /// Redo. Behavior is policy-dependent (linear: re-apply last
    /// undone; tree: navigate branches).
    fn redo(&mut self) -> Option<UndoableEdit>;

    fn can_undo(&self) -> bool;
    fn can_redo(&self) -> bool;

    /// Human-readable description of what undo/redo would do.
    /// Must respect sensitivity — never auto-generate descriptions
    /// from sensitive property names or values.
    fn undo_description(&self) -> Option<&str>;
    fn redo_description(&self) -> Option<&str>;

    /// Group multiple edits into a single undo unit.
    fn begin_group(&mut self, description: &str);
    fn end_group(&mut self);

    /// Whether to record this edit at all. Default checks the
    /// optic's is_undoable() flag.
    fn should_record(&self, optic: &dyn DynOptic) -> bool {
        optic.is_undoable()
    }

    /// Clear all history.
    fn clear(&mut self);
}
```

### Built-in policies

**LinearPolicy** — standard stack. Redo is lost when a new edit is
recorded after undo. The sam/acme model and the default for most
applications.

**TreePolicy** — branching undo tree. Redo is never lost — undoing
then editing creates a new branch. The emacs undo-tree / vim
persistent-undo model. For applications where history preservation
matters (document editors, design tools).

**CoalescingPolicy** — wraps another policy, groups adjacent edits to
the same property within a configurable timeout (default 2s) into a
single undo unit. Be's BTextView got this right for typing:
consecutive character insertions become one undo step, broken when the
cursor moves or the timeout expires.

### UndoableEdit

```rust
pub struct UndoableEdit {
    /// Which property was changed (optic name, or custom label).
    pub property: String,
    /// Previous value (for property undo via optics).
    pub old_value: Option<AttrValue>,
    /// New value.
    pub new_value: Option<AttrValue>,
    /// Human-readable description.
    pub description: String,
    /// When the edit happened.
    pub timestamp: Instant,
}
```

For content undo (non-optic edits), `old_value` and `new_value` may
be None, and the handler applies the inverse through domain-specific
logic rather than optic set.

### UndoManager

Convenience wrapper that combines an UndoPolicy with save-point
tracking:

```rust
pub struct UndoManager<P: UndoPolicy = LinearPolicy> {
    policy: P,
    save_point: Option<usize>,  // edit count at last save
}

impl<P: UndoPolicy> UndoManager<P> {
    pub fn new(policy: P) -> Self;

    /// Record an edit.
    pub fn record(&mut self, edit: UndoableEdit);

    /// Undo/redo delegated to policy.
    pub fn undo(&mut self) -> Option<UndoableEdit>;
    pub fn redo(&mut self) -> Option<UndoableEdit>;

    /// Mark the current state as saved.
    pub fn mark_saved(&mut self);

    /// Is the current state at the save point?
    pub fn is_saved(&self) -> bool;

    // Delegated queries
    pub fn can_undo(&self) -> bool;
    pub fn can_redo(&self) -> bool;
    pub fn undo_description(&self) -> Option<&str>;
    pub fn redo_description(&self) -> Option<&str>;
    pub fn begin_group(&mut self, description: &str);
    pub fn end_group(&mut self);
}
```

### RecordingOptic wrapper

Instruments `DynOptic::set` to automatically capture edits:

```rust
pub struct RecordingOptic<'a, P: UndoPolicy> {
    inner: &'a dyn DynOptic,
    undo: &'a mut UndoManager<P>,
}

impl<P: UndoPolicy> RecordingOptic<'_, P> {
    pub fn set(
        &mut self,
        state: &mut dyn Any,
        value: AttrValue,
    ) -> Result<(), ScriptError> {
        // Check sensitivity
        if !self.undo.policy.should_record(self.inner) {
            return self.inner.set(state, value);
        }

        // Capture old value before mutation
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

The wrapper preserves optic laws (Clarke et al. Proposition 2.3):
the underlying optic does the actual state manipulation; the wrapper
only observes. The recording is a side effect that does not affect
the optic's get/set behavior.

PutPut subtlety: consecutive sets to the same optic via the recording
wrapper generate separate undo entries. The undo system captures the
pre-modification state, not relying on the optic to preserve
intermediate history. CoalescingPolicy handles grouping adjacent
same-property edits into one step.

---

## Security

### Sensitive properties

The `DynOptic` trait gains a sensitivity annotation:

```rust
pub trait DynOptic: Send + Sync {
    // ... existing methods ...

    /// Whether edits to this property should be recorded for undo.
    /// Override to false for sensitive properties (passwords, tokens).
    fn is_undoable(&self) -> bool { true }
}
```

When `is_undoable()` returns false:
- `RecordingOptic` skips recording. The edit happens but no
  `(old_value, new_value)` is pushed to the undo stack.
- The undo stack has a gap — undo skips over the sensitive edit.
- `attrs/undo-description` never references the sensitive property.

Consequence: undo is impossible for sensitive edits. This is correct —
the alternative (storing sensitive data in the undo stack where it
persists in memory and is potentially observable through attributes)
is worse.

### Description safety

`attrs/undo-description` must not auto-generate descriptions that
leak sensitive information. Even property names can be sensitive
("Changed credit-card-number" is information leakage without the
value). The `UndoableEdit.description` field is set explicitly by
the recording layer, and `RecordingOptic` only generates descriptions
for properties where `is_undoable()` is true.

### Memory hygiene

Undo state is handler-local, in-process, not filesystem-projected,
not stored. Sensitive deleted text lives only in the handler's process
memory and dies with the pane. No special zeroization needed for the
undo stack itself — the sensitive property exclusion prevents sensitive
data from entering the stack in the first place.

---

## Filesystem Interface

Undo is accessible to external tools through the existing pane-fs
ctl and attrs mechanisms:

### Control

```
echo undo > /pane/{id}/ctl     # trigger undo
echo redo > /pane/{id}/ctl     # trigger redo
```

These are ctl commands, consistent with `close`, `save`, etc. The
handler processes the command, invokes `UndoPolicy::undo()`, and
applies the inverse. The `attrs/` files reflect the new state on
next read.

### Observable attributes

```
/pane/{id}/attrs/can-undo          "true" / "false"
/pane/{id}/attrs/can-redo          "true" / "false"
/pane/{id}/attrs/undo-count        integer
/pane/{id}/attrs/redo-count        integer
/pane/{id}/attrs/undo-description  human-readable
/pane/{id}/attrs/redo-description  human-readable
/pane/{id}/attrs/is-saved          "true" / "false" (at save point)
```

These are pane-store indexable attributes. A session manager can
query "which panes have `is-saved = false`?" to warn before logout.

### No history projection

The undo history (the log of operations, tree structure, grouping)
is not projected to the filesystem. It is too large, too volatile,
and too local to be a useful filesystem artifact. The ctl + attrs
interface gives external tools everything they need: trigger undo,
observe whether undo is available, and read what it would do.

### Broadcast undo safety

```bash
for p in /pane/by-sig/com.pane.editor/*/ctl; do
    echo undo > "$p"
done
```

This is powerful and intentional. Safety is at the handler level, not
the filesystem level — the handler's undo logic decides whether to
honor the command. Remote agents are constrained by `.plan` governance
(read-only access prevents destructive ctl commands). The filesystem
does not add a confirmation layer — that would violate the principle
that pane-fs is a translation layer with no logic of its own.

Observability after broadcast:
```bash
for p in /pane/by-sig/com.pane.editor/*; do
    echo undo > "$p/ctl"
    cat "$p/attrs/undo-description"   # verify what was undone
done
```

---

## Clipboard-Undo Interaction

"Paste" decomposes into two independent operations:

1. **Clipboard read** — a protocol operation. The handler reads data
   from the clipboard (via `clipboard.read()` or the async lock flow).

2. **State mutation** — a local operation. The handler applies the
   pasted data through a RecordingOptic (or explicit UndoableEdit).
   This is recorded in the undo stack.

These compose sequentially. The clipboard protocol does not know about
undo; the undo system does not know about the clipboard. They are
orthogonal.

For paste as a single undo unit:
```rust
self.undo.begin_group("paste");
recording_optic.set(&mut self.state, pasted_data)?;
self.undo.end_group();
```

Be's BTextView coupled clipboard and undo (CutUndoBuffer::RedoSelf
puts cut text back on the clipboard). Pane keeps them orthogonal —
undo reverses the state change but does not restore the clipboard.
This is simpler and avoids the question of whose clipboard state to
restore when multiple panes have modified the clipboard since the cut.

---

## Dependencies

- **pane-optic**: `DynOptic` gains `is_undoable()` method
- **pane-app**: `UndoPolicy` trait, `UndoManager`, `RecordingOptic`,
  `UndoableEdit`, built-in policies (all in pane-app, not a separate
  crate)
- **pane-fs**: for ctl and attrs projection (deferred)
- **pane-store**: for indexable attributes (deferred)

The core undo framework (trait, manager, recording optic, policies)
can be built immediately. Filesystem and store integration are
additive layers.
