# Research: Optics for Multi-View Object Design

Research on optics (lenses, prisms, traversals, etc.) as they relate to pane's
central design problem: a single object (the pane) that presents multiple coherent
views to different consumers — user, filesystem, protocol client, compositor,
debugger, screen reader.

Sources:

- Pickering, Gibbons, Wu, "Profunctor Optics: Modular Data Accessors" (2017). <https://arxiv.org/abs/1703.10857>
- Riley, "Categories of Optics" (2018). <https://arxiv.org/abs/1809.00738>
- Clarke, Elkins, Gibbons, et al., "Profunctor Optics, a Categorical Update" (2024). <https://arxiv.org/abs/2001.07488>
- Milewski, "Profunctor Optics: The Categorical View" (2017). <https://bartoszmilewski.com/2017/07/07/profunctor-optics-the-categorical-view/>
- Grenrus, "Glassery" — optics taxonomy and lattice (2017). <https://oleg.fi/gists/posts/2017-04-18-glassery.html>
- Well-Typed, "Announcing the optics library" (2019). <https://www.well-typed.com/blog/2019/09/announcing-the-optics-library/>
- nLab, "Lens (in computer science)". <https://ncatlab.org/nlab/show/lens+%28in+computer+science%29>
- Pierce et al., bidirectional lenses / Harmony project. <https://www.cis.upenn.edu/~bcpierce/papers/lenses-etapsslides.pdf>
- Pierce et al., "Quotient Lenses". <https://www.cis.upenn.edu/~bcpierce/papers/quotient-lenses.pdf>
- Ekmett, lens wiki — van Laarhoven derivation. <https://github.com/ekmett/lens/wiki/Derivation>
- Haskell `optics` library (profunctor-based). <https://hackage.haskell.org/package/optics>
- Rust `lens-rs` crate. <https://docs.rs/lens-rs/latest/lens_rs/>
- Rust `optics` crate (v0.3.0). <https://docs.rs/optics/latest/optics/>
- lager (C++ value-oriented UI architecture with lenses). <https://sinusoid.es/lager/>

---

## 1. Optics Fundamentals

### What optics are

An optic is a first-class, composable accessor into part of a data structure. Where
a function `S -> A` extracts a part, and a function `(A, S) -> S` puts a modified
part back, an optic packages these operations together with composition laws. The
taxonomy of optics reflects the different structural relationships a part can have to
a whole.

The key insight: optics separate the _navigation_ into a structure from the
_operation_ performed at the focus. You define how to reach a subpart once, then
reuse that path for getting, setting, modifying, folding, traversing — whatever
operation makes sense for that kind of optic.

### The taxonomy

Each optic kind corresponds to a different structural relationship between the whole
(`S`) and the focus (`A`):

**Iso** (isomorphism): `S` and `A` are the same information, differently shaped. Two
mutually inverse functions `S -> A` and `A -> S`. Bijective. Example: a temperature
in Celsius vs Fahrenheit — same data, different representation.

**Lens**: `S` contains exactly one `A` as a subpart. Product-type accessor. Get
always succeeds, set always succeeds. `S` is isomorphic to `(A, C)` for some
complement `C`. Example: the `x` field of a `Point { x, y }`.

**Prism**: `S` may or may not contain an `A`. Sum-type accessor (pattern matching).
Get may fail (returns `Option<A>`), but you can always construct an `S` from an `A`.
`S` is isomorphic to `Either<D, A>` for some residual `D`. Example: the `Ok` variant
of `Result<A, E>`.

**Affine** (optional): `S` may or may not contain an `A`, but you can't necessarily
construct an `S` from just an `A`. Lens composed with Prism. Get may fail, set
requires an existing `S`. Example: "the `name` field of the `Left` variant" — only
present if the value is `Left`, and you can't build the whole structure from just a
name.

**Traversal**: `S` contains zero or more `A`s. Generalization of Lens to multiple
foci. Can get all of them, modify all of them. Example: all elements of a `Vec<A>`,
or all `name` fields in a list of records.

**Getter**: read-only Lens. Can extract `A` from `S`, cannot modify. Just a function
`S -> A` wrapped to compose with other optics.

**Fold**: read-only Traversal. Extracts zero or more values, cannot modify.

**Setter**: write-only. Can modify all foci, cannot extract. A function
`(A -> B) -> S -> T`.

**Review**: dual of Getter. Can construct `S` from `A`, cannot extract. The
"construction" direction of a Prism.

### The subtyping lattice

Optics form a lattice where more specific optics can be used wherever less specific
ones are expected:

```
                Iso
               /   \
            Lens   Prism
           / |  \ / |
     Getter  | Affine
             |  / \  \
             | /   AffineFold
          Traversal    |
           /    \      |
         Fold   Setter |
          |            |
        (these are the read-only / write-only leaves)
```

An Iso can be used as a Lens or a Prism. A Lens can be used as a Getter, an Affine,
a Traversal, or a Setter. A Prism can be used as an Affine or a Review.
Composition of two optics yields the least upper bound in this lattice: Lens composed
with Prism yields Affine. Traversal composed with Prism yields Traversal.

