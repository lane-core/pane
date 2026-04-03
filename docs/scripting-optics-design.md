# Scripting Protocol Design: Optics + Session Types

Design exploration for pane's scripting protocol. The optic foundation
types (`pane-optic` crate, `ScriptableHandler`, `DynOptic`, `PropertyInfo`)
are implemented; the wire protocol and mediator are deferred. Grounded in
Clarke et al. "Profunctor optics, a categorical update" (Compositionality
2023), the "Don't Fear the Profunctor Optics" tutorial, and Fu/Xi/Das
"Dependent Session Types for Verified Concurrent Programming" (TLL+C,
PACMPL 2026).

---

## 1. Optic Trait Design for Rust

### The Problem

We need composable, bidirectional accessors that satisfy the optic laws
(GetPut, PutGet, PutPut for lenses; MatchBuild, BuildMatch for prisms)
within Rust's type system. The profunctor encoding makes composition
trivial — it's just function composition — but requires `forall p .
Constraint p => p a b -> p s t`, which is rank-2 polymorphism. Rust
doesn't have that directly.

### Approach A: Concrete Existential Optics

Clarke et al. Definition 2.1 defines the general optic as a coend:

    Optic((A,B),(S,T)) = integral^{M in M} C(S, M . A) x D(M . B, T)

In Haskell, this is modeled as a GADT with an existential type for the
residual M (the paper's Appendix, Definition A.8). In Rust, existential
types are trait objects. The concrete optic stores two closures with a
shared but hidden residual type:

```rust
/// A concrete optic: existential over the residual type M.
/// S → (M, A) and (M, B) → T for some hidden M.
///
/// This is the coend representation (Clarke et al. Def 2.1).
pub struct Optic<S, T, A, B> {
    // Using closures to hide M behind `impl Fn`.
    split: Box<dyn Fn(&S) -> (Box<dyn Any>, A) + Send + Sync>,
    merge: Box<dyn Fn(Box<dyn Any>, B) -> T + Send + Sync>,
}
```

**Problem:** The `Any`-based residual loses type safety. Composition
requires matching residual types at runtime.

A better encoding: generic over M, with composition existentializing it.

```rust
/// Concrete optic with visible residual type M.
pub struct ConcreteOptic<S, T, A, B, M> {
    split: fn(&S) -> (M, A),
    merge: fn(M, B) -> T,
}
```

Composition of `ConcreteOptic<S, T, A, B, M>` with
`ConcreteOptic<A, B, X, Y, N>` produces
`ConcreteOptic<S, T, X, Y, (M, N)>` — the residual tensors, exactly as
the paper predicts (the monoidal product on residuals, Clarke et al.
Section 2, the composition rule for Optic).

```rust
impl<S, T, A, B, M> ConcreteOptic<S, T, A, B, M> {
    fn then<X, Y, N>(
        self,
        inner: ConcreteOptic<A, B, X, Y, N>,
    ) -> ConcreteOptic<S, T, X, Y, (M, N)>
    where
        M: 'static, N: 'static,
    {
        let split_outer = self.split;
        let merge_outer = self.merge;
        let split_inner = inner.split;
        let merge_inner = inner.merge;
        ConcreteOptic {
            split: move |s| {
                let (m, a) = (split_outer)(s);
                let (n, x) = (split_inner)(&a);
                ((m, n), x)
            },
            merge: move |(m, n), y| {
                let b = (merge_inner)(n, y);
                (merge_outer)(m, b)
            },
        }
    }
}
```

**Problem:** The M type parameter leaks into the user-facing API. The
user must name the full residual chain: `ConcreteOptic<S, T, X, Y, (M,
(N, P))>`. This defeats composability — the whole point is that composed
optics should have the same type regardless of how they were composed.

### Approach B: Closure-Based Van Laarhoven Encoding

Clarke et al. Section 5.1 shows that lenses admit a Van Laarhoven
encoding: `forall F. Functor F => (A -> F B) -> S -> F T`. This is
closer to what Rust can express because it only requires a single
universally-quantified functor, not a profunctor.

The key insight from the profunctor paper (Theorem 4.4, the profunctor
representation theorem) is:

    integral_{P in Tamb} V(P(A,B), P(S,T)) ~= Optic((A,B),(S,T))

For *lenses specifically*, the Tambara constraint is Cartesian (Strong
in Haskell). The Van Laarhoven encoding specializes this to
representable profunctors (UpStar F). But we can get most of the benefit
without fully general profunctor quantification.

**Van Laarhoven lenses in Rust:**

```rust
/// A Van Laarhoven-style lens.
/// The F is "any functor" — in practice we instantiate it to
/// Identity (for set/modify) and Const<A> (for get).
pub trait VLLens<S, T, A, B> {
    fn run<F: Functor>(&self, f: impl Fn(A) -> F::Of<B>) -> impl Fn(S) -> F::Of<T>;
}
```

This requires Rust GATs for `F::Of<B>`. Worse, it requires the caller
to be generic over F — Rust doesn't have the rank-2 polymorphism to
hide F. Every lens consumer must be generic, which infects call sites.

### Approach C: Concrete Pairs with Type-Erased Composition (Recommended)

Don't fight Rust's type system. Use the concrete encoding where each
optic family is a struct holding its characteristic operations, and
provide a uniform `DynOptic` trait for type-erased composition at the
protocol boundary.

The insight: optics *within* a single handler are static and monomorphic.
The concrete types work beautifully — no composition across type
boundaries needed. Dynamic composition only happens at the *scripting
protocol boundary*, where we're already in `dyn`-land because the
specifier chain comes from the wire.

```rust
/// A lens: total get, contextual set.
///
/// Laws (Clarke et al. Def 3.1, specialized to Set):
///   GetPut: set(s, view(s)) == s
///   PutGet: view(set(s, b)) == b
///   PutPut: set(set(s, b1), b2) == set(s, b2)
pub struct Lens<S, A> {
    view: fn(&S) -> &A,
    set: fn(&mut S, A),
}

