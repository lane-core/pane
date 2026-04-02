# Prototype Subsystem Inventory

What was implemented in the prototype, what design knowledge it
carries, and how it relates to the architecture spec. This document
captures functional intent — not implementation details. The code
is in git history if specific patterns need to be referenced.

---

## Clipboard

**What it did:** Named clipboards with transactional writes.
`Clipboard::system()` for the platform clipboard, `Clipboard::named()`
for application-defined clipboards (kill-ring, registers, etc.).
Write access was gated by `ClipboardWriteLock` — a typestate handle
where `commit()` consumed the lock and `Drop` sent `Revert`.
Metadata included MIME content type, sensitivity policy (normal vs
secret with TTL auto-clear), and locality (local-only vs federated).

**Design knowledge to preserve:**
- The typestate lock pattern worked well. One of the cleanest
  examples of the linear discipline in practice.
- Sensitivity and locality are first-class metadata, not
  afterthoughts. Passwords get zeroized on clear; locality
  controls whether remote namespace mounts can see the entry.
- The clipboard is a service, not a widget. The data lives
  in a clipboard service process, not in the pane.

**Architecture spec status:** Clipboard is a service protocol
with `Handles<Clipboard>` + derive macro dispatch. The lock
pattern carries forward as `ClipboardWriteLock` with a richer
`CommitError` (Disconnected, LockRevoked, ValidationFailed).
The kit types (Clipboard, ClipboardMetadata, Sensitivity,
Locality) are stable vocabulary.

---

## Undo/Redo

**What it did:** Pluggable undo framework with three components:

1. **UndoPolicy trait** — abstraction over undo history structure.
   `record()`, `undo()`, `redo()`, `begin_group()`/`end_group()`.
   Two implementations:
   - `LinearPolicy` — standard stack. Redo lost on new edit.
     Groups collapse multiple edits into one undo step.
   - `CoalescingPolicy` — wraps another policy, merging adjacent
     same-property edits within a timeout window. Typing "hello"
     becomes one undo step, not five.

2. **UndoManager** — convenience wrapper adding save-point tracking.
   `mark_saved()` + `is_saved()` for dirty-document indicators.

3. **RecordingOptic** — instruments a `DynOptic` to automatically
   capture (old_value, new_value) on `set()`. Respects
   `is_undoable()` for sensitive properties (passwords don't go
   in the undo history).

**Design knowledge to preserve:**
- The policy/manager split is good. The policy handles the data
  structure (stack vs tree vs branching); the manager adds
  application concerns (save points, dirty tracking).
- Coalescing belongs in the policy layer, not the UI layer.
  The timeout-based coalescing mirrors Be's BTextView behavior
  (consecutive keystrokes = one undo) without hardcoding it.
- Integration with optics (RecordingOptic) means property-level
  undo is automatic — the app doesn't manually push edits for
  standard properties.

**Architecture spec status:** Undo is handler-local state, not a
protocol. No service registration needed. The types carry forward
unchanged — they have no protocol dependencies.

---

## Scripting / Property System

**What it did:** Structured, discoverable access to pane state.
Recovery of BeOS's `ResolveSpecifier` with optic-backed type safety.

- **PropertyInfo** — static property declarations (name, type,
  supported operations, specifier forms). The metadata table
  that tooling queries. Replaces Be's `property_info` struct
  with typed enums instead of `u32` bitmasks.

- **ScriptableHandler trait** — `resolve_specifier()` returns an
  optic for the named property, or `NotFound`. `supported_properties()`
  lists the full table. `state_mut()` provides handler state access.

- **DynOptic trait** — type-erased optic at the protocol boundary.
  Wraps concrete optics from `pane-optic` and handles serialization
  to/from `AttrValue`. Operations: get, set, count, is_writable,
  is_undoable.

- **AttrValue** — the value type at the scripting wire. Closed enum
  (String, Bool, Int, Float, Bytes, Rect). Custom types go through
  Bytes with application-defined serialization. Shared with
  filesystem attributes.

- **ScriptQuery / Specifier** — addressing system for property
  access chains. Specifier forms: Direct (lens), Index (traversal),
  Named (affine). The specifier chain is an immutable vec with a
  cursor, not Be's mutate-the-message-in-flight pattern.

