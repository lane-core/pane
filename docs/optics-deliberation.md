# Optics Deliberation: Findings for Implementers

This document records findings from a structured deliberation involving four
specialist agents (optics theorist, session type consultant, Be systems
engineer, Plan 9 systems engineer) on two questions:

1. What if pane were rebuilt with profunctor optics as the foundational
   state access mechanism?
2. What if core subsystems were implemented in Haskell or OCaml?

Both were explored as thought experiments. The conclusions and harvestable
insights are recorded here for agents proceeding on the current redesign
(concrete optics, Rust, par + EAct).

---

## Part I: Profunctor optics as foundation

### Verdict

Don't do it. The current three-concern separation — session types for
protocol, optics for state decomposition, actors for concurrency — reflects
three genuinely independent algebraic structures. Unifying them under optics
would collapse distinctions that are load-bearing.

Optics belong where they already are: as the consistency guarantee between
views of state. Not as the primary mutation or communication mechanism.

### Three harvestable insights

#### 1. Obligation handles are linear lenses

Clarke et al. Definition 4.12. A `ClipboardWriteLock` decomposes state
(extract clipboard content), then commits a new value (set), exactly once.
This IS the linear lens pattern: `S -> (A, B -> T)` where the continuation
`B -> T` is used once.

**Design decision:** Don't build a `LinearLens` type. The current bespoke
obligation handle pattern (move-only, `#[must_use]`, Drop sends failure
terminal) is the correct Rust encoding. The linear lens recognition is
explanatory, not prescriptive.

**What to document:** Add a comment on each obligation handle type naming
the linear lens structure:

```rust
/// Obligation handle for clipboard writes.
///
/// Linear lens structure: handler state decomposes into
/// (lock_token, commit_continuation). The continuation is
/// used exactly once via .commit(). Drop fires the remainder
/// term (Revert).
pub struct ClipboardWriteLock { ... }
```

**What to test:** Obligation handles can't have GetPut/PutGet tests (the
handle is consumed). Test the two completion paths instead:

```rust
#[test] fn commit_completes_obligation() { /* .commit() consumes, no Drop side-effect */ }
#[test] fn drop_sends_failure_terminal() { /* drop without .commit() sends ReplyFailed/Revert */ }
```

**What NOT to do:** Don't create an `ObligationHandle` trait or
`LinearLens<S,A,B,T>` abstraction. Each obligation handle has bespoke
semantics. Abstracting over them adds type machinery without adding safety.

#### 2. Optic subtyping for capability negotiation

The subtyping lattice: `Iso > Lens > Getter`, `Iso > Prism > Review`.
A client requesting a Lens that gets offered only a Getter can still work
(read-only degradation).

**Design decision:** Extend the existing `Attribute` / `ReadOnlyAttribute`
distinction into a capability vocabulary for Phase 2 multi-server.

**Concrete types** (in `pane-proto/src/property.rs`):

```rust
/// What operations an attribute supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrAccess {
    /// Lens-backed: read and write.
    ReadWrite,
    /// Getter-backed: read only.
    ReadOnly,
    /// Computed: derived value, no backing field.
    Computed,
}
```

This maps to FUSE permissions: `ReadWrite` -> `0660`, `ReadOnly` -> `0440`.
It maps to Plan 9 permissions: Lens = rw, Getter = r-. And it provides the
vocabulary for Phase 2 capability negotiation: "I need a Lens" / "I can
offer a Getter" -> downgrade to read-only.

**Where it lives:** `pane-proto/src/property.rs` (type definition),
`pane-fs/src/attrs.rs` (carried on `AttrAccessor`).

**What NOT to do:** Don't put the full optic subtyping lattice (Iso,
Lens, Prism, AffineTraversal, Traversal, Getter, Review, Fold, Setter)
into the API. Three levels (ReadWrite, ReadOnly, Computed) cover all
current needs. Extend only when a real use case demands it.

#### 3. PutPut as coalescing predicate

`set(set(s, a), b) = set(s, b)` — the formal condition under which queued
writes coalesce to just the last one.

**Design decision:** PutPut is definitional for Lens, so all `Attribute<S,A>`
instances satisfy it by construction (they're backed by lawful lenses with
PutPut tested). Coalescing is always safe for attributes. No marker trait
needed.

If a future optic type doesn't satisfy PutPut (e.g., an append-only log),
it should NOT be an `Attribute<S,A>` — it's a different optic (a Setter
without the Lens laws). Don't build that type until a concrete use case
exists.

**What to test:** The existing PutPut test in `property.rs` is sufficient.
Every new Attribute definition should include the same three law tests
(GetPut, PutGet, PutPut).

