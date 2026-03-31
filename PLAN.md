# Plan

Current implementation roadmap. This is a living document — update it when tasks complete, priorities change, or new work is identified.

**Rule:** At the end of every task, update this file. Mark completed items, add discovered work, adjust priorities. If this file is stale, the process broke, and we must immediately consult the user for clarification before proceeding further.

## Now

### Phase 4: Compositor Integration

Prerequisite for each item: consult Be engineer on how Be/Haiku implemented the equivalent (see docs/workflow.md).

- [ ] **Rendering** — compositor draws pane chrome (title bar from Tag), body area receives client content. Currently renders blank window with glyph atlas. Revised plan at `.claude/plans/typed-sleeping-falcon.md`. Be engineer BWindow research complete. Seven parts: Decorator trait, PaneState extension, font config, TextBuffer (CPU text), GPU texture pipeline, renderer rewrite, cleanup.
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

- [ ] **`pending_creates` → typestate handle** — `app.rs` uses `VecDeque<mpsc::Sender<CompToClient>>` for manual CreatePane→PaneCreated correlation. First candidate for C2 typestate refactoring (e.g., `PaneCreateFuture` consumed by the response). Deferred to Tier 2 protocol work (requires request IDs in CreatePane/PaneCreated).
- [x] **pane-server read pump → calloop SessionSource** — replaced thread-per-client read pump with calloop SessionSource for event-driven message dispatch. Messages dispatch immediately on fd-readiness instead of polled once per frame.
- [x] **Pulse timer cancellation** — `set_pulse_rate()` now cancels the previous timer via shared `Arc<Mutex<Option<TimerToken>>>`. `Duration::ZERO` cancels cleanly.

### Haiku Book audit (completed)

Audited all 7 implemented types against their Haiku Book `.dox` entries. Full audit reports in session history. Summary of actionable findings:

**Bug:**
- [x] **TimerToken cancel: `Relaxed` → `Release`/`Acquire`** — fixed. Release on store, Acquire on load.

**Must address:**
- [x] **Application-defined messages** — `Message::App(Box<dyn Any + Send>)` + `Messenger::post_app_message<T>()` + `Handler::message_received()`. Sending is generic, erasure is internal.
- [x] **pane-notify Event struct** — restructured with NodeRef, StatFields bitmask, AttrCause, move cookies. WatchFlags separated from EventKind.
- [x] **pane-notify `Modify`/`Attrib` split** — replaced with `StatChanged { fields }` + `AttrChanged { attr, cause }`. Follows Haiku's model.
- [x] **pane-notify move model** — `MovedFrom`/`MovedTo` with inotify cookie for correlation.
- [x] **Synchronous send-reply** — `send_and_wait` (blocking) + `send_request` (async) + `ReplyPort` (session-type handle, exactly-one-reply via ownership). Generalizes `pending_creates`.
- [x] **Document `send_message()` blocking** — documented. Added `try_send_message()` (non-blocking, returns `ChannelFull`) and `is_valid()`.

**Should address (Tier 2 prerequisites):**
- [ ] **`ScreenChanged` event** — DPI/scale awareness. Real Wayland capability (`wl_output` changes).
- [ ] **`RequestActivate`** — apps can't programmatically pull focus to a pane.
- [ ] **Fullscreen request** — `ClientToComp::SetFullscreen`.
- [x] **`Messenger::is_valid()`** — checks looper channel attachment.
- [ ] **Runtime filter mutation** — no add/remove filters from within a handler. Need `Messenger::add_filter()`.
- [x] **Timer consolidation** — `recv_timeout` in the looper, zero timer threads. Timers fire through the looper's event loop.
- [ ] **pane-notify: mount/unmount events** — pane-store needs these for new volume indexing.
- [ ] **pane-notify: recursive watching** — build into pane-notify, at least as opt-in `watch_path_recursive()`.
- [ ] **App-level quit protocol** — no atomic "ask all panes, any can veto" for save-all-or-cancel.
- [ ] **`RefsReceived` equivalent** — file delivery from file managers.

**Document (divergences not yet recorded):**
- [x] **Handler: no handler chain** — documented in `# BeOS Divergences` section on Handler trait.
- [x] **Handler: observer pattern decision** — documented on Handler + serena memory `pane/observer_pattern_decision`. Filesystem attributes, not messaging.
- [x] **Filter: retargeting absent** — documented on MessageFilter trait.
- [x] **pane-notify: `WatchFlags` vs `EventKind`** — separated. WatchFlags is the subscription, EventKind is the notification.
- [x] **Show/Hide: boolean vs cumulative** — documented on `Messenger::set_hidden`. Boolean (idempotent), compositor owns visibility.

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
- [x] EAct session-type design principles (C1-C6, gaps 1-4, anti-patterns)
- [x] Haiku Book audit (7 types verified, findings addressed)
- [x] Session-type debt: pulse timer fix, read pump → SessionSource, timer consolidation, reply mechanism
- [x] Message::AppMessage + post_app_message (worker thread results)
- [x] pane-notify restructure (NodeRef, StatFields, AttrCause, WatchFlags, move cookies)
- [x] ReplyPort + send_and_wait + send_request (session-typed request-reply)
- [x] Timer factory closures (eliminate Clone panic)
- [x] BeOS divergence documentation (handler chain, observer pattern, filter retargeting, show/hide)
- [x] Naming conventions audit (AppMessage, request_received, matches)
- [x] Licensing: BSD-3-Clause protocol, BSD-2-Clause kits
- [x] Adversarial tests: 11 new timer + reply tests
- [x] Messenger::is_valid + try_send_message
- [x] pane-optic crate: Getter/Setter/PartialGetter/PartialSetter traits, FieldLens/FieldAffine/FieldTraversal, composition, optic law tests
- [x] Scripting foundation: PropertyInfo (replaces Attribute), ScriptableHandler trait, DynOptic trait, ScriptReply, CompletionReplyPort, ScriptError, AttrValue, ValueType, SpecifierForm, Specifier

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
