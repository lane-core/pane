---
type: status
status: current
supersedes: [archive/status/2026-04-06, pane/current_state]
created: 2026-04-10
last_updated: 2026-04-12T3
importance: high
keywords: [status, crates, tests, calloop, looper, invariants, send_and_wait, watchdog, NLnet]
agents: [all]
---

# Status (2026-04-12)

## Where we are

Six crates, 371 regular tests + 48 stress/adversarial + 6 benchmarks.
All invariants verified or detection-enforced (22 of 22 + N1-N4
identified, extraction pending).

**Next session priority:** pane-session MPST foundation extraction.
Read `decision/pane_session_mpst_foundation` first.

| Crate | Role | Tests |
|---|---|---|
| pane-proto | Protocol vocabulary, no IO | 99 |
| pane-session | Session-typed IPC, transport, framing, server | 102 + 21 stress |
| pane-app | Actor framework, dispatch, looper | 162 + 12 stress + 12 adversarial + 14 integration + 6 bench |
| pane-fs | Filesystem namespace | 5 |
| pane-hello | First running pane app (binary) | 0 |
| pane-notify | Filesystem notification abstraction | preserved from prototype |

### Landed since 2026-04-12

- **C2: Failure cascade policy** — New TeardownSet obligation
  type in pane-session/teardown.rs [N3]: #[must_use], Drop panics
  in debug / logs in release if tokens unconsumed (affine-gap
  compensation matching ReplyPort pattern). drain() yields all
  tokens and marks consumed. cascade_session_failure() and
  cascade_connection_failure() on ActiveSession define the
  pane-session/pane-app boundary — currently return empty sets
  (token-to-session mapping not yet in ActiveSession). 8 TeardownSet
  tests (drain, Drop panic, len/is_empty) + 3 ActiveSession cascade
  tests. 9 new tests total (pane-session: 93 → 102). TeardownSet exported from pane-session. Phase C2 complete;
  MPST extraction Phase C done.
- **C1: ActiveSession container** (`90cb3e1`) — New type in
  pane-session/active_session.rs: post-handshake session-layer
  state container [EAct σ]. Fields: RequestCorrelator,
  revoked_sessions (HashSet<u16>, H3), primary_connection
  (PeerScope), max_message_size (u32), max_outstanding_requests
  (u16). Constructor from Welcome-negotiated params, 7 correlator
  delegation methods, 3 session lifecycle methods (revoke_session,
  is_revoked, revoked_sessions accessor), 5 field accessors.
  Standalone (Option A) — does not yet replace LooperCore fields.
  8 unit tests. Plan 9 precedent: devmnt.c Mnt. Phase C1 complete.
- **A4: NonBlockingSend trait** (`5823d0d`) — New trait in
  pane-session/send.rs codifying N1 (all looper-thread sends are
  non-blocking). Single method: try_send_frame(&self, service,
  payload) → Result<(), Backpressure>. No implementations yet —
  SharedWriter and mpsc wrapper implementations come in Phase B+C.
  Module-level docs cite [FH] E-Send §3.2, heritage from BeOS
  BDirectMessageTarget and Plan 9 mountio. Also tracked
  backpressure.rs and correlator.rs that were missing from git
  after A2+A3. Phase A now complete.
- **A1: FrameReader + FrameWriter extraction** (`7685ecf`) —
  Moved non-blocking frame codec (FrameReader, FrameWriter,
  ReadState, ReadProgress, read_into, WRITE_HIGHWATER_BYTES)
  from pane-app/connection_source.rs to pane-session/frame.rs.
  Establishes N4 (frame atomicity) as a pane-session guarantee.
  FrameReader::try_read_frame now returns FrameError (not
  ConnectionError); From<FrameError> for ConnectionError bridges
  the boundary. 16 tests moved + 1 new (non-permissive path).
  pane-session exports: FrameReader, FrameWriter,
  WRITE_HIGHWATER_BYTES. First task in MPST extraction plan.
