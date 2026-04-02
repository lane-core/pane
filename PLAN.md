# Plan

Current implementation roadmap. This is a living document — update it when tasks complete, priorities change, or new work is identified.

**Rule:** At the end of every task, update this file. Mark completed items, add discovered work, adjust priorities. If this file is stale, the process broke, and we must immediately consult the user for clarification before proceeding further.

**Source of truth:** `docs/architecture.md` is the design spec. This file tracks execution against that spec.

## Now

### Phase 1 — Core

Single server (N=1), headless, no suspension, no streaming. All multi-server data structures present with one entry (functoriality principle — Phase 2 adds entries, not structure).

**Prerequisite for each item:** consult Be engineer on how Be/Haiku implemented the equivalent (see docs/workflow.md).

#### Protocol foundation (pane-proto)

- [ ] **ServiceId** — UUID + reverse-DNS name, `ServiceId::new()` with UUIDv5 derivation
- [ ] **Protocol trait** — `SERVICE_ID: ServiceId` + `type Message: Send + 'static`
- [ ] **Framework protocols** — `Lifecycle`, `Display` as Protocol impls; `ControlMessage` enum (wire service 0)
- [ ] **ClientToServer / ServerToClient** — headless-first naming, per-service wire framing `[length][service][payload]`
- [ ] **PeerAuth** — `Kernel { uid, pid }` (SO_PEERCRED) and `Certificate { subject, issuer }` (TLS) variants
- [ ] **Handshake types** — Hello with `interests: Vec<ServiceInterest>`, Welcome with `bindings: Vec<ServiceBinding>`, `max_message_size` negotiation
- [ ] **DeclareInterest / InterestAccepted / InterestDeclined** — late-binding active-phase messages
- [ ] **Cancel { token }** — advisory request cancellation (Tflush equivalent)

#### Session layer (pane-session)

- [ ] **Verify Chan<S,T> compatibility** — ensure session-typed channels work with new handshake types
- [ ] **ProtocolAbort** — Chan Drop sends `[0xFF][0xFF]`, peer frees session thread immediately
- [ ] **SessionEnum derive** — N-ary enum branching with `#[session_tag]` wire stability

#### Kit API (pane-app)

- [ ] **Message split** — Clone-safe `Message` enum (value events) + internal obligation types (ReplyPort, ClipboardWriteLock, CompletionReplyPort)
- [ ] **Handler trait** — ~11 lifecycle + messaging methods, headless-complete
- [ ] **DisplayHandler trait** — ~10 display methods, extends Handler
- [ ] **Handles\<P\> trait** — `fn receive(&mut self, proxy: &Messenger, msg: P::Message) -> Result<Flow>`
- [ ] **ProtocolHandler derive macro** — generates `Handles<P>::receive` match from named methods; rustc exhaustive match IS the guarantee
- [ ] **Flow** — `Continue` / `Stop`, orthogonal to `Result`
- [ ] **Messenger** — scoped `Handle` + `ServiceRouter` (HashMap, 1 entry in Phase 1)
- [ ] **ConnectionSource** — calloop EventSource for a single Connection (read + buffered write), replaces pump threads
- [ ] **Dispatch\<H\>** — per-request typed dispatch entries for request/reply; `send_request<H, R>` with typed callbacks, `CancelHandle`
- [ ] **AppPayload** — `Clone + Send + 'static` marker trait, compile-time exclusion of obligation handles
- [ ] **Filter chain** — `MessageFilter` on Clone-safe `Message` only; `FilterAction::Pass/Transform/Consume`
- [ ] **Service registration** — `open_clipboard()` etc. resolves capability → Connection → DeclareInterest → typed calloop source
- [ ] **Looper** — calloop-backed, per-protocol typed channels, unified batch, coalescing

#### Server (pane-server)

- [ ] **ProtocolServer** — service-aware routing via ServiceRouter per client
- [ ] **Per-service wire dispatch** — demux by service discriminant, per-service error isolation (`ServiceTeardown`)
- [ ] **pane_owned_by()** — PeerAuth-based ownership check on every operation

#### Headless binary (pane-headless)

- [ ] **pane-headless** — calloop event loop, unix listener, handshake with new protocol types

#### Invariants to validate

All of I1–I9 and S1–S6 from the architecture spec. Phase 1 is the proof that the linear discipline works end-to-end.

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

Session suspension/resumption, streaming (Queue pattern), RoutingHandler. These interact — streams must close before suspend — so they're designed together.

- [ ] **Session suspension/resumption** — serializable token, re-declare interests on resume
- [ ] **Streaming** — `StreamSend<T, S>` / `StreamRecv<T, S>`, backpressure via write buffer high-water mark
- [ ] **RoutingHandler** — headless command surface via DeclareInterest

### Phase 4 — Performance

Correctness before performance.

- [ ] **Batch coalescing optimizations**
- [ ] **Write buffer tuning**
- [ ] **Connection pooling**

### Compositor

Orthogonal to protocol phases — can proceed in parallel once Phase 1 server exists.

- [ ] **pane-comp** — smithay/winit backend providing the Display view
- [ ] **Rendering** — compositor draws pane chrome (title bar from Tag), body area receives client content
- [ ] **Input routing** — keyboard/mouse events → DisplayHandler methods
- [ ] **Multi-pane layout** — tiling, splits, focus tracking

### Applications

- [ ] **pane-hello** — canonical first app, closure form
- [ ] **pane-shell** — VT parser, PTY bridge, screen buffer

## Crates preserved from prototype

These crates survive the redesign with minimal or no changes:

- **pane-session** — session-typed channels, transport abstraction, calloop integration
- **pane-optic** — composable optic types, law tests
- **pane-notify** — filesystem notification abstraction

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
