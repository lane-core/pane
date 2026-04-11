---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [squier, rewriting, hott, kraus, von_raumer, local_confluence, coherence, cells]
related: [reference/papers/_hub, reference/papers/duploids]
agents: [session-type-consultant, optics-theorist]
---

# Squier's theorem in homotopy type theory

**Authors:** Kraus, von Raumer
**Path:** `~/gist/squier-rewriting-hott.gist.txt`

## Summary

Proves Squier's theorem (about rewriting systems and coherence)
in homotopy type theory. The key insight: **rewriting steps
are cells**. Local confluence at the cell level generates
higher coherence cells. Global coherence follows from local
confluence + termination.

The interpretation: in a system with multiple resolution
mechanisms, you don't need to specify a global ordering —
local confluence at each pair of mechanisms suffices to make
the whole system coherent.

## Concepts informed

- pane's looper has multiple resolution mechanisms (polarity
  frames, CBV focusing, signal precedence). Local confluence
  at each pair makes the whole dispatch coherent.
- The six-phase batch ordering in `architecture/looper` is the
  termination side of Squier's theorem — without termination,
  local confluence doesn't generate global coherence

## Used by pane

- Background reference for reasoning about whether multiple
  pane invariants compose without conflict
