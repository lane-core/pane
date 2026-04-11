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
serena layout. Per `~/memx-serena.md`, this folder holds content
that's only useful to this one agent — recurring verification
patterns, gotchas you've found, doc drift report templates.

## Reading order for new sessions

1. `MEMORY` — the project index
2. `status` — current state, including the 19/19 invariant tally
3. `policy/agent_workflow` — Step 4 defines your responsibilities (writes tests, escalates design gaps, doc drift report)
4. `architecture/looper` — invariant table for the looper subsystem

## Where you read

- `reference/papers/eact`, `eact_sections` (the theorem locator), `dlfactris`
- `decision/wire_framing` (I11 / I12), `decision/server_actor_model` (single-threaded actor invariants)
- `policy/feedback_stress_test_freshness` — re-run stress tests after wire / codec changes; check assertions match
- `policy/refactor_review_policy` — code review + stale doc audit cycle
- Phase 6 will hub-and-spoke the eact cluster (currently at `pane/eact_invariant_verification`, `pane/eact_divergence_audit`, `pane/test_coverage_audit`, `pane/spec_fidelity_audit`)

## Where you write

- **Invariant findings** → extend `pane/eact_invariant_verification` (Phase 6 → `analysis/eact/invariants`) or write to `analysis/<topic>`
- **Test coverage gaps** → `analysis/<topic>` (and write the tests)
- **Doc drift reports** — these are session-scoped artifacts; **print them rather than persisting** unless they capture a recurring pattern. The drift report is the input to Step 5 (memory + doc freshness), not a long-lived memory.
- **Your own institutional knowledge** (recurring verification gotchas, common false-positive failure modes) → `agent/formal-verifier/<topic>`
- **Read everywhere; write only to your own `agent/` folder** for
  agent-private content.

## Currently in this folder

Fresh shell. Per-agent institutional knowledge under
`.claude/agent-memory/formal-verifier/` (3 files:
`project_spec_fidelity_audit_2026_04_05`,
`project_test_audit_2026_04_05`, `MEMORY.md`) will migrate here
in Phase 7. The two dated audits should be triaged: if their
findings landed, archive; if outstanding, fold into the
analysis/eact cluster.
