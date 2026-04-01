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

- [ ] **Clipboard** — named clipboards, transactional lock/clear/commit (Be's BClipboard pattern). Kit types done: `Clipboard`, `ClipboardWriteLock` (typestate), `ClipboardMetadata` (sensitivity/TTL/locality), Handler integration. Service process and pane-fs projection deferred. Spec: `docs/superpowers/specs/2026-03-31-clipboard-design.md`.
- [x] **Undo/redo framework** — `UndoPolicy` trait, `LinearPolicy`, `CoalescingPolicy`, `UndoManager` with save-point, `RecordingOptic` with sensitive exclusion via `DynOptic::is_undoable()`. Spec: `docs/superpowers/specs/2026-03-31-undo-design.md`.
- [ ] **Observer pattern** — `Messenger::start_watching(property, watcher)` for inter-pane property change notification. Needs protocol. This is a mini-session per watcher relationship.
- [ ] **Drag and drop** — Message::DragEnter/DragOver/Drop + Messenger::drag_message(). Needs protocol + compositor. Typestate handle: `DragSession` tracks enter→over→drop progression.
- [ ] **Application registry** — stub kit types, implement when compositor supports it. Access point model (C4) applies here.

### C1: Multi-Source Looper Evolution

Incremental migration from single-channel looper to calloop-backed multi-source event loop. Full plan at `.claude/plans/jolly-riding-russell.md`.

- [x] **Phase 1: calloop looper backend** — replaced mpsc::recv_timeout with calloop::EventLoop. LooperMessage channel is calloop::channel. All senders use calloop::channel::Sender. Unbounded channel (bounded backpressure removed). drain_channel handles calloop's 1024-msg-per-dispatch limit.
- [x] **Phase 2: Timer migration** — replaced hand-rolled Timers struct with calloop Timer sources. TimerToken is non-Clone with cancel-on-drop (closes affine gap). CancelTimer message for eager source removal. Deleted ~100 lines of manual deadline scheduling (TimerEntry, Timers, next_timeout, fire_due). Timer callbacks push directly into LooperState.batch.
- [ ] **Phase 3: Channel topology split** — clipboard, observer, etc. as separate calloop sources with per-channel message types.

### Code review debt (2026-03-31 seven-reviewer audit)

Full findings in serena memory `pane/code_review_findings_2026_03_31`.

**Critical (fix before TCP deployment):**
- [x] **Server pane ownership check** — `pane_owned_by()` guard on every match arm except CreatePane.
- [x] **Store identity after handshake** — `ClientSession.identity` stored, threaded through `CompletedHandshake`.
- [x] **Remove `send_periodic`** — removed along with `try_send_message`. `send_periodic_fn` is the only periodic API.

**Moderate (fix soon):**
- [x] **Exhaustive `try_from_comp` match** — all 13 CompToClient variants explicit, no catch-all.
- [x] **Document mutual `send_and_wait` deadlock** — same bug as BeOS. Documented on `send_and_wait`.
- [x] **Document closure handler request asymmetry** — documented on `Pane::run`.
- [x] **Fix UndoManager `wrapping_sub`** — `saturating_sub`.
- [x] **Convert `try_reconnect` to iterative loop** — no recursion, bounded by timeout.
- [ ] **TLS integration in pane-headless** — transport exists, listener is plaintext-only.

**Documentation debt:**
- [x] **`# BeOS` annotations** — Message enum, Resize/CloseRequested/Pulse, Clipboard, Pane::run/run_with.
- [x] **`# Plan 9` annotation** — Locality enum (snarf federation).
- [x] **UndoManager/UndoPolicy doc comments** — all public methods documented.

### Session-type debt (discovered by EAct audit)

Small concrete items identified by auditing the codebase against the session-type principles. Not blockers — cleanup for when the relevant code is next touched.

- [x] **`pending_creates` → typestate handle** — `PaneCreateFuture` with `wait()`/`wait_timeout()`, `#[must_use]`, Drop cancel-sender (clunk-on-abandon). Fixes orphan pane leak.
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
- [x] **Document `send_message()` blocking** — documented. Added `try_send_message()` (deprecated after unbounded channel migration) and `is_valid()`.

**Should address (Tier 2 prerequisites):**
- [ ] **`ScreenChanged` event** — DPI/scale awareness. Real Wayland capability (`wl_output` changes).
- [ ] **`RequestActivate`** — apps can't programmatically pull focus to a pane.
- [ ] **Fullscreen request** — `ClientToComp::SetFullscreen`.
- [x] **`Messenger::is_valid()`** — checks looper channel attachment.
- [x] **Runtime filter mutation** — `Messenger::add_filter()`/`remove_filter()` via LooperMessage, batch-boundary timing. FilterToken for removal.
- [x] **Timer consolidation** — `recv_timeout` in the looper, zero timer threads. Timers fire through the looper's event loop.
- [ ] **pane-notify: mount/unmount events** — pane-store needs these for new volume indexing.
- [ ] **pane-notify: recursive watching** — build into pane-notify, at least as opt-in `watch_path_recursive()`.
- [x] **App-level quit protocol** — `App::request_quit()` → `QuitResult` (Approved/Vetoed/Unreachable). `Handler::quit_requested(&self)` with &self-only constraint for deadlock freedom.
- [ ] **`RefsReceived` equivalent** — file delivery from file managers.

**Document (divergences not yet recorded):**
- [x] **Handler: no handler chain** — documented in `# BeOS Divergences` section on Handler trait.
- [x] **Handler: observer pattern decision** — documented on Handler + serena memory `pane/observer_pattern_decision`. Filesystem attributes, not messaging.
- [x] **Filter: retargeting absent** — documented on MessageFilter trait.
- [x] **pane-notify: `WatchFlags` vs `EventKind`** — separated. WatchFlags is the subscription, EventKind is the notification.
- [x] **Show/Hide: boolean vs cumulative** — documented on `Messenger::set_hidden`. Boolean (idempotent), compositor owns visibility.

## Next

### Distributed Computing Foundation

Design spec: `docs/distributed-pane.md`. Plan 9 research: `docs/superpowers/plan9-distributed-mapping.md`. Consult both the be-systems-engineer and plan9-systems-engineer agents before implementing new subsystems.

**Phase 1: Network Transport + Headless Server**
- [x] **TcpTransport** — `pane-session/src/transport/tcp.rs`. Same pattern as unix.rs.
- [x] **Generalize SessionSource** — parameterized over `AsFd + Read`, `UnixSessionSource` alias.
- [x] **Protocol extensions** — `PeerIdentity`, `ConnectionTopology`, `instance_id` in handshake types. `PaneRefused` variant. Server rejection on version mismatch / missing identity.
- [x] **pane-server extensions** — `new_unmanaged()`, generic `ClientStream` enum, generic handshake over Transport with rejection.
- [x] **pane-headless binary** — calloop event loop, dual listeners (unix + TCP), handshake timeouts.
- [x] **App::connect_remote** — TCP connection path, identity forwarding, generic pump threads.
- [x] **TLS transport** — `pane-session/src/transport/tls.rs`, rustls, eager handshake completion.

**Phase 2: Nix Flake Architecture**
- [x] **Target-agnostic service definitions** — `nix/lib/services.nix`, consumed by platform backends.
- [x] **NixOS module** — `nixosModules.core` (systemd backend, adoption on-ramp).
- [x] **Darwin module** — `darwinModules.core` (launchd backend).
- [x] **sixos modules** — `sixosModules.core`, `.compositor`, `.desktop` (s6-rc backend, native Pane Linux).
- [x] **pane-headless package** — builds on all platforms.

**Phase 2 specs (design complete, implement when crate is built):**
- [ ] **pane-roster federation** — cross-instance service discovery, init system abstraction (s6/launchd/systemd).
- [ ] **pane-store core/full** — SQLite backend (core) vs xattrs + fanotify (full).
- [ ] **pane-fs unified namespace** — computed views, remote mounting, core/full FUSE backend.
- [ ] **pane-watchdog** — headless tier (not Linux-only), platform-abstracted restart.
- [ ] **Network-aware .plan** — Landlock + network namespaces from `.plan`, remote agent verification.

**Architecture decisions:**
- sixos as base for Pane Linux (not custom system builder) — see `docs/architecture.md` §9
- Unified namespace (local + remote interleaved under `/pane/`) — see `docs/distributed-pane.md` §3
- pane-fs as query system (BFS queries as Plan 9 synthetic filesystem paths)
- UUIDs for globally unique PaneIds
- Host as contingent server (no architectural privilege for local machine)

### Plan 9 Patterns (discovered from reference audit)

Concrete patterns extracted from vendored Plan 9 man pages and papers. See serena memory `pane/plan9_reference_insights` for full citations.

**Near-term (implementable now):**
- [x] **Protocol tracing transport** — `ProxyTransport` wrapper that logs all protocol messages between app and compositor. `--protocol-trace <file>` flag on pane-headless traces handshake (via ProxyTransport) and active-phase incoming messages. From `exportfs -d` and the `iostats` pattern in the names paper.
- [x] **`ReconnectingTransport`** — transparent filter that buffers messages during temporary disconnection and replays on reconnection (configurable timeout, default 60s). Client-side only (server-side buffering deferred). From `aan(8)`, used via `import -p`. Critical for mobile/WiFi.

**Medium-term (needs subsystem prerequisites):**
- [ ] **Blocking-read event files** (pane-fs prerequisite) — `/pane/<id>/event` blocks on read until state changes. The Plan 9 observer pattern: no subscription, just blocking reads. Cheapest way to make pane scriptable. From `rio(4)` wctl.
- [ ] **`consctl` lease pattern** (pane-shell prerequisite) — RAII mode handles where holding the handle holds the mode, drop reverts. From `rio(4)` consctl: "Closing the file makes the window revert to default state." Generalize the `ClipboardWriteLock` pattern.
- [ ] **Plumber `click` context refinement** (routing prerequisite) — cursor-position annotation in routing messages for smarter regex matching. From `plumb(6)`: "longest leftmost match that contains or abuts the textual location identified by the click."
- [ ] **`cpu` reverse export** — local compositor exports display/input to remote headless instance. From `drawterm(8)` and the names paper. pane's `import` equivalent inverted.

### Other

- [ ] **pane-shell** — VT parser, PTY bridge, screen buffer. The first real application. Consult Be engineer on Terminal app architecture. Plan 9 reference: `rio(4)` file hierarchy (cons, consctl, label, text, wdir, wsys), 8½ paper for recursive architecture.
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
- [x] Bounded channel backpressure (sync_channel 256 — later replaced by unbounded calloop channel in C1 Phase 1)
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
