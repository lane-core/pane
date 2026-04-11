---
type: index
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: high
---

# Pane Memory Index

Query-organized entry point. Read this first; pick the section
that matches your task.

## Start here (every session)

- [`status`](status.md) — current state, what's done, what's next

## When designing a feature

- [`policy/agent_workflow`](policy/agent_workflow.md) — four-design-agent process, pane-architect, formal-verifier, memory freshness
- [`policy/design_decision_escalation`](policy/design_decision_escalation.md) — when to ask Lane

## When you need a process rule

- [`policy/block_escalation_policy`](policy/block_escalation_policy.md) — escalate blocks immediately
- [`policy/refactor_review_policy`](policy/refactor_review_policy.md) — review + stale doc audit after refactors
- [`policy/no_stability_commitment`](policy/no_stability_commitment.md) — no users, no deprecations
- [`policy/ghost_state_discipline`](policy/ghost_state_discipline.md) — typestate over correlation IDs
- [`policy/non_exhaustive_extensions`](policy/non_exhaustive_extensions.md) — planned extensions for `#[non_exhaustive]` types
- [`policy/beapi_naming_policy`](policy/beapi_naming_policy.md) — three-tier Be naming
- [`policy/agent_naming`](policy/agent_naming.md) — generic human names in examples
- [`policy/heritage_annotations`](policy/heritage_annotations.md) — Be / Plan 9 citation format
- [`policy/style_and_conventions`](policy/style_and_conventions.md) — pointer to STYLEGUIDE.md
- [`policy/technical_writing`](policy/technical_writing.md) — Plan 9 voice
- [`policy/feedback_accept_as_is`](policy/feedback_accept_as_is.md) — roundtable verdicts need Lane
- [`policy/feedback_per_pane_threading`](policy/feedback_per_pane_threading.md) — intra-pane blocking is backpressure
- [`policy/feedback_stress_test_freshness`](policy/feedback_stress_test_freshness.md) — re-run after wire/codec changes
- [`policy/feedback_synthesis_abstraction`](policy/feedback_synthesis_abstraction.md) — synthesize ideas, not feature mappings
- [`policy/feedback_workflow_prominence`](policy/feedback_workflow_prominence.md) — project workflow > generic skills
- [`policy/feedback_relay_mail`](policy/feedback_relay_mail.md) — handoff memo workflow
- [`policy/feedback_tee_build_output`](policy/feedback_tee_build_output.md) — tee long-running output to /tmp
- [`policy/feedback_no_python_extraction`](policy/feedback_no_python_extraction.md) — agents have Write permission

## When working on a subsystem

- [`architecture/looper`](architecture/looper.md) — calloop event loop, six-phase batch ordering, watchdog, send_and_wait/I8
- [`architecture/rustix_migration`](architecture/rustix_migration.md) — pane-session FFI → rustix migration plan

## Migration in progress

The following query categories don't have fully migrated content
yet. Fall back to `list_memories()` and read from the old paths:

- **When you need theoretical grounding** — analysis hubs land in
  Phase 6. Current locations:
  - `pane/duploid_analysis`, `pane/duploid_deep_analysis`
  - `pane/eact_analysis_gaps`, `pane/eact_divergence_audit`,
    `pane/eact_invariant_verification`, `pane/eact_what_not_to_adopt`
  - `pane/polarity_classifications`
  - `pane/optics_implementation_guidance`,
    `pane/optics_scope_deliberation`,
    `pane/panefs_optic_taxonomy`, `pane/linearity_gap_analysis`
  - `pane/session_type_design_principles`,
    `pane/session_optic_boundary_rules`,
    `pane/coprocess_session_type_correction`
  - `pane/shell_sequent_calculus_analysis`,
    `pane/functoriality_principle`
- **When making or recalling a decision** — Phase 5. Current
  locations: `pane/messenger_addressing_decisions`,
  `pane/observer_pattern_decision`, `pane/server_actor_model_decision`,
  `pane/host_as_contingent_server`, `pane/headless_strategic_priority`,
  `pane/panefs_query_unification`, and others under `pane/*`.
- **When citing external knowledge** — Phase 3. Current locations:
  `reference/haiku_book`, `reference/beos_scripting_protocol`,
  `reference/appserver_concurrency_model`,
  `reference/haiku_decorator_architecture`,
  `reference/smithay_assessment`, `plan9/foundational_paper`,
  `plan9/papers_writing_voice`, `plan9/papers_technical_insights`,
  `pane/plan9_reference_insights`, `pane/plan9_distribution_model`,
  `pane/plan9_divergences`, `pane/beapi_internals`,
  `pane/beapi_divergences`, `pane/beapi_translation_rules`.
- **When recalling agent-specific knowledge** — Phase 7. Currently
  in `.claude/agent-memory/<agent>/` (will move to serena
  `agent/<n>/`).

Run `mcp__serena__list_memories()` to see everything currently in
serena.

## How memory works in this project

See `~/memx-serena.md` for the principles. Briefly:

- One memory store per project (this serena project), all agents
  share it
- Read everywhere, write only to your own `agent/<n>/` folder
- Frontmatter has `supersedes:` / `superseded_by:` for write-once
  status discipline
- Hub-and-spokes for clusters of 4+ related memories
- Fact-level granularity beats session-level
- Low-confidence rejection over stretching
