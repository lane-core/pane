---
type: reference
status: current
sources: [.claude/agent-memory/optics-theorist/reference_fp_library_optics]
created: 2026-04-03
last_updated: 2026-04-10
importance: normal
keywords: [fp_library, optics, profunctor, lens, prism, traversal, ArcBrand, RcBrand, Send, Sync, rust_crate, monomorphization]
related: [reference/papers/profunctor_optics, reference/papers/dont_fear_optics]
agents: [optics-theorist, pane-architect]
---

# fp-library optics API reference

**Crate:** `fp-library` v0.15.0 (crates.io). A Rust port of
PureScript's `purescript-profunctor-lenses`.

**Source location:** `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/fp-library-0.15.0/src/types/optics/`

## Key types

All parameterized by `'a, PointerBrand, S, T, A, B`:

- **`Lens` / `LensPrime`** — Strong profunctor constraint.
  `from_view_set(get, set)`, `.view(s)`, `.set(s, b)`,
  `.over(s, f)`
- **`AffineTraversal` / `AffineTraversalPrime`** — Strong +
  Choice. `from_preview_set(preview, set)`,
  `.preview(s) -> Result<A, T>`, `.set(s, b)`
- **`Prism` / `PrismPrime`** — Choice. Has preview and review
  fields.
- **`Traversal` / `TraversalPrime`** — Wander constraint.
  Parameterized by TraversalFunc F.
- **`Getter` / `GetterPrime`** — Read-only. Has `view_fn` field.
- **`Setter` / `SetterPrime`** — Write-only.
- **`Fold` / `FoldPrime`** — Read-only multi-focus.
- **`Iso` / `IsoPrime`** — Bidirectional conversion.
- **`Grate` / `GratePrime`** — Closed profunctor.
- **`Composed<'a, S, T, M, N, A, B, O1, O2>`** — composition
  via `Optic` trait.

## Composition

`optics_compose(first, second)` returns `Composed`. The
`Composed` struct implements `Optic<P>` when O1 and O2
implement `Optic<P>`, so constraint union happens via the trait
system.

**Helper functions:** `optics_view`, `optics_set`, `optics_over`,
`optics_preview` work with trait-level optic abstractions
(GetterOptic, SetterOptic, FoldOptic).

## Trait hierarchy (`classes/optics.rs`)

- **`Optic<'a, P, S, T, A, B>`** — base trait, requires
  `P: Profunctor`
- **`LensOptic`** — requires Strong
- **`GetterOptic`** — works with Forget profunctor
- **`SetterOptic`** — works with function profunctor
- `IndexedLensOptic`, `IndexedGetterOptic`, `IndexedSetterOptic`

## PointerBrand

`RcBrand` for single-threaded, `ArcBrand` for Send+Sync. Pane
uses RcBrand in current `property.rs`.

## Send+Sync analysis (verified 2026-04-03)

- fp-library has a parallel send-aware trait hierarchy:
  `SendRefCountedPointer`, `SendUnsizedCoercible`,
  `SendCloneableFn`
- `ArcBrand` implements ALL of these — it CAN produce
  `Arc<dyn Fn + Send + Sync>` via `SendCloneableFn::SendOf`
- BUT `Lens` (and all optic types) hardcode `CloneableFn::Of`,
  which produces `Arc<dyn Fn>` (no Send+Sync on trait object)
- Therefore `Lens<ArcBrand, ...>` is `!Send` despite using Arc
  — the trait object inside lacks Send+Sync bounds
- This is NOT fixable by changing the brand. It requires
  rewriting the optic types to use `SendCloneableFn::SendOf`
- Estimated cost of a parallel SendLens / SendShop / etc:
  400–600 lines tracking upstream

## Performance: by-value cost

`from_view_set` clones S once for view, twice for set (nested
closure captures clone, then re-clones per call). Concrete
encoding's `fn(&S)->A` / `fn(&mut S, A)` avoids all clones.

## Critical note

`Composed` doesn't expose `.view()` or `.set()` directly — must
use `optics_view` / `optics_set` helper functions or the
`Optic::evaluate` trait method with a specific profunctor.

## Tension with concrete encoding

The `optics-design-brief.md` (archived) explicitly decided NOT
to use the profunctor encoding for pane's main optic types,
favoring concrete encoding. But `property.rs` DOES use
fp-library's profunctor encoding. **This tension needs
resolution** — to be addressed in Phase 6's `analysis/optics/`
hub.
