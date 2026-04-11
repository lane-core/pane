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
serena layout. Per `~/memx-serena.md`, this folder holds content
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
- Phase 6 will hub-and-spoke the optics analysis cluster (currently at `pane/optics_implementation_guidance`, `pane/optics_scope_deliberation`, `pane/panefs_optic_taxonomy`, `pane/linearity_gap_analysis`)

## Where you write

- **Optics theoretical results** → extend `reference/papers/<paper>` if it's a paper finding
- **Optics design decisions** → `decision/<topic>`
- **Optics analyses** → `analysis/optics/<spoke>` (Phase 6 will introduce the hub)
- **Your own institutional knowledge** (paper section locators, derivations you've worked through, MonadicLens edge cases) → `agent/optics-theorist/<topic>`
- **Read everywhere; write only to your own `agent/` folder** for
  agent-private content.

## Currently in this folder

Fresh shell. Per-agent institutional knowledge under
`.claude/agent-memory/optics-theorist/` (10 content files including
project_concrete_encoding_validation, project_optics_layer_design,
reference_fp_library_optics, reference_profunctor_theory) will
migrate here in Phase 7.

## Open issue

The concrete-encoding-vs-fp-library tension: the
optics-design-brief explicitly chose concrete encoding, but
`property.rs` uses fp-library's profunctor encoding. Flagged in
`reference/fp_library` for Phase 6 resolution under
`analysis/optics/`. **This needs your judgment when the analysis
hub is built.**