/// A prism: partial get, total inject.
///
/// Laws (Clarke et al. Def 3.5):
///   MatchBuild: match(build(a)) == Ok(a)
///   BuildMatch: preview(s).map(build) or s unchanged
pub struct Prism<S, A> {
    preview: fn(&S) -> Option<&A>,
    inject: fn(A) -> S,
}

/// An affine: partial get, contextual set.
///
/// The composition of Lens then Prism (Clarke et al. Def 3.7).
/// Tambara constraint: Cartesian + Cocartesian.
pub struct Affine<S, A> {
    preview: fn(&S) -> Option<&A>,
    set: fn(&mut S, A) -> bool, // false if target absent
}

/// A traversal: zero-or-more targets.
///
/// Clarke et al. Def 3.12: Optic for power series.
/// Concrete: extract all targets, fill them back.
pub struct Traversal<S, A> {
    /// Collect all targets.
    contents: fn(&S) -> Vec<&A>,
    /// Apply a function to each target in place.
    modify: fn(&mut S, &dyn Fn(&A) -> A),
}
```

Type-invariant (S = T, A = B) is the right default for pane's scripting
protocol. BeOS scripting never used type-changing updates — you get a
string, you set a string. The architecture spec's "semantic equality up
to failures" (Section 4 below) means we're not doing polymorphic
updates.

**Static composition within a handler:**

```rust
impl<S, A> Lens<S, A> {
    /// Compose with an inner lens. Produces a new lens.
    ///
    /// This is the concrete composition from "Don't Fear" Section 1:
    ///   (|.|) :: Lens s a -> Lens a x -> Lens s x
    fn then<X>(&self, inner: &Lens<A, X>) -> Lens<S, X> {
        let view_outer = self.view;
        let set_outer = self.set;
        let view_inner = inner.view;
        let set_inner = inner.set;
        Lens {
            view: move |s| (view_inner)((view_outer)(s)),
            set: move |s, x| {
                let mut a = (view_outer)(s).clone();
                (set_inner)(&mut a, x);
                (set_outer)(s, a);
            },
        }
    }
}
```

(In practice the closures capture function pointers, not owned state, so
this compiles to zero-cost inlining.)

**Dynamic composition at the protocol boundary** — the `DynOptic` trait:

```rust
/// Type-erased optic for dynamic composition at the scripting
/// protocol boundary. This is where we cross from static to dynamic.
///
/// The Value type is the serialized representation that crosses the
/// wire — pane's AttrValue or similar.
pub trait DynOptic: Send + Sync {
    /// The name of this property (for discovery).
    fn name(&self) -> &str;

    /// Get the property value as a serialized AttrValue.
    fn get(&self, state: &dyn Any) -> Result<AttrValue, ScriptError>;

    /// Set the property value from a serialized AttrValue.
    fn set(&self, state: &mut dyn Any, value: AttrValue) -> Result<(), ScriptError>;

    /// Whether this optic supports set (lenses yes, folds no).
    fn is_writable(&self) -> bool;

    /// For traversals: count of targets.
    fn count(&self, state: &dyn Any) -> Result<usize, ScriptError>;
}
```

**Assessment:** This is the right approach for pane. Static
monomorphic optics within handlers give full type safety and zero cost.
Dynamic `dyn DynOptic` at the protocol boundary gives the composability
that BeOS's ResolveSpecifier had. The boundary between static and
dynamic is exactly where it should be: at the handler's published
scripting interface.

The profunctor encoding's great insight — that composition is just
function composition — manifests differently in Rust. Within a handler,
composition is monomorphic method chaining (`lens.then(lens)`). Across
handlers, composition is the ResolveSpecifier loop, which is inherently
dynamic. Trying to unify these two into a single profunctor-style
encoding would fight Rust's type system for no practical gain.

### Open sub-problems

1. **Derive macro.** `#[derive(Scriptable)]` should generate `DynOptic`
   implementations from struct fields. This is straightforward — each
   named field becomes a lens, each `Option<T>` field becomes an affine,
   each `Vec<T>` field becomes a traversal. The macro inspects field
   types and generates the appropriate optic.

2. **Serialization boundary.** The `AttrValue` type needs to cover all
   types that cross the scripting wire. This overlaps with the filesystem
   attribute format (`xattr`). Recommend a shared `pane-attr` crate.

3. **Indexed access.** BeOS's `B_INDEX_SPECIFIER` accessed children by
   index. In pane this is a `Traversal::at(i)` — a traversal focused to
   a single index. This needs a combinator:
   ```rust
   impl<S, A> Traversal<S, A> {
       fn at(&self, index: usize) -> Affine<S, A> { ... }
   }
   ```
   The type changes from Traversal to Affine because `at(i)` may fail.

---

## 2. Connecting Optics to the Session-Typed Protocol

### The Problem

