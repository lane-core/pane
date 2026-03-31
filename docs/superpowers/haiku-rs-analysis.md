# Optics Readiness: haiku-rs Cross-Evaluation and API Foundation Assessment

Research document for the optics design agent. March 30, 2026.

---

## 1. Introduction

[haiku-rs](https://github.com/nielx/haiku-rs) (crate `haiku`, v0.3.0,
MIT, by Niels Sascha Reedijk) is a set of Rust FFI bindings to Haiku's
C++ API. It is an *independent* Be-to-Rust translation by a Haiku core
developer, covering the Application Kit (Message, Messenger, Looper,
Handler, Application, Roster), Kernel Kit (Port, Team), Storage Kit
(file attributes via `AttributeExt`), and Support Kit (Flattenable,
HaikuError). It is NOT a compatibility target for pane. It is valuable
as a second opinion on translation choices: where two independent
translators converge, the translation is likely sound; where they
diverge, the divergence deserves scrutiny.

This report evaluates pane's current API as a foundation for the optic
layer, using haiku-rs as a cross-reference. The evaluation criteria
come from the optics design brief (`docs/optics-design-brief.md`):

1. **Convergence constraint** -- Messenger property setters and
   scripting optics must share implementation (or be designed so they
   *can* share it when Phase 6 lands).
2. **DynOptic design** -- Type-erased optic dispatch at protocol
   boundaries must be dyn-compatible, reference-returning, and carry
   enough metadata for introspection.
3. **AttrValue / type codes** -- The serialization boundary type must
   serve both the scripting wire and filesystem attributes.
4. **Affine as default** -- Scripting targets may not exist; partial
   optics are the right default, not total lenses.
5. **Ghost state discipline** -- Correlation IDs at API surfaces
   should become typed handles where possible; remaining tokens are
   recognized as ghost state.
6. **PropertyInfo richness** -- Property declarations must carry
   operations, specifier forms, and value types, not just
   name/description/writable.

For each area below: Is pane's translation sound? Is it the right
foundation for optics? If not, what changes before optics work begins?

---

## 2. Per-Area Mutual Evaluation

### 2.1 Message Type