This means you build complex accessors by composing simple ones, and the type system
tracks what operations are available on the result.

### The lens laws

Well-behaved lenses satisfy three laws:

- **GetPut**: `set l (get l s) s = s` — if you get a value and immediately put it
  back, the structure is unchanged.
- **PutGet**: `get l (set l a s) = a` — if you set a value and immediately get it
  back, you get what you set.
- **PutPut**: `set l a' (set l a s) = set l a' s` — setting twice is the same as
  setting once with the final value.

These laws ensure a lens behaves like a "well-behaved product projection" — the
focus is genuinely independent of the complement. Violation of these laws means the
accessor has hidden coupling or side effects.

The categorical perspective: a lawful lens `(S, A)` is a coalgebra of the costate
comonad `Store A`. This makes lawful lenses precisely the "product projections up to
isomorphism" — they identify `S` with `(A, C)` for some complement `C`.

### Composition

The power of optics is composition. Given:
- a lens from `S` into its field `A`
- a prism from `A` into its variant `B`

Composing them yields an affine from `S` into `B` — an accessor that reaches through
a field and then pattern-matches. The composition is associative and the result type
is determined by the lattice. This is the mechanical advantage: you define small,
simple accessors and compose them into complex paths.

---

## 2. The Profunctor Formulation

### Why it matters

There are three main encodings of optics:

1. **Concrete** (getter/setter pairs): simple but don't compose well — you need
   explicit composition operators for each optic kind.
2. **Van Laarhoven** (polymorphic over functors): the Haskell `lens` library's
   approach. Lenses are `forall f. Functor f => (a -> f b) -> s -> f t`. Composition
   is ordinary function composition. Elegant but can't express Affine optics
   directly, and type errors are cryptic.
3. **Profunctor** (polymorphic over profunctors): the `optics` library's approach.
   Optics are `forall p. C p => p a b -> p s t` where `C` is a constraint
   determining the optic kind. Composition is function composition. Can express all
   optic kinds, including Affine.

The profunctor encoding is the modern formulation. Different optic kinds correspond
to different constraints on the profunctor:

| Optic | Constraint |
|-------|-----------|
| Iso | `Profunctor p` |
| Lens | `Strong p` (cartesian) |
| Prism | `Choice p` (cocartesian) |
| Affine | `Strong p, Choice p` |
| Traversal | `Traversing p` |
| Getter | `Bicontravariant p` |
| Setter | `Mapping p` |
| Grate | `Closed p` |

### The categorical story

Riley (2018) showed that optics form a category: the objects are pairs of types
`(S, A)`, and morphisms are optics between them. Different kinds of optics arise from
different monoidal structures:

- **Lenses** come from the cartesian monoidal structure (products, `×`)
- **Prisms** come from the cocartesian monoidal structure (coproducts, `+`)
- **Grates** come from the closed monoidal structure (exponentials, `→`)

The optic construction "freely adds counit morphisms to a symmetric monoidal
category." This unifies the zoo of optic kinds under one categorical umbrella:
they're all the same construction, parameterized by the monoidal structure.

The profunctor encoding arises from the equivalence between existential optics
(concrete "residual + two functions" representation) and natural transformations
between profunctors — via a generalization of the Yoneda lemma to Tambara modules.

Milewski's derivation: the profunctor representation `forall p. Strong p => p a b ->
p s t` is equivalent to the existential `exists c. (s -> (c, a), (c, b) -> t)`. The
`c` is the "complement" or "residual" — the part of `S` that isn't `A`. The
profunctor encoding hides the existential, making composition automatic.

---

## 3. Bidirectional Transformations and the View-Update Problem

### The database analogy

The view-update problem in databases is: given a view `V` derived from a source `S`
by a query `get : S -> V`, how do you propagate updates to `V` back to `S`? This is
exactly the problem pane faces: the filesystem "view" is derived from the pane's
internal state, and writes to the filesystem must propagate back.

The difficulty: `get` is typically not injective (many source states map to the same
view), so there's no unique `put`. You need a `put : V × S -> S` that takes the
updated view AND the original source and produces the new source. The original source
provides the "complement" — the information lost by the view.

This is precisely a lens. Pierce's Harmony project formalized this connection:
bidirectional transformations between a source and a view are lenses. The lens laws
(GetPut, PutGet) ensure round-trip consistency.

### Quotient lenses

Pierce et al. extended basic lenses to handle equivalence classes. In real systems,
the same abstract value can have multiple representations (a set can be stored sorted
or unsorted, whitespace can vary). A quotient lens operates up to an equivalence
relation: `get` and `put` need only preserve equivalence classes, not exact syntax.

This is directly relevant to pane: the filesystem representation of a pane's state
needn't be syntactically identical to the internal representation. What matters is
semantic equivalence — reading back what was written should be semantically equivalent,
not byte-identical.

### Symmetric lenses

Basic lenses are asymmetric: one side is "the source" and the other is "the view."
Symmetric lenses (Hofmann, Pierce, Wagner 2011) generalize to bidirectional
synchronization between two "peers" — neither is primary. Each side has its own
complement (hidden state), and updates on either side propagate to the other.

Pane's multi-view situation is closer to symmetric lenses than asymmetric ones in
theory — but in practice, the internal state is the source of truth, and the views
are projections. So asymmetric lenses (with internal state as source) are the right
starting model.

---

## 4. Optics in Rust

### The landscape

Two Rust optics crates exist with real implementations:

**`lens-rs`** provides derive macros (`#[derive(Lens)]`, `#[derive(Prism)]`,
`#[derive(Review)]`) that generate optics for struct fields and enum variants. The
API uses `view()`, `view_mut()`, `preview()`, `preview_mut()`, `traverse()`,
`traverse_mut()`, and `review()`. An `optics!()` macro composes field/variant paths.
Ownership is handled through separate `_ref()` and `_mut()` method variants —
borrow-checking applies at each access point.

