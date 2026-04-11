---
type: reference
status: current
citation_key: TCP11
aliases: []
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [dependent_session_types, toninho, caires, pfenning, intuitionistic_linear, type_dependence, session_types]
related: [reference/papers/_hub, reference/papers/eact, reference/papers/forwarders, policy/ghost_state_discipline]
agents: [session-type-consultant, pane-architect]
---

# Dependent session types via intuitionistic linear type theory

**Authors:** Toninho, Caires, Pfenning (PPDP 2011)
**Path:** `~/gist/dependent-session-types/`

## Summary

Extends session types with type-level dependence on prior message
contents — the protocol type can use values exchanged earlier
in the conversation. Built on intuitionistic linear logic, so
the linear discipline is preserved.

§3 introduces the dependent arrow notation `↪` for "the type of
the next message depends on the value just received."

## Concepts informed

- pane's `policy/ghost_state_discipline` was synthesized partly
  from this paper — dependent session types formalize what pane
  does well in places (`ReplyPort`, `TimerToken`) and reveals
  where it doesn't yet (`CompletionRequest` token correlation)
- The horizontal-arrow notation `↪` for type-dependent
  continuations is borrowed for pane's protocol documentation
- Why pane's typestate handles work as a substitute for
  full-blown dependent types in many cases

## Used by pane

- `policy/ghost_state_discipline` — direct source
- `reference/plan9/divergences` — `Address` / `ServiceHandle`
  pattern uses ghost-state framing
