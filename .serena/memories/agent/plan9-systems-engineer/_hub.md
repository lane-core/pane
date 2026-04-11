---
type: agent
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: normal
keywords: [plan9-systems-engineer, agent_hub, institutional_knowledge]
related: [policy/agent_workflow, MEMORY, reference/plan9/_hub]
agents: [plan9-systems-engineer]
---

# plan9-systems-engineer

The home for this agent's institutional knowledge in the new
serena layout. Per `policy/memory_discipline`, this folder holds content
that's only useful to this one agent — recurring questions,
specific reference passages I've cited, corrections I've made,
and reading orders I've found useful.

## Reading order for new sessions

1. `MEMORY` — the project index
2. `status` — current state
3. `policy/agent_workflow` — the four-design-agent process
4. `reference/plan9/_hub` — your domain hub

## Where you read

- `reference/plan9/*` — all spokes (foundational, voice, papers_insights, man_pages_insights, distribution_model, divergences, decisions)
- `decision/host_as_contingent_server`, `decision/headless_strategic_priority`, `decision/server_actor_model`, `decision/panefs_query_unification`, `decision/wire_framing`
- `architecture/looper` — the calloop event loop with EAct-derived invariants
- `policy/heritage_annotations` — how to cite Plan 9 in Rust doc comments

Phase 6 hub-and-spoked the analysis cluster currently at
`analysis/eact/*`, `analysis/duploid/*`, `analysis/optics/*`,
`analysis/session_types/*`. All migrated 2026-04-11.

## Where you write

- **Plan 9 reference findings** → extend `reference/plan9/<spoke>` in place
- **Plan 9 → pane decisions** → `decision/<topic>` (one memory per decision)
- **Plan 9-side analysis** → `analysis/<topic>`
- **Your own institutional knowledge** → `agent/plan9-systems-engineer/<topic>`
- **Read everywhere; write only to your own `agent/` folder** for
  agent-private content. To record cross-agent supersession or
  contradiction, write in your own folder and use `supersedes:` /
  `contradicts:` frontmatter pointing at the other agent's memory.

## Currently in this folder

Migrated 2026-04-11 from the retired
`.claude/agent-memory/plan9-systems-engineer/` layer. ~24
content files: `project_*` analyses (handler architecture,
language split, pane-as-trait debate, three-channel error
model, c1 looper evolution, phase3 channel topology),
`reference_plan9_decisions` (primary-source Plan 9 notes),
and `user_lane_context.md` (background on Lane).