**`optics` (v0.3.0)** is a self-described "layman's implementation" that provides
Lens, Prism, Iso, Traversal, Getter, Setter, and FallibleIso as explicit structs
(`LensImpl`, `PrismImpl`, etc.) with composition methods (`compose_with_lens`,
`compose_with_prism`, etc.). Zero dependencies, `no_std` compatible. The author
notes it's "unfinished" but "good enough to start experimenting."

### Rust's ownership model and optics

The fundamental tension: Haskell optics work on immutable values with pervasive
sharing. Rust has ownership, borrowing, and lifetimes. This creates three practical
challenges:

1. **Get vs borrow**: A Haskell lens `get` returns a value. In Rust, you usually
   want to return a `&A` or `&mut A`, not clone the value. This means the optic
   must be parameterized over the borrow mode.

2. **Composition and lifetimes**: Composing two lenses where the inner one borrows
   from the outer one's result requires careful lifetime threading. The composed
   accessor's lifetime depends on both components.

3. **Partial moves**: A prism that extracts a variant might need to move data out
   of the enum. Rust's ownership tracking makes this non-trivial — you can't move
   out of a borrowed value.

Both `lens-rs` and `optics` handle this by having separate method variants for
owned, shared-reference, and mutable-reference access. This is manual and somewhat
verbose compared to Haskell's unified interface, but it works within Rust's type
system.

### Practical assessment

Rust optics libraries are immature compared to Haskell's `lens` or `optics`
ecosystem. Neither Rust crate has significant adoption. The profunctor encoding
requires higher-kinded types (HKTs) which Rust doesn't have, so Rust
implementations use either concrete getter/setter pairs or trait-based dispatch,
losing some of the composability that makes Haskell optics shine.

The honest evaluation: full optics-as-a-library is not the right approach for a Rust
project. Rust's ownership model provides its own compositional story — `&T`, `&mut T`,
pattern matching, and the visitor pattern cover most of the ground that optics cover
in Haskell. What's worth importing is the *thinking*, not the library.

---

## 5. Optics Thinking Applied to Pane's Multi-View Problem

### The problem restated

A pane has internal state (buffers, protocol state, metadata). Multiple consumers see
this state through different projections:

| Consumer | What they see | Direction |
|----------|---------------|-----------|
| User | Visual display (cells, widgets) | read + write (input) |
| Filesystem (`/srv/pane/`) | Files and directories | read + write |
| Protocol client | Session-typed messages | read + write |
| Compositor | Rendering primitives | read-only |
| Debugger | Internal state, traces | read-only |
| Screen reader | Semantic structure | read-only |

Each view is a projection from the internal state. Some views are read-only (the
compositor just reads rendering primitives). Others are read-write (the filesystem
interface allows both reading and writing state).

### Mapping to optics

Each view is a lens-like mapping from internal state to a consumer's representation:

```
internal_state ──lens_fs──────> filesystem representation
               ──lens_proto───> protocol messages
               ──lens_visual──> cell grid / widget tree
               ──lens_a11y───> semantic structure
               ──lens_debug──> introspection data
```

The read-only views (compositor, debugger, screen reader) are **getters** — they
extract a representation but never modify the source. The read-write views
(filesystem, protocol) are **lenses** — they both project from internal state and
propagate updates back.

The filesystem view is the most lens-like. Reading `/srv/pane/1/tag` is `get`:
project the tag text from the pane's internal state. Writing to
`/srv/pane/1/tag` is `put`: update the pane's tag text from the filesystem
representation. The lens laws tell us what "well-behaved" means here:

- **GetPut**: reading the tag and writing it back should be a no-op.
- **PutGet**: writing a tag value and reading it back should return what was written.
- **PutPut**: writing twice should be the same as writing once with the final value.

These are exactly the consistency properties you want from a filesystem interface.
If they're violated — if reading back what you wrote gives you something different,
or if writing the same value twice has a different effect than writing it once —
the interface feels broken.

### Where optics help and where they don't

**Where they help — design discipline:**

The lens laws are a correctness specification for each view. For every read-write
view, you can ask: "does this satisfy GetPut and PutGet?" If not, either the view
is lossy (which is fine for getters but not for lenses) or there's a bug.

