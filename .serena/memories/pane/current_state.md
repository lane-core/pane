# Pane Project Current State (2026-03-29)

## What exists and works
- **pane-app kit**: 130 tests, full BeOS-style API (App, Pane, Messenger, Message, Handler, MessageFilter)
- **pane-proto**: wire types, session-typed handshake (ClientHandshake/ServerHandshake), 20 proptest roundtrips
- **pane-session**: session-typed channels (Chan<S,T>), memory + unix transports, calloop integration
- **pane-server**: compositor protocol server (socket listener, handshake, message routing) — builds on macOS
- **pane-comp**: compositor with winit backend, protocol server, frame telemetry — cross-builds for Linux
- **pane-hello**: canonical first app, ran successfully against real compositor in VM
- **App::connect()**: works over real unix sockets with session-typed handshake

## What's next
See PLAN.md at the project root for the current roadmap. PLAN.md is the living document — update it at the end of every task.

## Phase 4 continuation
1. **Compositor rendering**: pane-comp draws pane chrome (title bar), body area shows client content. Currently renders blank window.
2. **Input routing**: smithay keyboard/mouse events → protocol → kit → handler. No input forwarding yet.
3. **Tier 2 API features**: clipboard, observer pattern (start_watching), drag-and-drop — all need protocol + compositor work.

## Dev workflow
- `cargo check` / `cargo test` works on macOS (default-members excludes pane-comp)
- `just build-comp` cross-builds compositor via nix
- `just vm-push` deploys to running VM via nix copy
- Foundational specs live in docs/ (immutable). Kit-level API docs live in Rust doc comments (source of truth for implemented crates).
- Style guide: `docs/kit-documentation-style.md` — Be Book-derived, credits both Be and Haiku
- Haiku Book (MIT) hosted at `reference/haiku-book/` — primary API reference for heritage
- serena is sole memory system
- Be engineer must be consulted before implementing new subsystems, producing a reading list of specific `.dox` files and a verification checklist

## Design guidance

All Tier 2 protocol work (clipboard, observer, DnD, inter-pane messaging) should be designed against the EAct-derived session-type principles:
- `pane/session_type_design_principles` — 6 principles (C1–C6) for protocol and API design
- `pane/eact_analysis_gaps` — 4 structural gaps to address as features are added
- `pane/eact_what_not_to_adopt` — explicit anti-patterns to avoid

Key takeaways: sub-protocols use typestate handles at the API surface (C2), new channels into the looper are separate typed channels (C1), failure modes consider per-conversation callbacks (C3).

## Recent work
- Kit API documentation: heritage annotations on all four kit crates (pane-proto, pane-session, pane-app, pane-notify)
- Haiku Book hosted at `reference/haiku-book/` (273 .dox files, MIT)
- Documentation style guide written (`docs/kit-documentation-style.md`)
- `docs/pane-app.md` archived — content absorbed into pane-app crate doc comments
- `docs/architecture.md` §4 trimmed — implemented kit descriptions removed, points to `cargo doc`
- Retroactive Haiku Book audit in progress (PLAN.md)
- Workflow updated: Be engineer consultations produce reading list + verification checklist
