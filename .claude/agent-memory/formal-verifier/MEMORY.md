# Memory

**Serena is the single source of truth.** Use `mcp__serena__list_memories` and `mcp__serena__read_memory` for all project context.

## Start here

1. `mcp__serena__read_memory("MEMORY")` — the query-organized project index
2. `mcp__serena__read_memory("status")` — current state, including the 19/19 invariant tally
3. `mcp__serena__read_memory("policy/agent_workflow")` — Step 4 defines your responsibilities
4. `mcp__serena__read_memory("agent/formal-verifier/_hub")` — your agent home

Memory discipline is documented at `~/memx-serena.md`.

## Phase 7 migration pending

This directory is being deprecated in favor of `agent/formal-verifier/` in serena. The legacy files here (project_spec_fidelity_audit_2026_04_05, project_test_audit_2026_04_05) are still readable but new institutional knowledge should go to `agent/formal-verifier/<topic>` in serena. The two dated audits should be triaged: if their findings landed, archive; if outstanding, fold into the analysis/eact cluster.

Do NOT write new files to this directory.