**haiku-rs:** Dynamic bag. `Message` stores a `message_header`
(repr(C), binary-compatible with Haiku's `1FMH` format), a
`Vec<field_header>`, and a raw `Vec<u8>` data buffer. Access via
`add_data::<T: Flattenable>("name", &value)` /
`find_data::<T: Flattenable>("name", index)`. Type safety is entirely
runtime -- wrong type code yields `HaikuError`.

**pane:** Typed enum. `Message` is a flat `enum` with typed payloads
(`Key(KeyEvent)`, `Resize(PaneGeometry)`, `AppMessage(Box<dyn Any>)`).
Dispatch is exhaustive pattern matching. User-defined data enters via
`AppMessage(Box<dyn Any + Send>)`, with the recommended pattern being
a private enum + single downcast.

**Translation soundness:** Both translations are correct for their
context. haiku-rs MUST reproduce the dynamic format for wire
compatibility with Haiku's C++ apps. pane SHOULD use typed enums
because it defines both endpoints. The two translators agree on the
fundamental insight: Rust's type system makes the dynamic bag
unnecessary when you control both sides.

**Optics foundation assessment:** The Message enum is NOT the optics
surface -- it is the *transport*. Optics operate on handler *state*,
not on messages. The enum's exhaustiveness is a strength: every
compositor event reaches a handler method, and the dispatch function
in `looper.rs` (`dispatch_to_handler`) is a mechanical 1:1 mapping.
This means the optic layer does not need to touch Message at all --
it lives in `ScriptableHandler::State`, accessed through `DynOptic`,
and transported over a *separate* scripting sub-protocol.

The one exception: `Message::AppMessage(Box<dyn Any + Send>)` is the
current escape hatch for user data. When the optic layer arrives,
scripting queries will need their own transport path (the brief's
deferred `ScriptQuery`/`ScriptResponse`). `AppMessage` remains for
non-scripting user data (worker thread results). No change needed.

**Verdict: solid ground.** The Message enum is sound, correctly
separated from the state access concern, and not load-bearing for
optics.

### 2.2 Messenger / IPC

**haiku-rs:** `Messenger { port: Port, token: i32 }`. Three send
modes: `send_and_wait_for_reply` (sync, creates temp reply port per
call), `send_and_ask_reply` (async with explicit reply destination),
`send` (fire-and-forget). No deadlock guard. Global state via
`ROSTER`/`LAUNCH_ROSTER` for `from_signature` lookup.

**pane:** `Messenger` wraps `mpsc::Sender<ClientToComp>` (to
compositor) + `Option<mpsc::SyncSender<LooperMessage>>` (to own
looper). Three send modes: `send_message` (self-delivery, blocking),
`send_and_wait` (sync with `is_looper_thread()` deadlock guard),
`send_request` (async, returns token). Property setters like
`set_title()`, `set_content()` send directly to compositor via
`ClientToComp` enum. `ReplyPort` enforces reply discipline.

**Translation soundness:** Strong convergence on the core shape
(handle + send methods) with pane adding safety mechanisms that
haiku-rs inherits from Be's lack of:

- Deadlock prevention (`WouldDeadlock` error) -- neither Be nor
  haiku-rs has this.
- Reply discipline (`ReplyPort` with `#[must_use]` + drop-sends-failure)
  -- haiku-rs has nothing equivalent; unreplied messages hang.
- Backpressure (bounded channel) -- haiku-rs relies on kernel port
  capacity (default ~200 messages) with no explicit backpressure.

haiku-rs's `from_signature` (system-wide lookup by MIME type) is a
gap in pane, tracked as Tier 2 in PLAN.md. Not optics-blocking.

**Optics foundation assessment -- the convergence constraint:**

This is where the optics brief's most critical concern lives.
Currently, Messenger has hardcoded property setters:

```rust
pub fn set_title(&self, title: PaneTitle) -> Result<()> {
    self.send(ClientToComp::SetTitle { pane: self.id, title })
}
pub fn set_content(&self, content: &[u8]) -> Result<()> { ... }
pub fn set_hidden(&self, hidden: bool) -> Result<()> { ... }
```

Each of these sends a specific `ClientToComp` variant to the
compositor. When the optic layer lands, each must be expressible as
`optic.set(state, value)` where the optic is the ground truth. The
current design does NOT prevent this -- the methods are thin wrappers
around `self.send(ClientToComp::Variant { ... })`, and could be
reimplemented as:

```
set_title(t) -> title_optic.set(&mut state, t) -> ClientToComp::SetTitle
```

The key structural requirement: the `DynOptic` for "title" must be
able to produce a `ClientToComp::SetTitle` message as a *side effect*
of its `set` operation, or the Messenger method must call the optic
and then produce the message. Either path works with the current
design.

haiku-rs's messenger has no property setters at all -- it's pure
message transport. This is correct for FFI bindings but confirms that
pane's "convenience setter on Messenger" pattern is a pane-specific
design choice. The optic layer needs to be the authority behind these
setters, but the *existence* of the setters is fine.

**Verdict: solid ground, with one design constraint.** The optic
layer must be designed so that `Messenger::set_title(t)` can delegate
to a title optic's `set` operation, which then produces the
`ClientToComp` message. The current thin-wrapper structure supports
this. No refactoring needed before optics work; the convergence
happens when optics are wired in.

### 2.3 Handler Dispatch

**haiku-rs:** Single method: `message_received(&mut self, context, message)`.
All dispatch is manual `match msg.what()` in user code. This is
faithful to Be but maximally un-ergonomic in Rust.

**pane:** Per-event methods on the `Handler` trait, each with a
default returning `Ok(true)`. Automatic dispatch by looper
(`dispatch_to_handler` in `looper.rs`). Return value controls the
loop: `Ok(true)` = continue, `Ok(false)` = stop, `Err` = error exit.

**Translation soundness:** Divergence, and pane is right. haiku-rs
*can't* do per-event dispatch because its messages are dynamic.
pane *must* do it because it has typed messages. Both translators
agree that the trait replaces C++ virtual inheritance -- the right
Rust idiom (translation rule 4).

**Optics foundation assessment:**

The Handler trait is where `ScriptableHandler` will attach. The
current trait has no scripting methods -- no `resolve_specifier`, no
`supported_properties`. The optics brief proposes:

```rust
pub trait ScriptableHandler {
    type State;
    fn resolve_specifier(&self, spec: &Specifier) -> Resolution;
    fn supported_properties(&self) -> Vec<PropertyInfo>;
    fn state_mut(&mut self) -> &mut Self::State;
}
```

The question: should `ScriptableHandler` be a supertrait of `Handler`,
a separate trait, or a blanket impl?

My recommendation: **separate trait, not a supertrait.** Reasons:

1. Not every handler needs to be scriptable. A splash screen pane has
   no properties worth exposing. Forcing `ScriptableHandler` as a
   supertrait would add ceremony to simple handlers.
2. Be's architecture had the same separation in spirit: `BHandler`
   had `ResolveSpecifier` and `GetSupportedSuites` as virtual methods
   with default no-op implementations. But in pane, these defaults
   would need a `type State` associated type, which means even
   non-scriptable handlers would need to specify a state type.
3. The `#[derive(Scriptable)]` proc macro (deferred) can generate the
   `ScriptableHandler` impl from struct annotations, keeping the
   Handler trait clean.

The handler's `&mut self` receiver is the optic target -- it provides
the `&mut State` that optics operate on. This is exactly right:
single-threaded access within the looper, no locking needed, the
borrow checker replaces `BLooper::Lock()`.

**Verdict: solid ground.** Handler trait is the right shape. Add
`ScriptableHandler` as a companion trait, not a modification.

### 2.4 Application Lifecycle

**haiku-rs:** `Application<A: ApplicationHooks + Send>` --
parameterized over a state type. Inherits looper behavior, runs the
main message loop, processes application-level events
(`ready_to_run`, `argv_received`, `message_received`). Global context
via `ROSTER` / `LAUNCH_ROSTER`.

**pane:** `App` -- not a looper, not generic. Factory + wait.
`connect(signature)` establishes compositor connection,
`create_pane(tag)` spawns panes, `run()` blocks until all panes
close. No application-level message handling.

**Translation soundness:** Deliberate divergence, documented in
`beapi_divergences.md`. pane's App is simpler because per-pane loops
replace the application-level loop. haiku-rs keeps Application as a
looper because Haiku's system services send messages to the
application port. pane doesn't have this constraint. Both translations
are correct for their context.

**Optics foundation assessment:**

App is not an optic target. It has no state that scripting should
access. Application-level properties (signature, pane count) are
metadata, not handler state. If system-wide scripting is ever needed
("get me the list of panes for app X"), that's the compositor's
job, not App's.

haiku-rs's generic `Application<A>` shows what it looks like to
parameterize over app state at the type level. pane chose not to do
this, and that's fine -- the state lives in individual handlers, not
in App. The optic layer targets handler state, not application state.

**Verdict: solid ground.** App is correctly out of the optic path.

### 2.5 State Access Patterns

This is the core optics concern. How does state flow through the
system, and where do optics intercept it?

**haiku-rs:** No explicit state access pattern. Handler state is
whatever fields the `Handler` implementor has. The `Context<A>`
provides messengers and shared application state via
`Mutex<A>`. No property system, no introspection, no scripting
support. haiku-rs faithfully reproduces Be's lack of first-class
state access.

**pane (current):** Handler state is `&mut self` on the Handler trait.
Messenger property setters (`set_title`, `set_content`, etc.) bypass
handler state entirely -- they send `ClientToComp` messages directly
to the compositor. The scripting stub (`scripting.rs`) has an
`Attribute` struct with name/description/writable, `ScriptQuery`,
`ScriptOp`, and `ScriptReplyToken` -- all Phase 6 placeholders.

**The gap:** There is currently no mechanism to go from "I want to
read the title of pane X" to "call the getter that returns the
title." The Messenger setters write *to* the compositor, but there's
no corresponding read path that would go *through* the handler's
state. This is the gap the optic layer fills.

**pane (with optics, per the brief):**

```
Messenger::set_title(t) -> title_optic.set(&mut handler_state, t)
                        -> side effect: ClientToComp::SetTitle
scripting: "get Title"  -> title_optic.get(&handler_state)
                        -> returns AttrValue::String(title)
```

Both paths go through the same optic. The optic is the ground truth.

**What haiku-rs teaches here:** Nothing directly about optics (it has
none), but its `Flattenable` trait and `AttributeExt` trait for file
attributes reveal the *serialization boundary* pattern that `AttrValue`
must serve. haiku-rs's `Flattenable` is the type-erased serialization
layer -- `flatten() -> Vec<u8>`, `unflatten(&[u8]) -> Result<T>`,
with `type_code() -> u32` for runtime type identification. pane's
`AttrValue` serves the same role but as an enum rather than a trait:

```rust
// haiku-rs: trait-based, open (any Flattenable type)
msg.add_data::<String>("title", &t)?;
let t: String = msg.find_data("title", 0)?;

// pane (proposed): enum-based, closed (known variant set)
optic.set(state, AttrValue::String(t))?;
let v: AttrValue = optic.get(state)?; // AttrValue::String(...)
```

The closed enum is the right choice for pane because:

1. The optic layer must validate incoming values at the type boundary.
   With a closed enum, validation is exhaustive pattern matching.
   With an open trait, it's runtime type code checking (Be's approach,
   and the source of Be's "type confusion at wire boundary" problem
   that the brief identifies as something optics fix).
