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
- [x] **ControlMessage** — wire service 0 envelope with all 7 variants (Lifecycle, DeclareInterest, InterestAccepted/Declined, ServiceTeardown, RevokeInterest, Cancel); DeclineReason, TeardownReason (11 tests)
- [ ] **Display protocol** — `Display` as Protocol impl; `DisplayMessage` enum
- [x] **PeerAuth** — `PeerAuth { uid, source: AuthSource }` with `AuthSource::Kernel { pid }` (SO_PEERCRED) and `AuthSource::Certificate { subject, issuer }` (TLS); `#[non_exhaustive]`, full Eq/Hash (10 tests)
- [x] **Address** — `Address { pane_id, server_id }`, `#[non_exhaustive]`, Copy, resolved pane address for routing (13 tests)
- [x] **Handshake types** — Hello, Welcome, ServiceInterest, ServiceBinding, Rejection with RejectReason; session type `Send<Hello, Recv<Result<Welcome, Rejection>>>`; bridge roundtrip tested (3 tests in bridge)
- [ ] **DeclareInterest / InterestAccepted / InterestDeclined** — late-binding active-phase messages
- [ ] **Cancel { token }** — advisory request cancellation (Tflush equivalent)
- [ ] **ProtocolHandler derive macro** — generates `Handles<P>::receive` match from named methods

#### Session layer (pane-session)

- [x] **Transport** — `Read + Write + Send + 'static` blanket trait, `MemoryTransport::pair()` byte-level (5 tests)
- [x] **Bridge** — two-phase connect (verify_transport + par handshake via FrameCodec), `connect_and_run`/`accept_and_run` with reader loop (7 tests)
- [x] **FrameCodec** — `[length: u32 LE][service: u8][payload]`, reserved 0xFF abort, known_services bitset, max_message_size enforcement + set_max_message_size (20 tests)
- [x] **ProtocolServer** — single-threaded actor, provider index from Hello.provides, DeclareInterest handler, frame routing with session_id rewriting, connection drop cleanup with ServiceTeardown (8 unit + 3 integration tests)
- [x] **peer_cred** — SO_PEERCRED (Linux) / getpeereid + LOCAL_PEERPID (macOS) → PeerAuth (1 test)
- [ ] **Verify Chan<S,T> compatibility** — ensure session-typed channels work with new handshake types
- [ ] **SessionEnum derive** — N-ary enum branching with `#[session_tag]` wire stability

#### Kit API (pane-app)

- [x] **Dispatch\<H\>** — per-request typed dispatch entries, token uniqueness, fail_connection, cancel (6 tests)
- [x] **LooperCore\<H\>** — catch_unwind boundary, destruction sequence, `dispatch_lifecycle`, `run()` with channel-driven main loop (14 tests, including 2 vertical slice integration tests)
- [x] **PaneBuilder\<H\>** — two-phase lifecycle, open_service stub, duplicate rejection (3 tests)
- [x] **Pane** — non-generic connection identity (stub)
- [x] **Messenger** — scoped handle with Address, `address()` accessor (3 tests)
- [x] **ServiceHandle\<P\>** — Drop RevokeInterest, protocol-scoped `send_request<H,R>()` with real serialization, `send_notification()`, `with_channel()` constructor, `target_address()` (6 tests)
- [x] **ExitReason** — Graceful/Disconnected/Failed/InfraError
- [ ] **Messenger full impl** — `set_content`, `set_pulse_rate`, `post_app_message` (send_request moved to ServiceHandle)
- [ ] **ConnectionSource** — calloop EventSource for a single Connection (read + buffered write) — **bumped priority: enables real Messenger/ServiceHandle routing**
- [x] **Service registration wiring** — `PaneBuilder::connect()` + `open_service::<P>()` sends DeclareInterest through real ProtocolServer, `ServiceHandle::Drop` sends RevokeInterest, `LooperCore` dispatches service frames through `ServiceDispatch<H>` (12 new tests)
- [ ] **Eager Hello.interests** — move initial service interests into Hello, resolved during handshake via Welcome.bindings (four-agent recommendation, deferred from service registration wiring)
- [ ] **Provider-side Request dispatch** — `dispatch_service` ServiceFrame::Request needs typed message + ReplyPort delivery
- [ ] **Consumer-side Reply/Failed routing** — `dispatch_service` Reply/Failed route through Dispatch<H> by token
- [ ] **ActivePhase\<T\>** — explicit shift operator (ω_X) carrying negotiated state (max_message_size, PeerAuth, known_services)
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
- [x] **I13** (open_service blocks until accepted) — PaneBuilder::open_service blocks on mpsc channel for InterestAccepted/Declined, buffers unrelated messages
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
| pane-proto | Protocol vocabulary, no IO | Active (86 tests) |
| pane-session | Session-typed IPC, transport, framing, server | Active (45 tests) |
| pane-app | Actor framework, dispatch, looper | Active (43 tests + 10 integration) |
| pane-fs | Filesystem namespace | Active (5 tests) |
| pane-hello | First running pane app (binary) | Active (0 tests, manual verification) |
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
