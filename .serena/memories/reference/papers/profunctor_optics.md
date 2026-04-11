---
type: reference
status: current
citation_key: CBG24
aliases: [CBG, MixedOptics]
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [profunctor_optics, clarke, boisseau, gibbons, tambara, mixed_optics, monadic_lens, representation_theorem]
related: [reference/papers/_hub, reference/papers/dont_fear_optics, reference/papers/fcmonads]
agents: [optics-theorist, pane-architect]
---

# Profunctor Optics: A Categorical Update

**Authors:** Clarke, Boisseau, Gibbons (Compositionality, 2024)
**Path:** `~/gist/profunctor-optics/`

## Summary

The formal paper on profunctor optics. Defines the optic
hierarchy via Tambara modules, proves the representation
theorem (concrete optics ≅ profunctor optics), introduces
mixed optics (different categories on view side and set side),
and characterizes monadic lenses.

Key results pane uses:

- **Definition 4.6** — MonadicLens (set side in Kl(M))
- **Proposition 4.7** — `MndLens_Ψ ≅ Optic_(×, ⋊)`. View side
  is in W (cartesian, pure); set side is in Kl(Ψ) (writer
  monad Kleisli). Mixed optic.
- **§3.2** — coends as the quotient of a coproduct (relevant
  to the categorical reading of hub-and-spokes)
- **Theorem `th:profrep`** — representation theorem justifying
  pane's concrete encoding
- **§4 affine traversal** — the optic for "0 or 1 focus,"
  isomorphic to MemX's low-confidence rejection at the type
  level
- **Figure 2** — the optic hierarchy

## Concepts informed

- pane's MonadicLens is `Optic_(×, ⋊)` per Proposition 4.7
- Concrete encoding (function pointers) is sound by the
  representation theorem — rank-2 polymorphism not required
- Affine traversal as the right shape for navigation that may
  return zero results
- Mixed optics: read path and write path can be in different
  categories without breaking laws

## Used by pane

- `crates/pane-proto/src/monadic_lens.rs` — concrete encoding
- `analysis/optics/scope_boundaries` (Phase 6)
- `analysis/optics/panefs_taxonomy` — every optic chosen with
  reference to this paper
