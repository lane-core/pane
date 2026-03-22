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
