# Memory

**Serena is the single source of truth.** Use `mcp__serena__list_memories` and `mcp__serena__read_memory` for all project context.

## Start here

1. `mcp__serena__read_memory("MEMORY")` — the query-organized project index
2. `mcp__serena__read_memory("status")` — current state
3. `mcp__serena__read_memory("policy/agent_workflow")` — the four-design-agent process
4. `mcp__serena__read_memory("agent/optics-theorist/_hub")` — your agent home (includes the open issue: concrete encoding vs fp-library tension)

Memory discipline is documented at `~/memx-serena.md`.

## Phase 7 migration pending

This directory is being deprecated in favor of `agent/optics-theorist/` in serena. The legacy files here (10 content files including project_concrete_encoding_validation, project_optics_layer_design, reference_fp_library_optics, reference_profunctor_theory) are still readable but new institutional knowledge should go to `agent/optics-theorist/<topic>` in serena.

Do NOT write new files to this directory.
