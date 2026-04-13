File write denied. Compressing directly:

---

---
type: policy
status: current
created: 2026-04-11
last_updated: 2026-04-12
importance: high
keywords: [agents, read-write, shared, private, cross-agent, namespace]
extends: policy/memory/_hub
agents: [all]
---

# Agents and the project store

Agents share knowledge through store, not parallel layer.
Session specialist writes protocol analysis → implementation
specialist reads. Verifier finds gap → design agents see next
consultation. Without consolidation, knowledge fragments by
tool, not topic.

## Read-everywhere, write-only-to-own-folder

Each agent has `agent/<agent-name>/`. Discipline:

- **Any agent reads any memory**, including other agents' folders.
  Cross-agent visibility is access pattern consolidated store
  exists for.
- **Agent only writes to own folder under `agent/`**, plus
  project-level types (`policy/`, `decision/`, `architecture/`,
  `analysis/`, `reference/`) when content is multi-agent. Never
  write to another agent's folder.

Mirrors Plan 9 per-user namespace: `/usr/$user/` is yours,
`/usr/$other/` readable but not yours. Agent A records "B's
analysis superseded by my finding Y" writing in own folder
with `supersedes:` / `contradicts:` in frontmatter. Supersession
visible both directions because frontmatter searchable.

## Shared-vs-agent-private rule

Memory lives in `agent/<n>/` only if **only useful to that agent**.
Agent-private examples:
- Reference passages bearing on recurring questions
- Cross-references between primary sources + project decisions
- Recurring misconceptions corrected within agent's scope
- Agent's "reading order for new sessions"

Multiple agents consult → project level instead:
- Process rules (`policy/`)
- Design decisions from multi-agent consultation (`decision/`)
- Subsystem docs informing multiple agents (`architecture/`)
- Audit results any agent might cite (`analysis/`)

Rule: project-level = retrieval crossing agents; agent-level =
retrieval that doesn't.

## When reading another agent's memory

Treat as **input to own analysis**, not authoritative for own scope.
Session specialist reads optics-theorist's note on monadic
lenses → context. Protocol analysis produced is its own,
written into own folder, optics note cited via `extends:` or
`related:`. Agents don't merge; they cite.

---

Cuts were light — text was already dense. Main removals: articles, "specific", "personal", filler verbs ("would consult" → "consult"), redundant connectives.