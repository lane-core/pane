---
type: reference
status: current
citation_key: MMSZ
aliases: [ProjMPST]
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [projections, mpst, multiparty_session_types, global, local, end_point]
related: [reference/papers/_hub, reference/papers/forwarders, reference/papers/async_global_protocols]
agents: [session-type-consultant]
---

# Generalizing projections in multiparty session types

**Path:** `~/gist/generalizing-projections-in-multiparty-session-types/`

## Summary

Refines the projection operator from global session types to
local (per-endpoint) session types. The classical projection
rules are conservative; this paper widens what's projectable
without losing safety.

For pane, the relevant question is: given a global protocol
involving compositor + multiple panes, what's each participant's
local view, and is it implementable?

## Concepts informed

- When local views can be reconstructed from a global protocol
- Decidable subclasses where projection always exists
- Why some natural global protocols don't project cleanly

## Used by pane

- Reference for designing protocols with three+ parties
  (currently rare in pane — most are bilateral pane↔server)
