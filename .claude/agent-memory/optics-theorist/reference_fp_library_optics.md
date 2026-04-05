---
name: fp-library Optics API Reference
description: Key fp-library 0.15.0 optics types, their signatures, and how they map to profunctor theory
type: reference
---

fp-library 0.15.0 (crates.io dependency of pane-proto) provides profunctor-encoded optics ported from PureScript's purescript-profunctor-lenses.

**Source location:** `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/fp-library-0.15.0/src/types/optics/`

**Key types (all parameterized by `'a, PointerBrand, S, T, A, B`):**
- `Lens` / `LensPrime` — Strong profunctor constraint. `from_view_set(get, set)`, `.view(s)`, `.set(s, b)`, `.over(s, f)`
- `AffineTraversal` / `AffineTraversalPrime` — Strong + Choice. `from_preview_set(preview, set)`, `.preview(s) -> Result<A, T>`, `.set(s, b)`
- `Prism` / `PrismPrime` — Choice. Has preview and review fields.
- `Traversal` / `TraversalPrime` — Wander constraint. Parameterized by TraversalFunc F.
- `Getter` / `GetterPrime` — Read-only. Has `view_fn` field.
- `Setter` / `SetterPrime` — Write-only.
- `Fold` / `FoldPrime` — Read-only multi-focus.
- `Iso` / `IsoPrime` — Bidirectional conversion.
- `Grate` / `GratePrime` — Closed profunctor.
- `Composed<'a, S, T, M, N, A, B, O1, O2>` — Composition via `Optic` trait.

**Composition:** `optics_compose(first, second)` returns `Composed`. The `Composed` struct implements `Optic<P>` when O1 and O2 implement `Optic<P>`, so constraint union happens via the trait system.

**Helper functions:** `optics_view`, `optics_set`, `optics_over`, `optics_preview` work with trait-level optic abstractions (GetterOptic, SetterOptic, FoldOptic).

**Trait hierarchy in classes/optics.rs:**
- `Optic<'a, P, S, T, A, B>` — base trait, requires P: Profunctor
- `LensOptic` — requires Strong
- `GetterOptic` — works with Forget profunctor
- `SetterOptic` — works with function profunctor
- `IndexedLensOptic`, `IndexedGetterOptic`, `IndexedSetterOptic`

**PointerBrand:** `RcBrand` for single-threaded, `ArcBrand` for Send+Sync. Pane uses RcBrand in current property.rs.

**Critical note:** `Composed` doesn't expose `.view()` or `.set()` directly — must use `optics_view`/`optics_set` helper functions or the `Optic::evaluate` trait method with a specific profunctor.