**Where coalescing happens:** In the looper's batch processing (Phase 4).
The looper collects events from all calloop sources into a unified batch.
If two `set` operations target the same attribute within one batch, the
looper can discard the earlier one. This is safe because PutPut holds for
all Attributes.

---

## Part II: Session / optic boundary rules

### The boundary

```
pane-session (par, Transport, Bridge)     -- session world
pane-proto   (Protocol, Message, Handler, Handles<P>, Attribute)
             -- shared vocabulary: session contracts AND optic definitions
pane-app     (Pane, PaneBuilder<H>, Dispatch<H>, Messenger, ServiceHandle<P>)
             -- actor runtime, consumes session contracts
pane-fs      (PaneEntry<S>, AttrSet<S>, AttrReader<S>)
             -- optic world
```

pane-proto is the membrane. Session vocabulary (`Protocol`, `Handles<P>`)
and optic vocabulary (`Attribute`, `ReadOnlyAttribute`) coexist in the same
crate, different modules. This is correct.

### The looper as mediator

The looper sequences both worlds:

1. Receive deserialized `P::Message` from bridge (session world)
2. Dispatch to `Handles<P>::receive(&mut handler, msg)` (session→handler)
3. After dispatch, clone/snapshot handler state (handler→optic)
4. Write snapshot to `PaneEntry::update_state` (optic world entry point)

The looper is not in either world. It is the runtime that sequences them.

### The value/obligation split IS an optic boundary

| | Values | Obligations |
|---|---|---|
| Optic type | Cartesian (Lens, Getter) | Linear (one-shot decompose/recompose) |
| pane-proto type | `Message` (Clone + Serialize) | move-only, !Clone, !Serialize |
| Dispatch path | `Handles<P>::receive` | typed callback |
| Filter visibility | yes (filter chain) | no (bypass) |
| State snapshot | yes (AttrReader reads from snapshot) | no (consumed, not projected) |
| Algebraic property | idempotent read (GetPut) | exactly-once consumption |

### Rules for implementers

**R1.** Never put an `Attribute<S,A>` inside a `Message` enum variant.
Attribute contains `Lens<'a, RcBrand, ...>` which is `!Send` and
`!Serialize`. Send the projected *value*, not the lens.

**R2.** Never send an `AttrValue` over a session channel. AttrValue is a
String wrapper for the pane-fs text interface. Protocol messages carry typed
data. Don't conflate filesystem format with wire format.

**R3.** Obligation handles are never `Message` variants. They're `!Clone`
and `!Serialize`. The `#[pane::protocol_handler]` macro generates a separate
dispatch path. Enforced by the type system.

**R4.** `Handles<P>::receive` must not read or write through `Attribute<S,A>`
or `AttrSet<S>`. The handler mutates its own fields directly. The optic
layer reads from a state snapshot that the looper updates *after* dispatch.

**R5.** `AttrReader<S>` closures must be pure: no side effects, no IO, no
panics. They run on the pane-fs thread, outside the looper's catch_unwind
boundary. A panic here kills pane-fs, not the looper.

**R6.** `PaneEntry::update_state` is called only by the looper, after
dispatch, before the next event. This is the single synchronization point.

**R7.** Filters operate on `Message` types only. Never see obligation
handles. Enforced by `MessageFilter<M: Message>` bound.

**R8.** `ServiceHandle<P>` lives in the handler struct, not in the optic
layer. It's `!Clone` and its Drop fires `RevokeInterest`.

**R9.** `AppPayload: Clone + Send + 'static` prevents smuggling obligations
through `post_app_message`. Don't circumvent with `Arc<Mutex<Option<_>>>`.

**R10.** Protocol-specific methods on `ServiceHandle<P>` are the correct
way to initiate obligation-bearing interactions. The handler never calls
par session primitives directly.

---

## Part III: Be scripting protocol → pane optics

### Command mapping

| Be command | pane operation | Status |
|-----------|---------------|--------|
| B_GET_PROPERTY | `read /pane/<id>/attrs/<name>` via AttrReader | Exists |
| B_SET_PROPERTY | `write /pane/<id>/ctl` ("set <prop> <val>") | Needs AttrWriter |
| B_COUNT_PROPERTIES | `readdir /pane/<id>/attrs/` via AttrSet::names() | Exists |
| B_CREATE_PROPERTY | Doesn't map (pane attributes are fixed at setup) | Correct omission |
| B_DELETE_PROPERTY | Doesn't map (same reasoning) | Correct omission |
| B_EXECUTE_PROPERTY | `write /pane/<id>/ctl` ("invoke <cmd> [args]") | Needs ctl handler |

### What to build

