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
- [`policy/pre_implementation_consultation`](policy/pre_implementation_consultation.md) — required reading list from Haiku / Plan 9 primary sources before implementation
- [`policy/design_decision_escalation`](policy/design_decision_escalation.md) — when to ask Lane
- [`policy/functoriality_principle`](policy/functoriality_principle.md) — Phase 1 types must be full-architecture types, populated minimally

## When you need a process rule

- [`policy/memory_discipline`](policy/memory_discipline.md) — how memory is organized in this project (memx principles, ported)
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
- [`decision/vertical_slice_first_pane`](decision/vertical_slice_first_pane.md) — Path B: build first running hello-world pane end-to-end

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

- [`architecture/proto`](architecture/proto.md) — pane-proto vocabulary crate: Message, Protocol, Handles, Handler, ControlMessage, ServiceFrame, obligation handles, MonadicLens
- [`architecture/session`](architecture/session.md) — pane-session IPC: framing, transport, bridge, ProtocolServer single-threaded actor, watch/PaneExited
- [`architecture/app`](architecture/app.md) — pane-app actor framework: Handler, DispatchCtx, Messenger, ServiceHandle, install-before-wire, destruction sequence
- [`architecture/fs`](architecture/fs.md) — pane-fs filesystem namespace: PaneEntry, AttrSet, snapshot model, FUSE/ctl/PaneNode gaps
- [`architecture/looper`](architecture/looper.md) — calloop event loop, six-phase batch ordering, watchdog, send_and_wait/I8
- [`architecture/rustix_migration`](architecture/rustix_migration.md) — pane-session FFI → rustix migration plan

## When you need theoretical grounding

Analysis clusters are hub-and-spokes. Start at the hub, descend to spokes.

- [`analysis/eact/_hub`](analysis/eact/_hub.md) — EAct calculus audit: theorems, divergences, gaps, invariants, design principles not adopted
- [`analysis/session_types/_hub`](analysis/session_types/_hub.md) — protocol design: principles (C1–C6), optic boundary rules (R1–R10), coprocess worked example
- [`analysis/optics/_hub`](analysis/optics/_hub.md) — concrete `MonadicLens` kit + `AttrReader` FUSE path, writer monad, taxonomy, boundaries (what's NOT an optic)
- [`analysis/duploid/_hub`](analysis/duploid/_hub.md) — polarity structure, non-associativity, writer monad + mixed optic, shift operator
- [`analysis/verification/_hub`](analysis/verification/_hub.md) — invariant audits (I1–I13, S1–S6), spec fidelity, test coverage, fs scripting validation, namespace testing

Standalone analysis:\n\n- [`analysis/plan9_test_heritage`](analysis/plan9_test_heritage.md) — 24 Plan 9-derived tests (T1–T24): Tflush/Cancel, Tversion/handshake, fid/ServiceHandle, freefidpool/disconnect, walk/DeclareInterest\n\n- [`analysis/performance_plan9_precedents`](analysis/performance_plan9_precedents.md) — dispatch threading, routing hop, write batching: Plan 9 precedents + pane recommendations\n- [`analysis/shell_sequent_calculus`](analysis/shell_sequent_calculus.md) — sequent calculus grounding for pane-terminal / psh integration (Phase 2+)

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
- **Knowledge management:** `memx` (the rulebook for serena, canonicalized in `policy/memory_discipline`)
- **Unix history:** `unix_retrospective`

## When citing other external knowledge

- [`reference/smithay`](reference/smithay.md) — smithay v0.7.0 viability assessment for pane-comp
- [`reference/fp_library`](reference/fp_library.md) — fp-library 0.15.0 optics API + Send analysis

## When recalling agent-specific knowledge

Each project agent has its own hub for institutional knowledge.
Read-everywhere, write-only-to-own-folder discipline applies.

- [`agent/plan9-systems-engineer/_hub`](agent/plan9-systems-engineer/_hub.md)
- [`agent/be-systems-engineer/_hub`](agent/be-systems-engineer/_hub.md)
- [`agent/optics-theorist/_hub`](agent/optics-theorist/_hub.md) + [`linearity_gap`](agent/optics-theorist/linearity_gap.md)
- [`agent/session-type-consultant/_hub`](agent/session-type-consultant/_hub.md) + [`feedback_mailbox_type_retraction`](agent/session-type-consultant/feedback_mailbox_type_retraction.md) + [`backpressure_tier_review`](agent/session-type-consultant/backpressure_tier_review.md)
- [`agent/formal-verifier/_hub`](agent/formal-verifier/_hub.md)
- [`agent/pane-architect/_hub`](agent/pane-architect/_hub.md)

The legacy `.claude/agent-memory/<agent>/` layer was retired
on 2026-04-11; its content migrated to `agent/<n>/*` in serena.

## How memory works in this project

See [`policy/memory_discipline`](policy/memory_discipline.md) for
the full principles. Briefly:

- One memory store per project (this serena project), all agents
  share it
- Read everywhere, write only to your own `agent/<n>/` folder
- Frontmatter has `supersedes:` / `superseded_by:` for write-once
  status discipline; `sources:` and `verified_against:` for
  staleness traceability
- Hub-and-spokes for clusters of 4+ related memories
- Fact-level granularity beats session-level
- Low-confidence rejection over stretching