- **D12: Non-blocking write architecture** (`bdf130e`) — Server
  per-connection writer threads (actor enqueues via try_send, never
  blocks, overflow → connection teardown). Direct FrameWriter for
  looper-thread sends (SharedWriter Rc<RefCell<FrameWriter>> shared
  between dispatch and EventSource, 18% faster than mpsc). Builder
  channel verified unbounded.
- **ConnectionSource before_sleep + write highwater** (`a33dcf5`)
  — calloop lifecycle hook flushes write channel between polls.
  VecDeque highwater cap (8 frames) enables backpressure propagation.
  Bridge wireup stress test: 200×4KB frames, publisher measurably
  stalled at 738ms (backpressure confirmed).
- **IPC benchmark suite** (`0e6cb65`) — 4 benchmarks over real
  UnixStream: notification throughput (201K msg/sec at 64B, 20×
  D-Bus), request/reply latency (20.3μs P50), fan-out (155K at
  N=50), direct write path (+18% vs mpsc).
- **Write batching + batch limit** (`9d97def`) — contiguous Vec<u8>
  write buffer replacing VecDeque<Vec<u8>> (eliminates per-frame
  allocation), 2KB watermark flush. Phase 5 batch limit (64 msgs /
  8ms). Post-optimization: 900K msg/sec direct path, 18μs P50.
- **Adversarial stress suite** (`1afaa7f`) — 12 tests: thundering
  herd (100 subs), message flood (100K to non-reader), rapid
  connect/disconnect (1000 cycles), cancel storm (500 req+cancel),
  shutdown during traffic, half-close socket, malformed frames,
  max connections (500), dispatch stall, concurrent teardown+send,
  backpressure cascade isolation, boundary messages.
- **MPST foundation decision** (`4e36122`) — pane mapped onto
  Fowler-Hu EAct formalism. N1-N4 invariants derived. pane-session
  extraction plan: NonBlockingSend, FlowControl, RequestCorrelator,
  ActiveSession, FrameReader. See `decision/pane_session_mpst_foundation`.

### Landed since 2026-04-11

- **C1: Wire vocabulary** (`bece2bb`) — CBOR handshake format
  (D11), `max_outstanding_requests` in Hello/Welcome, ciborium
  dependency, `#[serde(default)]` functional for extensibility.
- **C2: Two-function send API** (`10d84a2`) — Backpressure cap
  tracking, `send_request` / `try_send_request` (D1/D7),
  `send_notification` / `try_send_notification`, outstanding
  request counter checked against negotiated cap. New module:
  `backpressure.rs`.
- **C3: Deferred revocation** (in C4) — `RevokeInterest` hybrid
  pattern (D8): local mark + looper-batched wire send, H1/H2/H3
  invariants tested.
