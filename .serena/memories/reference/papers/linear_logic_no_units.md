---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [linear_logic, no_units, houston, promonoidal, MLL, unit_free]
related: [reference/papers/_hub, reference/papers/duploids, reference/papers/fcmonads]
agents: [session-type-consultant]
---

# Linear logic without units

**Authors:** Houston (thesis)
**Path:** `~/gist/linear-logic-without-units.gist.txt`

## Summary

PhD thesis on **promonoidal categories** as models for unitless
multiplicative linear logic. The motivation: full MLL has
multiplicative units (1 and ⊥) that complicate the
categorical semantics. Removing them yields a cleaner theory
useful for systems where the unit isn't load-bearing.

Promonoidal structure generalizes monoidal structure by allowing
the tensor to be a profunctor rather than a functor — meaning
"composing two things" becomes a relation, not necessarily a
function.

## Concepts informed

- pane's session types are unitless: there's no "do nothing"
  protocol step
- The promonoidal generalization is what makes mixed optics
  work (the tensor on one side may not match the tensor on
  the other)
- Why pane's `Flow::Continue` / `Stop` is not the same as a
  unit — it's a control-flow primitive, not a session type

## Used by pane

- Background for the session-type consultant when proposals
  involve "the empty session" or unit-like channel states