A scripting interaction is a *conversation* with protocol structure:
the client sends a query (optic-addressed), the handler resolves it,
sends back a result or error. This has session type structure. Design
the session types for these interactions.

### Session type for property access

The basic interaction is request-response. Using pane-session's existing
primitives:

```rust
/// Session type for "get property X".
///
///   Client               Handler
///     |--- PropertyGet --->|
///     |<-- GetResult ------|
///     |       End          |
type GetProperty = Send<PropertyGet, Recv<GetResult, End>>;

/// Session type for "set property X to V".
type SetProperty = Send<PropertySet, Recv<SetResult, End>>;

/// Session type for "list available properties" (GetSupportedSuites).
type ListProperties = Send<ListRequest, Recv<PropertyList, End>>;
```

Where:
```rust
struct PropertyGet {
    /// Specifier chain — which property, possibly nested.
    specifiers: Vec<Specifier>,
}

struct PropertySet {
    specifiers: Vec<Specifier>,
    value: AttrValue,
}

enum GetResult {
    Value(AttrValue),
    Error(ScriptError),
    /// Forwarded to another handler — see Section 3.
    Forward(ForwardInfo),
}
```

### Session type for the full scripting conversation

The client doesn't know in advance whether it wants to get, set, or
list. The session type uses `Select` (internal choice by the client):

```rust
/// A scripting session. Client chooses the operation.
///
/// TLL+C insight (Section 3.1): protocol is abstract, channel types
/// give directional interpretation. The Select here is the client's
/// internal choice; the handler sees it as Branch (external choice).
type ScriptSession = Select<
    GetProperty,                         // left: get
    Select<
        SetProperty,                     // left-left: set
        Select<
            ListProperties,              // left-left-left: list
            Select<
                CountProperty,           // count
                ExecuteProperty,         // execute (Be's B_EXECUTE_PROPERTY)
            >,
        >,
    >,
>;
```

This is clunky. Binary choice trees for N options are the well-known
ergonomic problem with binary session types. The `session!` macro can
hide this:

```rust
session_type! {
    ScriptSession = choose {
        Get(PropertyGet) -> recv GetResult,
        Set(PropertySet) -> recv SetResult,
        List(ListRequest) -> recv PropertyList,
        Count(CountRequest) -> recv CountResult,
        Execute(ExecRequest) -> recv ExecResult,
    }
}
```

### Session type for chained access (forwarding)

Here's where it gets interesting. BeOS's `hey Tracker get Frame of Window 0`
involves three handlers:

