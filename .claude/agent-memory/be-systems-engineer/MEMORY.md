# Be Systems Engineer — Memory Index

- [reference_haiku_source.md](reference_haiku_source.md) — Key file paths in ~/src/haiku for verifying BeOS architecture claims (app_server, messaging, scripting, translation kit)
- [reference_scripting_protocol.md](reference_scripting_protocol.md) — BeOS scripting protocol mechanics (ResolveSpecifier chain, specifiers, property_info, hey) and mapping to pane's optics framework
- [project_architecture_review.md](project_architecture_review.md) — Architecture spec review findings (2026-03-20): scripting absent, optics ungrounded, kits thin, pane-route stale
- [project_architecture_draft.md](project_architecture_draft.md) — Key decisions from architecture draft (2026-03-20): router eliminated, watchdog pattern, session type transport strategy, s6 as init, Input Kit grammar
- [project_dependency_review.md](project_dependency_review.md) — Dependency philosophy review findings (2026-03-20): Landlock missing, bcachefs outdated, fuser/io_uring gap, Wayland protocol audit, femtovg trajectory
- [project_overall_assessment.md](project_overall_assessment.md) — Final spec assessment (2026-03-20): design sound, key risks are optics concreteness, AI Kit scope, bus factor; strategic advice for shipping
- [project_smithay_assessment.md](project_smithay_assessment.md) — Smithay viability assessment (2026-03-20): use it fully for Wayland plumbing, build personality on top, rendering split is fine, from-scratch costs 12-24 months
- [project_session_type_assessment.md](project_session_type_assessment.md) — Session type build-vs-buy (2026-03-20): recommends custom minimal typestate over par/dialectic; par panics on drop, no transport; dialectic requires async; custom fits calloop + crash handling
- [project_codebase_assessment.md](project_codebase_assessment.md) — Codebase assessment (2026-03-21): two crates, ~50% of pane-proto stale, pane-comp is demo only, refactor proposal written
- [project_session_crate_review.md](project_session_crate_review.md) — pane-session code review (2026-03-22): typestate correct, crash safety holds, calloop needs rework (blocking mode, no max msg size, framing duplication)
- [project_vector_similarity_design.md](project_vector_similarity_design.md) — Vector similarity design (2026-03-26): HNSW in pane-store, threshold live queries, embedding xattr format, model migration
- [project_pane_app_kit_design.md](project_pane_app_kit_design.md) — pane-app kit design decisions (2026-03-26): App not a looper, flat enum, filesystem scripting, TOML routing rules
- [project_fs_scripting_validation.md](project_fs_scripting_validation.md) — FS scripting validation (2026-03-28): 10 hey scenarios mapped, bet holds, no dynamic Message needed, ctl syntax + by-sig index recommended
- [project_translation_rules.md](project_translation_rules.md) — Be-to-pane translation decision tree, naming conventions, full header audit, 10 inconsistencies found (2026-03-28)
- [reference_decorator_architecture.md](reference_decorator_architecture.md) — Decorator class hierarchy, rendering flow, chrome/content split, threading, and mapping to pane's compositor
- [project_eact_analysis.md](project_eact_analysis.md) — EAct paper analysis (2026-03-29): 6 design principles, sub-protocol typing strategy, heterogeneous session loop as key evolution
