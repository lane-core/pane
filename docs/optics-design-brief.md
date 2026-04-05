# Optics Design Brief

What optics are in pane, how they work, and the rules for code
that touches them. Read this before working on pane-proto's
property module, pane-fs, or any handler state projection.

Background deliberation: `docs/optics-deliberation.md`.

---

## Role of optics

A pane is organized state with views. Display is one view. The
filesystem namespace at `/pane/` is another. Optics are the
mechanism that keeps views consistent with internal state.

Optics are NOT the primary mutation path. The handler mutates
its own fields directly (`self.cursor += 1`). Optics project
that state outward — to the filesystem, to the compositor, to
remote observers. The developer writes a handler struct and
annotates the fields they want visible. The optic machinery is
invisible unless they need it.

---

## Three layers

```
pane-proto    MonadicLens<S,A>         typed, law-verified, read-write + effects
              ReadOnlyAttribute<S,A>   typed, read-only
                     |
                     | registration (PaneBuilder phase)
                     v
pane-fs       AttrSet<S>               type-erased, read + write + parse
              AttrReader<S>            type-erased read path (view)
              AttrSet<S>               named collection of readers
                     |
                     | trait-object erasure
                     v
              PaneNode (dyn)           fully erased, FUSE-facing
```

### Layer 1: Typed optics (`pane-proto/src/property.rs`)

```rust
pub struct MonadicLens<S, A> {
    pub name: &'static str,
    pub view: fn(&S) -> A,
    pub set: fn(&mut S, A) -> Vec<Effect>,
    pub parse: fn(&str) -> Result<A, String>,
}
```

Concrete fn-pointer encoding (replaces fp_library dependency).
View is pure. Set mutates state and returns effects. Parse
converts text from the ctl file to the typed value A.

Law tests: GetPut, PutGet, PutPut on the (view, set) pair,
ignoring effects (effects are a side channel, not part of the
lens laws). The laws operate on the full state S, not just the
focused field — a setter that touches non-focused fields must
still satisfy GetPut.

`ReadOnlyAttribute` is a Getter: view only, no set. For
computed or derived values.

The existing `Attribute<'a, S, A>` backed by fp_library is
superseded by `MonadicLens<S, A>`. Both encode the same laws;
MonadicLens adds the effect channel and uses concrete fn
pointers instead of fp_library's branded optics. Migration
happens in build order step 1.

### Layer 2: Type-erased accessors (`pane-fs/src/attrs.rs`)

```rust
pub struct AttrReader<S> {
    pub name: &'static str,
    reader: Box<dyn Fn(&S) -> AttrValue + Send + Sync>,
}
```

`AttrValue` is `String`. Serialization is through `Display`.
Deserialization (for writes) is through `FromStr`. This is the
Plan 9 convention: synthetic files contain human-readable text.

`AttrSet<S>` is a `HashMap<&'static str, AttrReader<S>>`.

### Layer 3: FUSE-facing (`pane-fs/src/namespace.rs`)

```rust
pub struct PaneEntry<S> {
    pub id: u64,
    pub tag: String,
    pub attrs: AttrSet<S>,
    pub state: S,
}
```

State is a snapshot, updated by the looper after each dispatch
cycle. FUSE threads read from the snapshot. The looper is the
only writer.

---

## What exists vs. what's missing