- **ScriptReply** — newtype over ReplyPort for scripting responses.
  Consumed by `ok()` or `error()`. Transparent to ReplyPort's Drop.

**Design knowledge to preserve:**
- The type-erased optic boundary (DynOptic) is the right
  abstraction for scripting. Static optics inside, dynamic
  dispatch at the wire boundary.
- Specifiers as an immutable chain with cursor is better than
  Be's mutable message. Separation of address (specifiers) from
  operation (ScriptOp) is cleaner.
- `is_undoable()` on DynOptic bridges scripting and undo cleanly.
- The `ValueType` enum as a closed set avoids Be's type confusion
  at wire boundaries.

**Architecture spec status:** PropertyInfo, ScriptableHandler,
DynOptic, Specifier, AttrValue are all listed as preserved
vocabulary. The `#[derive(Scriptable)]` macro was deferred and
remains deferred.

---

## Request/Reply

**What it did:** Two mechanisms:

1. **ReplyPort** — typestate handle for "I owe a reply." Handler
   receives it via `request_received()`. `reply()` consumes it.
   Drop sends `ReplyFailed`. The reply_fn closure abstracted the
   delivery destination (looper channel for async, oneshot for
   blocking).

2. **CompletionReplyPort** — same pattern specialized for compositor
   completion requests. `reply()` sends completions on the wire.
   Drop sends empty completions.

3. **send_and_wait** — blocking request/reply for non-looper
   callers. Thread-local guard prevented self-deadlock but not
   mutual deadlock (same limitation as BeOS).

4. **send_request** — async request/reply via looper channels.
   Token-based correlation between request and reply.

**Design knowledge to preserve:**
- The typestate ownership pattern (reply is an obligation,
  Drop compensates) is proven. Both ReplyPort and
  CompletionReplyPort validated it.
- Thread-local deadlock detection works for self-deadlock.
  Mutual deadlock remains a runtime hazard with send_and_wait.
- Separation of blocking (send_and_wait) from async
  (send_request) is necessary. Blocking must not be called
  from handler methods.

**Architecture spec status:** ReplyPort carries forward.
CompletionReplyPort carries forward. Token-based correlation
is replaced by Dispatch entries (typed callbacks, no ghost state).
`reply_received` / `reply_failed` handler methods are eliminated
in favor of per-request typed dispatch.

---

## Filter Chain

**What it did:** Ordered list of MessageFilter impls that process
events before the handler sees them. Each filter can Pass
(with possibly modified event), or Consume. Pre-filtering via
`matches()` skips irrelevant events.

Runtime mutation via `add_filter()` / `remove_filter()` through
the looper channel — mutations take effect at batch boundaries,
not mid-batch. FilterToken for removal (analogous to TimerToken).

**Concrete filter: ShortcutFilter** — transforms key combos into
CommandExecuted messages. Key→Command bridge. Modifiers + key
matching on press events only.

**Design knowledge to preserve:**
- Filter chain is a proven pattern. Registration order,
  composability, batch-boundary mutation — all correct.
- ShortcutFilter as a separate composable filter (not hardcoded
  in the handler or looper) is the right factoring.
- Runtime filter mutation is important for dynamic behavior
  (e.g., disabling shortcuts in text input mode).

**Architecture spec status:** FilterAction changes from
`Pass(Message)` to `Pass` (immutable borrow) + `Transform(Message)`.
FilterChain and MessageFilter carry forward with the refined
action types. ShortcutFilter carries forward.

---

## Tag / Command Vocabulary

**What it did:** Builder API for pane identity and commands:
- `Tag::new("title")` — pane title (text + optional short form)
- `.command(cmd("name", "description").shortcut("Ctrl+S"))` —
  command with optional keyboard shortcut
- `.groups(...)` — categorized command groups
- Dynamic update via `Messenger::set_title()` and
  `Messenger::set_vocabulary()`

The tag + vocabulary is the pane's public identity: what it's
called and what it can do. The compositor renders the title and
makes commands available through the command surface.

**Design knowledge to preserve:**
- The builder pattern is ergonomic. Tag is constructed once,
  passed to create_pane, optionally updated dynamically.
- Commands are declarative (name + description + shortcut), not
  imperative (register callback). The handler receives
  CommandExecuted with the name; it decides what to do.
- Short titles for constrained contexts (tab strip) are
  important for real use.

