---
type: agent
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: normal
keywords: [session-type-consultant, agent_hub, institutional_knowledge, eact, dlfactris]
related: [policy/agent_workflow, MEMORY, reference/papers/eact, reference/papers/dlfactris]
agents: [session-type-consultant]
---

# session-type-consultant

The home for this agent's institutional knowledge in the new
serena layout. Per `policy/memory_discipline`, this folder holds content
that's only useful to this one agent — theorem citations,
soundness arguments I've reused, affine-gap analyses.

## Reading order for new sessions

1. `MEMORY` — the project index
2. `status` — current state
3. `policy/agent_workflow` — the four-design-agent process
4. `reference/papers/eact` + `reference/papers/eact_sections` — primary EAct reference and the deep theorem locator

## Where you read

- `reference/papers/eact`, `eact_sections`, `dlfactris`, `forwarders`, `dependent_session_types`, `multiparty_automata`, `projections_mpst`, `async_global_protocols`, `interactive_complexity`, `refinement_session_types`
- `decision/server_actor_model`, `decision/messenger_addressing`, `decision/wire_framing`, `decision/clipboard_and_undo`
- `policy/ghost_state_discipline`, `policy/feedback_per_pane_threading` (the I2 / backpressure correction)
- `architecture/looper` — invariant table
- Phase 6 hub-and-spoked the eact and session_types clusters (currently at `analysis/eact/*` and `analysis/session_types/*`)

## Where you write

- **Session-type theoretical results** → extend `reference/papers/<paper>` or write a new anchor
- **Protocol soundness verdicts** → `decision/<topic>` if they shape pane's design
- **Session-type analyses** → `analysis/<cluster>/<spoke>`
- **Your own institutional knowledge** (theorem applications, soundness arguments, recurring questions about C1–C6) → `agent/session-type-consultant/<topic>`
- **Read everywhere; write only to your own `agent/` folder** for
  agent-private content.

## Currently in this folder

Migrated 2026-04-11 from the retired
`.claude/agent-memory/session-type-consultant/` layer. ~30
content files, the largest per-agent corpus. Key files:
`project_handler_*_debate`, `project_eact_*_analysis`,
`project_phase3_channel_topology`,
`project_distributed_protocol_review`,
`project_architecture_spec_review`,
`project_protocol_audit_2026_03_31`.

Institutional highlight: `feedback_mailbox_type_retraction`
(pre-existing spoke) — the session-type consultant initially
recommended mailbox types for the coprocess channel, then
retracted after Lane pointed out per-tag binary sessions are
the right model.

The `reference_eact_paper` content is at
`reference/papers/eact_sections` (Phase 3d); the copy in
this folder is provenance only.