| Component | Status | Location |
|-----------|--------|----------|
| `Attribute<S,A>` with lens laws | Exists | `pane-proto/src/property.rs` |
| `ReadOnlyAttribute<S,A>` | Exists | `pane-proto/src/property.rs` |
| `AttrReader<S>`, `AttrSet<S>` | Exists | `pane-fs/src/attrs.rs` |
| `PaneEntry<S>` | Exists | `pane-fs/src/namespace.rs` |
| `AttrSet::to_json(&S)` (bulk read) | Missing | Goes in `pane-fs/src/attrs.rs` |
| `AttrWriter<S>` (write path) | Missing | Goes in `pane-fs/src/attrs.rs` |
| `MonadicLens<S,A>` | Missing | Goes in `pane-proto/src/property.rs` |
| `Effect` enum | Missing | Goes in `pane-proto/src/property.rs` |
| `AttrSet<S>` (type-erased collection) | Missing | Goes in `pane-proto/src/property.rs` |
| `AttrAccess` enum | Missing | Goes in `pane-proto/src/property.rs` |
| `AttrInfo` struct | Missing | Goes in `pane-proto/src/property.rs` |
| `Scriptable` trait | Missing | Goes in `pane-proto/src/property.rs` |
| `supported_attrs()` on Handler | Missing | Goes in `pane-proto/src/handler.rs` |
| `ctl_fallback()` on Handler | Missing | Goes in `pane-proto/src/handler.rs` |
| Snapshot synchronization (ArcSwap) | Missing | Goes in `pane-fs/src/namespace.rs` |
| `PaneNode` trait (type erasure) | Missing | Goes in `pane-fs/src/namespace.rs` |
| ctl file parsing | Missing | Goes in new `pane-fs/src/ctl.rs` |
| `#[derive(Scriptable)]` macro | Not yet (build last) | Separate crate |

---

## Types to add

### AttrAccess and AttrInfo (`pane-proto/src/property.rs`)

```rust
/// What operations an attribute supports.
/// Maps to FUSE permissions: ReadWrite -> 0660, ReadOnly -> 0440.
/// Maps to optic type: ReadWrite = Lens, ReadOnly = Getter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrAccess {
    ReadWrite,
    ReadOnly,
    Computed,
}

/// Static description of a scriptable attribute.
pub struct AttrInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub access: AttrAccess,
    pub value_type: &'static str,
}
```

### Scriptable trait (`pane-proto/src/property.rs`)

```rust
/// Handler state that declares its scriptable surface.
/// Returns an AttrSet containing both the read path
/// (AttrReaders derived from view) and the write path
/// (AttrWriters derived from set + parse).
pub trait Scriptable: Clone + Send + 'static {
    fn supported_attrs() -> &'static [AttrInfo];
    fn attr_set() -> AttrSet<Self>;
}
```

### Handler additions (`pane-proto/src/handler.rs`)

```rust
pub trait Handler: Send + 'static {
    // ... existing methods ...

    /// Declare scriptable properties. Default: none.
    fn supported_attrs(&self) -> &'static [AttrInfo] { &[] }

    /// Freeform escape for ctl commands that are not
    /// optic-expressible: lifecycle (close) and IO-first
    /// (reload). State-mutating commands route through
    /// the monadic lens layer and never reach this method.
    /// Default: unknown command error.
    fn ctl_fallback(&mut self, command: &str, args: &str) -> CtlResult {
        let _ = (command, args);
        CtlResult::Err(CtlError::UnknownCommand)
    }
}
```

### AttrWriter (`pane-fs/src/attrs.rs`)

Type-erased write path, constructed from a MonadicLens by
capturing `parse` + `set`. Analogous to how AttrReader
captures `view` for the read path.

```rust
pub struct AttrWriter<S> {
    pub name: &'static str,
    writer: Box<dyn Fn(&mut S, &str) -> Result<Vec<Effect>, WriteError> + Send + Sync>,
}

pub enum WriteError {
    ParseError(String),
    ReadOnly,
    NotFound,
}
```

Constructed from a MonadicLens during PaneBuilder registration:

```rust
fn from_monadic_lens<S, A>(lens: &MonadicLens<S, A>) -> AttrWriter<S> {
    let parse = lens.parse;
    let set = lens.set;
    AttrWriter {
        name: lens.name,
        writer: Box::new(move |s, text| {
            let val = parse(text).map_err(WriteError::ParseError)?;
            Ok(set(s, val))
        }),
    }
}
```

The ctl dispatcher calls `AttrWriter::write()`, which returns
`Vec<Effect>`. The framework executes effects and publishes
the snapshot. No separate wiring — both AttrReader and
AttrWriter are derived from the same MonadicLens.

