---
type: policy
status: current
supersedes: [pane/ghost_state_discipline, auto-memory/feedback_ghost_state_discipline]
created: 2026-03-30
last_updated: 2026-04-10
importance: normal
keywords: [ghost_state, correlation_id, typestate, ownership, dependent_session_types]
agents: [pane-architect, session-type-consultant, optics-theorist]
---

# Ghost State Discipline

Each time a correlation ID appears at the API surface, ask whether
a typestate handle could replace it. When it can, compile-time
protocol enforcement replaces runtime token-matching. When it
can't (async gap ownership can't bridge), keep the token but
recognize it as ghost state — correctness depends on matching
logic, not types.

**Why:** Derived from synthesizing a dependent linear session type
theory paper against pane's existing messaging model. The paper
formalizes what pane already does well in places (ReplyPort,
TimerToken) and reveals where it doesn't yet (CompletionRequest
token correlation).

**How to apply:** Use as a design frame for new subsystems
(clipboard, DnD, observers). Define the protocol, derive the
handles, push ghost state below the API surface wherever ownership
can carry the weight. When reviewing API designs, flag exposed
correlation IDs as candidates for ownership promotion.
