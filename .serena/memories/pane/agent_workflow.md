# Standard Development Workflow

Per-feature process for new subsystems and significant changes. For small fixes, the task completion checklist is sufficient.

## Steps 1-2: Design consultation loop (iterate until converged)

### Initial round: all four agents in parallel

- **plan9-systems-engineer** — namespace model, 9P, distributed state
- **be-systems-engineer** — BeOS/Haiku API design, app_server, BLooper/BHandler
- **optics-theorist** — lens/prism/traversal design, law verification
- **session-type-consultant** — protocol soundness, linear/affine gap, invariants

Why all four: each brings a different perspective. Skipping one leaves a blind spot.

### Synthesize and present to Lane

Distill agent findings into a unified summary with open questions. Lane refines, resolves conflicts, makes decisions.

### Follow-up rounds (as needed)

If open questions remain after Lane's refinement, launch another consultation round. Follow-up rounds may be **targeted** — not every question needs all four agents. Use judgment:
- Structural/type design questions → all four (they see different facets)
- Domain-specific questions → the 1-2 agents with relevant expertise

Iterate until the design converges. Three rounds was typical for PeerAuth (initial analysis → accessor design → canonicalization). One round is fine for simpler features.

### Design agents should note Rust-specific implications

When a design recommendation includes Rust attributes (`#[non_exhaustive]`, `#[must_use]`, etc.) or patterns with ergonomic consequences, agents should state the practical API impact. Example: `#[non_exhaustive]` on a struct prevents struct literal construction from external crates, requiring a `::new()` constructor. The pane-architect will catch these regardless, but surfacing them during design avoids surprises.

## Step 3: pane-architect implements

Writes Rust code faithful to project foundations. One task per dispatch, review between — not bulk. Runs task completion checklist.

## Step 4: formal-verifier validates and writes tests

Audits implementation against architecture spec invariants (I1-I13, S1-S6). **Writes tests for every gap found** — the verifier is the subject matter expert on what invariants need testing.

If a test cannot be written because the protocol or design is incomplete, **escalate the design question to Lane** before proceeding. Do not skip the test, write a workaround, or defer it to a report. The inability to test an invariant is a design signal, not a test infrastructure problem.

**Must include a doc drift report:** grep the old type/API syntax across `docs/` and `.serena/memories/`, report every hit with file:line. This turns manual scavenger hunts into checklists. The formal-verifier flagging "docs are stale" without enumerating locations is insufficient — Step 5 needs a concrete list to work from.

## Step 5: Memory and doc freshness

After validation passes:
- Fix every doc drift location from the formal-verifier's report.
- Update serena `pane/current_state` if crate structure, test count, or phase status changed.
- Update PLAN.md — mark completed items, add discovered work.
- Commit results per CLAUDE.md commit format.

## Rules

- Don't skip any of the four design agents in the initial round
- Don't substitute general-purpose agents for any named agent
- Follow-up rounds can use a targeted subset of agents
- Design agents run in parallel; steps 3-5 run sequentially
- Step 5 is not optional — stale memory is a bug
- pane-architect: one task per dispatch, review between dispatches
- formal-verifier: writes tests, escalates design gaps — does not defer them