### Snapshot synchronization (`pane-fs/src/namespace.rs`)

Replace `pub state: S` with `ArcSwap<S>`. The looper publishes
new snapshots via atomic swap. FUSE threads load snapshots
without blocking the looper.

```rust
use arc_swap::ArcSwap;

pub struct PaneEntry<S> {
    pub id: u64,
    pub tag: String,
    pub attrs: AttrSet<S>,
    state: ArcSwap<S>,
}

impl<S> PaneEntry<S> {
    /// Looper thread: publish new snapshot. Takes &self.
    pub fn update_state(&self, state: S) {
        self.state.store(Arc::new(state));
    }

    /// FUSE thread: read attribute from current snapshot.
    pub fn read_attr(&self, name: &str) -> Option<AttrValue> {
        let state = self.state.load();
        self.attrs.read(name, &*state)
    }
}
```

### PaneNode trait (`pane-fs/src/namespace.rs`)

Type-erased interface so the namespace can hold entries with
different state types:

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

---

## Boundary rules

These are hard rules. They reflect structural constraints, not
preferences.

**B1. Handles\<P\>::receive never touches an optic.** The handler
mutates its own fields directly. The optic layer reads from a
state snapshot the looper updates *after* dispatch completes.
Mutation and projection are sequential phases, not interleaved.

**B2. MonadicLens\<S,A\> never appears in a Message enum.**
MonadicLens contains fn pointers into handler state. Send the
projected value, not the lens.

**B3. AttrValue never crosses a session channel.** AttrValue is
a String wrapper for the filesystem text interface. Protocol
messages carry typed data. Don't conflate filesystem format
with wire format.

**B4. Obligation handles are never Message variants.** They're
`!Clone`, `!Serialize`. The protocol_handler macro generates a
separate dispatch path. The type system enforces this.

**B5. AttrReader closures must be pure.** No side effects, no IO,
no panics. They run on the pane-fs thread, outside the looper's
catch_unwind boundary.

**B6. PaneEntry::update_state is called only by the looper.**
After dispatch, before the next event. This is the single
synchronization point between the session world and the optic
world.

**B7. Filters see Message types only.** Obligation handles bypass
all filters. Enforced by `MessageFilter<M: Message>` bound.

**B8. ServiceHandle\<P\> lives in the handler struct, not the optic
layer.** It's `!Clone` and its Drop fires `RevokeInterest`.

---

## Anti-patterns

### Don't force events through optics

A key press is an event, not an attribute projection. Events flow
through `Handles<Display>::receive`. Properties project state
through `AttrReader`. If your lens's `set` doesn't make semantic
sense, it's an event. Route it through `Handles<P>`.

### Don't expose optic vocabulary to app developers

The developer writes `#[scriptable]` on a struct field and never
hears "lens" or "profunctor." If `Lens`, `Getter`, or `Traversal`
appear in doc examples aimed at app developers, the abstraction
is leaking.

### Don't rebuild the BHandler tree

No `ResolveSpecifier`, no handler chains, no
`Vec<Box<dyn ScriptableHandler>>`. The filesystem path IS the
address. Each pane's attribute surface is flat.

### Don't allow dynamic attribute registration after run_with

Attributes are declared at setup time (PaneBuilder phase). If
`AttrSet::add()` is called after the looper starts, the design
is wrong. If something needs to expose properties dynamically,
it gets its own pane.

### Don't bypass the optic layer for state-mutating ctl commands

State-mutating ctl commands (`cursor 42`, `set-tag "foo"`,
`goto 42`, `focus`) route through the monadic lens layer.
The ctl dispatcher parses the command, looks up the attribute,
and calls the monadic setter. This eliminates wiring
divergence by construction — the same lens is used for both
the read path (AttrReader) and the write path (ctl).

Lifecycle commands (`close`) and IO-first commands (`reload`)
bypass optics and dispatch to a freeform handler method.
These are categorically different — control flow and external
IO, not state projection. See §Ctl dispatch architecture.

