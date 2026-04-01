# Session Type Consultant Memory

- [Optics soundness review](project_optics_soundness_review.md) -- pane-optic design conditionally sound, four invariants, reviewed 2026-03-30
- [Distributed protocol review](project_distributed_protocol_review.md) -- reconnect, UUID PaneId, payload changes: polarity fix, Option B unsound, no Drop on Chan
- [Distributed code review](project_distributed_code_review.md) -- TCP active phase NOT dropped (false positive), TLS lazy, non-blocking unenforced, duality correct
- [Kit API improvements review](project_kit_api_improvements_review.md) -- filter mutation, quit protocol, PaneCreateFuture: deadlock constraint + orphan-pane race
- [Clipboard + undo analysis](project_clipboard_undo_analysis.md) -- two-interface problem, TTL/security, undo sensitivity, six invariants for implementation
- [C1 looper evolution](project_c1_looper_evolution.md) -- calloop multi-source select: conditionally sound, six invariants, three-phase migration
- [Protocol audit 2026-03-31](project_protocol_audit_2026_03_31.md) -- full audit: ownership gap, reconnect unsound, Chan no Drop, Message::Clone partial, duality correct
