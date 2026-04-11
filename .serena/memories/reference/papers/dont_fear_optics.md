---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [optics, profunctor_optics, lens, prism, traversal, accessible_introduction, boisseau, gibbons]
related: [reference/papers/_hub, reference/papers/profunctor_optics]
agents: [optics-theorist, pane-architect]
---

# Don't Fear the Profunctor Optics

**Authors:** Boisseau, Gibbons (and contributors)
**Path:** `~/gist/DontFearTheProfunctorOptics/`

## Summary

Three-part accessible introduction to profunctor optics.
Builds from monomorphic lenses (concrete pairs of getter +
setter), through polymorphic lenses, to the profunctor
representation. Each step motivates the next with a concrete
limitation in the previous representation.

Part 1 covers monomorphic / polymorphic / van Laarhoven lenses.
Part 2 introduces profunctors and the four typeclasses
(`Strong`, `Choice`, `Closed`, `Traversing`). Part 3 shows the
unified profunctor encoding for the optic hierarchy.

`Optics.md` line 187 has the operational reading of affine
laws — used by `optics-theorist` when reasoning about pane's
accessor patterns.

## Concepts informed

- Why pane's MonadicLens is a Lens (cartesian decomposition)
  rather than something stronger
- The affine traversal (zero / one / many focus) shape that
  matches MemX's low-confidence rejection rule at the type level
- The "concrete encoding" used by pane-proto's MonadicLens —
  function pointers, no rank-2 polymorphism

## Used by pane

- `analysis/optics/implementation_guidance` (Phase 6 → analysis/optics/)
- `analysis/optics/scope_boundaries`
- `analysis/optics/panefs_taxonomy`
- The optics-theorist agent reads this first when consulted
