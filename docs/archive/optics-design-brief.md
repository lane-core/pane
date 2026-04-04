# Optics Design Brief

Self-contained synthesis of the design research for pane's optic layer.
Produced from three theoretical papers, two be-engineer assessments,
a Rust ecosystem survey, and iterative design review.

---

## 1. Decision: Build the Optic Layer Now (Approach C)

The optic layer is foundational infrastructure, not a Phase 6 feature.
Every Tier 2 subsystem (clipboard, observer, DnD) involves state access
across actor boundaries. Building the optic layer first means those
subsystems compose correctly from day one — they're built over the
equational theory rather than inventing ad-hoc state access patterns
that Phase 6 would have to reconcile.

Applications are free algebras over the optic operations (get/set/count).
The optic laws are the equations of the theory. Getting the equations
right now means every application built on top automatically satisfies
the consistency properties.

**Done:**
- Concrete optic types (`FieldLens`, `FieldAffine`, `FieldTraversal`) — `pane-optic` crate
- `DynOptic` trait for type-erased dynamic dispatch at protocol boundaries
- `AttrValue` serialization type, `ValueType`, `OpKind`, `SpecifierForm`
- `ScriptableHandler` trait, `PropertyInfo`, `ScriptReply`, `CompletionReplyPort`
- `ScriptError` error domain
- Optic law tests (GetPut, PutGet, PutPut + partial variants)

**Not yet done:**
- Route existing Messenger property methods through the optic layer (Phase 6 convergence)

**Defer:**
- Scripting wire protocol (`ScriptQuery`/`ScriptResponse`)
- `hey`-like scripting tool
- Compositor mediator role for inter-pane forwarding
- `#[derive(Scriptable)]` proc macro
- Filesystem attribute bridge

---

## 2. Theoretical Foundations

Three papers inform the design. The theory is the engine room — it
makes the system sound, but app developers don't need to know about it.

### Profunctor optics (Clarke et al. 2023)

Source: "Profunctor optics, a categorical update," Compositionality 2023.
Also: "Don't Fear the Profunctor Optics" tutorial.

**What we take:** The optic families (lens, prism, affine, traversal),
their laws (GetPut, PutGet, PutPut, MatchBuild, BuildMatch), and the
composition rules. Clarke et al. Proposition 2.3: optics form a category
under composition, so composed optics preserve laws.

**What we don't take:** The profunctor encoding. The representation
theorem (Theorem 4.4) proves `forall p. Tambara p => p a b -> p s t`
is isomorphic to the concrete optic, but this requires rank-2
polymorphism. Rust doesn't have it. The concrete encoding (split/merge
pairs) is the right Rust representation.

### Dependent linear session types (Fu, Xi, Das — TLL+C)

Source: "Dependent Session Types for Verified Concurrent Programming,"
PACMPL 2026.

**Ghost state discipline:** Each time a correlation ID appears at the
API surface, ask whether a typestate handle could replace it. When it
can, compile-time protocol enforcement replaces runtime token-matching.
When it can't (async gap ownership can't bridge), keep the token but
recognize it as ghost state. Applied to pane: CompletionRequest's
`token: u64` is ghost state that should become a typed handle.