- **C4: ConnectionSource EventSource** (`66a6316`) — calloop
  EventSource wrapping a post-handshake UnixStream fd. Non-blocking
  FrameReader (byte-level WouldBlock state machine replacing
  FrameCodec's blocking read_exact), FrameWriter with partial
  write tracking, dynamic interest management (READ always, BOTH
  when write queue non-empty), transitional mpsc write channel
  integration. 23 new tests. New module: `connection_source.rs`.
- **C5: Handshake handoff** (`9cc5d50`) — `LooperMessage::NewConnection`
  variant, Looper-side ConnectionSource registration via
  `LoopHandle::insert_source`, oneshot ack. Bridge-side integration
  (replacing bridge threads with ConnectionSource for real
  connections) documented as follow-up.
- **C6: CancelHandle wiring** (`0553563`) — CancelHandle closure
  captures ctl channel sender (D7/D10), server-side cancel-if-present
  semantics, `SendAndWaitError::Cancelled` variant.
- **Provider-side API** (`da75432`) — SubscriberSender<P> type
  (sending-only, no lifecycle ownership), Handler callbacks
  subscriber_connected/subscriber_disconnected, batch routing
  of InterestAccepted (phase 3) and ServiceTeardown (phase 2),
  Messenger::subscriber_sender<P>() factory. New module:
  `subscriber_sender.rs`. 9 new tests.
- **Pub/sub integration tests** (`e7802b3`, `dd3a0d6`) — push
  and long-poll patterns, fan-out, churn, mixed, backpressure
  stress tests. 4 regular + 5 stress tests.
### Landed since 2026-04-06

- **Calloop Looper** (commits `bbc7026`, `a3aedff`) — six-phase
  batch ordering (Reply/Failed → ServiceTeardown → PaneExited /
  Lifecycle → ctl writes → Requests/Notifications → post-batch),
  forwarding thread from bridge, single-thread by construction.
  See `architecture/looper`.
- **Timer source** (`0bd0dab`) — TimerToken obligation handle,
  set_pulse_rate via calloop Timer, Drop cancels.
- **Heartbeat watchdog** — separate thread, configurable timeout
  (default 5s), detects I2/I3 violations after the fact. Cannot
  preempt Rust code; logs and diagnoses. (4 watchdog tests.)
- **send_and_wait** (`60567ab`) — synchronous blocking request
  from non-looper threads. ThreadId stored at `Looper::run()`,
  checked in `ServiceHandle::send_and_wait`, panics if called
  from the looper thread itself (I8 enforcement). (6 tests.)
- **Pane death notification** (`e5cd130`) — Watch / Unwatch /
  PaneExited ControlMessage variants on ProtocolServer, watch
  table on server, fire-and-forget delivery, Handler::pane_exited
  callback.
- **I9 fix** (`6e0130b`) — `catch_unwind` on Reply / Failed
  dispatch branches after formal-verifier found regression in
  destruction sequence ordering.
- **NLnet GenAI compliance** (`1b396a9`, `6a375a5`, `93f38a2`) —
  commit format, git notes, log archive, GENAI.md rewrite.

### Invariant status (19 of 19 + 3 new)

I1 (panic=unwind), I4 (typestate handles), I5 (Clone-safe Messages),
I6 (sequential single-thread dispatch), I7 (service dispatch fn
pointers sequential), I9 (dispatch cleared before handler drop),
I10/I11 (ProtocolAbort), I12 (unknown discriminant), I13
(open_service blocks): all tested.

I2 / I3 (no blocking, handlers terminate): convention,
detection-enforced via heartbeat watchdog. Cannot prevent at
compile time.

I8 (send_and_wait panics from looper thread): runtime ThreadId
check.

S1 (token uniqueness), S2 (sequential dispatch), S4
(fail_connection scoped), S5 (cancel without callbacks), S6
(panic=unwind): tested.

S3 (six-phase batch ordering): implemented in Looper, three batch
ordering tests (reply_before_teardown, lifecycle_after_teardown,
notifications_last).

H1 (Looper liveness after local mark), H2 (idempotent cleanup —
process_disconnect skips already-revoked sessions), H3 (stale
dispatch suppression via revoked_sessions set): all tested (D8
deferred revocation invariants).

Outstanding request counter: tested against negotiated cap (D9).
Cap-and-abort on overflow (send_request), Backpressure return on
try_send_request. Counter monotonic within batch phases.

## What's next

### IMMEDIATE: pane-session MPST extraction

Extract protocol-layer abstractions from pane-app to pane-session,
grounded in Fowler-Hu EAct formalism. See
`decision/pane_session_mpst_foundation` for full plan.

**Phase A — Foundation (complete):**
- [x] A1: FrameReader + FrameWriter → pane-session/frame.rs [N4]
- [x] A2: Backpressure → pane-session/backpressure.rs
- [x] A3: Token + PeerScope → pane-session
- [x] A4: NonBlockingSend trait [N1]

**Phase B — RequestCorrelator (depends on A):**
- [ ] B1: RequestCorrelator → pane-session/correlator.rs [N1+N2]

**Phase C — ActiveSession + Failure Cascade (depends on B):**
- [x] C1: ActiveSession → pane-session/active_session.rs
- [x] C2: Failure cascade policy [N3]

Three adversarial bugs (bidirectional deadlock, partial-frame hang,
HoL blocking) become impossible by construction through N1-N4.

### Phase 1 — Core (in progress)

**ConnectionSource C1-C6 landed.** ConnectionSource exists as a
calloop EventSource (C4). Looper-side registration works (C5).
Bridge-side integration — replacing bridge reader/writer threads
with ConnectionSource for real connections — is the remaining
integration task before ConnectionSource is fully operational.

Other Phase 1:

- Display protocol + DisplayMessage enum
- DeclareInterest / InterestAccepted / InterestDeclined late-binding
- Cancel { token } (Tflush equivalent)
- ProtocolHandler derive macro
- Verify Chan<S,T> compatibility with new handshake types
- SessionEnum derive (N-ary enum branching)
- Messenger full impl (set_content, post_app_message, watch /
  unwatch wire send)
- Eager Hello.interests
- ActivePhase<T> (explicit ω_X shift operator)
- AppPayload marker trait
- pane-fs: snapshot synchronization (ArcSwap), PaneNode trait, ctl
  parsing module, FUSE integration, `#[derive(Scriptable)]` macro
- pane-server: service-aware routing, per-service wire dispatch,
  pane_owned_by()
- pane-headless: calloop event loop, unix listener, handshake

### Phase 2 — Distribution

Connections N>1, TLS + PeerAuth::Certificate, service map full
precedence chain, version range negotiation.

### Phase 3 — Lifecycle

Session suspension / resumption, streaming (Queue pattern),
Handles<Routing>.

## Known open questions

- **pane-session MPST foundation** — the highest priority open
  item. Adversarial testing exposed that session type discipline
  drops after handshake. N1-N4 invariants identified but not yet
  extracted to pane-session. See `decision/pane_session_mpst_foundation`.
- **Server version/msize validation** — server.rs doesn't validate
  Hello.version or enforce max_message_size reduction. Exposed by
  Plan9 heritage test audit (T6, T8). Real protocol bugs.
- **Three adversarial bugs** — bidirectional buffer deadlock,
  partial-frame hang on server reader, HoL during queue fill.
  All addressed by N1-N4 extraction. Tests currently work around
  them; the extraction makes them impossible by construction.
- **Haiku-inspired test suite** — 86 tests proposed, none
  implemented. Sources:
  - `agent/be-systems-engineer/haiku_test_audit` — 25 Haiku ports
    across 6 tiers, with priority mapping and Haiku source citations
  - `agent/session-type-consultant/test_extension_analysis` — 32
    novel tests across 7 areas (obligation handles, two-function
    split, phase ordering, D8, D11, Cancel, provider API)
  - `analysis/plan9_test_heritage` — 24 Plan9-heritage tests across
    5 areas (Tflush races, Tversion negotiation, fid binding, 
    freefidpool cleanup, open/walk rejection). T20 (SelfProvide)
    and T8 (VersionMismatch) are highest priority — they test
    implemented code at zero coverage and expose real validation gaps.
  - optics-theorist — 5 optic law tests (PutPut, GetPut, PutGet,
    AffineTraversal, Traversal identity)
  Priority: T20/T8 (zero-coverage code paths), then Tier 1 Haiku
  ports (lifecycle + watch), then obligation handle tests.

- **Notification-triggers-request** — a notification handler
  cannot send requests (no DispatchCtx access). Deferred to
  Phase 2 via self-messaging or Messenger carrying dispatch
  context. Roundtable confirmed no ratchet, EAct E-Self is not
  a formal rule, safety theorems hold without it.
- **Messenger watch / unwatch wire send** — stub methods exist,
  need write_tx on Messenger.
- **Agda formalization** — four properties identified (ReplyPort
  exactly-once, Dispatch one-shot, destruction sequence ordering,
  install-before-wire). Deferred until architecture stabilizes.

## Dev workflow

```
cargo test --workspace          # 349 regular tests
cargo test -- --ignored         # 36 stress tests
cargo test --test bench_ipc --release -- --ignored --nocapture bench_all  # benchmarks
cargo fmt
cargo clippy --workspace        # zero warnings
cargo run -p pane-hello         # canonical app
```