**Architecture spec status:** Tag/CommandBuilder vocabulary
carries forward. Wire types (PaneTitle, CommandVocabulary,
Command) carry forward in pane-proto.

---

## Exit / Quit Protocol

**What it did:** Structured exit with three reasons:
- `HandlerExit` — voluntary (user pressed Escape, handler
  returned false). Kit sends RequestClose.
- `CompositorClose` — compositor-initiated (user closed window).
  No RequestClose needed.
- `Disconnected` — connection lost. Can't send anything.

App-level quit via `request_quit()` → `QuitResult`:
- `Approved` — all panes agreed, they've been closed.
- `Vetoed` — at least one pane refused (unsaved changes).
- `Unreachable` — pane timed out or disconnected.

ExitBroadcaster — push-based pane death notification.
`Messenger::monitor()` registers interest; pane exit broadcasts
`PaneExited { id, reason }` to all watchers.

**Design knowledge to preserve:**
- Three-way exit reason (voluntary/requested/disconnected)
  determines whether the kit sends RequestClose. This is the
  right model.
- Quit protocol with veto is essential for unsaved-changes
  dialogs. `quit_requested()` takes `&self` (not `&mut self`)
  for deadlock freedom — side effects must happen before
  returning true.
- ExitBroadcaster is push-based (not polling) and prunes
  disconnected watchers on send.

**Architecture spec status:** ExitReason carries forward as
`Flow::Stop` + error distinction. QuitResult carries forward.
Monitor/broadcast carries forward. The `&self` constraint on
`quit_requested` is preserved.

---

## Timers

**What it did:** calloop Timer sources with cancel-on-drop:
- `send_delayed(duration, msg)` — one-shot delayed event
- `send_periodic_fn(interval, f)` — periodic event via closure
- `set_pulse_rate(duration)` — recurring handler pulse

TimerToken — non-Clone, cancel-on-drop. Dual-path cancellation:
AtomicBool for immediate callback suppression + CancelTimer
looper message for eager source removal.

**Design knowledge to preserve:**
- Timer-as-calloop-source is correct (no separate timer threads).
- Cancel-on-drop is the right default. Drop the token, timer
  stops. No orphan timers.
- Dual cancellation (immediate flag + deferred source removal)
  handles the race between timer fire and cancellation.
- Pulse is a special case of periodic (shared token via
  `Arc<Mutex<Option<TimerToken>>>` across Messenger clones).

**Architecture spec status:** TimerToken carries forward.
calloop timer sources carry forward. The pulse mechanism
carries forward.

---

## Routing (stub)

**What it did:** Placeholder for content routing — pattern-matched
dispatch of content to handlers. Rule files in well-known directories
(`/etc/pane/route/rules/`, `~/.config/pane/route/rules/`).
RouteTable, RouteResult (Match/MultiMatch/NoMatch), RouteCandidate
with quality rating.

**Design knowledge to preserve:**
- Quality-based selection (0.0–1.0) from Be's Translation Kit.
- File-based rules (drop a file, gain a behavior) from Plan 9.
- Multi-match presents options to user, not silent first-match.

**Architecture spec status:** Routing is a future service protocol.
The design vocabulary is preserved in conceptual docs.

---

## Transport Layer (pane-session — preserved)

Not struck, but for completeness:
- Unix, TCP, TLS, Memory, Proxy, Reconnecting transports
- ProxyTransport — protocol tracing (exportfs -d pattern)
- ReconnectingTransport — exponential backoff with message
  buffering and replay (aan(8) pattern)
- Session-typed channels (Chan<S,T>) with CLL-derived types

All preserved in the surviving pane-session crate.

---

## Optics (pane-optic — preserved)

Not struck, but for completeness:
- Getter, Setter, PartialGetter, PartialSetter traits
- FieldLens, FieldAffine, FieldTraversal concrete types
- Composition via `.then()`
- Optic law tests (GetPut, PutGet, PutPut)

All preserved in the surviving pane-optic crate.

---

## Filesystem Notifications (pane-notify — preserved)

Not struck, but for completeness:
- NodeRef, StatFields bitmask, AttrCause
- WatchFlags (subscription) vs EventKind (notification)
- MovedFrom/MovedTo with inotify cookie correlation
- fanotify on Linux, polling stub on macOS

All preserved in the surviving pane-notify crate.
