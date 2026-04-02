# Pane Project Current State (2026-03-31)

## What exists and works
- **pane-app kit**: 138 tests, calloop-backed looper (C1 Phase 1+2 complete), TimerToken cancel-on-drop, clipboard kit types, undo framework. Full BeOS-style API (App, Pane, Messenger, Message, Handler, MessageFilter, CompletionReplyPort, ReplyPort).
- **pane-optic**: composable optic types (FieldLens, FieldAffine, FieldTraversal), composition, optic law tests. Pure crate, no pane-specific deps.
- **pane-proto**: wire types, session-typed handshake (ClientHandshake/ServerHandshake), PeerIdentity, ConnectionTopology, 20 proptest roundtrips
- **pane-session**: session-typed channels (Chan<S,T>), memory + unix + TCP + TLS transports, calloop integration, ProxyTransport (protocol tracing), ReconnectingTransport (aan pattern). 40 tests.
- **pane-server**: compositor protocol server (socket listener, handshake, message routing) — builds on macOS
- **pane-comp**: compositor with winit backend, protocol server, frame telemetry — cross-builds for Linux
- **pane-headless**: headless server binary, dual listeners (unix + TCP), `--protocol-trace` flag, calloop event loop
- **pane-hello**: canonical first app, ran successfully against real compositor in VM
- **App::connect()**: works over real unix sockets with session-typed handshake
- **App::connect_remote()**: works over TCP with identity forwarding
- **Nix flake**: NixOS, Darwin, sixos modules for service deployment

## What's next
See PLAN.md at the project root for the current roadmap. PLAN.md is the living document — update it at the end of every task.

## Known issues (from 2026-03-31 seven-reviewer audit)
Full findings: serena memory `pane/code_review_findings_2026_03_31`.
- Server pane ownership not verified (critical for TCP)
- PeerIdentity discarded after handshake (no access control possible)
- Message::Clone panics on 4 variants (deprecate send_periodic)
- TLS not integrated into pane-headless (plaintext TCP only)

## Dev workflow
- `cargo check` / `cargo test` works on macOS (default-members excludes pane-comp)
- `just build-comp` cross-builds compositor via nix
- `just vm-push` deploys to running VM via nix copy
- Specs in docs/ (immutable). Kit-level API docs in Rust doc comments (source of truth).
- Style guide: `docs/kit-documentation-style.md` — heritage annotations use both `# BeOS` and `# Plan 9` sections
- Haiku Book (MIT) at `reference/haiku-book/` — primary BeOS/Haiku API reference
- Plan 9 Programmer's Manual (MIT) at `reference/plan9/` — man pages + paper sources
- serena is sole memory system. Divergence trackers: `pane/beapi_divergences`, `pane/plan9_divergences`
- Be engineer + Plan 9 engineer must be consulted before new subsystems

## Design guidance
All Tier 2 protocol work should be designed against the EAct-derived session-type principles:
- `pane/session_type_design_principles` — 6 principles (C1–C6)
- `pane/eact_analysis_gaps` — 4 structural gaps
- `pane/eact_what_not_to_adopt` — anti-patterns
- `pane/plan9_reference_insights` — Plan 9 patterns per subsystem

Key: sub-protocols use typestate handles (C2), new channels are separate typed sources (C1), failure modes consider per-conversation callbacks (C3).