2. `DynOptic::get` returns `Result<AttrValue, ScriptError>` -- the
   return type must be object-safe. An enum is inherently object-safe;
   a trait object would require boxing.
3. The variant set is stable: strings, bools, integers, floats, bytes,
   and geometric types cover the scripting surface. Custom types go
   through `Bytes(Vec<u8>)` with application-defined serialization.

**Verdict: the scripting stub needs replacement, but the rest is
solid ground.** Handler's `&mut self` pattern, Messenger's thin
setters, and the looper's single-threaded dispatch are all the right
foundation. The `Attribute`/`ScriptQuery`/`ScriptReplyToken` stubs
are explicitly placeholders and will be replaced by
`PropertyInfo`/`DynOptic`/`ScriptReply(ReplyPort)` per the brief.

### 2.6 Error Handling

**haiku-rs:** `HaikuError` modeled on `std::io::Error`, with
`ErrorKind` enum (7 variants) and `Repr` that can carry an OS
`status_t`, a simple kind, or a boxed custom error. Maps Haiku's
`status_t` codes (`B_BAD_DATA`, `B_NAME_NOT_FOUND`, etc.) to
`ErrorKind` variants.

**pane:** Two-level enum: `Error` (top-level: Connect, Pane, Session,
Io) with `ConnectError` and `PaneError` as domain-specific enums.
`PaneError` has 5 variants including `WouldDeadlock` and `ChannelFull`
-- pane-specific conditions that have no Be equivalent.

