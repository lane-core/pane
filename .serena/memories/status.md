---
type: status
status: current
supersedes: [archive/status/2026-04-06, pane/current_state]
created: 2026-04-10
last_updated: 2026-04-11
importance: high
keywords: [status, crates, tests, calloop, looper, invariants, send_and_wait, watchdog, NLnet]
agents: [all]
---

# Status (2026-04-10)

## Where we are

Six crates, 251 regular tests + 28 stress + 5 integration. All
invariants verified or detection-enforced (19 of 19).

| Crate | Role | Tests |
|---|---|---|
| pane-proto | Protocol vocabulary, no IO | 99 |

### Landed since 2026-04-10

- **Provider-side API** (`da75432`) — SubscriberSender<P> type
  (sending-only, no lifecycle ownership), Handler callbacks
  subscriber_connected/subscriber_disconnected, batch routing
  of InterestAccepted (phase 3) and ServiceTeardown (phase 2),
  Messenger::subscriber_sender<P>() factory. 9 new tests.
| pane-session | Session-typed IPC, transport, framing, server | 51 + 21 stress |
| pane-app | Actor framework, dispatch, looper | 96 + 7 stress + 5 integration |
| pane-fs | Filesystem namespace | 5 |
| pane-hello | First running pane app (binary) | 0 |
| pane-notify | Filesystem notification abstraction | preserved from prototype |

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

### Invariant status (19 of 19)

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

## What's next

### Phase 1 — Core (in progress)

Highest priority: **ConnectionSource** — calloop EventSource for a
single Connection, enables real Messenger / ServiceHandle routing.

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
cargo test --workspace          # 246 regular tests
cargo test -- --ignored         # 28 stress tests
cargo fmt
cargo clippy --workspace        # zero warnings
cargo run -p pane-hello         # canonical app
```
