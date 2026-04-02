# Session Type Consultant Memory

- [Optics soundness review](project_optics_soundness_review.md) -- pane-optic design conditionally sound, four invariants, reviewed 2026-03-30
- [Distributed protocol review](project_distributed_protocol_review.md) -- reconnect, UUID PaneId, payload changes: polarity fix, Option B unsound, no Drop on Chan
- [Distributed code review](project_distributed_code_review.md) -- TCP active phase NOT dropped (false positive), TLS lazy, non-blocking unenforced, duality correct
- [Kit API improvements review](project_kit_api_improvements_review.md) -- filter mutation, quit protocol, PaneCreateFuture: deadlock constraint + orphan-pane race
- [Clipboard + undo analysis](project_clipboard_undo_analysis.md) -- two-interface problem, TTL/security, undo sensitivity, six invariants for implementation
- [C1 looper evolution](project_c1_looper_evolution.md) -- calloop multi-source select: conditionally sound, six invariants, three-phase migration
- [Protocol audit 2026-03-31](project_protocol_audit_2026_03_31.md) -- full audit: ownership gap, reconnect unsound, Chan no Drop, Message::Clone partial, duality correct
- [Phase 3 channel topology](project_phase3_channel_topology.md) -- register_channel<E> generic, ClipboardEvent extraction, commit() should return Result
- [Service disconnect analysis](project_service_disconnect_analysis.md) -- proactive required, generic variant, commit()->Result, not opt-in
- [Handler trait debate](project_handler_trait_debate.md) -- monolithic Handler vs per-protocol: concessions, attack points, key theorems
- [Handler debate FINAL](project_handler_debate_final.md) -- settled: commit register_channel+EventKind+commit()->Result, defer trait split, five invariants
- [Greenfield architecture](project_greenfield_architecture.md) -- from-scratch: per-protocol traits, CompositorEvent split, Sigma<H>, builder registration
- [Cross-proposal review](project_cross_proposal_review.md) -- Be+Plan9 review: Sigma conceded, messaging extraction argued, DeclareInterest adopted