1. Tracker (the application) peels off "Application Tracker" (identity)
2. Window 0 (the window) peels off "Window 0"
3. Frame (the window's frame property) resolves "Frame"

Each step is: receive specifier, either resolve locally or forward to
a child handler. The session type for a *single resolution step* is:

```rust
/// One step of specifier resolution.
/// The resolver either answers directly or delegates.
///
/// Haiku's BLooper::resolve_specifier (Looper.cpp:1428) does exactly
/// this loop: call ResolveSpecifier on current target, if new target
/// differs, repeat.
type ResolveStep = Recv<Specifier, Select<
    // Resolved: send back the result
    Send<ResolveResult, End>,
    // Forward: indicate which sub-handler to try next
    Send<ForwardTo, ResolveStep>,  // <-- recursive!
>>;
```

**This is a recursive session type.** TLL+C Section 4.2 handles this
via mu-binders: `fix X. ?(spec). +(resolved | forwarded . X)`. The
RecProto rule requires the recursion to be guarded by a protocol action
(a receive, here), which it is.

In pane-session, recursive session types require a newtype wrapper
(Rust doesn't have equi-recursive types):

```rust
/// Recursive specifier resolution session.
///
/// Mirrors BLooper::resolve_specifier's loop structure:
/// "loop { target = target.ResolveSpecifier(msg); if stable, break }"
///
/// The recursion is always productive (guarded by Recv) because each
/// step either resolves or peels off one specifier — the chain has
/// finite length.
struct ResolveLoop;
// Represented as: Recv<Specifier, Branch<Resolved, Forwarded>>
// where Forwarded contains a new Chan<ResolveLoop, T>.
```

### Approach A: Recursive session type (theoretically clean, practically awkward)

The recursive session type exactly mirrors BeOS's ResolveSpecifier loop.
Each step consumes a specifier and either resolves or recurses. The
session type guarantees progress (each step processes one specifier)
and termination (the specifier chain is finite).

**Problem:** pane-session's `Chan<S, T>` doesn't currently support
recursive session types. Adding them requires either iso-recursive
unfolding (explicit `unfold()` calls that change the type) or
equi-recursive types (which Rust can't express).

### Approach B: Iterative protocol with escape (recommended)

Don't model the recursion in the session type. Model each
request-response as a flat session. The *client* drives the chain by
issuing one request per step, examining the result, and deciding whether
to issue another.

```rust
/// A single scripting request-response.
/// The client issues one of these per specifier in the chain.
/// If the result is "forward to handler X", the client opens a
/// new session with handler X and continues.
type ScriptRequest = Send<ScriptQuery, Recv<ScriptResponse, End>>;

enum ScriptResponse {
    /// Direct answer.
    Value(AttrValue),
    /// Set succeeded.
    Ok,
    /// Property list (GetSupportedSuites equivalent).
    Properties(Vec<PropertyInfo>),
    /// Forward: the query should be redirected to this target.
    /// The client opens a new session with the target.
    Forward { target: Id, remaining: Vec<Specifier> },
    /// Error.
    Error(ScriptError),
}
```

Each individual request-response is session-typed (send query, receive
response, done). The chain is driven by the client: if it gets `Forward`,
it opens a new session with the indicated target and sends the remaining
specifiers.

This is how HTTP redirects work, and it's how `hey` actually works too —
the `hey` tool constructs the full specifier chain and the resolution
happens inside the target application. The "forwarding" is internal to
the app; from the outside it's one request, one response.

**Key realization:** BeOS's ResolveSpecifier was *internal* to a single
application. The `hey` tool sent one message to the application; the
application resolved the entire chain internally. The forwarding was
between handlers *within the same looper*. This means the session type
for the *external* protocol is just request-response. The internal
resolution is ordinary method dispatch within the handler.

This changes the design significantly. The session type is simple:

```rust
/// The scripting protocol as seen from outside: one request, one response.
/// Internal resolution (the chain walk) is hidden behind the handler.
type ScriptingProtocol = Send<ScriptQuery, Recv<ScriptResponse, End>>;
```

The complexity lives in how the handler resolves the query internally,
which is the subject of Section 3.

### Assessment

Approach B wins. The scripting protocol's session type is simple because
the protocol boundary is at the application edge, not at each handler.
Within the application, the ResolveSpecifier loop is ordinary Rust code
operating on `&mut HandlerState`. Session types add value at the
process boundary (compositor <-> client, client <-> client); they add
only ceremony within a single-threaded handler loop.

This aligns with TLL+C's protocol/channel separation. The *protocol*
(what messages flow) is simple. The *channel type* (the ownership and
direction interpretation) is simple. The *internal logic* (how the
handler resolves a chain) is where the optics live.

### Open sub-problems

1. **Multi-step external queries.** If pane ever supports querying
   *across* applications (not just across handlers within one app),
   then the forwarding becomes an external protocol concern. Defer this
   — BeOS didn't support it either (`hey` targeted one app at a time).

2. **Streaming results.** A traversal get should return multiple values.
   Options: (a) return all values in one response (simple, may be large),
   (b) use a streaming session type `Send<Query, Recv<Count, RecvN<Value, End>>>`.
   Recommend (a) for now — scripting queries are small.

---

## 3. The Dynamic Composition Problem

### The Problem

BeOS's `hey Tracker get Frame of Window 0` was resolved by:
1. `BApplication::ResolveSpecifier` sees "Window 0", returns `BWindow*`
2. `BWindow::ResolveSpecifier` sees "Frame", returns `this`
3. `BWindow::MessageReceived` handles `B_GET_PROPERTY` for "Frame"

(Verified in Haiku source: `Looper.cpp:1428-1466`, `Handler.cpp:469-483`,
`Window.cpp:2698+`)

Each handler peels off one specifier and either resolves or delegates.
The chain length is unknown at compile time. In optics terms, this is
*dynamic optic composition* — a runtime-constructed sequence of
statically-typed steps.

### Approach A: Trait-Object Chain (the direct translation)

Each handler registers its available optics as `Box<dyn DynOptic>`. The
resolution loop walks the specifier chain, matching each specifier
against the current handler's optics:

```rust
/// Resolve a specifier chain against a handler's state.
///
/// Direct translation of BLooper::resolve_specifier.
/// The loop terminates because:
///   1. Each step consumes one specifier (the chain shrinks)
///   2. The chain has finite length (it came from the wire)
fn resolve_chain(
    handler: &mut dyn ScriptableHandler,
    query: &ScriptQuery,
) -> ScriptResponse {
    let mut specifiers = query.specifiers.as_slice();
    let mut target: &mut dyn ScriptableHandler = handler;

    while let Some((spec, rest)) = specifiers.split_first() {
        match target.resolve_specifier(spec) {
            Resolution::Resolved(optic) => {
                // This handler owns the property.
                if rest.is_empty() {
                    // Terminal: execute the operation.
                    return execute_op(&query.operation, optic, target.state_mut());
                } else {
                    // The optic focuses on a sub-object that is itself
                    // scriptable. Narrow the state and continue.
                    //
                    // This is where the optic's "view" function gives
                    // us the sub-state, and we recurse.
                    match optic.get_sub_handler(target.state_mut()) {
                        Some(sub) => {
                            target = sub;
                            specifiers = rest;
                        }
                        None => return ScriptResponse::Error(
                            ScriptError::NotAContainer(spec.clone())
                        ),
                    }
                }
            }
            Resolution::NotFound => {
                return ScriptResponse::Error(
                    ScriptError::PropertyNotFound(spec.clone())
                );
            }
        }
    }
    ScriptResponse::Error(ScriptError::EmptyChain)
}
```

**The key type:** `ScriptableHandler` is the trait that handlers
implement to participate in scripting. It's the typed equivalent of
overriding `ResolveSpecifier` + `GetSupportedSuites`:

```rust
/// A handler that exposes scriptable properties.
///
/// The pane equivalent of overriding BHandler::ResolveSpecifier
/// and BHandler::GetSupportedSuites.
trait ScriptableHandler {
    /// The handler's state type (for internal optic access).
    type State;

    /// Resolve one specifier against this handler's properties.
    fn resolve_specifier(&self, spec: &Specifier) -> Resolution;

    /// List all available properties (GetSupportedSuites).
    fn supported_properties(&self) -> Vec<PropertyInfo>;

    /// Access the handler's state mutably.
    fn state_mut(&mut self) -> &mut Self::State;
}

enum Resolution {
    /// This handler owns the property. Here's the optic.
    Resolved(Box<dyn DynOptic>),
    /// Not my property.
    NotFound,
}
```

### Approach B: Free-Algebra Interpretation (the principled path)

The specifier chain is a *syntax tree* of optic operations. Rather than
immediately executing each step, reify the chain as a data structure
and interpret it:

```rust
/// A reified specifier chain — a free algebra of optic operations.
///
/// Analogous to a free monad: the structure captures "what to do"
/// without "doing it." The interpreter (the handler) gives meaning.
enum OpticChain {
    /// Terminal: access this property.
    Property(String, ScriptOp),
    /// Composed: focus through this property, then continue.
    Through(String, Specifier, Box<OpticChain>),
}
```

Interpretation walks the chain, resolving each step against the handler
tree:

```rust
fn interpret(
    chain: &OpticChain,
    handler: &mut dyn ScriptableHandler,
) -> ScriptResponse {
    match chain {
        OpticChain::Property(name, op) => {
            match handler.resolve_specifier(&Specifier::Direct(name.clone())) {
                Resolution::Resolved(optic) => execute_op(op, optic, handler.state_mut()),
                Resolution::NotFound => ScriptResponse::Error(/* ... */),
            }
        }
        OpticChain::Through(name, spec, rest) => {
            // ... resolve name, get sub-handler, recurse
        }
    }
}
```

### Maintaining optic laws under dynamic composition

The optic laws (GetPut, PutGet) must hold at each individual step.
Dynamic composition preserves them if each step independently satisfies
them — this follows from Clarke et al. Proposition 2.3 (optics form a
category under composition).

Concretely: if `Lens<Window, Frame>` satisfies GetPut/PutGet, and
`Lens<App, Window>` satisfies GetPut/PutGet, then their composition
`App -> Window -> Frame` satisfies GetPut/PutGet. The dynamic chain
is just evaluating this composition step by step.

The risk is *partial mutation*: if a set operation crashes midway through
a chain, the intermediate state may be inconsistent. BeOS had the same
problem — ResolveSpecifier held the looper lock, but the mutation itself
wasn't transactional.

Pane's mitigation: the handler's `&mut self` access is single-threaded
(it's in the looper). If a set operation fails partway, the handler
sees the partial state on its next event and can repair it. The
filesystem view eventually catches up (attributes are written after
the handler method returns).

### Assessment

Approach A (trait-object chain) is the right choice. It's the direct
translation of BeOS's ResolveSpecifier, it's simple, and it works.
Approach B (free algebra) adds a reification layer that doesn't buy
anything — the interpretation is the same loop, just with an extra
indirection.

The connection to existential types is real but doesn't change the
implementation: `Box<dyn DynOptic>` *is* the existential `exists M.
(S -> M x A) x (M x B -> T)` — the `Any`-based residual is hidden
behind the trait object's vtable.

### Open sub-problems

1. **Sub-handler navigation.** The `get_sub_handler` method needs to
   return a `&mut dyn ScriptableHandler` for the sub-state. This
   requires that container properties (things with children) implement
   `ScriptableHandler` themselves. This is the `View` -> child `View`
   navigation pattern from BeOS. In pane, the primary unit is the pane
   itself — no deep widget hierarchy. Sub-handler navigation may be
   limited to "pane exposes named sub-objects."

2. **Specifier types.** BeOS had 7 specifier types (direct, index,
   reverse index, range, reverse range, name, id). Pane should start
   with 3: direct (by name), index (into ordered collection), name
   (into keyed collection). Add more as needed.

---

## 4. The Multi-View Consistency Problem

### The Problem

Pane state has four views: visual (compositor renders it), protocol
(the session-typed wire format), filesystem (`/pane/<id>/attrs/`),
and semantic (accessibility). An optic governs the projection from
internal state to each view. When state changes through one view,
the others must reflect it.

### The Categorical Structure

There is a categorical structure. Define:

- **C_internal**: the category whose objects are handler state types and
  whose morphisms are state transformations
- **C_visual**, **C_protocol**, **C_fs**, **C_semantic**: the categories
  of each view's state representation

Each view is a functor F_v : C_internal -> C_v that projects internal
state to view state. The optics are the bidirectional refinement of
these functors — they're not just projections (functors) but
bidirectional connections (lenses).

The four views form a *cospan* (or more precisely, a diagram) with
the internal state at the center:

```
            C_visual
           /
C_internal --- C_protocol
           \
            C_fs
           \
            C_semantic
```

Each arrow is a lens (or affine, for views where some properties aren't
representable). The consistency requirement is that the diagram
*commutes* in a suitable sense: changing internal state and then
projecting to any view gives the same result regardless of which view
triggered the change.

### Bidirectional transformations and the optic laws

This connects to the bidirectional transformations literature (Foster
et al., "Combinators for bidirectional tree transformations," POPL
2005 — the origin of the lens concept). The GetPut law says: projecting
to a view and immediately updating back is identity. The PutGet law
says: updating from a view and projecting back gives what you set.

The multi-view extension requires a *consistency condition*: for views
V1 and V2, if you set through V1, the projection to V2 must reflect
the change. This is automatically satisfied if both V1 and V2 are
lenses on the same internal state. The internal state is the single
source of truth; the views are projections.

```
set via filesystem: /pane/1/attrs/title = "New Title"
  -> internal state update: handler.title = "New Title"
  -> visual projection: compositor re-renders title bar
  -> protocol projection: next ScriptQuery("title") returns "New Title"
  -> semantic projection: accessibility tree updated
```

The optic laws guarantee each individual projection is consistent. The
single-source-of-truth architecture guarantees cross-view consistency.

### "Semantic equality up to failures"

The architecture doc mentions this. It means: the four views are
*eventually* consistent. A crashed compositor means the visual view
is stale; a crashed filesystem watcher means the fs view is stale. The
optic laws hold *when the system is healthy*. Under failure, the views
diverge temporarily.

Formally, this is a *quotient* on the equality relation used in the
optic laws. Instead of `GetPut: set(s, view(s)) == s` using strict
equality, we use an equivalence relation `~` that identifies states
that differ only in stale/crashed views:

    GetPut_relaxed: set(s, view(s)) ~ s

where `s1 ~ s2` iff `s1` and `s2` agree on all non-failed views.

This is the same relaxation that eventual consistency databases use,
and it's the right model for pane. The recovery semantics (reconnect to
compositor, re-sync filesystem attributes) restore strict equality.

### Approach A: Eager synchronization

Every state change immediately propagates to all four views. The handler
method returns, then the looper:
1. Sends updated attributes to the filesystem
2. Sends updated visual state to the compositor
3. Pushes any protocol notifications to watchers
4. Updates the semantic tree

**Problem:** this is expensive and often unnecessary. Most state changes
only affect one or two views. A keystroke in a text editor changes the
visual and protocol views but not the title (filesystem/semantic).

### Approach B: Lazy/Demand-Driven Projection (Recommended)

The internal state is the truth. Views are projected *on demand*:

- **Visual:** projected when the compositor requests a frame
- **Protocol:** projected when a scripting query arrives
- **Filesystem:** projected when an attribute is read (or on a
  coalesced timer for proactive sync)
- **Semantic:** projected when the accessibility bridge queries

This is the same pattern as Be's app_server: the window didn't
eagerly push every state change to the server. It marked areas as dirty,
and the server pulled on the next frame.

The optics make this natural. Each view's optic is a `view` function
(the get direction). The handler owns the truth; the views just call
`view` when they need to.

For the set direction (external change flowing in), the handler receives
an event and updates its internal state. The update invalidates the
relevant views, which re-project lazily.

### Assessment

Approach B. The internal state is the source of truth. Views are
projections computed on demand. The optics provide the projection
functions. Cross-view consistency is automatic because there's one
source. Failure tolerance is automatic because stale views just
need to re-project.

This is not novel — it's the standard Model-View pattern. The optics
just formalize what the projections are and guarantee they satisfy
the laws.

### Open sub-problems

1. **Change notification.** Some views need push notification, not just
   demand-driven projection. Filesystem watchers (`pane-notify`) need to
   know *which* attributes changed. The handler should emit a set of
   "dirty properties" after each event, and the sync layer writes only
   those attributes.

2. **Bidirectional filesystem.** The filesystem view is writable (you
   can `echo "New Title" > /pane/1/attrs/title`). This means the
   filesystem is both a projection *and* an input. The flow is:
   fs write -> pane-notify event -> handler's `attr_changed` hook ->
   internal state update -> other views invalidated. This is a
   bidirectional lens, and the optic laws must hold in both directions.

---

## 5. Integration with the Existing Handler Model

### The Problem

The `Handler` trait currently gives `&mut self` access to all state on
every message. If optics become the access mechanism for scripting, how
does this interact with the existing handler model?

### Handler state decomposition

Handlers should NOT be decomposed into optic-accessible fields at the
trait level. The handler owns its state however it wants. Optics are
a *scripting interface* — they describe what the handler *exposes*,
not how it *stores*.

This is exactly how BeOS worked. A BHandler could store its state
however it liked. The scripting interface (GetSupportedSuites,
ResolveSpecifier) was a separate concern that the handler opted into.

```rust
/// A text editor's handler state.
struct EditorHandler {
    buffer: TextBuffer,
    cursor: Position,
    selection: Option<Range>,
    dirty: bool,
    filename: Option<PathBuf>,
    // ... lots of internal state
}

/// The scripting interface exposes a curated subset.
impl ScriptableHandler for EditorHandler {
    type State = Self;

    fn resolve_specifier(&self, spec: &Specifier) -> Resolution {
        match spec.property() {
            "title" => Resolution::Resolved(Box::new(TitleLens)),
            "content" => Resolution::Resolved(Box::new(ContentLens)),
            "selection" => Resolution::Resolved(Box::new(SelectionAffine)),
            "line_count" => Resolution::Resolved(Box::new(LineCountGetter)),
            _ => Resolution::NotFound,
        }
    }

    fn supported_properties(&self) -> Vec<PropertyInfo> {
        vec![
            PropertyInfo::new("title", "Pane title", true),
            PropertyInfo::new("content", "Buffer content", true),
            PropertyInfo::new("selection", "Current selection", true),
            PropertyInfo::new("line_count", "Number of lines", false),
        ]
    }

    fn state_mut(&mut self) -> &mut Self { self }
}
```

### Messenger evolution

`Messenger` should NOT gain generic `view<L: Lens>` and `set<L: Lens>`
methods. Messenger is for *sending messages*, not *accessing state*. The
scripting protocol goes through the message system — a scripting query
arrives as a message, the handler resolves it, and the response goes
back as a message.

The new surface area on Messenger is minimal:

```rust
impl Messenger {
    /// Send a scripting query to this pane and wait for the response.
    /// Like send_and_wait but specialized for the scripting protocol.
    pub fn script_get(
        &self,
        target: &Messenger,
        property: &str,
    ) -> Result<AttrValue, ScriptError> {
        // Construct a ScriptQuery with a single direct specifier,
        // send it via send_and_wait, interpret the ScriptResponse.
        todo!()
    }

    pub fn script_set(
        &self,
        target: &Messenger,
        property: &str,
        value: AttrValue,
    ) -> Result<(), ScriptError> {
        todo!()
    }
}
```

These are convenience wrappers around the existing `send_and_wait` /
`send_request` machinery. The scripting protocol reuses the existing
message transport — no new channel type needed.

### Optics and MessageFilter interaction

A MessageFilter intercepts messages before they reach the handler.
Can a filter be defined in terms of an optic?

Not directly, and it shouldn't be. Filters operate on messages (events
in flight). Optics operate on handler state (the accumulated data model).
These are different categories. A filter might inspect a scripting query
and reject it based on access control — but that's pattern matching on
the message, not optic access into the handler's state.

The one useful connection: a filter could restrict which optics are
accessible:

```rust
/// A scripting access control filter.
struct ScriptAccessFilter {
    /// Properties this pane exposes to external scripting.
    /// Others are handler-internal only.
    allowed: HashSet<String>,
}

impl MessageFilter for ScriptAccessFilter {
    fn filter(&self, msg: &Message) -> FilterResult {
        if let Message::ScriptQuery(query) = msg {
            if !self.allowed.contains(&query.specifiers[0].property()) {
                return FilterResult::Reject;
            }
        }
        FilterResult::Pass
    }
}
```

### The `#[derive(Scriptable)]` story

A derive macro generates the `ScriptableHandler` implementation and the
individual optic registrations:

```rust
#[derive(Scriptable)]
struct EditorState {
    /// Exposed as read-write string property.
    #[scriptable(name = "title")]
    title: String,

    /// Exposed as read-write. Serialized as UTF-8 bytes.
    #[scriptable(name = "content")]
    content: TextBuffer,

    /// Exposed as read-only integer property.
    #[scriptable(name = "line_count", read_only)]
    line_count: usize,

    /// Not exposed to scripting.
    cursor: Position,
    dirty: bool,
}
```

The macro generates:
1. A `resolve_specifier` method matching on property names
2. A `supported_properties` method listing all `#[scriptable]` fields
3. Individual `DynOptic` implementations for each field (lens for
   read-write, getter for read-only)
4. Serialization/deserialization between the field type and `AttrValue`

This requires a `Scriptable` trait on the field types that provides
to/from `AttrValue` conversion:

```rust
trait ScriptableValue: Sized {
    fn to_attr(&self) -> AttrValue;
    fn from_attr(value: &AttrValue) -> Result<Self, ScriptError>;
}
```

Implemented for: `String`, `bool`, `i32`, `i64`, `f32`, `f64`,
`Vec<u8>`, and types that implement `serde::Serialize + DeserializeOwned`.

### Assessment

The integration is clean because the scripting protocol is a *layer on
top of* the existing handler model, not a replacement for it. The handler
keeps its `&mut self` access. Scripting goes through messages like
everything else. The optics live at the boundary between the handler's
internal world and the external scripting protocol.

### Open sub-problems

1. **Nested scriptable objects.** A handler might expose a property that
   is itself scriptable (e.g., a text editor exposing its buffer, which
   has its own properties like "line_count", "selection"). The derive
   macro should support `#[scriptable(nested)]` for fields that
   implement `ScriptableHandler`.

2. **Dynamic properties.** Some properties aren't known at compile time
   (a generic key-value store, a document with user-defined metadata).
   The trait should support a fallback method:
   ```rust
   fn resolve_dynamic(&self, name: &str) -> Option<Box<dyn DynOptic>>;
   ```

---

## 6. The Affine/Linear Gap

### The Problem

Pane's session channels are `#[must_use]` — dropping one generates a
compiler warning. But Rust can't enforce *linear* use (exactly once);
it can only enforce *affine* use (at most once). A crashed process
silently drops its channels. The TLL+C paper's linear types guarantee
channels are used exactly once, preventing stuck peers. Rust can't
provide this. Can optics help?

### The connection between affine optics and affine types

An affine optic (Clarke et al. Def 3.7) has a partial get: `preview :
S -> Either A T`. The target may or may not be present. If present, you
can get/set it; if absent, you can't.

An affine type system allows values to be used *at most once* (can be
dropped, can't be duplicated). Rust's ownership model is affine: every
value is used at most once (moved), with `Drop` as the implicit "use
zero times" escape hatch.

The connection: affine optics and affine types share the same
categorical structure. An affine optic is a Tambara module for the
monoidal actions of the cartesian product AND the coproduct — it
requires both Cartesian and Cocartesian structure on the profunctor.
An affine type is a resource that may or may not be consumed, with
both a "use" path and a "discard" path — the same product/coproduct
duality.

But this connection is *structural*, not *operational*. Knowing that
affine optics share structure with affine types doesn't give us a
mechanism to enforce linear channel usage.

### Approach A: Optic-indexed session steps

Idea: make each session step carry an optic that describes what state
it accesses. If the session channel is in state `Chan<Send<A, S>, T>`,
the optic associated with this step tells us which part of the handler's
state will be modified by the send. If the channel is dropped, we know
which optic access was skipped.

```rust
/// A session step annotated with the optic it accesses.
struct AnnotatedSend<A, S, O: DynOptic> {
    _marker: PhantomData<(A, S, O)>,
}
```

**Problem:** The optic annotation is ghost state — it exists for
verification, not runtime behavior. Rust doesn't have ghost types
(TLL+C does, via implicit arguments). The annotation would just be
dead code that the compiler ignores.

### Approach B: Recovery-oriented design (the pragmatic path, recommended)

Accept that Rust can't enforce linearity. Design for recovery:

1. **`#[must_use]`** catches most accidental drops at compile time.
   This is the "90% solution."

2. **Drop impl for cleanup.** `Chan<S, T>`'s Drop sends a cancellation
   message to the peer. This is what pane already does with `ReplyPort`
   — dropping it sends `ReplyFailed`. Generalize this:

   ```rust
   impl<S, T: Transport> Drop for Chan<S, T> {
       fn drop(&mut self) {
           // Best-effort cancellation notification.
           let _ = self.transport.send_raw(&CANCEL_SENTINEL);
       }
   }
   ```

3. **Crash recovery at the protocol level.** When a peer crashes, the
   surviving peer gets `SessionError::Disconnected`. The surviving peer
   knows what session state it was in (the type parameter S) and can
   clean up accordingly. For scripting sessions: if the client crashes
   mid-query, the handler's `ScriptReplyToken` is dropped, which is a
   no-op (the response has nobody to receive it). If the handler crashes
   mid-resolution, the client gets `Disconnected` and reports an error.

4. **Affine optics as the appropriate model.** The scripting protocol's
   optics are naturally affine, not linear. A "get Frame of Window 0"
   query may fail (the window might not exist — that's the affine
   `preview` returning `None`). The protocol *already* handles the
   "target absent" case through error responses. Affine types and
   affine optics are the right match.

### What TLL+C's linear types give that Rust can't

TLL+C's linearity guarantees (Section 4.2) that every channel is used
exactly once — no stuck peers, no orphaned channels. In Rust:

- **No stuck peers:** handled by `Drop` + `Disconnected` error. Not
  as elegant, but operationally equivalent.
- **No orphaned channels:** `#[must_use]` catches most cases. For the
  rest, the Drop-based cleanup handles it.
- **No replay:** Rust's move semantics prevent using a channel twice.
  This is the one guarantee Rust *does* provide, and it's the most
  important one.

TLL+C's ghost state discipline (Section 2.1 of the theory chapter)
offers one more thing: compile-time proof that a protocol is followed
correctly, including that all branches are covered. Rust's exhaustive
match on `Offer<L, R>` partially recovers this — you must handle both
branches. But you can still drop the resulting channel without using it.

### Assessment

The affine/linear gap is real but manageable. Rust's affine type system
(move semantics + Drop) covers the practical cases. The remaining gap
(can drop without using) is handled by Drop-based cleanup and the crash
recovery protocol.

The optics connection is conceptual, not operational. Affine optics
don't help enforce linear channel usage. What they do is provide the
right *model* for scripting access that may fail — and that's valuable
in its own right. The scripting protocol should use affine optics
(not lenses) as the default, because targets may not exist.

### Open sub-problems

1. **`LinearLint` or custom analysis.** A custom clippy lint could
   enforce that `Chan` values are consumed (sent/received/closed),
   not just dropped. This would be the practical approximation of
   TLL+C's linear typing. Worth investigating once the session crate
   is stable.

2. **Protocol-level idempotency.** If the client retries a query
   (because it got Disconnected and isn't sure if the set took effect),
   the handler should be idempotent. This is a design principle, not a
   type system feature: PutPut (`set(set(s, a), a) == set(s, a)`)
   guarantees that repeated sets converge. All lenses satisfy this
   by definition. Another reason to build on optic laws.

---

## Summary of Recommendations

| Area | Recommendation | Key Insight |
|------|---------------|-------------|
| **Optic encoding** | Concrete structs + `DynOptic` trait | Static within handler, dynamic at protocol boundary |
| **Session type** | Simple request-response | Resolution is internal to handler, not a session |
| **Dynamic composition** | Trait-object chain (ResolveSpecifier loop) | BeOS's pattern translates directly |
| **Multi-view consistency** | Single source of truth, demand-driven projection | Standard Model-View, optics formalize projections |
| **Handler integration** | Scripting as a layer on top, not a replacement | Handler owns its state; optics describe the external interface |
| **Affine/linear gap** | Drop-based cleanup + `#[must_use]` | Affine optics are the right model; linearity via convention |

### What to prototype first

1. `DynOptic` trait + a hand-written implementation for 2-3 properties
2. `ScriptableHandler` trait + the resolution loop
3. `#[derive(Scriptable)]` macro on a simple struct
4. Integration test: construct a ScriptQuery, resolve it, verify the response
5. Filesystem bridge: attributes in `/pane/<id>/attrs/` backed by DynOptic

The session type for the scripting protocol is simple enough to be the
existing `Send<ScriptQuery, Recv<ScriptResponse, End>>` — no new session
type machinery needed for v1.