**Affine as default:** Session channels are affine (Rust's ownership
model), not linear (TLL+C's model). Scripting targets may not exist
(closed pane, empty selection). Affine optics (`preview: S → Option<A>`)
are the right default, not total lenses (`view: S → A`).

### DLfActRiS (Jacobs, Hinrichsen, Krebbers — POPL 2024)

Source: "Deadlock-Free Separation Logic: Linearity Yields Progress for
Dependent Higher-Order Message Passing."
Full review: `docs/superpowers/dlfactris-review.md`

**Key finding:** The ownership transfer semantics live at the transport
layer, not the optic layer. Rust's `&` vs `&mut` on state already
encodes the observation/mutation split. `DynOptic` should NOT carry
ownership/authority annotations — that's redundant with the type system.

**Connectivity graph discipline:** For inter-pane messaging, track
which panes have open `send_and_wait` calls and check for blocking
cycles. This is a debug tool at the transport layer, not something
baked into optic traits. The optic layer is single-threaded (runs
within a looper); connectivity concerns live at the message transport.

**Affine/linear gap:** Accepted, not solved. Rust can't enforce linear
channel usage statically. Pane's strategy — `#[must_use]` + `Drop`
sends `ReplyFailed`/`Disconnected` — is validated as principled
engineering. The paper proves linearity is *necessary* for deadlock
freedom from types alone; pane handles the gap at runtime.

---

## 3. Be-Engineer Findings

Two assessments from the be-systems-engineer, referencing Haiku source.

### Convergence constraint (critical)

> Be's biggest scripting win was that `hey Tracker set Title of Window 0`
> exercised the same code path as `BWindow::SetTitle("New Title")`.

Messenger's hardcoded property setters (`set_title`, `set_content`, etc.)
and scripting optics MUST share implementation when Phase 6 lands. Don't
merge them yet, but the optic layer must be designed so that each
Messenger method can delegate to the corresponding `DynOptic` without
a second code path. The optics are the ground truth; the methods become
convenience sugar.

### PropertyInfo must be rich

Be's `property_info` tables (e.g., `sWindowPropInfo` in `Window.cpp:125-184`)
carried four things per property:
1. Property name
2. Supported operations (get, set, count, execute, create, delete)
3. Supported specifier forms (direct, index, name)
4. Expected value types

`PropertyInfo` (`scripting.rs`) now carries the full operation set
(`&'static [OpKind]`), specifier forms (`&'static [SpecifierForm]`), and
value type (`ValueType`). Replaces the earlier `Attribute` stub. This is
what `GetSupportedSuites` serialized for introspection.

### ResolveSpecifier: mostly internal, with one exception

`BLooper::resolve_specifier` (`Looper.cpp:1428-1466`) runs the entire
chain walk within a single locked looper. But `BApplication::ResolveSpecifier`
(`Application.cpp:733-847`) forwards to a different looper via
`BMessenger::SendMessage` after `PopSpecifier`. In pane (separate
processes per pane), this becomes inter-process forwarding.

The flat session type (`Send<ScriptQuery, Recv<ScriptResponse, End>>`)
works for individual hops. The chain is driven by the client or a
mediator — deferred to Phase 6.

### What Be got wrong that optics fix

1. **No law enforcement.** No guarantee that get-after-set returns
   what was set. Optic laws (GetPut, PutGet) are testable properties.
2. **Fragile FindMatch dispatch.** Integer case indices matching
   property table order — reordering broke everything. Optics use
   named, typed accessors with no index matching.
3. **Mutable message anti-pattern.** `PopSpecifier` mutated the message
   in flight. Pane uses immutable specifier chain with separate cursor.
4. **No transactional guarantees.** Partial mutation on chain failure.
   Optic `set` returns `Result` — validate first, apply atomically.
5. **Type confusion at wire boundary.** No validation of incoming value
   type against property schema. `DynOptic` + `AttrValue` validates at
   the serialization boundary.

### resolve_chain safety checks

Not connectivity checks (those belong at the transport layer). Instead:
- **Max depth:** `MAX_SPECIFIER_DEPTH = 16` to prevent unbounded chains
- **Cycle detection:** If `resolve_specifier` returns same handler, stop
- **Same-pane constraint:** Sub-handlers must be local (enforced by `&mut`)

---

## 4. Ecosystem Survey: Build Our Own

No existing Rust optics crate satisfies all three requirements:

| Crate | Ref-returning? | dyn-safe? | Composable? |
|-------|---------------|-----------|-------------|
| `optics` v0.3 | No (clones) | Yes | Yes |
| `lens-rs` v0.3 | Yes | No | Yes |
| `karpal-optics` | Yes | No (HKT) | Yes |
| Druid lens | Yes (CPS) | No | Yes |

**Decision:** Build a small, focused optics layer (~300-500 lines).
Combine Druid's `Field` pattern (reference-returning closures) with
the `optics` crate's trait hierarchy (dyn-safe via separate traits).

The key: reference-returning getters ARE dyn-compatible:

```rust
trait Getter<S, A: ?Sized> {
    fn get<'s>(&self, source: &'s S) -> &'s A;
}
```

No generic parameters, no GATs, no HKTs. Stable Rust.

---

## 5. API Philosophy

The Be principle: powerful internals, simple surface. App developers
write `set_title("Hello")` and `#[scriptable] title: String`. They
don't need to know about profunctor optics, separation logic, or
connectivity graphs.

The optic layer serves two audiences:
- **App developers:** See `#[derive(Scriptable)]`, `PropertyInfo`,
  `ScriptableHandler` trait. The derive macro generates everything.
  Hand-implement `ScriptableHandler` for custom resolution logic.
- **Framework developers:** See `Lens<S, A>`, `Affine<S, A>`,
  `DynOptic`, `AttrValue`, composition, law tests. This is where
  the theory lives.

Follow the existing pane pattern: implementation modules are
`pub(crate)` or `#[doc(hidden)]`. The public re-exports in `lib.rs`
are the developer API. BeOS cross-references in doc comments.

---

## 6. Concrete Type Sketches

### New crate: `pane-optic`

Depends on nothing pane-specific. Pure optic types + traits.
`pane-app` depends on `pane-optic`.

### Core traits (reference-returning, dyn-compatible)

```rust
/// Total getter — field always exists.
pub trait Getter<S, A: ?Sized> {
    fn get<'s>(&self, source: &'s S) -> &'s A;
}

/// Total setter — field always writable.
pub trait Setter<S, A> {
    fn set(&self, source: &mut S, value: A);
}

/// Lens = Getter + Setter (product types, always present).
/// Laws: GetPut, PutGet, PutPut.
pub trait Lens<S, A>: Getter<S, A> + Setter<S, A> {}

/// Partial getter — field may not exist.
pub trait PartialGetter<S, A: ?Sized> {
    fn preview<'s>(&self, source: &'s S) -> Option<&'s A>;
}

/// Partial setter — set may fail if target absent.
pub trait PartialSetter<S, A> {
    fn set_partial(&self, source: &mut S, value: A) -> bool;
}

/// Affine = PartialGetter + PartialSetter (the default for scripting).
/// Laws: GetPut, PutGet weakened to Option.
pub trait Affine<S, A>: PartialGetter<S, A> + PartialSetter<S, A> {}
```

### Concrete struct (zero-cost, monomorphic within handlers)

```rust
/// A field lens built from function pointers.
/// Druid's Field pattern adapted for pane.
pub struct FieldLens<S, A> {
    get: fn(&S) -> &A,
    get_mut: fn(&mut S) -> &mut A,
}
```

### Composition

```rust
pub struct Then<L1, L2> { outer: L1, inner: L2 }

// Lens.then(Lens) -> Lens
// Lens.then(Affine) -> Affine
// Affine.then(Lens) -> Affine
// Affine.then(Affine) -> Affine
```

### DynOptic (protocol boundary, type-erased)

```rust
/// Type-erased optic for dynamic dispatch at the scripting boundary.
/// No ownership annotations — &/&mut on state is sufficient.
pub trait DynOptic: Send + Sync {
    fn name(&self) -> &str;
    fn get(&self, state: &dyn Any) -> Result<AttrValue, ScriptError>;
    fn set(&self, state: &mut dyn Any, value: AttrValue) -> Result<(), ScriptError>;
    fn is_writable(&self) -> bool;
    fn count(&self, state: &dyn Any) -> Result<usize, ScriptError>;
    fn value_type(&self) -> ValueType;
    fn operations(&self) -> &'static [OpKind];
    fn specifier_forms(&self) -> &[SpecifierForm];
}
```

### AttrValue (serialization boundary)

```rust
/// Values that cross the scripting wire.
/// Shared with filesystem attributes.
pub enum AttrValue {
    String(String),
    Bool(bool),
    Int(i64),
    Float(f64),
    Bytes(Vec<u8>),
    Rect { x: f64, y: f64, w: f64, h: f64 },
}
```

### ScriptableHandler

```rust
/// A handler that exposes scriptable properties.
/// Be lineage: BHandler::ResolveSpecifier + GetSupportedSuites.
pub trait ScriptableHandler {
    type State;
    fn resolve_specifier(&self, spec: &Specifier) -> Resolution;
    fn supported_properties(&self) -> &'static [PropertyInfo];
    fn state_mut(&mut self) -> &mut Self::State;
}
```

### PropertyInfo (replacing Attribute)

```rust
/// Property declaration — what a handler exposes for scripting.
/// Carries the full operation set, specifier forms, and value type
/// that Be's property_info tables had.
pub struct PropertyInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub value_type: ValueType,
    pub operations: &'static [OpKind],
    pub specifier_forms: &'static [SpecifierForm],
}
```

---

## 7. Integration Points

### Messenger convergence path

`set_title(t)` → internal optic `set` → `ClientToServer` message via ServiceRouter.
The optic becomes the ground truth; the Messenger method is sugar.

### CompletionRequest → CompletionReplyPort

Replace `token: u64` correlation with a typed ownership handle.
Same pattern as `ReplyPort` — consumed by `.reply(completions)`,
Drop sends empty completion list. Eliminates ghost state.

### ScriptReplyToken → ReplyPort

Delete `ScriptReplyToken`. Use `ReplyPort` directly. Optionally
wrap in `ScriptReply` newtype for schema enforcement:

```rust
pub struct ScriptReply(ReplyPort);
impl ScriptReply {
    pub fn ok(self, value: AttrValue) { ... }
    pub fn error(self, err: ScriptError) { ... }
}
```

### Observer pattern constraint

If direct (non-filesystem) watches are ever added, mandate async-only
delivery via `send_message`, never `send_and_wait`. Prevents blocking
cycles in the connectivity graph even when notification routing has
cycles (A watches B, B watches A).

---

## 8. Sources

- Clarke, Elkins, Gibbons, Sheridan-Sherrington, Wu. "Profunctor optics,
  a categorical update." Compositionality 2023. arXiv:1703.10857
- "Don't Fear the Profunctor Optics" tutorial (~/gist/DontFearTheProfunctorOptics/)
- Fu, Xi, Das. "Dependent Session Types for Verified Concurrent Programming."
  PACMPL 2026. (~/gist/dependent-session-types/)
- Jacobs, Hinrichsen, Krebbers. "Deadlock-Free Separation Logic." POPL 2024.
  doi:10.1145/3632889. Review: docs/superpowers/dlfactris-review.md
- Haiku source: Looper.cpp, Handler.cpp, Application.cpp, Window.cpp
- Design exploration: docs/scripting-optics-design.md
- Rust ecosystem: optics v0.3, lens-rs v0.3, karpal-optics v0.4, Druid lens
