---
type: policy
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [refactoring, extraction, intermediate_state, ordering, incremental, migration]
agents: [all]
---

# Intermediate State Principle

**Incremental refactoring is safer when the intermediate state is
a natural resting point. If the intermediate state is something
nobody would ever choose to be in permanently, it's a liability,
not a safety net.**

This is the operational test for Lane's refactoring principle
(global CLAUDE.md): "find the solution requiring the least
investment in infrastructure that is ultimately orthogonal to the
functionality it temporarily supports."

## The litmus test

Before splitting an extraction into two steps, ask: **would anyone
design the intermediate state on purpose?** If the answer is no,
combine the steps. A smaller diff is not inherently safer — an
incoherent intermediate state is riskier than a larger atomic move
to a coherent endpoint.

## When incremental IS safer

Incremental extraction wins when each intermediate state is a
plausible resting place — a configuration someone might
intentionally ship. Example: extracting a scripting protocol
incrementally, where each step produces a working subset
(property_info alone is useful, then specifier resolution, then
full integration).

## When incremental is a trap

Incremental extraction fails when the intermediate state creates a
cross-boundary coherence obligation that exists only transiently.
Example: extracting a credit counter to crate B while the tokens
it counts remain in crate A. The intermediate requires both crates
to agree on the count — an invariant that neither the before-state
nor the after-state requires, and that exists only because of the
extraction ordering.

## Provenance

Derived 2026-04-12 from be-systems-engineer's analysis of the
RequestCorrelator + FlowControl extraction ordering. The Be agent
drew on Haiku's BLooper refactoring history, where incremental
internal refactors that passed through incoherent intermediate
states were consistently more error-prone than single-step
restructurings to coherent endpoints. Endorsed unanimously by
session-type-consultant, plan9-systems-engineer, and
optics-theorist during the MPST extraction planning roundtable.

Cross-reference: `decision/pane_session_mpst_foundation` (the
extraction this principle was derived from),
`policy/agent_workflow` (where extraction ordering decisions are
made).