**Translation soundness:** Both correctly adopt `Result<T, E>` over
`status_t`. pane's domain-specific enums are more structured than
haiku-rs's flat `ErrorKind`. No issues.

**Optics foundation assessment:**

The optic layer will need `ScriptError` for scripting-specific
failures (property not found, type mismatch, read-only violation,
specifier resolution failure). This is a new error domain, parallel
to `PaneError`. It should be a new enum variant under `Error`:

```rust
pub enum Error {
    Connect(ConnectError),
    Pane(PaneError),
    Script(ScriptError),  // new
    Session(pane_session::SessionError),
    Io(std::io::Error),
}
```

haiku-rs's `ErrorKind::InvalidData` and `ErrorKind::NotFound` map to
scripting errors (type mismatch, property not found) but as flat
variants. pane should be more specific: `ScriptError::TypeMismatch`,
`ScriptError::PropertyNotFound`, `ScriptError::ReadOnly`,
`ScriptError::SpecifierFailed`.

**Verdict: solid ground.** Add `ScriptError` enum when the optic
layer lands. No changes to existing error types.

---

## 3. Optics-Specific Concerns

### 3.1 Convergence Constraint

**Question:** Does haiku-rs's property system inform how Messenger
methods should delegate to optics?

haiku-rs has no property system at all. Its Messenger is pure
message transport. Be's original C++ `BMessenger::SendMessage` was
also pure transport -- property access went through the scripting
protocol (`hey Tracker set Title of Window 0` sent a `B_SET_PROPERTY`
message to the Tracker's looper, which dispatched to
`BHandler::MessageReceived`, which called `ResolveSpecifier`, which
walked the specifier chain to the right handler, which called the
appropriate setter).