---

## Consistency model

**Per-pane snapshot consistency.** All attributes read within one
FUSE operation come from the same dispatch cycle.

**Bulk read via `json` files.** `json` is a reserved filename
at every directory level in the namespace. Each returns a
structured snapshot of its parent directory in one FUSE read:

```
/pane/json              [{"id":1,"tag":"Editor"}, ...]
/pane/<n>/json          {"id":1,"tag":"Editor","body":"...","attrs":{...},"commands":[...]}
/pane/<n>/attrs/json    {"cursor":"42","buffer_length":"11","dirty":"false"}
```

Values are strings (Display representation), not typed JSON.
Two successive reads of individual files might hit different
snapshots. `json` files eliminate this race — cross-field
invariants are testable from a single read. The pane-level
`json` subsumes `attrs/json` (attrs are nested), but both
exist because `cat /pane/1/attrs/json` is cheaper than parsing
the full pane JSON to extract just the attributes.

**Per-ctl-write barrier.** Ctl writes are synchronous: the FUSE
write blocks until the looper processes the command and updates
the snapshot. A read after a ctl write sees the effect.

**No cross-pane ordering.** Different panes have different
loopers. `/pane/3/attrs/cursor` and `/pane/7/attrs/cursor` have
no ordering guarantee.

---

## Failure model

A crashed pane's namespace entry is removed immediately. Not
left stale. Concurrent reads in flight return EIO. New reads
return ENOENT.

FUSE error mapping:
- ENOENT — attribute or pane not found
- EPERM — write to read-only attribute
- EIO — pane crashed or getter failed
- EINVAL — bad ctl command or parse failure
- ENXIO — pane exited, looper dead
- ETIMEDOUT — ctl write exceeded 5s timeout

---

## Law testing

Every `MonadicLens<S,A>` must pass three properties. Effects
are ignored — the laws govern state, not effects.

```rust
fn assert_monadic_lens_laws<S, A>(
    lens: &MonadicLens<S, A>, s: S, a1: A, a2: A,
)
where S: Clone + PartialEq + Debug, A: Clone + PartialEq + Debug
{
    // GetPut: set(s, get(s)) == s (full state, not just focus)
    let mut s_copy = s.clone();
    let val = (lens.view)(&s);
    let _ = (lens.set)(&mut s_copy, val);
    assert_eq!(s_copy, s, "GetPut violated");

    // PutGet: get(set(s, a)) == a
    let mut s_copy = s.clone();
    let _ = (lens.set)(&mut s_copy, a1.clone());
    assert_eq!((lens.view)(&s_copy), a1, "PutGet violated");

    // PutPut: set(set(s, a1), a2) == set(s, a2)
    let mut left = s.clone();
    let _ = (lens.set)(&mut left, a1);
    let _ = (lens.set)(&mut left, a2.clone());
    let mut right = s;
    let _ = (lens.set)(&mut right, a2);
    assert_eq!(left, right, "PutPut violated");
}
```

PutPut is load-bearing: it's the formal condition under which
the looper can coalesce queued writes to the same attribute.
If two writes target the same attribute within one batch, the
looper discards the earlier one. This is always safe for
MonadicLens because PutPut holds by construction.

---

## Obligation handles and linear lenses

Obligation handles (ReplyPort, ClipboardWriteLock, ServiceHandle,
TimerToken, CancelHandle, CreateFuture) are structurally linear
lenses (Clarke et al. Definition 4.12): decompose state, then
recompose exactly once. Drop fires the failure terminal.

This is explanatory. Do NOT build a `LinearLens` trait or
abstraction. Each obligation handle has bespoke semantics. The
linear lens recognition means:

1. Document the decompose/recompose structure on each handle.
2. Test both completion paths: success (.commit/.reply/.wait)
   and failure (Drop).
3. Never smuggle an obligation through `AppPayload` (which
   requires Clone, defeating linearity).

---

## Ctl dispatch architecture

### Synchronous write mechanism