The quotient lens idea is directly useful: the filesystem representation of a pane's
state uses "format per endpoint" (plain text for tag, JSON for cells, etc.). The
internal representation is Rust structs. These are different syntactic
representations of the same semantic content. A quotient lens says: round-tripping
through the representation should preserve semantic equivalence, not byte-level
identity. This is the right correctness criterion for pane-fs.

The complement concept explains why `put` needs the original source: when you write
to `/srv/pane/1/tag`, the filesystem layer needs the current pane state (not just
the new tag text) because the tag text alone doesn't determine the full pane state.
The "complement" is everything else about the pane that isn't the tag.

**Where they don't help — the reactive/notification problem:**

Classic optics are about one-shot access: you get a value, or you set it. Pane's
problem is also about continuous synchronization: when the user types in a pane, the
filesystem view must be notified. When a protocol client sends a message, the visual
display must update. This is the reactive/observer dimension, and optics have nothing
to say about it directly.

The composition is:

```
state change ──notify──> [for each view that cares] recompute projection
```

This is a push-based reactive system, not an optics operation. The optic tells you
*what* to project; the reactive system tells you *when*. These are orthogonal
concerns.

**Where they don't help — the protocol dimension:**

Session-typed protocol interactions are not lens-like. A protocol client doesn't
"get the rendering of the pane's state as a message" — it engages in a conversation
where the sequence of messages matters. The session type is the correctness
specification for the protocol, not the lens laws. Optics and session types address
different aspects of the same problem (coherent multi-consumer access) but through
fundamentally different mechanisms.

### The Elm Architecture connection

The Elm Architecture (Model → View → Update) is optics thinking made architectural.
A single Model is the source of truth. The View function is a getter: `Model -> Html`.
User actions produce messages. The Update function is the "put" direction:
`(Message, Model) -> Model`.

lager (C++ library) takes this further by using lenses as "cursors" — a cursor
focuses a lens on a subpart of the model, and a view component receives only the
cursor it needs. The view component can read (via get) and write (via actions that
update through the lens). Different view components hold different cursors into the
same model, each seeing only what they need.

This is essentially pane's architecture: the internal state is the Model. Each
consumer holds a "cursor" (a projection) into the relevant part. Updates flow through
the model and are projected out through each consumer's lens. The difference is that
pane's "cursors" cross process boundaries (filesystem, protocol, compositor) while
lager's are in-process.

---

## 6. What This Means for Pane's Design

### The design insight

Optics thinking gives pane a vocabulary and a correctness discipline for its
multi-view architecture:

1. **Each view is a well-defined projection.** The internal state is the source of
   truth. Each consumer gets a projection function (a "getter" or "lens") that
   extracts the relevant representation. Making these projections explicit — as
   named, testable functions — prevents ad-hoc coupling between internal state and
   external representations.

2. **Read-write views satisfy lens laws.** For the filesystem interface and the
   protocol interface, the lens laws (GetPut, PutGet) are the correctness criteria.
   If you write a value through the filesystem and read it back, you should get what
   you wrote (up to quotient — format normalization is fine). If you read a value and
   write it back unchanged, the internal state should be unchanged. These are
   testable properties.

3. **The complement is the hidden state.** When you write to `/srv/pane/1/tag`, the
   `put` function needs the full pane state (not just the tag) because the complement
   (everything else) must be preserved. This means the filesystem layer must hold a
   reference to (or communicate with) the pane's state — it cannot reconstruct the
   pane from the filesystem representation alone. This is obvious in retrospect but
   the lens framework makes it explicit: a view is lossy, and `put` compensates for
   the loss with the original source.

4. **Quotient equivalence is the right correctness criterion.** The internal Rust
   structs and the filesystem text/JSON representations are different syntactic
   representations of the same semantic content. Round-trip correctness means
   semantic equivalence, not byte-level identity. This should be the property tested
   in pane-fs's property-based tests: serialize, deserialize, compare semantically.

### What NOT to do

**Don't build an optics library.** Rust's ownership model gives you lenses for free
at the language level: `&pane.tag` is a getter, `pane.tag = new_value` is a setter,
pattern matching is a prism. The `view()` / `set()` vocabulary of optics libraries
adds indirection without benefit when you have direct struct access within a process.

**Don't force optics across process boundaries.** The filesystem interface isn't
literally a lens composition — it's a FUSE daemon that translates between filesystem
operations and the pane protocol. The lens *thinking* (correctness laws, complement
preservation, quotient equivalence) is valuable; the lens *machinery* is not.

**Don't conflate optics with reactivity.** The question "when does view X get
notified of a change?" is not an optics question. Pane already has pane-notify for
filesystem change detection and session-typed messages for protocol notification.
These are the notification mechanisms. Optics tells you what the correct projection
*is*; reactivity tells you when to recompute it.

### The simplest version that helps

The actionable takeaway is not a library or a type system feature. It's a design
discipline:

