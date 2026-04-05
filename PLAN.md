# Plan

Current implementation roadmap. This is a living document — update it when tasks complete, priorities change, or new work is identified.

**Rule:** At the end of every task, update this file. Mark completed items, add discovered work, adjust priorities. If this file is stale, the process broke, and we must immediately consult the user for clarification before proceeding further.

**Source of truth:** `docs/architecture.md` is the design spec. `docs/optics-design-brief.md` is the optics/ctl/pane-fs design spec. This file tracks execution against those specs.

## Now

### Phase 1 — Core

Single server (N=1), headless, no suspension, no streaming. All multi-server data structures present with one entry (functoriality principle — Phase 2 adds entries, not structure).

**Prerequisite for each item:** consult all four design agents (Plan 9, Be, optics, session types) in parallel, refine with Lane, then forward to pane-architect for implementation and formal-verifier for audit. See serena memory `pane/agent_workflow`.

#### Protocol foundation (pane-proto)

- [x] **ServiceId** — UUID + reverse-DNS name, `ServiceId::new()` with UUIDv5 derivation (3 tests)
- [x] **Protocol trait** — `SERVICE_ID: ServiceId` + `type Message: Send + 'static`
- [x] **Lifecycle protocol** — `Lifecycle` as Protocol impl; `LifecycleMessage` enum
- [x] **Message trait** — `Clone + Serialize + DeserializeOwned + Send + 'static` blanket impl
- [x] **Handles\<P\> trait** — `fn receive(&mut self, msg: P::Message) -> Flow` (3 tests)
- [x] **Handler trait** — lifecycle methods, blanket `Handles<Lifecycle>` impl (3 tests)
- [x] **Flow** — `Continue` / `Stop`
- [x] **MessageFilter** — typed per-protocol, `FilterAction::Pass/Transform/Consume` (3 tests)
- [x] **MonadicLens\<S,A\>** — concrete fn-pointer optics with effectful set, law test harness (16 tests)
- [x] **Obligation handles** — ReplyPort, CompletionReplyPort, CancelHandle with `#[must_use]`, Drop compensation (14 tests)
- [ ] **Framework protocols** — `Display` as Protocol impl; `ControlMessage` enum (wire service 0)
- [ ] **PeerAuth** — `Kernel { uid, pid }` (SO_PEERCRED) and `Certificate { subject, issuer }` (TLS) variants
- [ ] **Handshake types** — Hello with `interests: Vec<ServiceInterest>`, Welcome with `bindings: Vec<ServiceBinding>`, `max_message_size` negotiation
- [ ] **DeclareInterest / InterestAccepted / InterestDeclined** — late-binding active-phase messages
- [ ] **Cancel { token }** — advisory request cancellation (Tflush equivalent)
- [ ] **ProtocolHandler derive macro** — generates `Handles<P>::receive` match from named methods

#### Session layer (pane-session)

- [x] **Transport** — `Transport` trait, `MemoryTransport::pair()` for testing (3 tests)
- [x] **Bridge** — two-phase connect (verify_transport + par handshake) (2 tests)
- [x] **FrameCodec** — `[length: u32 LE][service: u8][payload]`, reserved 0xFF abort, known_services bitset, max_message_size enforcement (20 tests)
- [ ] **Verify Chan<S,T> compatibility** — ensure session-typed channels work with new handshake types
- [ ] **SessionEnum derive** — N-ary enum branching with `#[session_tag]` wire stability

#### Kit API (pane-app)

- [x] **Dispatch\<H\>** — per-request typed dispatch entries, token uniqueness, fail_connection, cancel (6 tests)
- [x] **LooperCore\<H\>** — catch_unwind boundary, destruction sequence (fail_connection → clear → handler drop → notify), exited guard (12 tests)
- [x] **PaneBuilder\<H\>** — two-phase lifecycle, open_service stub, duplicate rejection (3 tests)
- [x] **Pane** — non-generic connection identity (stub)
- [x] **Messenger** — scoped handle (stub)
- [x] **ServiceHandle\<P\>** — stub with Drop RevokeInterest placeholder
- [x] **ExitReason** — Graceful/Disconnected/Failed/InfraError
- [ ] **Messenger full impl** — `send_request`, `set_content`, `set_pulse_rate`, `post_app_message`
- [ ] **ConnectionSource** — calloop EventSource for a single Connection (read + buffered write)
- [ ] **Service registration** — `PaneBuilder::open_service::<P>()` with real DeclareInterest exchange
- [ ] **Looper** — calloop-backed, per-protocol typed channels, unified batch, coalescing
- [ ] **AppPayload** — `Clone + Send + 'static` marker trait

#### Optics and namespace (pane-fs)

- [x] **AttrReader\<S\>, AttrSet\<S\>** — type-erased read path from monadic lens view (3 tests)
- [x] **AttrWriter\<S\>** — type-erased write path from monadic lens set+parse
- [x] **AttrSet::to_json_str()** — bulk read, all attrs from one snapshot
- [x] **PaneEntry\<S\>** — snapshot-based namespace entry (2 tests)
- [x] **Ctl dispatch** — monadic lens routing for state mutations, freeform fallback for lifecycle/IO
- [x] **`json` reserved filename** — at every directory level in the namespace
- [ ] **Snapshot synchronization** — ArcSwap for lock-free looper→FUSE snapshot publishing
- [ ] **PaneNode trait** — type erasure so namespace holds different state types
- [ ] **Ctl parsing module** — line-oriented command parsing with synchronous oneshot mechanism
- [ ] **FUSE integration** — actual FUSE mount serving the namespace
- [ ] **`#[derive(Scriptable)]` macro** — last, after hand-coded path works