The critical insight: **Be's property setters were never on the
messenger.** They were on the *handler* (via virtual methods like
`BWindow::SetTitle`), and the scripting protocol reached them through
message dispatch. pane's `Messenger::set_title()` is a shortcut that
bypasses this chain entirely -- it sends `ClientToComp::SetTitle`
directly to the compositor.

This shortcut is fine for the compositor-owned properties (title,
visibility, size), because those properties live in the compositor's
state, not the handler's. But for handler-owned properties (selection,
content model, application state), the optic path must go through the
handler.

**Recommendation for the optic layer:**

Two property classes, two paths:

1. **Compositor-owned properties** (title, hidden, size limits):
   `Messenger::set_title()` continues to send `ClientToComp` directly.
   The optic for "title" on the *compositor side* is internal to the
   compositor. On the client side, `set_title` is sugar over the wire
   protocol, not over a local optic.

2. **Handler-owned properties** (application state exposed for
   scripting): The optic lives in the handler. `ScriptableHandler`
   implementations declare these via `PropertyInfo`. The scripting
   protocol (Phase 6) queries them via `DynOptic`.

This is a refinement of the brief's "Messenger method delegates to
optic" model. For compositor-owned properties, the "optic" is on the
compositor side of the wire, not the client side. For handler-owned
properties, the optic is local and the brief's model applies directly.

### 3.2 DynOptic Design

**Question:** What does haiku-rs's Flattenable teach about
type-erased boundaries?

haiku-rs's `Flattenable<T>` trait:

```rust
pub trait Flattenable<T> {
    fn type_code() -> u32;
    fn is_fixed_size() -> bool;
    fn flattened_size(&self) -> usize;
    fn flatten(&self) -> Vec<u8>;
    fn unflatten(buffer: &[u8]) -> Result<T>;
}
```

This is the type-erasure pattern for Be's type system: each type has
a numeric code, a serialization, and a deserialization. It's the
moral equivalent of `DynOptic`'s `value_type() -> ValueType` method.

Key lessons:

1. **Type code as discriminant works.** haiku-rs proves that a small
   set of numeric type identifiers (`B_STRING_TYPE`, `B_INT32_TYPE`,
   etc.) is sufficient for a property system. pane's `ValueType` enum
   serves the same role but as a Rust enum rather than raw `u32`
   constants.

2. **`unflatten` is fallible.** haiku-rs returns `Result<T>` from
   deserialization, validating buffer length and format. `DynOptic::set`
   must similarly return `Result<(), ScriptError>`, validating the
   `AttrValue` variant against the property's declared `ValueType`.

3. **`Flattenable<T>` is NOT dyn-safe** (static methods
   `type_code()`, `is_fixed_size()` prevent it). haiku-rs works around
   this by using generics everywhere (`add_data::<T>`), which means
   the caller must know the concrete type. `DynOptic` avoids this by
   using `AttrValue` (the enum) as the type-erased boundary instead
   of `dyn Flattenable`. This is strictly better for pane's use case.

4. **The `dyn Any` downcast in `DynOptic::get/set` is the
   unavoidable cost.** The brief's `DynOptic` signature:

   ```rust
   fn get(&self, state: &dyn Any) -> Result<AttrValue, ScriptError>;
   fn set(&self, state: &mut dyn Any, value: AttrValue) -> Result<(), ScriptError>;
   ```

   The `&dyn Any` + downcast is where monomorphic handler state meets
   polymorphic scripting dispatch. haiku-rs doesn't face this because
   it never does type-erased state access -- its Handler trait is
   already monomorphic. pane needs the `dyn Any` boundary because
   `DynOptic` must work across handlers with different `State` types.

**No design changes needed.** The brief's DynOptic sketch is sound.
haiku-rs confirms that type-code-based discrimination works but that
pane's enum-based approach is superior to the numeric-code approach.

### 3.3 AttrValue / Type Codes

**Question:** What's the principled choice, informed by both
haiku-rs's Be type codes and pane's serde approach?

haiku-rs uses Be's `u32` type codes: `B_STRING_TYPE` (0x43535452),
`B_INT32_TYPE` (0x4c4f4e47), etc. These are four-char codes inherited
from Be's C++ era. Flattenable implementations map Rust types to
these codes.

