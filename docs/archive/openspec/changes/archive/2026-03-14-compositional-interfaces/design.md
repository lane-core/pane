## Context

Pane's architecture is built on sequential composition of independent servers. The kits (client libraries) need to expose APIs that compose naturally — chaining protocol operations, combining reactive state, piping pattern match results. Rust has strong native support for monadic patterns (`Result`/`?`, `Option` combinators, iterator adapters) but no HKT or do-notation. The design must work with the language, not against it.

Candidate crates identified during research:
- **result-like** (0.5.1): Derive macros that give custom enums the full `Option`/`Result` combinator API. 99 lines, BSD-2, 25k monthly downloads, no deps.
- **agility** (0.1.1): Reactive signals with `map`, `combine`, `contramap`. FRP inspired by SolidJS/Leptos. MIT/Apache, 88KB.

Neither is adopted as a dependency in this change. Specific crate choices are deferred to when the types that need them exist.

## Goals / Non-Goals

**Goals:**
- Establish a compositional design principle that all kits follow
- Define where each compositional pattern applies in the crate hierarchy
- Keep everything ergonomic — if it reads worse than the ad-hoc version, don't use it

**Non-Goals:**
- HKT emulation, monad transformers, or Haskell-style abstractions
- Replacing `Result`/`?` — Rust's error monad is already good
- Adding dependencies to pane-proto (no applicable types exist yet)
- Forcing monadic patterns onto imperative operations (cell grid mutation, calloop callbacks)
- Committing to specific crate dependencies before the consuming code exists

## Decisions

### 1. Three compositional layers

Each layer maps to a natural boundary in the crate hierarchy:

**Layer 1 — Result-like domain types (all kits, when applicable)**
Custom enums with success/failure or some/none shape derive combinator APIs (`map`, `and_then`, `unwrap_or`, `ok_or`) rather than hand-rolling them. Applies to types like plumber match results, store query results, and domain-specific outcomes. Standard `Result` and `Option` remain the default — derived combinators are for domain types that parallel their shape but carry different semantics.

Candidate implementation: `result-like` crate. Decision deferred to when consuming types exist (pane-plumb, pane-store-client, pane-app).

**Layer 2 — Protocol combinators (pane-app)**
A builder API for composing protocol operation sequences as values. `connect().and_then(create(...)).and_then(write_cells(...))` produces a `Proto<A>` that can be inspected, tested, and executed. This lives in pane-app because it requires runtime context (calloop, sockets). pane-proto stays pure types.

The `Proto<A>` type wraps `FnOnce(ProtocolState) -> Result<(A, ProtocolState), ProtocolError>` — the state monad with error handling. Composes via `and_then` (bind) and `map`. The executor runs it against a real connection; tests run it against in-memory state.

**Layer 3 — Reactive signals (pane-app, pane-store-client)**
Signals for observable state with `map`, `combine`, `contramap`. Change notifications from pane-store become signals. Live queries are compositions of query results and notification streams. UI state (focus, dirty, tag content) can be signals that views react to.

Candidate implementation: `agility` crate. Decision deferred to when pane-app and pane-store-client are built. The spec describes the interface (signals with map/combine), not the implementation.

### 2. Don't wrap imperative operations

Cell grid writes, calloop event dispatch, and direct IPC reads remain imperative. Monadic composition applies to *sequencing domain operations*, not to every function call. The test: if `and_then` chaining reads more clearly than sequential statements, use it. If it doesn't, don't.

### 3. Spec the interface, choose the implementation later

The architecture spec describes the patterns and where they apply. Specific crate dependencies (`result-like`, `agility`, or alternatives) are chosen when the crates that need them are being built. This avoids premature dependency commitments and lets us evaluate the ecosystem at decision time.

## Risks / Trade-offs

**[over-abstraction]** → Combinators can obscure control flow if overused. Mitigation: the design principle explicitly says "if it reads worse than the ad-hoc version, don't use it." This is a judgment call, not a mandate.

**[candidate crate maturity]** → agility is at 0.1.1. Mitigation: we specify the interface, not the crate. If agility matures, use it. If something better exists when we need it, use that. If the need is simple enough, write it ourselves.
