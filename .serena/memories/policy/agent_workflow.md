---
type: policy
status: current
supersedes: [pane/agent_workflow, auto-memory/feedback_agent_workflow]
created: 2026-04-05
last_updated: 2026-04-10
importance: high
keywords: [agent_workflow, four_design_agents, plan9, be, optics, session-type, pane-architect, formal-verifier]
agents: [all]
---

# Agent Workflow for Pane Development

Standard process for new subsystems and significant changes. For
small fixes, the task completion checklist is sufficient.

## Step 1: Four design agents in parallel

Launch ALL FOUR for every new design question:

- **plan9-systems-engineer** — Plan 9 namespace model, 9P protocol,
  /proc and rio precedent, distributed state
- **be-systems-engineer** — BeOS/Haiku API design, BLooper /
  BHandler / BMessage, hey scripting, app_server
- **optics-theorist** — profunctor optics theory, lens / prism /
  traversal design, law verification, Clarke et al.
- **session-type-consultant** — session type safety, linear / affine
  types, protocol soundness, invariant analysis

**Why all four:** Each brings a different lens. Plan 9 provides the
namespace / protocol model. Be provides the empirical API design
experience. Optics provides the state projection formalism. Session
types provide the safety analysis. Skipping one leaves a blind spot.

## Step 2: Synthesize and present to Lane

Distill agent findings into a unified summary with open questions.
Lane refines, resolves conflicts, makes decisions.

### Follow-up rounds (as needed)

If open questions remain after Lane's refinement, launch another
consultation round. Follow-up rounds may be **targeted** — not every
question needs all four agents:

- Structural / type design questions → all four
- Domain-specific questions → the 1–2 agents with relevant expertise

Iterate until the design converges. Three rounds was typical for
PeerAuth (initial → accessor design → canonicalization). One round
is fine for simpler features.

### Design agents should note Rust-specific implications

When a design recommendation includes Rust attributes
(`#[non_exhaustive]`, `#[must_use]`, etc.) or patterns with
ergonomic consequences, agents should state the practical API
impact. The pane-architect will catch these regardless, but
surfacing them during design avoids surprises.

## Step 3: pane-architect implements

Writes Rust code faithful to project foundations. **One task per
dispatch, review between — not bulk.** Runs the task completion
checklist.

**Why not general-purpose:** pane-architect reads the foundations
docs, checks naming conventions, and writes code faithful to the
project's theoretical grounding. A general-purpose agent doesn't
have this context.

## Step 4: formal-verifier validates and writes tests

Audits implementation against architecture spec invariants
(I1–I13, S1–S6). **Writes tests for every gap found** — the
verifier is the subject matter expert on what invariants need
testing.

If a test cannot be written because the protocol or design is
incomplete, **escalate the design question to Lane** before
proceeding. Do not skip the test, write a workaround, or defer
it to a report. The inability to test an invariant is a design
signal, not a test infrastructure problem.

**Must include a doc drift report:** grep the old type / API
syntax across `docs/` and serena memories, report every hit with
file:line. This turns manual scavenger hunts into checklists. The
formal-verifier flagging "docs are stale" without enumerating
locations is insufficient — Step 5 needs a concrete list to work
from.

## Step 5: Memory and doc freshness

After validation passes:

- Fix every doc drift location from the formal-verifier's report.
- Update `status` if crate structure, test count, or phase status
  changed.
- Update `PLAN.md` — mark completed items, add discovered work.
- Commit results per CLAUDE.md commit format.

## Rules

- Don't skip any of the four design agents in the initial round
- Don't substitute general-purpose agents for any named agent
- Follow-up rounds can use a targeted subset of agents
- Design agents run in parallel; steps 3–5 run sequentially
- Step 5 is not optional — stale memory is a bug
- pane-architect: one task per dispatch, review between dispatches
- formal-verifier: writes tests, escalates design gaps — does not
  defer them

## Provenance

Workflow established 2026-04-05 after agents were bypassed in
earlier sessions. Refined over sessions 2 and 3 with Step 5 (memory
freshness) added explicitly.
