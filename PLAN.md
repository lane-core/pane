# Plan

Current implementation roadmap. This is a living document — update it when tasks complete, priorities change, or new work is identified.

**Rule:** At the end of every task, update this file. Mark completed items, add discovered work, adjust priorities. If this file is stale, the process broke, and we must immediately consult the user for clarification before proceeding further.

## Now

### Phase 4: Compositor Integration

Prerequisite for each item: consult Be engineer on how Be/Haiku implemented the equivalent (see docs/workflow.md).

- [ ] **Rendering** — compositor draws pane chrome (title bar from Tag), body area receives client content. Currently renders blank window with glyph atlas.
- [ ] **Input routing** — smithay keyboard/mouse events → CompToClient::Key/Mouse → kit → Handler. Currently no input forwarding from compositor to clients.
- [ ] **Multi-pane layout** — tiling, splits, focus tracking. Currently one pane = full window.

### API Tier 2

Design each feature against the EAct-derived session-type principles (serena: `pane/session_type_design_principles`). Each new protocol relationship should be a separate typed channel into the looper (C1), expose typestate handles at the API surface (C2), and consider per-conversation failure (C3).

- [ ] **Clipboard** — named clipboards, transactional lock/clear/commit (Be's BClipboard pattern). Needs protocol + compositor. Typestate handle: `ClipboardLock` → write → commit.
- [ ] **Observer pattern** — `Messenger::start_watching(property, watcher)` for inter-pane property change notification. Needs protocol. This is a mini-session per watcher relationship.
- [ ] **Drag and drop** — Message::DragEnter/DragOver/Drop + Messenger::drag_message(). Needs protocol + compositor. Typestate handle: `DragSession` tracks enter→over→drop progression.
- [ ] **Application registry** — stub kit types, implement when compositor supports it. Access point model (C4) applies here.

### Session-type debt (discovered by EAct audit)

Small concrete items identified by auditing the codebase against the session-type principles. Not blockers — cleanup for when the relevant code is next touched.

- [ ] **`pending_creates` → typestate handle** — `app.rs` uses `VecDeque<mpsc::Sender<CompToClient>>` for manual CreatePane→PaneCreated correlation. First candidate for C2 typestate refactoring (e.g., `PaneCreateFuture` consumed by the response).
- [ ] **pane-server read pump → calloop SessionSource** — existing TODO in `pane-server/src/lib.rs:14`. Now additionally motivated by C1: when the server handles per-client heterogeneous sessions (clipboard, etc.), the read pump pattern won't scale.
- [ ] **Pulse timer cancellation** — `Messenger::set_pulse_rate()` starts a new periodic thread but doesn't cancel the previous one (`proxy.rs:145`). Calling `set_pulse_rate` twice creates two concurrent pulse streams. Fix: store the `TimerToken` and cancel on re-set or zero.

## Next

- [ ] **pane-shell** — VT parser, PTY bridge, screen buffer. The first real application. Consult Be engineer on Terminal app architecture.
- [ ] **DummyRenderer headless tests** — smithay's renderer_test feature for protocol integration tests without GPU (feature flag already added).
- [ ] **CI** — macOS job for kit crates, Ubuntu job for compositor.

## Done

- [x] Phase 3: pane-app kit (App, Pane, Messenger, Message, Handler, MessageFilter)
- [x] Stage 5: BeAPI modernization (self-delivery, coalescing, timers, timestamps, filter wants, command enabled)
- [x] Stage 6: stress tests, handshake protocol, handshake integration
- [x] App::connect() over unix sockets
- [x] Phase 4 Stage 2: compositor protocol server (pane-hello ran against real compositor in VM)
- [x] Pulse, ShortcutFilter, geometry control (resize_to, set_size_limits, set_hidden)
- [x] Crash monitoring (Messenger::monitor + Message::PaneExited)
- [x] Bounded channel backpressure (sync_channel 256)
- [x] BeAPI naming audit (all identifiers reviewed case-by-case)
- [x] Rust idiom audit (30 findings, all resolved)
- [x] Documentation consolidation (openspec retired, flat docs/)
- [x] Dev workflow (default-members, nix copy, VM recipes, frame telemetry)

## Session Start Checklist

Before beginning work each session:

1. Read this file — know what's current, what's next
2. Read `pane/current_state` in serena — verify it matches this file
3. Read recent git log (`git log --oneline -10`) — know what changed since last session
4. If starting a new subsystem: consult Be engineer first (docs/workflow.md)

## Session End Checklist

After completing work each session:

1. Update this file — mark completed items, add discovered work
2. Update `pane/current_state` in serena if the project state changed substantially
3. Run `cargo test` — confirm all tests pass
4. If any substantial refactor occurred: verify stale doc review was done
5. Commit this file with the session's final commit