1. **For each consumer, define the projection explicitly.** The function that extracts
   "what the filesystem sees" from the pane's internal state should be a named,
   tested function — not scattered across the FUSE implementation.

2. **For each read-write view, verify the lens laws via property tests.** Generate
   random pane states, project to the view, write back, check GetPut and PutGet
   (up to quotient equivalence).

3. **Document the complement.** For each view, state what information is lost. The
   filesystem tag view loses everything except the tag text. The protocol view loses
   rendering state. Making this explicit prevents bugs where someone assumes a view
   is lossless.

4. **Keep internal state canonical.** Quotient lenses work by canonicalizing
   representations. Pane should maintain one canonical internal representation, and
   each view's `get` should produce a deterministic projection from it. If two
   internal states are semantically equivalent, they should be identical (or
   explicitly documented as equivalent).

### How this relates to existing architecture

The architecture spec already describes this pattern in Design Pillar 6 (Semantic
Interfaces):

> "Every interface a pane exposes — filesystem, tag line, protocol messages — SHALL
> present the abstraction level semantically relevant to its consumer."

Optics thinking refines this: each "semantic level" is a projection (a getter or
lens) from the internal state, and for read-write interfaces, the lens laws are the
correctness specification. The abstraction level isn't just about what data is shown
— it's about what round-trip guarantees hold when you read and write through the
interface.

The architecture spec also describes pane-fs as "a translation layer between FUSE
operations and the socket protocol — it is just another client of the pane servers."
In optics terms: pane-fs composes two optics — the lens from internal state to
protocol messages (which the pane server implements) and the iso (or quotient lens)
from protocol messages to filesystem representation (which pane-fs implements). The
composition is a lens from internal state to filesystem representation.

---

## 7. Summary

Optics provide three things for pane's design:

1. **A correctness vocabulary.** The lens laws (GetPut, PutGet, PutPut) are the right
   specification for any read-write view. Quotient equivalence is the right
   relaxation when representations differ syntactically.

2. **An explicit model of information loss.** Each view loses information (the
   complement). Making this explicit prevents over-promising: you can't reconstruct
   a pane from its filesystem representation alone. `put` always needs the original
   source.

3. **A composition story.** A lens from state to substruct, composed with an iso from
   substruct to external format, gives a lens from state to external format. This
   mirrors pane-fs's architecture: server implements the first lens, pane-fs
   implements the second.

What optics do NOT provide: a notification/reactivity model, a session-typed protocol
discipline, or a Rust library worth depending on. The value is in the thinking, not
the implementation.

The simplest design rule: every read-write interface to a pane should satisfy GetPut
and PutGet (up to quotient equivalence), and this should be verified by property
tests. If an interface violates these laws, either it's intentionally lossy (document
it) or there's a bug (fix it).

---

## 8. Optics as What BMessage Field Access Wanted to Be

### The problem BMessage had

BMessage stored data as stringly-typed name-value pairs. The API surface:

```cpp
status_t AddInt32(const char* name, int32 value);
status_t AddString(const char* name, const char* string);
status_t FindInt32(const char* name, int32* value);
status_t FindString(const char* name, const char** string);
```

Three classes of bug were structural to this design:

**1. Name typos.** `AddInt32("width", 100)` on the sending side,
`FindInt32("widht", &w)` on the receiving side. Both calls succeed syntactically
(AddInt32 creates the field; FindInt32 returns `B_NAME_NOT_FOUND`). The bug is
silent. The receiver gets a zero or stale value and proceeds. The Be Book documents
that `FindData()` returns `B_NAME_NOT_FOUND` if the name doesn't exist, but there is
no mechanism to detect that you *meant* to access an existing field and misspelled it.
The name is a runtime string, not a compile-time identifier.

**2. Type mismatches.** `AddInt32("flags", 0x0001)` on the sending side,
`FindString("flags", &s)` on the receiving side. FindString returns `B_BAD_TYPE`.
Again, the error is only detected at runtime, and only if the caller checks the return
value. The Be Book notes that FindData "matches the label `name` with the type you are
asking for" -- but this matching happens at runtime, not at the type level. Peter
Potrebic's newsletter articles (Issue 2-36, "BMessages") document the correct
patterns, but correctness depends on developer discipline.

**3. Schema drift.** As protocols evolved, fields were added, renamed, or retyped.
Every sender and receiver had to be updated in lockstep, with no compiler assistance.
The `what` code identified the message type, but nothing enforced which fields a given
`what` code required. Owen Smith's article on the `BMessage(BMessage*)` constructor
deprecation (Issue 4-46) shows how even the BMessage API itself accumulated design
debt from convenience features that undermined type discipline.

### What optics formalize

A lens is a pair of functions `get : S -> A` and `set : S -> A -> S` where both the
type `A` of the part and the type `S` of the whole are statically known. A lens from
`PaneState` to `width: u32` cannot accidentally access a string field or a nonexistent
field. The name, the type, and the existence of the field are all verified at compile
time.