**AttrInfo** — static declaration of scriptable properties (Be's
`property_info`). Add to `pane-proto/src/property.rs`:

```rust
pub struct AttrInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub access: AttrAccess,
    pub value_type: &'static str,
}
```

**Scriptable trait** — connect handler state to the property system:

```rust
pub trait Scriptable: Clone + Send + 'static {
    fn supported_attrs() -> &'static [AttrInfo];
    fn attr_set() -> AttrSet<Self>;
}
```

**AttrWriter** — type-erased write path, add to `pane-fs/src/attrs.rs`:

```rust
pub struct AttrWriter<S> {
    pub name: &'static str,
    writer: Box<dyn Fn(&mut S, &str) -> Result<(), WriteError> + Send + Sync>,
}
```

**ctl file** — new module `pane-fs/src/ctl.rs`. Parse "set <prop> <val>"
and "invoke <cmd> [args]". Writes dispatch through a channel to the looper.
Reads return the list of accepted commands (self-documenting).

**Handler additions** — add to `pane-proto/src/handler.rs`:

```rust
fn supported_attrs(&self) -> &'static [AttrInfo] { &[] }
fn command_received(&mut self, command: &str, args: &[&str]) -> Flow {
    let _ = (command, args);
    Flow::Continue
}
```

**Derive macro** — `#[derive(Scriptable)]` on handler state structs.
Mark fields with `#[scriptable]`. Default: ReadWrite if `FromStr + Display`,
ReadOnly if only `Display`. Build this LAST — get the hand-coded path
working first.

### What NOT to build

1. **Don't force events through optics.** Key presses are events dispatched
   through `Handles<Display>::receive`. Not property mutations. If your
   lens's `set` method doesn't make semantic sense, it's an event, not a
   property.

2. **Don't expose optic vocabulary to app developers.** The developer writes
   `#[scriptable]` and never hears "lens" or "profunctor." If `Lens`,
   `Getter`, or `Traversal` appear in doc examples for app developers, the
   abstraction is leaking.

3. **Don't rebuild the BHandler tree.** No `ResolveSpecifier`, no handler
   chains, no `Vec<Box<dyn ScriptableHandler>>`. The filesystem path IS
   the address. Each pane is flat.

4. **Don't allow dynamic property registration after run_with.** Attributes
   are declared at setup time. If `AttrSet::add()` is called after the
   looper starts, the design is wrong.

5. **Ctl verbs have distinct semantics.** State-mutating commands
   are idempotent. Lifecycle commands are control flow. Effectful
   commands combine mutation with protocol effects. See
   `optics-design-brief.md` §Ctl dispatch architecture for the
   open question on optic-routed vs freeform dispatch.

---

## Part IV: pane-fs implementation guidance

### Namespace operations

| Operation | Optic equivalent | Implementation |
|-----------|-----------------|----------------|
| ls /pane/ | traversal over PaneEntries | Needs PaneRegistry |
| ls /pane/3/attrs/ | AttrSet::names() | Exists |
| cat /pane/3/attrs/cursor | AttrReader::read() | Exists |
| echo 42 > /pane/3/attrs/cursor | AttrWriter::write() | Needs AttrWriter |
| cat /pane/3/ctl | list accepted commands | Needs ctl module |
| echo quit > /pane/3/ctl | command dispatch via looper channel | Needs ctl module |

### Type erasure

`PaneEntry<S>` is generic but the namespace holds entries for different
state types. Introduce a `PaneNode` trait:

```rust
pub trait PaneNode: Send + Sync {
    fn id(&self) -> u64;
    fn tag(&self) -> &str;
    fn read_attr(&self, name: &str) -> Result<AttrValue, AttrReadError>;
    fn attr_names(&self) -> Vec<&'static str>;
    fn write_ctl(&self, cmd: &str) -> Result<(), CtlError>;
}
```

Namespace holds `HashMap<u64, Arc<dyn PaneNode>>`.

### Snapshot consistency

Replace `pub state: S` with `ArcSwap<S>` (atomic swap, zero-contention).
FUSE threads never block the looper. Looper never blocks FUSE threads.

```rust
pub struct PaneEntry<S> {
    pub id: u64,
    pub tag: String,
    pub attrs: AttrSet<S>,
    state: ArcSwap<S>,
}

impl<S> PaneEntry<S> {
    pub fn update_state(&self, state: S) {  // &self, not &mut self
        self.state.store(Arc::new(state));
    }
}
```

**Consistency guarantee:** per-pane snapshot consistency, per-ctl-write
barrier, no cross-pane ordering. Document this.

### Failure model

A crashed pane's namespace entry is **removed**, not left stale.
Concurrent reads in flight return EIO. New reads return ENOENT.

