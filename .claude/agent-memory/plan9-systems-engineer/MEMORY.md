# Plan 9 Systems Engineer Memory

- [Project: pane distributed model mapping](project_distributed_mapping.md) — completed research mapping Plan 9 mechanisms to pane's architecture
- [User: Lane's distributed systems context](user_lane_context.md) — Lane's background and how to calibrate advice
- [Reference: key Plan 9 design decisions for pane](reference_plan9_decisions.md) — which Plan 9 patterns were adopted/adapted/rejected and why
- [Project: Phase 1 protocol extension decisions](project_protocol_extension_decisions.md) — PeerIdentity placement, instance_id format, calloop channel, Cancel deferral
- [Project: test suite design for distributed pane](project_test_suite_design.md) — three-layer test taxonomy, MockCompositor vs headless roles, PaneId discipline
- [Project: code review findings](project_code_review_findings.md) — 14 findings: ownership bypass, identity loss, session continuity, TLS gap (2026-03-31)
- [Project: clipboard and undo design](project_clipboard_undo_design.md) — MIME negotiation, TTL, cross-machine policy, undo ctl pattern (2026-03-31)
- [Project: C1 looper evolution](project_c1_looper_evolution.md) — calloop multi-source select: wire stays single-stream, source priorities, coalescing risk (2026-03-31)
- [Project: Plan 9 lineage audit](project_plan9_lineage_audit.md) — 10 new annotations needed, licensing clear (MIT), divergences tracker proposed (2026-03-31)
- [Project: Plan 9 reference audit](project_reference_audit.md) — deep reading of vendored man pages + papers complete, insights in serena (2026-03-31)
- [Project: Phase 3 channel topology](project_phase3_channel_topology.md) — mount metaphor, priority deferral, async clipboard channel, no fd table, concrete-not-generic
- [Project: service disconnect model](project_service_disconnect_model.md) — fail at use site (Plan 9 Ehangup), no proactive notification, commit() returns Result
- [Project: Pane-as-trait assessment](project_pane_as_trait_assessment.md) — approved with caveats: no fd table, protocol caps needed separately, looper specialization is open risk
- [Project: Pane-as-trait debate](project_pane_as_trait_debate.md) — Plan 9 vs Be positions: concessions, attacks, unresolved tensions (2026-03-31)
- [Project: Handler architecture final](project_handler_architecture_final.md) — unified Handler + callback-struct, protocol caps, trait split deferred (2026-03-31)
- [Project: Pane-is-directory architecture](project_pane_is_directory_architecture.md) — clean-slate: services as opened handles, DeclareInterest protocol, Handler shrinks to compositor
