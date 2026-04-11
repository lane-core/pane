---
type: agent
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: normal
keywords: [pane-architect, agent_hub, institutional_knowledge, rust, implementation]
related: [policy/agent_workflow, MEMORY, status, architecture/looper]
agents: [pane-architect]
---

# pane-architect

The home for this agent's institutional knowledge in the new
serena layout. Per `policy/memory_discipline`, this folder holds content
that's only useful to this one agent — Rust patterns I've
learned, build / test gotchas, crate-specific conventions.

## Reading order for new sessions

1. `MEMORY` — the query-organized project index
2. `status` — crates, test counts, what's done, what's next
3. `policy/agent_workflow` — Step 3 defines your responsibilities (one task per dispatch, review between)
4. The architecture memories for the subsystem you're touching: `architecture/looper`, `architecture/rustix_migration`, etc.

## Where you read

### Rule sets you must follow

- `policy/beapi_naming_policy` — three-tier Be naming
- `policy/beapi_translation_rules` — systematic Be → pane translation
- `policy/heritage_annotations` — Be / Plan 9 citation format
- `policy/technical_writing` — Plan 9 voice for docs
- `policy/no_stability_commitment` — no users, no deprecations, rename freely
- `policy/ghost_state_discipline` — typestate over correlation IDs
- `policy/non_exhaustive_extensions` — `#[non_exhaustive]` audit obligations
- `policy/refactor_review_policy` — code review + stale doc audit cycle
- `policy/block_escalation_policy` — stop work and escalate, never silently work around
- `policy/feedback_per_pane_threading` — intra-pane blocking is backpressure, not a bug
- `policy/feedback_stress_test_freshness` — re-run after wire / codec changes
- `policy/feedback_tee_build_output` — `tee /tmp/pane-build.log | tail -30`

### Domain references

- `reference/haiku/_hub` — Be / Haiku translation source
- `reference/plan9/_hub` — Plan 9 grounding for distribution / namespace work
- `reference/papers/_hub` — theoretical foundations (EAct, DLfActRiS, optics, duploids)
- `reference/fp_library` — Rust optics crate API
- `reference/smithay` — Wayland compositor framework

### Decision context

Read `decision/<topic>` for any subsystem you touch. The decision memories explain WHY pane diverges from precedent.

## Where you write

- **New architectural commitments** → `architecture/<subsystem>`
- **Implementation decisions made during a task** → `decision/<topic>` (one memory per decision)
- **Be → pane translation extensions** → update `reference/haiku/beapi_divergences`
- **Your own institutional knowledge** (Rust patterns, build / test gotchas, crate-specific conventions you've learned) → `agent/pane-architect/<topic>`
- **Read everywhere; write only to your own `agent/` folder** for
  agent-private content.
- **Update `status` after completing any task** that changes crate structure, test counts, or phase status. `policy/agent_workflow` Step 5 — not optional.

## Currently in this folder

Fresh shell. The retired
`.claude/agent-memory/pane-architect/` layer had only
`MEMORY.md` (no content files). This agent starts
post-Phase-7 with a clean slate — the first real write
lands here.

## Build / test commands (per `status`)

```
cargo test --workspace          # 246 regular tests
cargo test -- --ignored         # 28 stress tests
cargo fmt
cargo clippy --workspace        # zero warnings
cargo run -p pane-hello         # canonical app
```
