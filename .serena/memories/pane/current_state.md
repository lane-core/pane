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
- docs/ is flat, specs are living documents
- serena is sole memory system
- Be engineer must be consulted before implementing new subsystems

## Recent work completed this session
- BeAPI naming audit (Message, Messenger, close_requested, activated/deactivated, resize_to, fallback, MessageFilter)
- Rust idiom audit (all 30 findings fixed — no panics in library code, Debug on all public types, error source chains)
- Documentation consolidation (openspec retired, flat docs/, serena as sole memory)
- Crash monitoring + backpressure added
- Pulse + ShortcutFilter + geometry control added
- Removed Builtin/CommandAction (all commands go through handler)
