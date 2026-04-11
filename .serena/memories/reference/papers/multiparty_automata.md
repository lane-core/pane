---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [multiparty, compatibility, automata, communicating_automata, asynchronous]
related: [reference/papers/_hub, reference/papers/forwarders, reference/papers/async_global_protocols]
agents: [session-type-consultant]
---

# Multiparty compatibility in communicating automata

**Path:** `~/gist/multiparty-compatbility-in-communicating-automata/`

## Summary

Automata-theoretic perspective on multiparty session-type
compatibility. Defines compatibility as a property of the
synchronous product of communicating automata, then characterizes
when async behavior preserves it.

Complements `reference/papers/forwarders` (linear-logic
perspective) — same property, different formal apparatus.

## Concepts informed

- Decidability boundary for compatibility checking
- Async vs sync compatibility distinctions
- State explosion when verifying multi-party protocols mechanically

## Used by pane

- Background reading for the session-type consultant when
  asked whether a proposed protocol is decidably compatible