pane uses serde/postcard for wire serialization. The `Message` enum's
variants are the type discrimination. There are no explicit type codes
anywhere in pane today.

**The principled choice:** `AttrValue` as a closed enum (per the
brief), with NO numeric type codes in the public API. Reasons:

1. **Rust's enum IS the type code.** `AttrValue::String(_)` carries
   its type identity in the discriminant. Adding a separate
   `type_code: u32` is redundant.

2. **File attribute compatibility is achievable without exposing
   codes.** When `pane-store` writes xattrs, it can use an internal
   mapping from `AttrValue` variants to BFS-compatible type codes for
   the attribute's type field. This mapping is an implementation
   detail of the storage layer, not an API surface.

3. **Scripting introspection uses `ValueType` enum, not raw codes.**
   `PropertyInfo` declares `value_type: ValueType`, where `ValueType`
   is an enum (`String`, `Bool`, `Int`, `Float`, `Bytes`, `Rect`).
   This is richer than a `u32` because it's exhaustive and
   self-documenting.

The brief's `AttrValue` sketch is correct as-is. One addition to
consider: `AttrValue::List(Vec<AttrValue>)` for traversal optics
that return multiple values (e.g., "get all items in a list
property"). Be's `BMessage` supported multiple values per field name
via the index parameter in `find_data("name", index)`. pane's
traversal optics will need a way to return collections.

### 3.4 Affine as Default

**Question:** Does haiku-rs's experience validate partial optics as
the default?

haiku-rs has no optics. But its Message API is relevant:
`find_data("name", index)` returns `Result<T>` -- the field might
not exist, the index might be out of range, the type might not match.
Every access is partial. This is Be's reality: message fields are
optional, handler properties might not be present, windows might be
closed between query and access.

pane's Handler methods already reflect this partiality:
`completion_request` has a `token: u64` that identifies a specific
request that might have expired. `pane_exited` reports on a pane
that no longer exists. `reply_received` matches against a token
for a conversation that might have been abandoned.

**Validation:** The affine-as-default choice is strongly validated
by both haiku-rs's experience and pane's own API:

- Scripting targets a pane that might be closed
  -> `PartialGetter::preview` returns `None`
- Scripting reads a property that the handler conditionally exposes
  -> `preview` returns `None`
- Scripting sets a read-only property
  -> `PartialSetter::set_partial` returns `false`

Total lenses (`Getter::get`) are appropriate only for properties that
*structurally must exist* -- e.g., a handler's title field if the
handler always has a title. The `#[derive(Scriptable)]` macro can
choose: `Option<T>` fields get affine optics, `T` fields get lenses,
and lenses auto-widen to affines at the `DynOptic` boundary.

**No design changes needed.** The brief's affine-as-default is
correct.

### 3.5 Ghost State Discipline

**Question:** haiku-rs has none -- what does the gap tell us?

haiku-rs's `Messenger` stores a `token: i32` that identifies the
target handler within a looper. This token is a global atomic counter
with no recycling (the code has `TODO` comments about this). It's
ghost state: a numeric correlation ID with no ownership semantics.
If the handler is removed, the token silently becomes invalid.

haiku-rs's `Message` stores reply routing in its header:
`reply_port`, `reply_target`, `reply_team` -- three i32 fields that
together address the reply destination. More ghost state: if the
reply port is closed between send and reply, the reply is lost.

pane has already eliminated the worst ghost state:

- **ReplyPort** replaces reply_port/reply_target/reply_team with a
  typed handle. Ownership enforced. Drop sends failure. This is the
  brief's ghost-state-to-typestate success story.

- **Handler tokens** don't exist in pane -- there's one handler per
  pane, addressed by `PaneId` (compositor-assigned, opaque).

Two ghost state instances remain in pane, identified by the brief:

1. **`CompletionRequest { token: u64, input: String }`** -- the
   `token` correlates a completion request with its response via
   `Messenger::set_completions(token, completions)`. This is
   untyped correlation that should become a `CompletionReplyPort`
   (consumed by `.reply(completions)`, Drop sends empty list).

2. **`Reply { token: u64, payload }` / `ReplyFailed { token: u64 }`**
   -- the `token` correlates async replies with their requests. This
   one is harder to eliminate because the handler needs to match the
   reply against the original request context. The token is
   *legitimate* ghost state: the async gap between `send_request`
   returning a token and `reply_received` delivering the reply cannot
   be bridged by ownership transfer (the handler needs to store the
   token somewhere to match against, and that somewhere is mutable
   handler state, not a linear channel).

**Recommendation:**

- `CompletionRequest` token -> **refactor before optics**. Replace
  with `CompletionReplyPort` per the brief. This is a small change
  (new type, consumed by reply, Drop sends empty). It eliminates a
  ghost state instance and demonstrates the pattern.

- `Reply`/`ReplyFailed` token -> **accepted ghost state**. Document
  it as such. The pattern "store token in handler state, match when
  reply arrives" is the right design for async request-response in a
  single-threaded event loop.

### 3.6 PropertyInfo Richness

**Question:** haiku-rs reproduces Be's property tables; how should
pane's version differ?

haiku-rs does not implement `property_info` tables at all -- it has
no scripting support. But Be's tables (faithfully preserved in Haiku's
source, e.g., `src/kits/interface/Window.cpp` lines 125-184) carried:

