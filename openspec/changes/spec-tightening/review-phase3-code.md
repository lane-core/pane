# Phase 3 Code Review: pane-session, pane-notify, pane-proto

Reviewer: Be Systems Engineer
Date: 2026-03-26
Scope: Full review of all source files in three crates, assessed against architecture spec and pane-app spec requirements.

**Verdict: pane-session is ready for pane-app to build on. pane-notify is ready as a stub. pane-proto needs tag.rs cleaned up before pane-app imports it.**

---

## 1. Correctness

### pane-session: solid

No bugs found in the core typestate machinery. The type-level mechanics are correct: Send/Recv duality, Select/Branch correspondence, the HasDual involution, the advance() transport forwarding. The `#[must_use]` on Chan is exactly right.

**The calloop SessionSource is correct but has one subtle edge case worth noting.** The `try_fill` function reads from a `&UnixStream` via `Read::read(&mut &*stream_ref, ...)`. This works because `Read` is implemented for `&UnixStream` (shared reference I/O), but the indirection through `&*stream_ref` is unnecessary -- `Read::read(&mut &self.reader, ...)` would be equivalent and clearer. Not a bug, just a readability nit.

**The accumulation buffer state machine is correct.** The ReadingLen -> ReadingBody -> (deliver, reset) cycle handles partial reads properly. The MAX_MESSAGE_SIZE guard is in the right place (after reading the length prefix, before allocating). The `std::mem::take` on the buffer in the body-complete path is efficient.

**One edge case in process_events**: if the callback returns `PostAction::Remove` or `PostAction::Disable`, the loop exits correctly. But if the callback returns an error, that error propagates through the `?` on line 146, which exits the inner closure. The outer `self.source.process_events` then returns that error. This is correct behavior -- a callback error is an application error, not a transport error.

**The framing protocol is duplicated.** `UnixTransport::send_raw`/`recv_raw` and `calloop::write_message`/`SessionSource` both implement length-prefixed framing independently. The two implementations are consistent (4-byte LE length prefix, same MAX_MESSAGE_SIZE guard), but duplication means they could drift. This is a minor concern now but becomes real when the framing evolves (e.g., adding a message type tag byte for active-phase multiplexing).

Severity: **minor** -- extract shared framing into pane-proto's `wire.rs` (which already has `frame()` and `frame_length()`), then use it from both sites. The functions are already there; they just aren't being used by the transport or calloop code.

### pane-notify stub: safe with a known leak

**The polling thread never terminates.** `watch_path_stub` spawns a thread that loops forever with `sleep(200ms)`. When the `WatchHandle` is dropped, nothing signals the thread to stop. The thread runs until the process exits.

This is fine for a dev stub, but the `WatchHandle` type's doc comment says "Drop it to unregister" -- that contract is not honored. The thread holds a clone of the `mpsc::Sender`, so the channel stays open even after the `Watcher` itself is dropped.

Severity: **minor for now** -- the stub is explicitly marked non-production. But when the Linux inotify implementation lands, the WatchHandle drop behavior needs to work. The fix is straightforward: WatchHandle holds a `oneshot::Sender<()>` or `Arc<AtomicBool>` that the thread checks each iteration.

**Race window in initial state capture.** `watch_path_stub` captures `initial_entries` before spawning the thread, which is correct -- any file created after `watch_path` returns will be detected. Good.

### pane-proto: no issues

Wire types are clean. The `frame()`/`frame_length()` utilities are correct. AttrValue is reasonable for the filesystem layer. Event types are well-structured.

---

## 2. API Readiness for pane-app

### What pane-app needs from pane-session

The pane-app spec describes a three-phase protocol: session-typed handshake, active-phase enum messaging, and teardown. pane-session provides everything needed for phases 1 and 3:

- **Handshake**: `Chan<PaneHandshake, UnixTransport>` with `send`/`recv`/`offer`/`select` -- done.
- **Transport recovery**: `Chan<End, T>::finish() -> T` to extract the transport after handshake -- done.
- **Active phase via calloop**: `SessionSource` for the compositor side -- done.
- **Active phase client side**: The client looper reads from the raw `UnixStream` after `finish()`. The stream is already available via `UnixTransport::into_stream()` -- done.

**One gap: no `try_recv` on Chan.** The current `recv()` is blocking. For the client-side looper, this is fine -- the looper thread blocks on recv, which is the BLooper pattern. But if pane-app ever needs non-blocking receive (e.g., for a combined channel that multiplexes pane messages with user-posted messages), it would need a `try_recv` or a way to select over multiple sources. This is not blocking for the initial pane-app implementation but is worth noting.

Severity: **low** -- the pane-app spec's looper design uses blocking recv on a dedicated thread, which matches what's available. If multiplexing becomes necessary, it's a straightforward addition to the Transport trait.

### What pane-app needs from pane-notify

The pane-app spec section 8 says: "The kit watches these directories via pane-notify for live updates." The current pane-notify API (`Watcher::new(sender)`, `watch_path(path, kinds)`) is exactly what pane-app needs. The channel-based delivery model matches the looper pattern.