A prism is the sum-type dual: `preview : S -> Option<A>` and `review : A -> S`.
BMessage's `Find*` methods are ad-hoc prisms -- they return `B_NAME_NOT_FOUND` or
`B_BAD_TYPE` when the extraction fails. But without compile-time types, the failure
mode is a runtime status code that callers routinely ignore.

Composability is the key property optics add beyond typed accessors. Given a lens from
`Message` to `Header` and a lens from `Header` to `Sender`, you compose them (function
composition) to get a lens from `Message` to `Sender`. BMessage had no composition --
accessing nested data meant multiple Find calls, each with its own potential for name
typos and type mismatches, and the nesting structure was implicit in code rather than
declared in types.

### What this means for pane

Pane's `pane-proto` spec already defines `TypedView` and `TypedBuilder` (see
`inter-server-views/spec.md`). These are ad-hoc optics:

- `TypedView::parse(msg) -> Result<Self, ViewError>` is a prism: it tries to extract
  a typed view from an untyped message, failing with `ViewError` if the message
  doesn't match.
- `TypedBuilder` with typestate is a lens in construction: each `.field(value)` call
  moves through type states, and the final `.into_message()` produces the whole.

The spec already enforces that "raw attr key access (`msg.attr("key")`) SHALL NOT be
used in production code paths -- only inside typed view `parse()` implementations."
This is the optics discipline: all access goes through typed, composable accessors.
The raw stringly-typed layer exists only at the boundary where untyped bytes arrive.

The insight: pane has already reinvented optics for its inter-server protocol. The
question is whether to make this explicit (using an optics vocabulary) or keep it
implicit (TypedView/TypedBuilder as bespoke patterns). Section 11 below evaluates the
practical recommendation.

**Source:** Haiku API documentation, <https://www.haiku-os.org/docs/api/classBMessage.html>.
Be Book BMessage reference, <https://www.haiku-os.org/legacy-docs/bebook/BMessage.html>.
Be Newsletter Issue 2-36 (Peter Potrebic, "BMessages"), Issue 4-46 (Owen Smith on
BMessage constructor deprecation).

---

## 9. BMessageFilter as Ad-Hoc Optics

### What BMessageFilter did

Peter Potrebic described the pattern (Issue 3-7, "BMessageFilter"; also William Adams,
Issue 2-36):

> "You didn't have to sub-class the BLooper or BHandler classes. You did have to
> sub-class BMessageFilter, but in a growing system, sub-classing a nice small object
> that is unlikely to change is probably easier than sub-classing a highly active
> object like BWindow or BApplication."

A BMessageFilter was installed on a BHandler or BLooper. Before any message reached
`MessageReceived()`, it passed through the filter chain. Each filter could:

1. **Inspect** the message (read fields, check the `what` code)
2. **Modify** the message (add/change/remove fields)
3. **Swallow** the message (prevent dispatch to the handler)
4. **Pass it through** unchanged

Filters were composable -- multiple filters on the same handler executed in sequence.
They were also decoupled from the handler: adding a filter didn't require modifying
the handler's code.

### The optics mapping

A BMessageFilter is an affine traversal over the message stream. The filter's
operations map to optics:

| BMessageFilter operation | Optics equivalent |
|---|---|
| Inspect message fields | `get` / `preview` through a lens/prism into message data |
| Modify message fields | `over` -- transform the focused part |
| Swallow message | Prism returning `None` -- the message doesn't match |
| Pass through | Identity optic -- message unchanged |

A filter *chain* is optic *composition*. Installing filters `f1` then `f2` on a
handler is composing their optics: the message passes through `f1`'s optic, and if it
emerges, through `f2`'s. This is function composition of profunctor-represented
optics -- the pipeline composes by construction.

### What optics add beyond BMessageFilter

BMessageFilter was powerful but had three gaps that optics fill:

**1. No type safety on the transformation.** A filter could change a `B_KEY_DOWN`
message's fields arbitrarily, and the handler had no way to know the message had been
modified or how. Optics make the transformation typed: a lens from `KeyEvent` to
`KeyCode` can only modify the key code, not the modifiers or the timestamp.

**2. No composition laws.** Two BMessageFilters composed by installation order, but
there were no guarantees about the composed behavior. Optics have laws (lens laws,
prism laws) that guarantee consistent behavior under composition.

**3. No inversion.** BMessageFilter was one-directional: message goes in, (possibly
modified) message comes out. Optics are bidirectional: the same optic that extracts a
field for processing can construct the reply with the updated field.

### Design suggestion for pane

If pane needs message filtering (e.g., middleware that intercepts protocol messages
before they reach handlers), it could be designed as an optics pipeline rather than a
callback chain. Each filter stage is an optic that focuses on the relevant parts of
the message. The pipeline composes by optic composition. Types enforce that each stage
only accesses and modifies what it declares.

This would give pane what BMessageFilter had (decoupled, composable message
interception) with what BMessageFilter lacked (type safety, composition laws,
bidirectionality).

