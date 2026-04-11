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
serena layout. Per `policy/memory_discipline`, this folder holds content
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

Migrated 2026-04-11 from the retired
`.claude/agent-memory/be-systems-engineer/` layer. ~23 content
files: `project_*` analyses (architecture drafts, kit design,
session/codebase assessments, eact framework assessment) and
`reference_*` citation maps (haiku book, haiku source, be naming
philosophy, scripting protocol, decorator architecture). Files
retain their `project_*` / `reference_*` prefixes as provenance;
frontmatter is minimal on most — add it as you re-read or
re-write.
