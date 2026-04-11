---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [fcmonads, cruttwell, shulman, vdc, virtual_double_category, segal, composites, restrictions, virtual_equipments]
related: [reference/papers/_hub, reference/papers/duploids, reference/papers/logical_aspects_vdc, reference/papers/profunctor_optics]
agents: [session-type-consultant, optics-theorist, pane-architect]
---

# fcmonads: A unified framework for generalized multicategories

**Authors:** Cruttwell, Shulman (Theory and Applications of Categories, 2010)
**Path:** `~/gist/fcmonads.gist.txt`

## Summary

Mathematical foundation for **virtual double categories (VDCs)**
and their generalized multicategories. Introduces VDCs as the
right setting for talking about "things with cells" where the
cells may not compose.

Key sections:

- **§3** defines VDCs (objects, vertical and horizontal arrows,
  cells)
- **§5** defines composites (Segal condition for when cells can
  be composed)
- **§6** defines restrictions (interface transformations on
  channel types)
- **§7** defines virtual equipments (when restrictions exist)

The Segal condition is the key compositionality property —
batch optimization in pane corresponds to checking when Segal
holds for a given composition.

## Concepts informed

- VDCs as the right framework for monadic lenses (Clarke et al.
  uses fcmonads as foundation)
- Restriction operations in pane-fs as VDC restrictions
- Segal condition as a guide for when batching message dispatch
  is safe
- The "non-trivial pane structure" — pane's session-typed
  channels are horizontal arrows, handlers are vertical, cells
  are dispatch events

## Used by pane

- `analysis/duploid/writer_monad_and_optics` — mixed optics use the
  monad / comonad structure from fcmonads
- `architecture/looper` — six-phase batch ordering checks the
  Segal condition implicitly
- The optics-theorist consults this for monadic lens background
