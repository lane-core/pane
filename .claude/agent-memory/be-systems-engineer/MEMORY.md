# Memory

**Serena is the single source of truth.** Use `mcp__serena__list_memories` and `mcp__serena__read_memory` for all project context.

## Start here

1. `mcp__serena__read_memory("MEMORY")` — the query-organized project index
2. `mcp__serena__read_memory("status")` — current state
3. `mcp__serena__read_memory("policy/agent_workflow")` — the four-design-agent process
4. `mcp__serena__read_memory("agent/be-systems-engineer/_hub")` — your agent home

Memory discipline is documented at `~/memx-serena.md`.

## Phase 7 migration pending

This directory is being deprecated in favor of `agent/be-systems-engineer/` in serena. The legacy files here (23 content files: project_*, reference_*) are still readable but new institutional knowledge should go to `agent/be-systems-engineer/<topic>` in serena.

Do NOT write new files to this directory.