Ctl writes block the FUSE thread until the looper processes the
command and publishes the updated snapshot. The mechanism:

```
FUSE write thread                    Looper thread
─────────────────                    ─────────────
parse command lines
create oneshot channel
send (cmd, oneshot_tx) ────────→     recv from calloop
block on oneshot_rx.recv()           dispatch command
                                     handler mutates state
                                     execute effects (if any)
                                     publish snapshot (Clone)
oneshot response ←──────────────     send result on oneshot_tx
return byte count / errno
```

This is the Plan 9 model. In Plan 9, `write(2)` on a ctl file
did not return until the command took effect. devproc.c ran
commands inline in the writing process's context. rio's wctl.c
ran commands on the Xfid thread and responded only after
completion. acme processed multiple newline-separated commands
sequentially in one write, stopping on first error.

BeOS's hey used the synchronous send-and-wait-for-reply variant
of BMessenger::SendMessage. The handler's MessageReceived
processed the message and sent a reply before the caller
unblocked. hey's problems were infinite timeouts (causing hangs)
and reply-before-side-effects (SET Frame returned before
app_server finished the resize). Pane avoids both: the oneshot
has a 5-second timeout, and the looper is the single point of
mutation — no second-hop async side-effect to race against.

**Multi-line writes.** `echo "cmd1\ncmd2" > ctl` delivers both
lines in one write(2). Process sequentially, stop on first
error, return bytes consumed up to the error. This is acme's
model (xfid.c line 601).

**Error reporting.** FUSE write returns errno:

| Condition | errno |
|-----------|-------|
| Bad syntax | EINVAL |
| Unknown command | EINVAL |
| Handler error | EIO |
| Handler panic | EIO |
| Pane exited / looper dead | ENXIO |
| Timeout (5s) | ETIMEDOUT |

**Concurrent writers.** Both FUSE threads block. The looper
serializes commands via calloop channel FIFO order. No
additional locking needed.

**Performance.** ~15-45us total (FUSE overhead + oneshot
round-trip + handler processing + snapshot clone). Within the
filesystem tier budget. Invisible at interactive speeds.

### Command taxonomy

| Category | Examples | Nature |
|----------|----------|--------|
| Simple field set | `set-tag "foo"`, `cursor 42` | State mutation |
| Compound mutation | `goto 42` (cursor + clear selection + scroll) | Invariant-maintaining state mutation |
| Side-effectful | `focus`, `hide`, `show` (state + compositor notify) | State mutation + protocol effect |
| Lifecycle | `close` | Control flow / Flow::Stop |
| IO-first | `reload` (re-read from source) | External IO determines new state |
| Query | `search "pattern"` | Read-only, produces output |

### Decision: optic-routed dispatch with effect separation

State-mutating ctl commands route through the optic layer via
monadic lenses (Clarke et al. Definition 4.6). Lifecycle and
IO-first commands dispatch to a freeform handler method.

This decision was validated by a prototype that implemented
the same editor handler under both approaches and ran five
tests comparing wiring safety, effect ordering, lens law
compliance, expressiveness boundaries, and ergonomics.

**Rationale from BeOS.** In the Haiku source, zero percent of
B_SET_PROPERTY implementations were pure field writes. Every
SET called a method with side effects. The monadic lens
extension resolves this by admitting effects as first-class
outputs of the set operation.

**What the prototype proved:**

1. Wiring divergence (ctl setter differs from optic setter) is
   structurally impossible — the same fn pointer serves both
   the read path (AttrReader) and the write path (ctl).
2. Effect ordering is framework-guaranteed, not handler-
   dependent. The setter returns effects; the framework
   dispatches them after state mutation, before snapshot
   publication.
3. Compound mutations (`goto` = set cursor + clear selection)
   satisfy all three lens laws when the setter is conditional
   (only clear selection when cursor actually changes).
4. IO-first commands (`reload`) don't fit the monadic lens
   signature — the freeform escape is necessary and small.
