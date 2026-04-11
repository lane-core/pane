---
type: agent
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: normal
keywords: [formal-verifier, agent_hub, institutional_knowledge, invariants, eact]
related: [policy/agent_workflow, MEMORY, architecture/looper, reference/papers/eact]
agents: [formal-verifier]
---

# formal-verifier

The home for this agent's institutional knowledge in the new
serena layout. Per `policy/memory_discipline`, this folder holds content
that's only useful to this one agent — recurring verification
patterns, gotchas you've found, doc drift report templates.

## Reading order for new sessions

1. `MEMORY` — the project index
2. `status` — current state, including the 19/19 invariant tally
3. `policy/agent_workflow` — Step 4 defines your responsibilities (writes tests, escalates design gaps, doc drift report)
4. `architecture/looper` — invariant table for the looper subsystem

## Where you read

- `reference/papers/eact`, `eact_sections` (the theorem locator), `dlfactris` (Jacobs/Hinrichsen/Krebbers POPL 2024, LinearActris — at `~/gist/2024-popl-dlfactris.pdf`)
- `decision/wire_framing` (I11 / I12), `decision/server_actor_model` (single-threaded actor invariants)
- `policy/feedback_stress_test_freshness` — re-run stress tests after wire / codec changes; check assertions match
- `policy/refactor_review_policy` — code review + stale doc audit cycle
- Phase 6 hub-and-spoked the eact cluster (currently at `analysis/eact/invariants`, `analysis/eact/divergences_2026_04_06`, `analysis/verification/test_coverage`, `analysis/verification/spec_fidelity`)

## Where you write

- **Invariant findings** → extend `analysis/eact/invariants` or write to `analysis/<topic>`
- **Test coverage gaps** → `analysis/<topic>` (and write the tests)
- **Doc drift reports** — these are session-scoped artifacts; **print them rather than persisting** unless they capture a recurring pattern. The drift report is the input to Step 5 (memory + doc freshness), not a long-lived memory.
- **Your own institutional knowledge** (recurring verification gotchas, common false-positive failure modes) → `agent/formal-verifier/<topic>`
- **Tier-2 audit runs** — per `policy/agent_workflow`
  §"Tier-2 audit for theoretical anchors" (ported 2026-04-11
  from psh), you own the meta-audit for new
  `analysis/<concept>.md` anchors that cite external papers
  or vendored references. Hub-structure check, pointer graph,
  cross-cluster consistency, frontmatter compliance. Report
  verdicts per the procedure; other domain agents handle
  content audits within their scope.
- **Read everywhere; write only to your own `agent/` folder** for
  agent-private content.

## Currently in this folder

Migrated 2026-04-11 from the retired
`.claude/agent-memory/formal-verifier/` layer. Two spokes:
`project_spec_fidelity_audit_2026_04_05` and
`project_test_audit_2026_04_05`. Note: both audits are dated
2026-04-05; the current verification record at
`analysis/verification/session_audit_2026_04_06` supersedes
their invariant status tables. Consult for historical
context, not current state.
