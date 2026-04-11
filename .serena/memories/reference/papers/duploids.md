---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [duploids, mangel, melliès, munch_maccagnoni, MM14b, MMM25, polarized, non_associative, hasegawa_thielecke, fuhrmann, thunkable, central, dialogue]
related: [reference/papers/_hub, reference/papers/fcmonads, reference/papers/linear_logic_no_units, pane/duploid_analysis, pane/duploid_deep_analysis]
agents: [session-type-consultant, optics-theorist, pane-architect]
---

# Classical Notions of Computation and the Hasegawa-Thielecke Theorem (duploids)

**Authors:** Mangel, Melliès, Munch-Maccagnoni (MMM25); foundational definitions in Munch-Maccagnoni 2014b (MM14b)
**Path:** `~/gist/classical-notions-of-computation-duploids.gist.txt`

## Summary

Defines **duploids** as non-associative polarized categories
that integrate call-by-value and call-by-name computation.
Three of four associativity equations hold; the **(+,−)
equation fails**, capturing the CBV/CBN distinction.

Key results:

- **Proposition 6 (MM14b)** — thunkable ⟹ central in any
  duploid
- **Hasegawa-Thielecke (MMM25)** — in a *dialogue duploid*,
  central = thunkable (the converse holds)
- The shift operator ↑ ω_X mediates between positive (CBV) and
  negative (CBN) subcategories
- Composition laws: (+,+) and (−,−) associate; (+,−) does not

## Concepts informed

- pane's analysis as a duploid: positive subcategory = wire
  types (`ServiceFrame`, `ControlMessage`, `Message`), negative
  subcategory = handlers and demand-driven reads
- The server deadlock was a non-associative bracket realized
  concurrently — the actor model prevents this from arising
  by serializing all polarity crossings
- Thunkability as the criterion for safe batch coalescing
- ActivePhase<T> as the explicit shift operator carrying
  negotiated state

## Used by pane

- `pane/duploid_analysis` (Phase 6 → `analysis/duploid/_hub`)
- `pane/duploid_deep_analysis` (writer monad, namespace
  commutativity, mixed optics, shift operator analysis)
- `pane/polarity_classifications` — every pane abstraction
  classified positive / negative / oblique
- `architecture/looper` — six-phase batch ordering respects
  polarity discipline
