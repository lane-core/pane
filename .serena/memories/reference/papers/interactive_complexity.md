---
type: reference
status: current
citation_key: DHMV
aliases: [IC]
created: 2026-04-10
last_updated: 2026-04-10
importance: low
keywords: [computational_complexity, interactive_behaviors, session_types]
related: [reference/papers/_hub]
agents: [session-type-consultant]
---

# Computational complexity of interactive behaviors

**Path:** `~/gist/computational-complexity-of-interactive-behaviors.gist.txt`

## Summary

Classifies the computational complexity of session-typed
interactive behaviors. Some compositions are PTIME-checkable;
others are EXPTIME or undecidable. Useful for predicting how
expensive a static analysis will be on a given protocol.

## Concepts informed

- Decidability boundary for static checks on session types
- When a protocol design crosses into undecidable territory
- Practical complexity of multi-party type checking

## Used by pane

- Background for keeping pane's protocols in the
  PTIME-checkable subset (not yet a binding constraint, but
  worth knowing)