5. Both approaches are ~55 lines for a 7-command vocabulary.
   Option B splits into declarative attribute definitions +
   freeform escape; option A is one match block.

**The monadic lens type:**

```rust
struct MonadicLens<S, A> {
    name: &'static str,
    view: fn(&S) -> A,
    set: fn(&mut S, A) -> Vec<Effect>,
    parse: fn(&str) -> Result<A, String>,
}

enum Effect {
    Notify(ServiceId, Box<dyn Message>),
    SetContent(Vec<u8>),
}
```

View is pure. Set mutates state and returns effects. Parse
converts the text argument from ctl into the typed value A
(the FromStr equivalent). The framework registers the view
function as an AttrReader for the read path and uses the same
lens for ctl dispatch on the write path.

`S: 'static` is required on AttrReader, AttrWriter, and
AttrSet because the type-erased closures are `Box<dyn Fn>`.
This is compatible with Handler's existing `Send + 'static`
bound on handler state.

**Ctl dispatch:**

```rust
fn dispatch_ctl<S>(
    attrs: &AttrSet<S>,
    state: &mut S,
    cmd: &str,
    args: &str,
    fallback: fn(&mut S, &str, &str) -> CtlResult,
) -> CtlResult {
    match attrs.find_writer(cmd) {
        Some(writer) => {
            let effects = writer.write(state, args)?;
            CtlResult::WithEffects(effects)
        }
        None => fallback(state, cmd, args),
    }
}
```

The fallback handles lifecycle (`close` → Flow::Stop) and
IO-first commands (`reload`). This is the freeform escape —
small, principled, and only for commands that are categorically
outside the optic vocabulary.

**Optic expressibility by category:**

| Command | Expressible? | Optic type |
|---------|-------------|------------|
| `set-tag` | Yes | Monadic Lens |
| `cursor` | Yes | Monadic Lens |
| `goto` | Yes | Monadic Lens (invariant-maintaining setter) |
| `replace` | Yes | Monadic Affine (range may be invalid) |
| `hide`/`show` | Yes | Monadic Lens (state + compositor notify) |
| `focus` | Yes | Monadic Lens (state + notification) |
| `search` | Yes | Fold (read-only) |
| `close` | No | Control flow (Flow::Stop) |
| `reload` | No | IO→state direction is backwards |

~80% optic-routed. ~20% freeform escape.

**Compound mutations and lens laws.** A compound mutation
like `goto 42` (set cursor, clear selection, scroll into view)
is a valid lens — the lens laws don't require that set touch
only one field. GetPut holds if the compound behavior is
idempotent when setting to the current value. PutGet holds
trivially (read back what you wrote). PutPut holds if the
side effects are deterministic functions of (state, value).
The setter focuses on "cursor position as perceived by the
goto semantic" and maintains invariants involving other fields.

**Lifecycle commands bypass optics.** `close` triggers
Flow::Stop — a control-channel signal, not a state mutation.
An optic into "lifecycle state" would violate GetPut (setting
the viewed value back should be a no-op, but close always
terminates the pane). Lifecycle commands are not optics. They
dispatch through the control channel directly.

**Effect ordering.** If effects are separated from state
mutations, the framework can guarantee ordering: execute
effects, then publish snapshot. This ensures external
observers never see post-mutation state before effects have
been dispatched. Without separation, effects are hidden inside
the handler method and the framework cannot reason about their
timing relative to snapshot publication.

---

## pane-fs as verification surface

pane-fs is the primary integration test surface for the session
type infrastructure. Every invariant in the session type
framework that has a namespace-observable consequence is
testable through filesystem operations.

### Invariant-to-namespace mapping

| Invariant | Observable? | Namespace consequence |
|-----------|-------------|----------------------|
| I1 (panic=unwind) | Yes | `/pane/<n>/` disappears after crash |
| I2 (no blocking) | Degradation | Snapshot stops updating (stale reads) |
| I3 (handlers terminate) | Degradation | Same as I2 |
| I4 (typestate handles) | Partial | Dropped ReplyPort → requester gets failure |
| I5 (filter/Clone-safe) | No | Filter chain is looper-internal |
| I6 (sequential dispatch) | Yes | Snapshot reads are always consistent |
| I7 (fn pointer sequential) | No | Subsumed by I6 from namespace view |
| I8 (send_and_wait panic) | Yes | Pane crashes → `/pane/<n>/` disappears |
| I9 (dispatch cleared before drop) | Yes | No zombie entries after exit |
| I10 (ProtocolAbort non-blocking) | No | Wire protocol internal |
| I11 (ProtocolAbort framing) | No | Wire protocol internal |
| I12 (unknown discriminant) | No | Cannot construct through pane-fs |
| I13 (open_service blocks) | Yes | Pane appears with full attr set atomically |
| S1 (token uniqueness) | No | Dispatch-internal |
| S2 (sequential dispatch) | I6 | Same observable consequence |
| S3 (control-before-events) | No | Batch-internal |
| S4 (fail_connection) | Yes | Attrs reflect service loss, requests fail |
| S5 (cancel no callbacks) | No | Dispatch-internal |
| S6 (panic=unwind) | I1 | Same observable consequence |

Seven invariants are cleanly namespace-testable: I1, I4, I6,
I8, I9, I13, S4. Three (I9, I13, I6-through-snapshots) are
testable ONLY through the namespace — unit tests on dispatch.rs
cannot cover the publication boundary.

### Three test levels

**Level 1: pure optic laws.** Test GetPut, PutGet, PutPut on
`MonadicLens<S,A>` directly. Fast, deterministic, no
concurrency. Testable with arbitrary S and A values.
Effects are ignored — the laws govern state. See
`assert_monadic_lens_laws` in §Law testing.

Note: the existing PutPut test in property.rs only asserts
`view(state2) == 20`, not that the full state equals
`set(s, 20)`. All three laws must compare the entire state S,
not just the focused field. A setter with unconditional side
effects on non-focused fields (e.g., flipping a dirty flag)
is caught by GetPut (setting the current value back should be
a no-op, but dirty flips). Tested and confirmed in
`monadic_lens::tests::claim_13`.

**Level 2: projection chain.** Test the AttrReader output
against Display of the lens's view. Test Display/FromStr
roundtrip for every type used in writable attributes. Test
snapshot update propagation through PaneEntry.

```rust
#[test]
fn projection_chain_faithful() {
    let reader = AttrReader::new("cursor", |s: &EditorState| s.cursor);
    let state = EditorState { cursor: 42, buffer: "hello".into() };
    assert_eq!(reader.read(&state).0, state.cursor.to_string());
}

