# Full Codebase Review Against Be Newsletter Engineering Principles

Reviewer: Be Systems Engineer (R3-R5 alumnus)
Date: 2026-03-27
Scope: pane-proto, pane-session, pane-notify, pane-app — all source files, all test files

---

## Methodology

Each finding is grounded in a specific Be Newsletter principle, with the
engineer quoted, the issue number cited, and file:line references into the
pane codebase. Severity levels:

- **Critical** — will cause bugs in production, data loss, or security issues
- **Moderate** — should fix before Phase 4 (compositor integration)
- **Minor** — quality improvements, API polish, documentation

---

## Critical Findings

### C-1. Truncating `as u32` cast in framing can silently corrupt messages

**Principle:** Raynaud-Richard (#4-46) — "Remember, these numbers don't include
whatever memory will be used by the objects once you start using them."

The write_framed function in two places performs `data.len() as u32` — a
truncating cast that silently drops the high bits if `data.len() > u32::MAX`:

- `pane-session/src/framing.rs:11` — `let len = (data.len() as u32).to_le_bytes();`
- `pane-proto/src/wire.rs:41` — `let len = (data.len() as u32).to_le_bytes();`

If a message body exceeds 4GB (unlikely today, but the 16MB max check is on
the *read* side, not the *write* side), the length prefix wraps and the
receiver reads the wrong number of bytes. The receiver's MAX_MESSAGE_SIZE check
catches most cases, but the write side should fail explicitly rather than
sending a corrupt frame.

Compare with `pane-proto/src/wire.rs:15-19` (`frame()`) which correctly uses
`try_into()` and returns an error. The other two call sites bypass this safety.

**Fix:** Replace `data.len() as u32` with `u32::try_from(data.len()).map_err(...)`.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-session/src/framing.rs:11`
- `/Users/lane/src/lin/pane/crates/pane-proto/src/wire.rs:41`

### C-2. Duplicated framing code between pane-proto and pane-session

**Principle:** Gassee (#1-4) — "People developing the system now have to contend
with two programming models and two pieces of system software and the
coordination headaches between them."

Framing logic is implemented twice:

1. `pane-proto/src/wire.rs` — `write_framed`, `read_framed`, `MAX_MESSAGE_SIZE`, `frame`, `frame_length`
2. `pane-session/src/framing.rs` — `write_framed`, `read_framed`, `MAX_MESSAGE_SIZE`

These are nearly identical but independently maintained. If one is fixed (e.g.,
the C-1 truncation bug), the other may not be. This is exactly the
"two programming models" problem Gassee warned about with the DSP architecture.

`pane-session/src/transport/unix.rs:17` acknowledges this with a comment:
"Re-exported from framing module for backward compatibility." — but re-exporting
a constant doesn't solve the code duplication.

**Fix:** Either make pane-proto the single source of truth for framing (pane-session
depends on pane-proto's wire module) or extract framing into its own micro-crate.
Given that pane-session already depends on postcard, the simplest path is having
pane-session re-use pane-proto's wire functions.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-proto/src/wire.rs:1-62`
- `/Users/lane/src/lin/pane/crates/pane-session/src/framing.rs:1-31`

### C-3. `extract_pane_id` returns `Option` but is exhaustive over all variants

**Principle:** Owen Smith (#4-46) — the BMessage(BMessage*) lesson about types
that promise one thing and deliver another.

The function `extract_pane_id` in `pane-app/src/app.rs:158-172` matches every
variant of `CompToClient` and returns `Some(...)` for all of them. The return
type is `Option<u64>`, but `None` is never returned. This is an API lie — the
function signature promises fallibility that doesn't exist.

More importantly: when a new `CompToClient` variant is added that lacks a
PaneId field, the match will need a `None` arm, but the exhaustive pattern
today gives no compile-time signal that this happened. The function will
silently match the new variant and return its `pane` field — unless the new
variant doesn't have one, in which case it won't compile (good), but only if
the developer remembers this function exists.

The real fix is that `CompToClient` should expose `fn pane_id(&self) -> PaneId`
as a method, since every variant carries a PaneId. This makes the invariant
structural rather than implicit.

**Fix:** Add `pub fn pane_id(&self) -> PaneId` to `CompToClient` in pane-proto.
Remove `extract_pane_id` from pane-app. If a variant is ever added without a
PaneId, the method won't compile — the right behavior.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-app/src/app.rs:158-172`
- `/Users/lane/src/lin/pane/crates/pane-proto/src/protocol.rs:55-113`

### C-4. Pane channel registration race in `create_pane_inner`

**Principle:** Cisler (#3-33) — "If, while you're snoozing, your spawning window
is deleted and a similar one is created in its place, aliased to the same
pointer value..."

In `pane-app/src/app.rs:113-145`, the flow is:

1. Push a oneshot to `pending_creates`
2. Send `CreatePane` to compositor
3. Wait for `PaneCreated` response (gets PaneId)
4. Create the per-pane channel `(pane_tx, pane_rx)`
5. Insert `pane_tx` into `pane_channels` map

Between steps 3 and 5, the dispatcher thread has already registered the PaneId
(via the oneshot), but events sent to that PaneId will be silently dropped
because the pane's channel isn't registered yet. If the compositor sends a
Focus or Resize event between PaneCreated and the channel registration, that
event is lost.

The mock compositor works around this with a 200ms sleep in injection
(`mock.rs:107-109`), which confirms the race exists.

**Fix:** Register the pane channel in `pane_channels` *before* sending CreatePane.
Use a sentinel or prebake the PaneId. Alternatively, have the dispatcher queue
messages for unknown PaneIds and flush them when the channel is registered.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-app/src/app.rs:113-145`
- `/Users/lane/src/lin/pane/crates/pane-app/src/mock.rs:107-109`

---

## Moderate Findings

### M-1. `PaneId::get()` returns `u32` but `pane_channels` keys on `u64`

**Principle:** Owen Smith (#4-46) — implicit conversions and type confusion.

`PaneId` wraps `NonZeroU32` and `get()` returns `u32`. But in `app.rs:134`,
the pane channel map uses `u64` keys: `self.pane_channels...insert(pane_id.get() as u64, ...)`.

Every use of `pane_channels` does `pane.get() as u64` — this widening cast is
always safe, but it's a type smell. The map should be `HashMap<PaneId, Sender>`,
not `HashMap<u64, Sender>`. PaneId already derives Hash and Eq.

This is exactly the Owen Smith lesson: an unnecessary conversion that adds
cognitive load and invites bugs.

**Fix:** Change `pane_channels` to `HashMap<PaneId, mpsc::Sender<CompToClient>>`.
Remove all `as u64` casts from pane ID handling.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-app/src/app.rs:24` (map type)
- `/Users/lane/src/lin/pane/crates/pane-app/src/app.rs:74-78` (lookup)
- `/Users/lane/src/lin/pane/crates/pane-app/src/app.rs:134` (insert)
- `/Users/lane/src/lin/pane/crates/pane-app/src/app.rs:158-172` (extract)

### M-2. `write_message` in calloop.rs bypasses non-blocking

**Principle:** Hoffman (#2-36) — "Keeping a window locked or its thread
occupied for long periods of time is Not Good."

`pane-session/src/calloop.rs:69-71`:
```rust
pub fn write_message(stream: &UnixStream, data: &[u8]) -> io::Result<()> {
    crate::framing::write_framed(&mut &*stream, data)
}
```

The `SessionSource::new()` sets the stream to non-blocking mode (line 51),
but `write_message` calls `write_all` on the same stream. In non-blocking
mode, `write_all` can return `WouldBlock` if the kernel send buffer is full
— for instance, if the client isn't reading and the buffer backs up.

In a calloop event loop, this would block the entire compositor's event
processing. Hoffman's warning about app_server sync calls applies directly:
the write side needs the same care as the read side.

**Fix:** Either:
(a) Buffer writes and use calloop's write interest to flush when writable, or
(b) Keep a separate stream clone for writing that stays in blocking mode
    (the `reader` clone pattern already used for reads — do the same for writes).
Option (b) is simpler for now, but (a) is correct long-term for a compositor
that serves many clients.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-session/src/calloop.rs:51` (set_nonblocking)
- `/Users/lane/src/lin/pane/crates/pane-session/src/calloop.rs:69-71` (write_message)

### M-3. `pane.run()` always sends `RequestClose` even on error

**Principle:** Adams (#2-36) — "The only way to affect changes is to send a
message." Yes, but sending a message after a disconnect is wasteful and
potentially confusing.

In `pane-app/src/pane.rs:93`, after `looper::run_closure` returns:
```rust
let _ = comp_tx.send(ClientToComp::RequestClose { pane: id });
```

This runs unconditionally — even when the looper returned because of a
disconnect (PaneEvent::Disconnected), in which case the compositor channel
is already dead and the send will fail silently (ignored via `let _`).

More subtly: if the handler returned `Ok(false)` in response to a
`PaneEvent::Close` from the compositor, the pane is *already being closed
by the compositor*. Sending RequestClose back is redundant and could confuse
a compositor implementation that treats it as a new close request.

**Fix:** Distinguish exit reasons. Only send RequestClose when the handler
voluntarily exited (not in response to Close or Disconnected). The looper
should return an enum indicating why it stopped.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-app/src/pane.rs:87-99`
- `/Users/lane/src/lin/pane/crates/pane-app/src/pane.rs:107-119`

### M-4. Stub watcher thread is leaked — no cancellation mechanism

**Principle:** Raynaud-Richard (#4-46) — hidden resource costs.

The stub watcher in `pane-notify/src/stub.rs:47-79` spawns a thread that loops
forever with 200ms sleeps. The `WatchHandle` that represents the watch is an
inert struct — dropping it does nothing. The doc comment on `WatchHandle`
(`watcher.rs:8-13`) says "Drop it to unregister" but there's no implementation
of this contract for the stub.

In tests this doesn't matter much, but if pane-app uses pane-notify on macOS
during development, every `watch_path()` call leaks a thread permanently.

**Fix:** WatchHandle should carry a cancellation token (e.g., `Arc<AtomicBool>`)
that the polling thread checks on each iteration. When the handle is dropped,
set the flag and the thread exits.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-notify/src/stub.rs:47-79`
- `/Users/lane/src/lin/pane/crates/pane-notify/src/watcher.rs:8-13`

### M-5. `from_comp` clones KeyEvent and string fields unnecessarily

**Principle:** Schillings (#1-26, benaphore) — "Optimize for the common case."

`pane-app/src/event.rs:50-81`: Every `PaneEvent::from_comp()` call clones
the event data from the `CompToClient` reference. Key events, mouse events,
command strings — all cloned. This runs on every event dispatch, which is the
hottest path in the system.

The function takes `&CompToClient` but the caller (`looper.rs:46`) has an
*owned* `CompToClient` from `receiver.recv()`. The function could take
ownership instead of borrowing, avoiding all clones.

This is the benaphore lesson applied to allocation: the common case (owned
message from channel) should be zero-cost. The current design penalizes
every event dispatch with unnecessary allocation for the sake of a function
signature that borrows when it doesn't need to.

**Fix:** Change `from_comp` to take `msg: CompToClient` by value. Extract
fields by destructuring, no clones needed.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-app/src/event.rs:50` (function signature)
- `/Users/lane/src/lin/pane/crates/pane-app/src/looper.rs:46` (call site)
- `/Users/lane/src/lin/pane/crates/pane-app/src/looper.rs:80` (call site)

### M-6. No `Ready` event in the pane lifecycle

**Principle:** Schillings (#1-2) — "Common things are easy to implement and the
programming model is CLEAR."

`PaneEvent::Ready(PaneGeometry)` exists in the enum (`event.rs:13`) and the
Handler trait has a `ready()` method (`handler.rs:27`), but nothing ever
generates this event. The `from_comp` function (`event.rs:50-81`) has no arm
that produces `PaneEvent::Ready`.

The initial geometry comes from `CompToClient::PaneCreated`, but that message
is handled internally by the kit and never forwarded to the looper. The
developer never learns their initial geometry through the normal event path.

The `Pane::geometry()` method (`pane.rs:57`) provides the initial geometry,
but the pane is consumed by `run()`, so you can only call it before entering
the event loop. The Hello-Pane quick-start example doesn't show geometry
handling at all.

For a Handler implementation, this is confusing: `ready()` is in the trait,
it has a default, but it never fires. The developer implements it and nothing
happens.

**Fix:** After the pane's looper starts, synthesize a `PaneEvent::Ready`
with the initial geometry as the first event dispatched to the handler.
This is analogous to how BWindow received B_WINDOW_ACTIVATED after creation.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-app/src/event.rs:13` (Ready variant)
- `/Users/lane/src/lin/pane/crates/pane-app/src/event.rs:50-81` (no generation)
- `/Users/lane/src/lin/pane/crates/pane-app/src/handler.rs:27` (ready method)
- `/Users/lane/src/lin/pane/crates/pane-app/src/looper.rs:30-61` (no synthesis)

### M-7. AttrValue::Attrs uses Vec<(String, AttrValue)> — O(n) lookup

**Principle:** Schillings (#1-26) — "Optimize for the common case."

`pane-proto/src/attrs.rs:18` defines nested attributes as
`Attrs(Vec<(String, AttrValue)>)`. Attribute lookup is O(n) linear scan.
For small attribute sets this is fine, but if this type is used for file
system attribute indexing (as the comment at line 5-6 suggests), the
cost scales with the number of attributes per file.

BeOS attributes were stored in B+tree nodes with O(log n) lookup. Using
a Vec here means every attribute access on a file with many attributes
is linear.

**Fix:** Consider `BTreeMap<String, AttrValue>` for deterministic ordering
and O(log n) lookup. Or keep Vec but provide an `as_attrs_map()` accessor
that builds a map on demand. The serialization format (postcard) handles
maps fine.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-proto/src/attrs.rs:18`

---

## Minor Findings

### m-1. `PaneEvent` does not derive `PartialEq` — tests use pattern matching instead of equality

**Principle:** Schillings (#1-2) — "the programming model is CLEAR."

`PaneEvent` (`event.rs:11`) derives `Debug, Clone` but not `PartialEq`. This
means tests must use `matches!()` and nested patterns instead of direct
equality assertions. Compare with `CompToClient` which does derive `PartialEq`.

The reason `PartialEq` is missing is likely that `KeyEvent` contains floats
or other non-Eq types — but checking the types: `KeyEvent` derives `PartialEq`
and `MouseEvent` derives `PartialEq`. `PaneGeometry` derives `PartialEq`.
There's no barrier.

**Fix:** Derive `PartialEq` on `PaneEvent`.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-app/src/event.rs:11`

### m-2. Stale doc comments

Several doc comments reference names or patterns that don't exist yet or
are from earlier design iterations:

- `pane-app/src/pane.rs:76-79`: Doc comment says closure receives `PaneEvent`
  but the actual signature is `FnMut(&PaneHandle, PaneEvent)`. The proxy arg
  is not mentioned.
- `pane-app/src/event.rs:48`: Duplicated comment — "Convert a CompToClient
  wire message..." appears twice (lines 46-48 and 48-49).
- `pane-proto/src/message.rs:20-22`: Comment references "PaneRequest, PaneEvent,
  etc." and "pane-session" — this is stale from before the types were defined.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-app/src/pane.rs:76-79`
- `/Users/lane/src/lin/pane/crates/pane-app/src/event.rs:46-49`
- `/Users/lane/src/lin/pane/crates/pane-proto/src/message.rs:20-22`

### m-3. `FilterChain` is not `Default`

**Principle:** Schillings (#1-2) — common things should be easy.

`FilterChain::new()` returns an empty chain. It should also implement
`Default` for consistency with Rust conventions. Every use site
(`pane.rs:47`, all test files) calls `FilterChain::new()`.

**Fix:** Add `impl Default for FilterChain`.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-app/src/filter.rs:31-34`

### m-4. `App::connect()` is a permanent stub returning `NotRunning`

This is documented behavior for Phase 3, but it means the real connection
path (unix socket discovery, session-typed handshake, roster registration)
is completely untested. The only tested path is `connect_test()`.

This is fine for now but becomes a risk at Phase 4. Flagging it so it
doesn't get forgotten.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-app/src/app.rs:40-42`

### m-5. `PaneId::new()` has no access control — doc says "only compositor should call"

**Principle:** Owen Smith (#4-46) — API footguns.

`PaneId::new()` (`message.rs:11`) is public and the doc comment says "Only
the compositor should call this." But there's nothing preventing client code
from constructing arbitrary PaneIds and using them to address other clients'
panes. This is a trust boundary issue.

In BeOS, BWindow IDs were assigned by the app_server and couldn't be forged
because the server validated them on every operation. In pane, the protocol
currently trusts the PaneId in every `ClientToComp` message.

This is minor now (compositor doesn't exist yet) but should be addressed
in Phase 4 design. The compositor must validate PaneId ownership per-session.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-proto/src/message.rs:11`

### m-6. proptest roundtrip for `AttrValue` doesn't cover `Float` variant

The proptest strategy `arb_attr_value()` in `roundtrip.rs:129-136` generates
`String`, `Int`, `Bool`, and `Bytes` but not `Float` or `Attrs` (nested).
This means the `AttrValue::Float(f64)` variant has no serialization roundtrip
coverage.

Float serialization is notoriously tricky (NaN, infinity, negative zero). If
postcard handles these correctly, the test should prove it. If not, better to
find out now.

**Fix:** Add `any::<f64>().prop_filter(...NaN...).prop_map(AttrValue::Float)`
to the strategy. For Attrs, add a recursive bounded strategy.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-proto/tests/roundtrip.rs:129-136`

### m-7. No roundtrip tests for `ClientToComp` or `CompToClient`

The proptest suite covers individual types (Color, KeyEvent, PaneTitle, etc.)
but not the composite protocol enums that are actually serialized on the wire.
These are the types that would break compatibility if a variant were added
incorrectly or a field order changed.

**Fix:** Add proptest strategies for `ClientToComp` and `CompToClient` and
roundtrip them.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-proto/tests/roundtrip.rs`

### m-8. `Pane` doc comment for `run()` shows wrong closure signature

`pane.rs:74-79` shows:
```rust
/// pane.run(|event| match event {
```
But the actual signature at line 87 is:
```rust
pub fn run(self, handler: impl FnMut(&PaneHandle, PaneEvent) -> Result<bool>)
```
The closure takes `(&PaneHandle, PaneEvent)`, not just `(PaneEvent)`.

**Files:**
- `/Users/lane/src/lin/pane/crates/pane-app/src/pane.rs:74-79`

---

## Principle-by-Principle Assessment

### 1. Schillings (#1-2): "Common things are easy, programming model is CLEAR"

**Grade: Strong.** The hello-pane example in `lib.rs:9-27` is genuinely simple
— 14 lines from connect to event loop. The Tag builder (`tag.rs`) is clean.
The `cmd()` function is a nice touch. The Handler trait defaults are sensible
(close defaults to exiting, everything else defaults to continuing).

Weak spots: the Ready event never fires (M-6), FilterChain lacks Default (m-3),
and some doc comments are stale (m-2). These are polish issues, not structural
problems.

### 2. Schillings (#1-26): "Optimize for the common case"

**Grade: Adequate with one hotpath issue.** The session type channel
implementation is lean — PhantomData zero-cost, transport is the only runtime
state. The benaphore lesson applies: in the common case (single pane, simple
handler, no contention), the overhead is minimal.

The `from_comp` cloning issue (M-5) is the notable gap. Every event dispatch
allocates unnecessarily. For a compositor processing input events at 1000Hz,
this matters.

### 3. Potrebic (#2-35 to #2-38): Handler/messaging discipline

**Grade: Very strong.** The Handler trait (`handler.rs`) is well-structured.
Default methods are correct. The BLooper pattern (one thread, one message
queue, sequential processing — `looper.rs:1-10` doc comment) is faithfully
implemented. The Filter chain (`filter.rs`) is clean BMessageFilter.

The pane-channel registration race (C-4) is the one threading violation.
Otherwise, ownership is clear: the App owns the dispatcher thread, each Pane
owns its receiver, and PaneHandle is the BMessenger (clone and send from
anywhere).

### 4. Hoffman (#2-36): "Interface Kit caches async calls"

**Grade: Incomplete but correctly designed.** The protocol is predominantly
async — `ClientToComp` messages are fire-and-forget sends via mpsc channels.
The only sync call is `create_pane()` (send CreatePane, wait for PaneCreated
with timeout). This matches Hoffman's advice: minimize sync calls.

Missing: no message batching. Every `set_content()` call is a separate channel
send. Hoffman noted that batching async calls was critical for app_server
performance. For Phase 4, consider a `PaneHandle::batch()` method that
accumulates updates and sends them as a single message.

The `write_message` non-blocking issue (M-2) is the compositor-side risk.

### 5. Adams (#2-36): "Only way to affect changes is to send a message"

**Grade: Correct.** PaneHandle (`proxy.rs`) is the only way to affect
compositor state from client code. State mutation always goes through a
`ClientToComp` message. The Pane struct is consumed by `run()`, so the
developer can't accidentally hold a mutable reference to pane state while
the event loop is running. This is structurally enforced ownership.

The unconditional RequestClose on exit (M-3) is the one place where a
message is sent that shouldn't always be sent.

### 6. Cisler (#3-33): Thread synchronization

**Grade: Sound with one race.** The threading model is clean: mpsc channels
for communication, Arc<Mutex<_>> for shared state (pane_channels, pending_creates,
log), AtomicUsize for counters. Condvar for wait().

The C-4 race is the only synchronization issue found. No deadlock potential
in the current code — locks are always acquired singly, never nested.

### 7. Owen Smith (#4-46): BMessage(BMessage*) lesson — API footguns

**Grade: Good, two gaps.** The API is generally explicit. Types are clear.
The Tag builder prevents invalid command configurations.

Two footguns: PaneId forgeability (m-5) and the u64 key type for pane channels
(M-1). The PaneId issue is a trust boundary problem for Phase 4. The u64
key is an unnecessary lossy conversion that adds cognitive noise.

### 8. Raynaud-Richard (#4-46): Memory/resource costs

**Grade: Good for the kit, concern in pane-notify.** Per-pane overhead is
minimal: one mpsc receiver, one mpsc sender clone, one atomic counter share.
No threads spawned per pane (the pane's thread is the caller's thread via
`run()`). This is lighter than BeOS's ~56KB per window.

The stub watcher thread leak (M-4) is a resource concern. Each `watch_path()`
leaks a thread permanently on macOS.

The calloop SessionSource (`calloop.rs:35`) allocates a Vec for the
accumulation buffer that grows with message size but never shrinks. For
long-lived compositor connections this could accumulate — not critical but
worth noting.

---

## Test Quality Assessment

### Coverage

**pane-proto** (12 tests): Proptest roundtrips cover all individual types.
FKey validation is thorough. Missing: no Float/Attrs coverage (m-6), no
composite protocol enum roundtrips (m-7).

**pane-session** (15 tests): Excellent coverage of the core session type
machinery. Tests cover: simple request-response, multi-step protocols,
crash recovery (both mid-conversation and immediate), branching (2-way
and 3-way), the request() combinator, finish() transport reclamation,
unix sockets, calloop integration, fragmented writes, and oversized
message rejection. This is the strongest test suite in the project.

**pane-notify** (3 tests): Minimal but functional. Creation detection,
deletion detection, nonexistent path error. Platform-gated correctly.

**pane-app** (49 tests across 7 files): Good behavioral coverage. Tests
cover: tag building, event conversion, pane lifecycle (hello-pane
acceptance test), PaneHandle message correctness and disconnect handling,
Handler defaults, closure-based and handler-based looper dispatch,
filter chain behavior (consume, transform, empty chain, consume-all),
error propagation, message ordering.

### Test Quality Observations

1. **The hello-pane test is excellent** (`hello_pane.rs`). It's a true
   acceptance test — creates an App, creates a pane, injects a Close,
   verifies the lifecycle completes cleanly. The eprintln diagnostics
   are good for debugging CI failures.

2. **The looper tests are thorough** (`looper.rs`). They test the exact
   contract: closure and handler dispatch, wrong-pane-id filtering,
   filter chain ordering, error propagation, message ordering before
   disconnect. The `filter_consume_all_then_disconnect` test is
   particularly smart — it verifies that Disconnected bypasses filters.

3. **Missing: stress/concurrency tests.** No test creates multiple panes
   simultaneously, which would exercise the dispatcher thread under
   contention. No test sends many events in rapid succession to test
   channel backpressure.

4. **Missing: pane-session calloop write tests.** The calloop integration
   tests (`calloop_integration.rs`) test read path thoroughly but the
   write path (`write_message`) is tested only incidentally.

---

## Cross-Crate Design Assessment

### What's Working Well

1. **The layering is correct.** pane-proto defines the protocol types
   (wire format). pane-session provides the session-typed handshake
   machinery. pane-app is the developer-facing kit that hides session
   types entirely. The developer never sees `Chan`, `Send`, `Recv`,
   or `Transport`. This matches the Be principle that the API is the
   user interface for developers — hide the mechanism, expose the intent.

2. **The BLooper → Looper translation is faithful.** One thread, one
   message queue, sequential dispatch. Filter chain for cross-cutting
   concerns. Handler trait with sensible defaults. This is the right
   level of abstraction.

3. **PaneHandle is the right BMessenger translation.** Cloneable,
   sendable, fire-and-forget. The only way to mutate compositor state
   from client code. Clean separation between the handle (which you
   pass around) and the pane (which you consume into the event loop).

4. **Session types for the handshake, typed enums for the active phase.**
   This is the right split. Session types buy you compile-time
   correctness for the handshake protocol where the state machine
   matters. The active phase where both sides send freely uses simple
   enum dispatch. Trying to session-type the active phase would be
   over-engineering.

### What Needs Attention Before Phase 4

1. **Fix the framing duplication (C-2).** This is a maintenance hazard
   that will bite when the wire format changes.

2. **Fix the pane registration race (C-4).** The compositor will send
   events immediately after PaneCreated. The mock's 200ms sleep is a
   band-aid.

3. **Emit the Ready event (M-6).** The Handler contract is incomplete
   without it.

4. **Take ownership in from_comp (M-5).** The event dispatch hotpath
   should be allocation-free for the common case.

5. **Address write_message blocking (M-2).** When the compositor is
   serving real clients, a blocked write stalls the event loop.

---

## Summary Counts

| Severity | Count | Key Issues |
|----------|-------|------------|
| Critical | 4 | Truncating cast, framing duplication, extract_pane_id lie, registration race |
| Moderate | 7 | u64 key type, write blocking, unconditional RequestClose, stub thread leak, from_comp clones, no Ready event, Vec-based attrs |
| Minor    | 8 | No PartialEq, stale docs, no FilterChain Default, connect stub, PaneId forgery, Float coverage, no protocol roundtrips, wrong doc signature |

Total: 19 findings.
