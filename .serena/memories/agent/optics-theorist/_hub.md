---
type: agent
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: normal
keywords: [optics-theorist, agent_hub, institutional_knowledge, profunctor_optics]
related: [policy/agent_workflow, MEMORY, reference/papers/profunctor_optics, reference/papers/dont_fear_optics]
agents: [optics-theorist]
---

# optics-theorist

The home for this agent's institutional knowledge in the new
serena layout. Per `policy/memory_discipline`, this folder holds content
that's only useful to this one agent — paper citations I've
verified, optic-shape derivations, profunctor encoding tradeoffs.

## Reading order for new sessions

1. `MEMORY` — the project index
2. `status` — current state
3. `policy/agent_workflow` — the four-design-agent process
4. `reference/papers/dont_fear_optics` and `reference/papers/profunctor_optics` — your primary references
5. `reference/fp_library` — the Rust crate's actual API + Send analysis

## Where you read

- `reference/papers/dont_fear_optics`, `reference/papers/profunctor_optics`, `reference/papers/fcmonads`
- `reference/fp_library` — and the concrete-vs-fp-library tension flagged for Phase 6 resolution
- `decision/observer_pattern`, `decision/panefs_query_unification`
- `policy/ghost_state_discipline`
- Phase 6 hub-and-spoked the optics analysis cluster (currently at `analysis/optics/implementation_guidance`, `analysis/optics/scope_boundaries`, `analysis/optics/panefs_taxonomy`, `agent/optics-theorist/linearity_gap`)

## Where you write

- **Optics theoretical results** → extend `reference/papers/<paper>` if it's a paper finding
- **Optics design decisions** → `decision/<topic>`
- **Optics analyses** → `analysis/optics/<spoke>`
- **Your own institutional knowledge** (paper section locators, derivations you've worked through, MonadicLens edge cases) → `agent/optics-theorist/<topic>`
- **Tier-2 audits** — per `policy/agent_workflow`
  §"Tier-2 audit for theoretical anchors", you audit anchors
  on duploids / VDCs / composition laws / decision procedure
  §8.5 / oblique maps / polarity / profunctor optics /
  `MonadicLens` / accessors / traversal laws. Check
  §pointers, epistemic strength per `policy/memory_discipline`
  §10, and pane-concrete type claims.
- **Read everywhere; write only to your own `agent/` folder** for
  agent-private content.

## Currently in this folder

Migrated 2026-04-11 from the retired
`.claude/agent-memory/optics-theorist/` layer. 10 content
files plus `linearity_gap` (Phase 6 spoke previously under
`agent/optics-theorist/linearity_gap`). Notable spokes:
`project_concrete_encoding_validation`,
`project_optics_layer_design`, `project_ctl_optic_boundary`,
`reference_fp_library_optics`, `reference_profunctor_theory`.

## Open issue

The concrete-encoding-vs-fp-library tension: the
optics-design-brief explicitly chose concrete encoding, but
`property.rs` uses fp-library's profunctor encoding. Flagged in
`reference/fp_library` for Phase 6 resolution under
`analysis/optics/`. **This needs your judgment when the analysis
hub is built.**
