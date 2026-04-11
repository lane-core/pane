---
type: index
status: current
created: 2026-04-10
last_updated: 2026-04-11
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
- [`policy/beapi_translation_rules`](policy/beapi_translation_rules.md) — systematic Be → pane translation
- [`policy/agent_naming`](policy/agent_naming.md) — generic human names in examples
- [`policy/heritage_annotations`](policy/heritage_annotations.md) — Be / Plan 9 citation format
- [`policy/style_and_conventions`](policy/style_and_conventions.md) — pointer to STYLEGUIDE.md
- [`policy/technical_writing`](policy/technical_writing.md) — Plan 9 voice
- [`policy/headless_development_unblocking`](policy/headless_development_unblocking.md) — develop against pane-headless by default
- [`policy/feedback_accept_as_is`](policy/feedback_accept_as_is.md) — roundtable verdicts need Lane
- [`policy/feedback_per_pane_threading`](policy/feedback_per_pane_threading.md) — intra-pane blocking is backpressure
- [`policy/feedback_stress_test_freshness`](policy/feedback_stress_test_freshness.md) — re-run after wire/codec changes
- [`policy/feedback_synthesis_abstraction`](policy/feedback_synthesis_abstraction.md) — synthesize ideas, not feature mappings
- [`policy/feedback_workflow_prominence`](policy/feedback_workflow_prominence.md) — project workflow > generic skills
- [`policy/feedback_relay_mail`](policy/feedback_relay_mail.md) — handoff memo workflow
- [`policy/feedback_tee_build_output`](policy/feedback_tee_build_output.md) — tee long-running output to /tmp
- [`policy/feedback_no_python_extraction`](policy/feedback_no_python_extraction.md) — agents have Write permission

## When making or recalling a decision

### Foundational commitments

- [`decision/host_as_contingent_server`](decision/host_as_contingent_server.md) — local hardware has no architectural privilege
- [`decision/headless_strategic_priority`](decision/headless_strategic_priority.md) — headless / distributed is the top near-term deliverable

### Subsystem decisions

- [`decision/messenger_addressing`](decision/messenger_addressing.md) — Address, Messenger, ServiceHandle, direct pane-to-pane
- [`decision/server_actor_model`](decision/server_actor_model.md) — ProtocolServer is a single-threaded actor
- [`decision/observer_pattern`](decision/observer_pattern.md) — observable state via filesystem attrs, not messaging
- [`decision/panefs_query_unification`](decision/panefs_query_unification.md) — pane-fs directories ARE queries
- [`decision/wire_framing`](decision/wire_framing.md) — ProtocolAbort framing, reserved discriminant, I11/I12 split
- [`decision/clipboard_and_undo`](decision/clipboard_and_undo.md) — MIME ctl, TTL, undo via ctl, RecordingOptic gaps
- [`decision/system_fonts`](decision/system_fonts.md) — Inter / Gelasio / Monoid as defaults

### Distribution / system layer

- [`decision/s6_init`](decision/s6_init.md) — sixos (s6 + Nix) for the Linux distribution layer
- [`decision/dependency_review`](decision/dependency_review.md) — Landlock, bcachefs, Wayland protocols, FUSE / io_uring

## When working on a subsystem

- [`architecture/looper`](architecture/looper.md) — calloop event loop, six-phase batch ordering, watchdog, send_and_wait/I8
- [`architecture/rustix_migration`](architecture/rustix_migration.md) — pane-session FFI → rustix migration plan

## When citing Haiku / BeOS reference

Start at the hub: [`reference/haiku/_hub`](reference/haiku/_hub.md) — orientation, spoke list, when-to-consult guide.

Spokes: `book`, `source`, `haiku_rs`, `scripting_protocol`, `naming_philosophy`, `appserver_concurrency`, `decorator_architecture`, `internals`, `beapi_divergences`.

## When citing Plan 9 reference

Start at the hub: [`reference/plan9/_hub`](reference/plan9/_hub.md) — orientation, spoke list, when-to-consult guide.

Spokes: `foundational`, `voice`, `papers_insights`, `man_pages_insights`, `distribution_model`, `divergences`, `decisions`.

## When citing a theoretical paper

Start at the hub: [`reference/papers/_hub`](reference/papers/_hub.md) — index of vendored gist papers, organized by topic.

Topics:

- **Session types:** `forwarders`, `multiparty_automata`, `dependent_session_types`, `refinement_session_types`, `projections_mpst`, `async_global_protocols`, `eact` (+ `eact_sections` deep locator), `dlfactris`, `interactive_complexity`
- **Profunctor optics:** `dont_fear_optics`, `profunctor_optics`
- **VDC and duploids:** `duploids`, `fcmonads`, `logical_aspects_vdc`, `linear_logic_no_units`, `squier_hott`
- **Sequent calculus:** `dissection_of_l`, `grokking_sequent_calculus`
- **Knowledge management:** `memx` (the rulebook for serena, ported via `~/memx-serena.md`)
- **Unix history:** `unix_retrospective`

## When citing other external knowledge

- [`reference/smithay`](reference/smithay.md) — smithay v0.7.0 viability assessment for pane-comp
- [`reference/fp_library`](reference/fp_library.md) — fp-library 0.15.0 optics API + Send analysis

## Migration in progress

The following query categories don't have fully migrated content
yet. Fall back to `list_memories()` and read from the old paths:

- **When you need theoretical grounding** — analysis hubs land
  in Phase 6. Current locations:
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
  - `pane/wiring_soundness_analysis`, `pane/spec_fidelity_audit`,
    `pane/test_coverage_audit`, `pane/writer_monad_analysis`,
    `pane/namespace_testing_design`, `pane/fs_scripting_validation`,
    `pane/messenger_addressing_decisions` (already forwarded),
    others
- **When recalling agent-specific knowledge** — Phase 7.
  Currently in `.claude/agent-memory/<agent>/` (will move to
  serena `agent/<n>/`).

Run `mcp__serena__list_memories()` to see everything currently
in serena.

## How memory works in this project

See `~/memx-serena.md` for the principles. Briefly:

- One memory store per project (this serena project), all agents
  share it
- Read everywhere, write only to your own `agent/<n>/` folder
- Frontmatter has `supersedes:` / `superseded_by:` for write-once
  status discipline; `sources:` and `verified_against:` for
  staleness traceability
- Hub-and-spokes for clusters of 4+ related memories
- Fact-level granularity beats session-level
- Low-confidence rejection over stretching
