---
name: Architecture draft key decisions
description: Key architectural decisions made in the Be engineer's architecture draft, including router elimination and session type transport strategy
type: project
---

Architecture draft written 2026-03-20 at openspec/changes/spec-tightening/architecture-draft-be-engineer.md.

Key decisions:
- **Router eliminated.** Routing moved to pane-app kit (client-side, local evaluation). No central router server. Rationale: BMessenger carried messages directly between apps in BeOS — no broker. A central router is a single point of failure that requires complex resilience infrastructure (circuit breakers, priority queues, external watchdog) only because it exists.
- **pane-watchdog replaces router resilience.** Minimal external process (Erlang heart pattern) monitors compositor and roster via direct pipes. Does heartbeats, journal flush, escalation. Nothing else.
- **Session type transport: Phase 1 approach.** Par for specification and testing, hand-written state machine for socket transport. Types keep them in sync. pragmatic and loses the least.
- **s6 is the concrete init choice** (not an abstraction over multiple init systems). s6-linux-init as PID 1, s6-svscan for supervision, s6-rc for compiled dependency management. s6-fdholder for pre-registered socket endpoints.
- **Input Kit as generalized grammar.** Vim's compositional structure (N operators × M objects) generalized beyond text. Insert-as-default for accessibility, Normal mode available via configurable key.

**Why:** Foundations spec is canonical. Every decision traces to a principle there. The architecture carries the engineering, not the philosophy.

**How to apply:** This draft is a proposal, not final. Open questions section identifies 12 items that need prototyping before commitment.