1. Property name (`"Title"`, `"Frame"`, `"Hidden"`)
2. Supported commands (bit flags: `B_GET_PROPERTY`, `B_SET_PROPERTY`,
   `B_COUNT_PROPERTIES`)
3. Supported specifier forms (`B_DIRECT_SPECIFIER`,
   `B_INDEX_SPECIFIER`, `B_NAME_SPECIFIER`)
4. Expected types for each command (`B_STRING_TYPE` for Title's
   GET/SET, `B_RECT_TYPE` for Frame's GET/SET)

pane's current stub (`scripting.rs`) has:

```rust
pub struct Attribute {
    pub name: String,
    pub description: String,
    pub writable: bool,
}
```

This is insufficient. The brief's `PropertyInfo` is the replacement:

```rust
pub struct PropertyInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub value_type: ValueType,
    pub operations: Vec<ScriptOp>,
    pub specifier_forms: Vec<SpecifierForm>,
}
```

**How pane's version should differ from Be's:**

1. **`&'static str` for name and description.** Be used C strings
   allocated in static tables. pane should use Rust static strings --
   zero-cost, no allocation, lifetime-safe. Properties are declared
   at compile time.

2. **`ValueType` enum instead of `u32` type code.** Exhaustive,
   self-documenting, pattern-matchable (discussed in 3.3).

3. **`Vec<ScriptOp>` instead of bit flags.** Be used `uint32`
   bitmasks (`B_GET_PROPERTY | B_SET_PROPERTY`). An enum + Vec is
   more Rust-idiomatic and can carry per-operation metadata if needed
   later.

4. **Descriptions.** Be's property tables had no descriptions.
   pane adds them for tooling (`hey`-like tools can display
   human-readable property descriptions).

5. **Type safety at declaration.** The `#[derive(Scriptable)]`
   macro (deferred) should validate that declared `ValueType` matches
   the field's Rust type. Be had no such validation -- you could
   declare a property as `B_STRING_TYPE` and return an `int32`.

**One concern:** `Vec<ScriptOp>` and `Vec<SpecifierForm>` allocate.
For static declarations, consider `&'static [ScriptOp]` and
`&'static [SpecifierForm]` instead. The proc macro can generate
static slices. This is a minor optimization but follows the principle
of zero-cost property tables.

---

## 4. Verdict: Refactor Scope Before Optics

### Solid Ground -- proceed directly

These are sound translations, good optics foundations, no changes
needed:

| Type/Trait | Why it's solid |
|-----------|---------------|
| `Message` enum | Correctly separated from state access; not optics-load-bearing |
| `Handler` trait | `&mut self` is the optic target; per-event dispatch is right; add `ScriptableHandler` alongside, not as modification |
| `Messenger` (core) | Thin setters over `ClientToComp` support convergence path; deadlock guard, reply discipline, backpressure all correct |
| `ReplyPort` | Ghost state eliminated; session-type ownership is the model for `CompletionReplyPort` and `ScriptReply` |
| `App` | Correctly out of the optic path |
| `Looper` internals | Single-threaded dispatch, coalescing, filter chain -- all correct, not optics-affected |
| `Error`/`PaneError` | Add `ScriptError` variant when optics land; no existing changes |
| `ExitReason` | Not optics-affected |
| `TimerToken` | Not optics-affected |
| `MessageFilter` / `FilterAction` | Not optics-affected |

### Refactor Before Optics

These need changes before the optic layer can be built soundly:

**1. Replace `Attribute` with `PropertyInfo`.**

File: `crates/pane-app/src/scripting.rs`

Current `Attribute` has name/description/writable. Replace with the
brief's `PropertyInfo`:

```rust
pub struct PropertyInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub value_type: ValueType,
    pub operations: &'static [ScriptOp],
    pub specifier_forms: &'static [SpecifierForm],
}
```

Add `ValueType` and `SpecifierForm` enums. Update `ScriptOp` to
match the brief (add `Execute`, `Create`, `Delete` to the existing
`Get`/`Set`/`Count`/`ListProperties`).

**2. Replace `ScriptReplyToken` with `ScriptReply(ReplyPort)`.**

File: `crates/pane-app/src/scripting.rs`

Current `ScriptReplyToken` is a stub with a raw `u64`. The optics
brief calls for `ScriptReply(ReplyPort)` -- a newtype over `ReplyPort`
with `ok(self, value: AttrValue)` and `error(self, err: ScriptError)`
methods. This unifies scripting replies with the existing reply
discipline.

**3. Replace `CompletionRequest { token: u64 }` with
`CompletionReplyPort`.**

Files: `crates/pane-app/src/event.rs` (Message variant),
`crates/pane-app/src/handler.rs` (handler method),
`crates/pane-app/src/proxy.rs` (`set_completions` method)

The `token: u64` in `CompletionRequest` and `set_completions(token,
completions)` is ghost state. Replace with a typed reply port:

```rust
// event.rs
CompletionRequest {
    input: String,
    reply: CompletionReplyPort,
}

// New type (in reply.rs or completions.rs)
pub struct CompletionReplyPort(ReplyPort);
impl CompletionReplyPort {
    pub fn reply(self, completions: Vec<Completion>) { ... }
    // Drop sends empty completion list
}
```

Remove `Messenger::set_completions`. The handler calls
`reply.reply(completions)` directly.

**4. Add `ScriptError` enum.**

File: `crates/pane-app/src/error.rs`

```rust
pub enum ScriptError {
    PropertyNotFound,
    TypeMismatch { expected: ValueType, got: ValueType },
    ReadOnly,
    IndexOutOfRange,
    SpecifierFailed(String),
}
```

Add `Error::Script(ScriptError)` variant.

### Free to Evolve

These are neither load-bearing for translation fidelity nor for optics
design. They can change or not as the project evolves:

| Item | Why it's free |
|------|-------------|
| `Tag` / `CommandBuilder` | UI configuration, orthogonal to optics |
| `KeyCombo` / shortcuts | Input handling, orthogonal to optics |
| `Pane::run` vs `Pane::run_with` | Two entry points to the same looper; optics work through Handler either way |
| Wire format (postcard) | Transport detail; optics are above the wire |
| `mock` module | Test infrastructure |
| `routing` module stub | Phase 6 placeholder, will be informed by optics but not blocked by current state |
| `connection` module | Handshake plumbing, orthogonal |

---

## Summary for the Optics Agent

pane's Be API translation is sound. haiku-rs confirms the major
design choices (typed Message over dynamic bag, trait Handler over
virtual class, ReplyPort over unguarded reply routing, no globals)
and reveals no translation flaws that would undermine the optics
foundation.

Four concrete refactors are recommended before optics implementation
begins:

1. `Attribute` -> `PropertyInfo` (richer property declarations)
2. `ScriptReplyToken` -> `ScriptReply(ReplyPort)` (reply discipline)
3. `CompletionRequest` token -> `CompletionReplyPort` (ghost state
   elimination)
4. Add `ScriptError` enum (scripting error domain)

All four are localized to `scripting.rs`, `event.rs`, `handler.rs`,
`proxy.rs`, and `error.rs`. None affects the core messaging,
threading, or dispatch architecture. The optic layer builds on solid
ground.
