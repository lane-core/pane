# Memory

**Serena is the single source of truth.** Use `mcp__serena__list_memories` and `mcp__serena__read_memory` for all project context.

## Start here

1. `mcp__serena__read_memory("MEMORY")` — the query-organized project index
2. `mcp__serena__read_memory("status")` — current state
3. `mcp__serena__read_memory("policy/agent_workflow")` — the four-design-agent process
4. `mcp__serena__read_memory("agent/session-type-consultant/_hub")` — your agent home

Memory discipline is documented at `~/memx-serena.md`.

## Phase 7 migration pending

This directory is being deprecated in favor of `agent/session-type-consultant/` in serena. The legacy files here (32 content files, the largest per-agent corpus) are still readable but new institutional knowledge should go to `agent/session-type-consultant/<topic>` in serena.

Do NOT write new files to this directory.

## Local feedback (corrections to my analysis) — pending Phase 7 migration

- [Mailbox type retraction](feedback_mailbox_type_retraction.md) — per-tag binary sessions, not mailbox types, for tag-multiplexed protocols