#### Server (pane-server)

- [ ] **ProtocolServer** — service-aware routing via ServiceRouter per client
- [ ] **Per-service wire dispatch** — demux by service discriminant via FrameCodec
- [ ] **pane_owned_by()** — PeerAuth-based ownership check on every operation

#### Headless binary (pane-headless)

- [ ] **pane-headless** — calloop event loop, unix listener, handshake with new protocol types

#### Invariants validated

From architecture spec I1–I13 and S1–S6. Status from formal-verifier audit (2026-04-05):

- [x] **I1** (panic=unwind, Drop fires) — partial: tested via obligation handle unwind tests
- [ ] **I2** (no blocking in handlers) — not testable without timeout watchdog
- [ ] **I3** (handlers terminate) — not testable without timeout watchdog
- [x] **I4** (typestate handles) — tested for ReplyPort, CompletionReplyPort, CancelHandle
- [ ] **I5** (filters see only Clone-safe Messages) — partial: filter tests exist, bypass path untested
- [ ] **I6** (sequential single-thread dispatch) — needs calloop integration
- [ ] **I7** (service dispatch fn pointers sequential) — needs fn-pointer dispatch table
- [ ] **I8** (send_and_wait panics from looper thread) — send_and_wait not implemented
- [x] **I9** (dispatch cleared before handler drop) — tested in destruction_sequence_ordering
- [x] **I10** (ProtocolAbort non-blocking) — partial: framing layer provides fallible write
- [x] **I11** (ProtocolAbort at framing layer) — tested: reserved 0xFF, all paths covered
- [x] **I12** (unknown discriminant → connection error) — tested: monotonic known_services
- [ ] **I13** (open_service blocks until accepted) — open_service is a stub
- [x] **S1** (token uniqueness) — tested: consecutive inserts differ
- [ ] **S2** (sequential dispatch) — follows from I6
- [ ] **S3** (control-before-events in batch) — no batch processing
- [x] **S4** (fail_connection scoped) — tested at both Dispatch and LooperCore levels
- [x] **S5** (cancel without callbacks) — tested with panic-on-call guards
- [x] **S6** (panic=unwind) — follows from I1

## Next

### Phase 2 — Distribution

Add Connections (N>1). TLS + PeerAuth::Certificate. Service map full precedence chain. Version range negotiation.

- [ ] **Multi-server App** — `connect_service()`, ServiceRouter with multiple entries
- [ ] **TLS transport** — `Connection::remote` requires TLS, `pane dev-certs` tooling
- [ ] **Per-Connection failure isolation** — Connection loss affects only its capabilities
- [ ] **Cross-Connection ordering** — sequential consistency per-pane, not causal across servers
- [ ] **Service map** — `$PANE_SERVICE_OVERRIDES` > manifest > `$PANE_SERVICES` > `/etc/pane/services.toml`
- [ ] **Version range negotiation** — min/max in DeclareInterest

### Phase 3 — Lifecycle

Session suspension/resumption, streaming (Queue pattern), Handles<Routing>. These interact — streams must close before suspend — so they're designed together.

- [ ] **Session suspension/resumption** — serializable token, re-declare interests on resume
- [ ] **Streaming** — `StreamSend<T, S>` / `StreamRecv<T, S>`, backpressure via write buffer high-water mark
- [ ] **Handles\<Routing\>** — headless command surface via DeclareInterest, attribute macro

### Phase 4 — Performance

Correctness before performance.

- [ ] **Batch coalescing optimizations**
- [ ] **Write buffer tuning**
- [ ] **Connection pooling**

### Compositor

Orthogonal to protocol phases — can proceed in parallel once Phase 1 server exists.

- [ ] **pane-comp** — smithay/winit backend providing the Display view
- [ ] **Rendering** — compositor draws pane chrome (title bar from Tag), body area receives client content
- [ ] **Input routing** — keyboard/mouse events → Handles<Display> methods
- [ ] **Multi-pane layout** — tiling, splits, focus tracking

### Applications

- [ ] **pane-hello** — canonical first app, closure form
- [ ] **pane-shell** — VT parser, PTY bridge, screen buffer

## Crates

| Crate | Role | Status |
|-------|------|--------|
| pane-proto | Protocol vocabulary, no IO | Active (47 tests) |
| pane-session | Session-typed IPC, transport, framing | Active (25 tests) |
| pane-app | Actor framework, dispatch, looper | Active (20 tests) |
| pane-fs | Filesystem namespace | Active (5 tests) |
| pane-notify | Filesystem notification abstraction | Preserved from prototype |

## Design documents

| Document | Scope |
|----------|-------|
| `docs/architecture.md` | Full architecture spec |
| `docs/optics-design-brief.md` | Optics, monadic lens, ctl dispatch, pane-fs verification surface |
| `docs/optics-deliberation.md` | Background deliberation (profunctor optics, language split) |

## Session Start Checklist

Before beginning work each session:

1. Read this file — know what's current, what's next
2. Read `pane/current_state` in serena — verify it matches this file
3. Read recent git log (`git log --oneline -10`) — know what changed since last session
4. If starting a new subsystem: run the four-agent workflow (see `pane/agent_workflow` in serena)

## Session End Checklist

After completing work each session:

1. Update this file — mark completed items, add discovered work
2. Update `pane/current_state` in serena if the project state changed substantially
3. Run `cargo test` — confirm all tests pass
4. If any substantial refactor occurred: verify stale doc review was done
5. Commit this file with the session's final commit