**Missing: EventKind bitflags or set API.** The spec's example usage shows `EventKind::Create | EventKind::Delete`, but EventKind is a plain enum, not a bitflags type. The actual API takes `&[EventKind]`, which works but is less ergonomic than what the doc example shows. The doc comment in watcher.rs line 65 uses `|` syntax that doesn't compile.

Severity: **minor** -- fix the doc example to match the actual API (`&[EventKind::Create, EventKind::Delete]`), or switch to bitflags if the pipe syntax is preferred. Not blocking.

### What pane-app needs from pane-proto

pane-app needs:

1. **The handshake message types** (ClientHello, ServerHello, ClientCaps, Accepted, Rejected). These don't exist yet. They need to be defined in pane-proto. Not surprising -- pane-app is the next phase.

2. **The active-phase enums** (ClientToComp, CompToClient / PaneEvent). These don't exist yet either. The PaneEvent enum in the pane-app spec is the target. It should live in pane-proto so both sides share the definition.

3. **The Tag builder.** The pane-app spec shows `Tag::new("Hello").commands(vec![cmd(...)])`. The current pane-proto has TagLine/TagAction/TagCommand, which are close but not aligned with the revised spec. See section 3 below.

4. **PaneId** -- exists and is correct.

5. **KeyEvent, MouseEvent, Modifiers** -- exist and are correct.

**No churn risk from pane-session.** The pane-session API is stable for pane-app's needs. The types (Chan, Send, Recv, Select, Branch, End, Transport, SessionError) are the right abstraction. The transport trait is clean. Nothing needs to change.

**Moderate churn risk from pane-proto.** The tag types need revision (see below), and the handshake/active-phase message types need to be added. This is expected new work, not rework.

---

## 3. The pane-proto tag.rs Situation

The current tag.rs defines:
- `TagLine { name, actions, user_actions }`
- `TagAction { label, command }`
- `TagCommand::BuiltIn | Shell | Route`
- `BuiltInAction::Del | Snarf | Get | Put | Undo | Redo`

The pane-app spec defines a different model:
- `Tag::new(title).commands(vec![cmd(...)])` -- builder pattern
- `cmd(name, description).shortcut(key).built_in(BuiltIn::Close)` -- named commands with metadata
- `BuiltIn::Close` (not `Del`)
- `CommandVocabulary` for the completion dropdown
- No `user_actions` split -- commands are flat with optional categories

**These are incompatible.** The current TagLine is a rendering-oriented struct (what to display). The spec's Tag is a semantic declaration (what the pane offers). The compositor translates the declaration into rendering. This is the right layering -- the pane-app spec got this right.

**Recommendation: delete the current tag.rs types now.** They will cause confusion if pane-app imports pane-proto and sees two models. The old types are not used by any runtime code (pane-session doesn't depend on pane-proto). Leaving them creates the risk that someone builds against the wrong types.

Replace with a TODO comment or a minimal `TagLine` type that matches the spec's wire format. The full Tag builder API belongs in pane-app (it's a kit-level convenience), but the wire type (what gets serialized and sent to the compositor) belongs in pane-proto.

Severity: **moderate** -- do this before starting pane-app implementation.

---

## 4. Test Coverage

### pane-session: 22 tests -- good coverage

**What's covered well:**
- Happy path for send/recv/close over both transports (memory, unix)
- Crash recovery: dropped channel, mid-conversation crash, server panic
- Branching: left/right/crash-before-selection, over both transports
- N-ary branching: 3-way choice with all arms exercised
- Combinators: request(), finish()
- Macros: offer! and choice! with 2-way and 3-way
- Calloop integration: message receipt, client crash detection

**What's not covered:**

1. **MAX_MESSAGE_SIZE enforcement.** No test sends a message claiming to be >16MB. Should verify the guard rejects it. Easy to add, high value -- this is a security boundary.

2. **Partial reads in calloop.** All tests deliver complete messages. No test verifies that the accumulation buffer correctly handles a message arriving in multiple read() calls. This would require a test that writes partial data to the socket (write the length prefix, sleep, write the body). The state machine looks correct but is unexercised.

3. **Zero-length messages.** What happens when someone sends `send_raw(&[])`? The length prefix is 0, recv_raw reads 0 bytes, returns an empty Vec. Postcard deserialization of an empty buffer will fail for most types. This is probably fine (Codec error), but it's an untested edge.

4. **Rapid successive messages.** No test verifies that multiple messages delivered in quick succession (possible in a single read() call for the calloop path) are correctly deframed and delivered individually.

5. **The `offer!` macro with exactly 1 arm.** The macro requires at minimum 2 arms (base case). This is correct -- a 1-arm branch doesn't make sense as a session type. But there's no compile-fail test verifying the error message is reasonable.

### pane-notify: 3 tests -- minimal but appropriate