**FUSE error mapping:**
- ENOENT: attribute not found / pane not found
- EPERM: write to read-only attribute
- EIO: pane crashed / getter failed
- EINVAL: bad ctl command or parse failure

### Ctl write semantics

Ctl writes are **synchronous**: FUSE write blocks until the looper processes
the command and updates the snapshot. This preserves write-then-read
consistency. Budget: ~15-30us round-trip.

### Rules for pane-fs implementers

1. AttrValue is always text. Serialize through Display, deserialize through
   FromStr. Never binary.
2. Ctl dispatch is an open question — see `optics-design-brief.md`
   §Ctl dispatch architecture. Lifecycle commands are not optics.
   State-mutating commands may route through optics (monadic lens).
3. Ctl writes are synchronous. Write-then-read consistency depends on it.
4. A crashed pane's entry is removed immediately. Not left stale.
5. Snapshot updates use atomic swap (ArcSwap), not locking.
6. Attribute mode is determined by optic type at registration time.
   Lens -> ReadWrite, Getter -> ReadOnly.
7. One command per ctl write. First word is verb, rest are arguments.
8. Attribute names are &'static str, filesystem-safe: ASCII lowercase +
   underscore, no slashes/NUL/whitespace.
9. Four FUSE errors cover the space: ENOENT, EPERM, EIO, EINVAL.
10. The tag file (/pane/<n>/tag) is read-only text.

---

## Part V: Language split deliberation

### Verdict

Stay with Rust. The migration cost exceeds the gains for pane's current
protocol surface.

### Strongest argument for OCaml

Native algebraic effects eliminate bridge threads and give direct-style
`send_request`. OCaml's mutable record fields preserve the `&mut self`
feel of Handler methods. Jane Street's `accessor` library encodes the optic
subtyping lattice as row-polymorphic variant types. OCaml 5 domains provide
multicore. CBV evaluation makes obligation handle lifetimes predictable.

### Strongest argument for Haskell

`LinearTypes` extension (`%1 ->`) closes the affine gap. Rich optics
ecosystem (lens, optics, generic-lens). Typeclasses express protocol
interfaces more naturally than Rust traits.

### Why neither (now)

1. **par is the most production-ready session type implementation in any
   language.** Neither OCaml nor Haskell has a mature equivalent. Trading a
   working system for one built from scratch is wrong.

2. **Postcard must be replaced before splitting.** Postcard is Rust-native,
   serde-coupled, no cross-language implementations. The wire format must
   be language-agnostic first. This is valuable regardless of the language
   question.

3. **The type-level protocol work that would benefit from a functional
   language is already done** (par). The developer-facing layer (pane-app)
   is inherently imperative — an event loop dispatching to mutable handlers.

4. **One toolchain is worth more than the expressiveness gap** for a
   pre-1.0 project where the protocol isn't stable.

### What to do now (regardless of language choice)

1. **Replace postcard on the wire.** Hand-specified binary for control
   messages, CBOR for service payloads. This unblocks the option to split
   later.

2. **Write a byte-level protocol specification.** Not Rust types — a wire
   format document. Every message, every field, every encoding.

3. **Build language-independent conformance tests.** Wire traces (hex
   dumps) that any implementation can verify against.

4. **Evaluate OCaml after Phase 1 ships.** The split should be motivated
   by real pain, not anticipated elegance.

### Lane's CBV hunch

Partially confirmed. OCaml's CBV eliminates strictness annotation burden
(no `NFData` on every sent type), makes obligation handle construction
predictable (allocated immediately, Drop fires predictably), and matches
pane's dispatch model (each step evaluated eagerly). Gay/Vasconcelos JFP
2010 S5 proves session fidelity holds under both evaluation strategies, so
it's ergonomic, not formal.

### Inferno/Limbo as precedent

The closest historical parallel. Limbo was strict, GC'd, with channels
and modules. It implemented 9P servers successfully. OCaml is strictly
more capable. The Inferno lesson: the split works when the protocol
boundary is clean, but the ecosystem cost was real (nobody used Limbo
outside Inferno). Pane avoids this by keeping Rust as the ecosystem-facing
language.

---

## Open design decision

**PutPut coalescing strategy.** Three options:

A. Marker trait `Coalescable` that attributes opt into.
B. Runtime flag on AttrAccessor.
C. All Attributes are coalescable by definition (PutPut is a lens law).

Assessment: C is strongest. PutPut is definitional for Lens. If you have
an `Attribute<S,A>`, it satisfies PutPut by construction. If something
doesn't satisfy PutPut, it's not an Attribute — it's a different optic
type. Build that type when a use case exists, not before.
