---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [logical_aspects_vdc, hayashi, das, fvdblTT, type_theory, vdc, protypes, restrictions, comprehension_types]
related: [reference/papers/_hub, reference/papers/fcmonads]
agents: [optics-theorist, session-type-consultant]
---

# Logical aspects of virtual double categories (FVDblTT)

**Authors:** Hayashi, Das et al.
**Path:** `~/gist/logical-aspects-of-vdc.gist.txt`

## Summary

The type theory of VDCs — FVDblTT. Defines:

- **Protypes** as channel types (horizontal arrows of the VDC)
- **Restrictions** as interface transformations on channel
  types (the type-theoretic counterpart of fcmonads §6)
- **Comprehension types** as observation of protocol state

The result is a programming language for reasoning about
VDC-shaped systems — including session-typed channels and
their composition.

## Concepts informed

- pane's session-typed channels as protypes
- Restriction operations as type-level coercions on channel
  endpoints
- Comprehension types as the formal model of "observing the
  current state of an in-flight session"

## Used by pane

- Background reference for the session-type consultant when
  reasoning about pane's channel composition rules at the
  type-theoretic level (rather than the categorical level
  in `fcmonads`)