The stub is a dev shim. The three tests (create, delete, nonexistent path) cover the essential contracts. The Linux implementation will need its own test suite.

**Not covered:** Modify and Attrib event kinds, MovedFrom/MovedTo, watching a file (vs directory), multiple simultaneous watches. These matter for the Linux implementation, not the stub.

### pane-proto: no tests

No tests for serialization round-trips of the wire types. This is fine for now -- the types are simple serde derives -- but round-trip property tests should be added when the handshake/active-phase types land. Postcard has edge cases with varint encoding of large values.

---

## 5. Code Quality

### Naming: excellent

The crate follows Be's naming discipline. Types are named for what they represent (Chan, not SessionChannel; Offer, not BranchResult; Dual, not Opposite). The transport abstraction names match their function (send_raw, recv_raw -- raw because the session layer handles serialization). The calloop types (SessionSource, SessionEvent) are clear.

### Documentation: good to excellent

Every public type and method has a doc comment explaining intent, not just mechanism. The linear logic annotations on the session types (tensor, par, plus, with, unit) are a nice touch -- they connect the implementation to its theoretical basis without being pedantic.

The `request()` combinator doc comment with the before/after comparison is exactly how Be documented things: show the improvement, let the developer judge.

### Module organization: clean

- pane-session: types, dual, error, transport/{mod, memory, unix}, calloop, macros. Each module has a single concern. No circular dependencies.
- pane-notify: event, watcher, stub, linux. Clean platform split.
- pane-proto: one module per concept (message, tag, event, color, attrs, wire). Flat, navigable.

### One naming issue

`pane_session::calloop` shadows the `calloop` crate name. Inside pane-session this is fine (the module is accessed as `crate::calloop`). But downstream consumers write `use pane_session::calloop::SessionSource` -- the path reads naturally. However, if someone also uses the `calloop` crate directly in the same file, the name collision could cause confusion. This is a Rust ecosystem convention issue, not a bug. Worth noting, not worth renaming.

---

## 6. Things That Would Make the Be Engineers Wince

### Nothing major. This is clean code.

The typestate pattern is the right abstraction at the right level. The transport trait is minimal. The error handling is honest (Disconnected vs Codec vs Io -- three causes, three variants, no conflation). The crash safety property is real and tested.

### Minor observations:

**1. The `close()` method is unnecessary.** `Chan<End, T>::close(self)` just calls `drop(self)`. Dropping a `Chan<End, T>` already does the right thing -- the transport is dropped, the socket closes. The `close()` method exists only for readability (`chan.close()` vs `drop(chan)`). At Be, we would have left this out -- if the language provides the behavior, don't wrap it. But it's not wrong, and the `finish()` alternative for transport recovery justifies having an explicit "I'm done and I don't need the transport" path.

**2. No `Display` impl for session types.** When debugging, you can't print what protocol state a channel is in. The `Debug` impl on Chan shows the type name via `type_name::<S>()`, which gives something like `pane_session::types::Send<alloc::string::String, pane_session::types::Recv<u64, pane_session::types::End>>`. Functional but ugly. A human-readable display (`Send<String, Recv<u64, End>>`) would help during development. Low priority.

**3. The `Offer` enum lives in types.rs but isn't a session type.** It's a runtime value returned by `offer()`. In the Haiku source, runtime values and type-level constructs were in separate headers. Consider moving Offer to its own home or at least documenting that it's a value type, not a session type marker.

**4. Framing duplication (mentioned above).** pane-proto's wire.rs has `frame()`/`frame_length()`. UnixTransport reimplements framing. SessionSource reimplements framing. `write_message()` reimplements framing. Four copies of length-prefixed framing. At Be, this would have been one function in the Support Kit. Consolidate before more consumers appear.

**5. The pane-notify Watcher holds `mpsc::Sender<Event>` and `AtomicU64` but no state about active watches.** There's no way to enumerate or manage existing watches. WatchHandle is opaque and dropping it does nothing (in the stub). When the Linux implementation lands, the Watcher needs to track watch descriptors so it can clean up on drop. The current design has the right shape but the internals are hollow.

---

## Summary

| Area | Status | Action needed |
|---|---|---|
| pane-session correctness | Ready | None |
| pane-session API for pane-app | Ready | None |
| pane-notify stub correctness | Acceptable | Fix thread leak before Linux impl |
| pane-notify API for pane-app | Ready | Fix doc example for EventKind |
| pane-proto tag.rs | Stale | Delete old types, add wire-level TagLine matching spec |
| pane-proto handshake types | Missing | Add before pane-app (expected) |
| Test: MAX_MESSAGE_SIZE | Missing | Add (security boundary) |
| Test: partial calloop reads | Missing | Add (correctness of accumulation) |
| Framing duplication | Tech debt | Consolidate into pane-proto wire.rs |

**Start building pane-app.** The foundation is sound. The session type machinery works, the crash safety property holds, the calloop integration is correct, and the API surface is right. Clean up tag.rs first, then proceed.
