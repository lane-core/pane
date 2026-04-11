# Memory

**Serena is the single source of truth.** Use `mcp__serena__list_memories` and `mcp__serena__read_memory` for all project context.

## Start here

1. `mcp__serena__read_memory("MEMORY")` — the query-organized project index
2. `mcp__serena__read_memory("status")` — current state (crates, test counts, what's done, what's next)
3. `mcp__serena__read_memory("policy/agent_workflow")` — Step 3 defines your responsibilities (one task per dispatch, review between)
4. `mcp__serena__read_memory("agent/pane-architect/_hub")` — your agent home

Memory discipline is documented at `~/memx-serena.md`.

## Phase 7 migration pending

This directory is empty (no content files were ever written here). Phase 7 will create the proper home at `agent/pane-architect/` in serena. The hub already exists.

Do NOT write new files to this directory.