#[test]
fn display_fromstr_roundtrip() {
    let values: Vec<usize> = vec![0, 1, 42, usize::MAX];
    for v in values {
        let s = v.to_string();
        let parsed: usize = s.parse().unwrap();
        assert_eq!(v, parsed);
    }
}
```

**Level 3: namespace integration.** Shell-level tests that
exercise the full loop: FUSE write → looper dispatch → handler
mutation → snapshot update → FUSE read. These test the
publication boundary — the point where internal looper state
becomes externally observable.

### Critical test scenarios

**Test 1: clean lifecycle (I1 + I9 + I13).**

```sh
# Create pane, verify structure, close, verify cleanup
n=$(pane-ctl create --sig com.test.lifecycle --tag "test")
test -d /pane/$n/           || fail "directory missing"
test -f /pane/$n/tag        || fail "tag missing"
test -d /pane/$n/attrs/     || fail "attrs missing"
tag=$(cat /pane/$n/tag)
test "$tag" = "test"        || fail "tag mismatch"
echo close > /pane/$n/ctl
test -d /pane/$n/ && fail "directory survived close"
```

This is the single most important namespace test. Covers
handshake, looper startup, snapshot initialization, synchronous
ctl, obligation compensation, and cleanup.

**Test 2: crash cleanup (I1 + I8 + I9).**

```sh
# Pane crashes, verify namespace cleanup
n=$(pane-ctl create --sig com.test.crash --tag "crash")
kill -TERM $pane_pid
test -d /pane/$n/ && fail "directory survived crash"
```

Tests the failure path. Variant: SIGKILL exercises the fd
hangup backstop.

**Test 3: snapshot consistency (I6).**

```sh
# Handler maintains invariant a + b = 100
# Rapidly mutate while reading bulk attrs
echo "update a=60 b=40" > /pane/$n/ctl
attrs=$(cat /pane/$n/attrs/json)
# Verify a + b = 100 in every read
```

If I6 is violated, a torn read could show values from
different dispatch cycles. The bulk `attrs/json` endpoint
returns all attributes from one snapshot in one FUSE read.

**Test 4: connection loss propagation (S4).**

```sh
# Kill server providing clipboard service
# Verify pane reflects service loss, pending requests fail
kill $clipboard_server_pid
status=$(cat /pane/$n/attrs/clipboard_status)
test "$status" = "unavailable" || fail "service loss not reflected"
```

**Test 5: synchronous ctl write-then-read.**

```sh
echo "set-tag new-title" > /pane/$n/ctl
tag=$(cat /pane/$n/tag)
test "$tag" = "new-title" || fail "tag not updated after sync write"
```

If ctl were asynchronous, this test would be a race. With
synchronous ctl, it is deterministic.

### Wiring consistency (eliminated by construction)

Under optic-routed dispatch, the ctl write path and the
optic read path use the same fn pointer. Wiring divergence
is structurally impossible — there is no separate ctl
handler that could drift from the lens definition.

The remaining consistency surface is the **Display/FromStr
roundtrip** — the text serialization between the FUSE
interface and the typed value. A PutGet violation through the
ctl path can still happen if `Display` and `FromStr` don't
agree (e.g., float precision loss). This is testable once
per type, not once per attribute:

```rust
#[test]
fn display_fromstr_roundtrip() {
    let values: Vec<usize> = vec![0, 1, 42, usize::MAX];
    for v in values {
        let s = v.to_string();
        let parsed: usize = s.parse().unwrap();
        assert_eq!(v, parsed);
    }
}
```

A GetPut violation can still occur if a monadic lens setter
has unconditional side effects on non-focused fields (e.g.,
setting cursor to its current value flips `dirty`). This is
caught by the standard lens law tests on the MonadicLens
definition — test GetPut on the full state S, not just the
focused field A.

### The optic projection chain

The full path from handler state to namespace read composes
three lookups:

```
Namespace lookup: HashMap<u64, PaneEntry<S>>  — AffineFold
Attribute lookup: HashMap<&str, AttrReader<S>> — AffineFold
Reader:           Fn(&S) -> AttrValue          — Getter
```

Composition: AffineFold . AffineFold . Getter = AffineFold.
Partial, read-only. The composed optic at the namespace level
is deterministic on the snapshot — reads are pure functions of
the current snapshot state.

The write path (ctl) uses the same monadic lens setter as the
read path's view function. The asymmetry is at the composition
level: reads compose through the namespace (AffineFold), writes
dispatch through the looper as parsed commands that invoke the
lens setter. Both paths use the same lens — the read side
captures view, the write side captures set.

---

## Build order

1. `MonadicLens<S,A>`, `Effect` enum, lens law test harness in pane-proto
2. `AttrAccess`, `AttrInfo`, `AttrSet<S>` in pane-proto
3. `Scriptable` trait, `supported_attrs()`, `ctl_fallback()` on Handler
4. `AttrSet::to_json_str()` for bulk read endpoint
5. Snapshot synchronization (`ArcSwap`)
6. `PaneNode` trait for type erasure
7. Ctl parsing module with monadic lens dispatch + freeform fallback
8. Integration: looper publishes snapshots, executes effects, pane-fs reads
9. `#[derive(Scriptable)]` macro — last, after hand-coded path works

Each step is independently testable. Don't skip ahead.
