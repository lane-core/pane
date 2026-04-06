# Standard Development Workflow

Per-feature process for new subsystems and significant changes. For small fixes, the task completion checklist is sufficient.

## Step 1: Four design agents analyze (ALL FOUR, in parallel)

- **plan9-systems-engineer** — namespace model, 9P, distributed state
- **be-systems-engineer** — BeOS/Haiku API design, app_server, BLooper/BHandler
- **optics-theorist** — lens/prism/traversal design, law verification
- **session-type-consultant** — protocol soundness, linear/affine gap, invariants

Why all four: each brings a different perspective. Skipping one leaves a blind spot.

## Step 2: Lane refines

Discuss findings, resolve open questions, make decisions.

## Step 3: pane-architect implements

Writes Rust code faithful to project foundations. Runs task completion checklist.

## Step 4: formal-verifier validates

Audits implementation against architecture spec invariants (I1-I13, S1-S6).

## Step 5: Memory and doc freshness

After validation passes:
- Update serena `pane/current_state` if crate structure, test count, or phase status changed.
- Update PLAN.md — mark completed items, add discovered work.
- Check docs that reference changed subsystems for staleness.
- Commit results per CLAUDE.md commit format.

## Rules

- Don't skip any of the four design agents in step 1
- Don't substitute general-purpose agents for any named agent
- Design agents run in parallel; steps 3-5 run sequentially
- Step 5 is not optional — stale memory is a bug