**Source:** Be Newsletter Issue 3-7 (Peter Potrebic, "BMessageFilter"), Issue 2-36
(William Adams on BMessageFilter as composition mechanism). Chris Penner, "Composable
Filters Using Witherable Optics," <https://chrispenner.ca/posts/witherable-optics>.
Chris Penner, "Generalizing jq and Traversal Systems Using Optics and Standard
Monads," <https://chrispenner.ca/posts/traversal-systems>.

---

## 10. Session Types + Optics: The Two Halves

### The division of labor

Session types and optics address orthogonal concerns that together cover the full
picture of inter-component communication:

**Session types** describe the *conversation structure* -- the morphisms between
components:
- What messages are sent and received
- In what order (sequencing)
- Who decides at branch points (internal/external choice)
- When the conversation ends
- That both parties follow complementary protocols (duality)

**Optics** describe the *state access structure* -- how each step of the conversation
reads from and writes to the component's internal state:
- Which fields of state a message step reads (get/preview)
- Which fields of state a message step modifies (set/over)
- How the accessed fields compose into the full state (composition)
- That the access is consistent (lens/prism laws)

Together: session types are the *horizontal* structure (what happens over time, between
components), and optics are the *vertical* structure (what happens at each step,
between the protocol and the component's state).

### Concrete example: the pane resize protocol

The pane-protocol spec describes resize as a conversation between compositor and
client:

```
Session type (horizontal):
  Compositor -> Client: Send<ResizeEvent>    -- compositor sends new geometry
  Client -> Compositor: Send<WriteCells>      -- client redraws for new size
```

At each step, optics govern state access (vertical):

```
Step 1 (compositor sends resize):
  compositor_state --[geometry_lens]--> window_geometry
  Construct ResizeEvent from window_geometry

Step 2 (client receives resize):
  Receive ResizeEvent
  client_state --[viewport_lens]--> viewport
  Update viewport with new geometry
  client_state --[body_lens]--> body_content
  Reflow body_content for new viewport
  Construct WriteCells from reflowed content
```

The session type guarantees step 2 happens after step 1 and that the client sends
WriteCells (not some other message). The optics guarantee step 2 accesses the viewport
and body content through typed, composable accessors -- not through raw field names
that could be misspelled or mistyped.

### Session-OCaml: existing work combining session types with lenses

The most direct precedent is **Session-OCaml** (Imai, Yoshida, Yuen; Coordination
2017, extended in Science of Computer Programming 2019). Session-OCaml is a library
for session-typed concurrent programming in OCaml that uses lenses to enforce
linearity.

The key idea: session channels are stored in a "slot" data structure (a type-level
heterogeneous collection). Lenses access specific channels in the slot. A
parameterized monad tracks pre- and post-conditions on the slot state. When you send
on a channel, a lens extracts the channel from the slot, the session type advances,
and a lens puts the updated channel back. The lens ensures you access the right
channel; the monad ensures you follow the protocol.

From the paper:

> "The key ideas are: (1) polarised session types, which give an alternative
> formulation of duality enabling OCaml to automatically infer an appropriate session
> type; and (2) a parameterised monad with a data structure called 'slots' manipulated
> with lenses, which can statically enforce session linearity including delegations."

This is the exact pattern pane needs: session types govern the conversation, and lenses
govern how each conversation step accesses the component's state. Session-OCaml proves
the combination is practical.

### The general correspondence

The composition of session steps corresponds to the composition of optic
transformations:

1. **Sequential composition** of session steps (send then receive) corresponds to
   sequential composition of state transformations (update state after send, update
   state after receive). Each transformation is mediated by optics.

2. **Choice** in session types (offer/select) corresponds to **prism** in optics: the
   branch chosen determines which part of the state is accessed. An
   `Offer { Resize(s1), Close(s2) }` at the session level corresponds to a prism over
   the handler state: the Resize branch focuses on geometry state; the Close branch
   focuses on cleanup state.

3. **Delegation** (passing a channel to another party) corresponds to a lens
   composition shift: the optic that accesses the delegated channel's state moves from
   one component's lens to another's.

4. **Recursion** in session types (looping protocols) corresponds to **traversal** in
   optics: each iteration of the protocol accesses the same structural path,
   accumulating state changes.

### What's missing from the literature

No published work (as of this research) combines *profunctor optics* with session
types in a single formal framework. Session-OCaml uses simple van Laarhoven lenses,
not the profunctor formulation. The Caires-Pfenning correspondence gives session types
a linear logic foundation; profunctor optics have their own categorical foundation
(Tambara modules, monoidal actions). A unified framework would connect these: session
types as morphisms in a category of protocols, optics as morphisms in a category of
state access, with a functor between them. This is an open research direction, not
something pane needs to solve -- but it's worth knowing the theoretical landscape.

Milewski's work on linear lenses (2024) is relevant here. A linear lens
`s %1-> (a, b %1-> t)` means you *must* produce the new whole -- you can't silently
drop the complement. This aligns with session types' linearity: you must consume the
channel, not silently abandon it. The linear optics in Haskell's `linear-base` package
(`Control.Optics.Linear`) are the closest existing formalization of optics in a linear
setting, but they operate on linear data, not session-typed channels.

**Source:** Imai, Yoshida, Yuen, "Session-OCaml: A Session-Based Library with
Polarities and Lenses," Coordination 2017 (LNCS 10319); extended in Science of
Computer Programming 167, 2019. <https://www.sciencedirect.com/science/article/pii/S0167642318303289>.
Preprint: <http://mrg.doc.ic.ac.uk/publications/session-ocaml-a-session-based-library-with-polarities-and-lenses/preprint.pdf>.
Milewski, "Linear Lenses in Haskell," 2024. <https://bartoszmilewski.com/2024/02/07/linear-lenses-in-haskell/>.
Haskell linear-base optics: <https://hackage-content.haskell.org/package/linear-base-0.5.0/docs/Control-Optics-Linear.html>.

---

## 11. Optics and Pane's Monoidal Structure (Categorical Connection)

### How optics connect to pane's compositional architecture

The pane architecture has a natural monoidal structure: spatial composition. Two panes
placed side-by-side (horizontal split) or stacked (vertical split) form a compound
pane. This is a tensor product on a category of panes.

Optics connect to this precisely. If the compound pane `A tensor B` has state
`(StateA, StateB)` (product), then:

- The **first projection** lens focuses on `StateA` -- the optic that accesses the
  left pane's state within the compound.
- The **second projection** lens focuses on `StateB` -- the right pane's state.
- These are the projection morphisms of the cartesian monoidal structure.

For sum types (a pane that is *either* a shell or an editor), the optics are prisms,
corresponding to the cocartesian monoidal structure.

The connection: **pane composition (tensor product) produces compound state (product
type), and the optics that decompose compound state into parts are exactly the
projections of the monoidal structure.** The category of optics and the monoidal
category of panes are related by the fact that optics are the "internal morphisms" for
accessing parts within monoidal products.

Riley (2018) showed this formally: optics form a category, and the optic construction
"freely adds counit morphisms to a symmetric monoidal category." Lenses arise from
the cartesian (product) monoidal structure. Prisms arise from the cocartesian
(coproduct) structure. The monoidal action determines the optic family.

### The three categorical structures

Pane's architecture involves three categorical structures, all consistent:

1. **Session types** form a category of protocols (morphisms = protocol steps,
   composition = sequential protocol composition). Foundation: Caires-Pfenning
   correspondence with linear logic.

2. **Optics** form a category of state access (morphisms = typed accessors, composition
   = nested access). Foundation: Riley's categories of optics, profunctor
   representation via Tambara modules.

3. **Panes** form a monoidal category (objects = pane types, tensor = spatial
   composition, morphisms = protocol connections between panes).

Optics decompose the state that session types transform, and both operate within the
monoidal structure of pane composition. This doesn't need to appear in the spec -- it
would be overspecification. But the designers should know: the existing commitments
(spatial composition, session-typed protocols, typed state access) are categorically
consistent. The theory validates the design rather than requiring changes to it.

**Source:** Riley, "Categories of Optics," 2018. <https://arxiv.org/abs/1809.00738>.
Clarke et al., "Profunctor Optics, a Categorical Update," 2020. <https://arxiv.org/abs/2001.07488>.
Milewski, "Profunctor Optics: The Categorical View," 2017. <https://bartoszmilewski.com/2017/07/07/profunctor-optics-the-categorical-view/>.

---

## 12. Practical Recommendation (Consolidated)

### Three options, in order of pragmatism

**Option 1: Optics as design vocabulary, not runtime library.** Use the optics
vocabulary (lens, prism, traversal, composition) in design documents and code
comments. Implement the patterns manually as `TypedView`/`TypedBuilder` (as the spec
already does). The optics thinking guides the design; the implementation is idiomatic
Rust (structs, enums, `From`/`TryFrom` impls, builder patterns). Lowest risk, highest
clarity.

**Option 2: Thin optics traits, no external dependency.** Define a minimal
`Lens<S, A>` trait (get + set) and `Prism<S, A>` trait (preview + review) in
pane-proto. Derive implementations for message views. Use these for composition where
it reduces duplication (e.g., composing a lens into a message with a lens into a
field). Small abstraction, no external crate.

**Option 3: Use the `optics` crate.** The `optics` crate is `no_std`,
zero-dependency, and provides the core abstractions. Could serve as a foundation for
pane's typed views. Risk: depending on a crate with uncertain maintenance.

**Recommendation:** Option 1 now. Option 2 when composition patterns become repetitive
enough to justify the abstraction. The spec's `TypedView` and `TypedBuilder` are
already optics in disguise. Making the vocabulary explicit costs nothing and helps
designers reason about composition. Introducing trait abstractions should wait until
concrete instances justify the generalization.

### What to name in the spec

When the spec or code says "typed view," it means prism. When it says "typed builder,"
it means review. When it says "multiple views of a pane," it means a family of lenses
from the same source. When it says "message filter pipeline," it means composed affine
traversals. The vocabulary is available. Using it makes the compositional structure
legible without requiring anyone to learn category theory -- the terms are
self-describing once you've seen the definitions in sections 1--2 above.
