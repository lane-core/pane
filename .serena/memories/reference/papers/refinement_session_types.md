---
type: reference
status: current
citation_key: UD
aliases: [RST]
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [refinement_types, session_types, inference, practical]
related: [reference/papers/_hub, reference/papers/dependent_session_types]
agents: [session-type-consultant]
---

# Practical refinement session-type inference

**Path:** `~/gist/practical-refinement-session-type-inference/`

## Summary

Combines refinement types (predicates on values) with session
types (protocols on channels). Inference algorithm makes the
combination usable in practice.

The promise: protocols whose message content is constrained by
predicates (e.g., "the next int must be positive") can be
checked at compile time without full dependent types.

## Concepts informed

- Lighter-weight alternative to full dependent session types
  for value-constrained protocols
- Inference algorithms that don't require type annotations
  everywhere

## Used by pane

- Background reference for proposals to add value-level
  constraints to pane's typed protocols (not yet adopted)
