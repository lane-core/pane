---
type: agent
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: normal
keywords: [be-systems-engineer, agent_hub, institutional_knowledge]
related: [policy/agent_workflow, MEMORY, reference/haiku/_hub]
agents: [be-systems-engineer]
---

# be-systems-engineer

The home for this agent's institutional knowledge in the new
serena layout. Per `~/memx-serena.md`, this folder holds content
that's only useful to this one agent — recurring questions,
specific Haiku source citations I've verified, corrections I've
made, and reading orders I've found useful.

## Reading order for new sessions

1. `MEMORY` — the project index
2. `status` — current state
3. `policy/agent_workflow` — the four-design-agent process
4. `reference/haiku/_hub` — your domain hub

## Where you read

- `reference/haiku/*` — all spokes (book, source, internals, scripting_protocol, appserver_concurrency, decorator_architecture, naming_philosophy, haiku_rs, beapi_divergences)
- `policy/beapi_naming_policy`, `policy/beapi_translation_rules`, `policy/heritage_annotations`, `policy/technical_writing`
- `decision/observer_pattern`, `decision/clipboard_and_undo`, `decision/server_actor_model`, `decision/messenger_addressing`
- `architecture/looper` — BLooper provenance and the I-table
- The local Haiku Book at `reference/haiku-book/` and Haiku source at `~/src/haiku/`

## Where you write

- **Haiku / BeOS source findings** → extend `reference/haiku/<spoke>` in place
- **New Be → pane translations** → update `reference/haiku/beapi_divergences` (the tracker)
- **Be-derived design decisions** → `decision/<topic>` (one memory per decision)
- **Your own institutional knowledge** (recurring questions, source citations you've verified, Be Newsletter issues you've quoted) → `agent/be-systems-engineer/<topic>`
- **Read everywhere; write only to your own `agent/` folder** for
  agent-private content.

## Currently in this folder

Fresh shell created during the Phase 1–5 memory restructure.
Per-agent institutional knowledge previously accumulated under
`.claude/agent-memory/be-systems-engineer/` will migrate here in
Phase 7. The legacy directory has 23 content files (project_*
analyses, reference_* citation maps, etc.) worth consulting
until Phase 7 completes.
