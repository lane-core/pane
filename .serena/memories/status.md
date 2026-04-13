---
type: status
status: current
supersedes: [archive/status/2026-04-06, pane/current_state]
created: 2026-04-10
last_updated: 2026-04-12
importance: high
keywords: [status, crates, tests, calloop, looper, invariants, pane-kernel, pane-router]
agents: [all]
---

# Status (2026-04-12)

## Where we are

Six crates, 401 regular tests + 48 stress/adversarial + 6 benchmarks.
All invariants verified or detection-enforced (22 of 22 + N1-N4).

**Async migration complete** (5 phases). ctx.schedule() live.
pane-kernel and pane-router designed, not yet implemented.

| Crate | Role | Tests |
|---|---|---|
| pane-proto | Protocol vocabulary, no IO | 99 |
| pane-session | Session-typed IPC, transport, framing, server | 103 + 21 stress |
| pane-app | Actor framework, dispatch, looper | 178 + 12 stress + 12 adversarial + 14 integration + 6 bench |
| pane-fs | Filesystem namespace | 5 |
| pane-hello | First running pane app (binary) | 0 |
| pane-notify | Filesystem notification abstraction | preserved from prototype |
| pane-kernel | System interface (DESIGNED) | — |
| pane-router | Signal-flow policy (DESIGNED) | — |

## Recent work (one-line per item, details in git log)

- `76a4a32` Phase 5: ctx.schedule() — handler async composition API
- `ca88c0d` Phase 4: ReplyFuture/ReplySender/AsyncDispatchEntry
- `b772314` Phase 3: StreamSource for par Dequeue (subscriptions)
- `7aa311c` Phase 2: Executor/Scheduler in Looper
- `ade0564` Phase 1: calloop executor + futures-io features
- `11772de` N1 integration: NonBlockingSend for SharedWriter
- `90cb3e1` C1: ActiveSession container
- C2: TeardownSet + failure cascade policy [N3]
- `5823d0d` A4: NonBlockingSend trait
- `7685ecf` A1: FrameReader/FrameWriter extraction to pane-session
- `bdf130e` D12: Non-blocking write architecture (SharedWriter)
- `a33dcf5` ConnectionSource before_sleep + write highwater
- `0e6cb65` IPC benchmark suite (201K msg/sec, 20μs P50)
- `9d97def` Write batching + batch limit (900K msg/sec direct)
- `1afaa7f` Adversarial stress suite (12 tests)
- `4e36122` MPST foundation decision (EAct formalism, N1-N4)
- ConnectionSource C1-C6 landed (calloop EventSource, handshake handoff, cancel wiring)
- Provider-side pub/sub API (SubscriberSender, subscriber_connected)
- Calloop Looper, timer source, heartbeat watchdog, send_and_wait, pane death notification

Designs (not yet implemented):
- pane-kernel: exokernel system interface. See `architecture/kernel`, `decision/kernel_naming`.
- pane-router: signal-flow policy. See `architecture/router`.

## Invariants

All 22 tested or detection-enforced. See code for specifics.

**Compile-time:** I1 (panic=unwind), I4 (typestate handles), I5
(Clone-safe), I6 (single-thread dispatch — strengthened by !Send
executor), I7 (fn pointer sequential), I9 (clear before drop),
I10/I11 (ProtocolAbort), I12 (unknown discriminant), I13
(open_service blocks).

**Runtime-enforced:** I2/I3 (heartbeat watchdog), I8 (ThreadId check).

**Session:** S1-S6 tested. S3 (six-phase batch ordering) in Looper.

**Deferred revocation:** H1/H2/H3 tested.

**Request cap:** D9 tested (cap-and-abort, Backpressure return).

**Router (designed, not implemented):** R-I1, R-I2, R-I3.

## What's next

### MPST extraction (remaining)

- [ ] B1: RequestCorrelator → pane-session/correlator.rs [N1+N2]

Phases A and C complete. B1 is the last extraction task. Three
adversarial bugs become impossible by construction through N1-N4.

### Phase 1 — Core

ConnectionSource C1-C6 landed. Bridge-side integration remaining.

Other Phase 1 items: Display protocol, DeclareInterest late-binding,
Cancel wire message, ProtocolHandler derive macro, Messenger full
impl, pane-fs (FUSE, PaneNode, ctl, Scriptable derive),
pane-kernel impl, pane-router impl, pane-server, pane-headless.

### Phase 2 — Distribution

Connections N>1, TLS, service map precedence, version negotiation.

### Phase 3 — Lifecycle

Session suspension/resumption, streaming (Queue), Handles<Routing>.

## Known open questions

- **B1 extraction** — highest priority. Session type discipline
  drops after handshake. See `decision/pane_session_mpst_foundation`.
- **Server validation gaps** — Hello.version and max_message_size
  not validated (T6, T8). Real protocol bugs.
- **Three adversarial bugs** — bidirectional deadlock, partial-frame
  hang, HoL blocking. Addressed by N1-N4 extraction.
- **86 proposed tests, none implemented** — sources:
  `agent/be-systems-engineer/haiku_test_audit` (25),
  `agent/session-type-consultant/test_extension_analysis` (32),
  `analysis/plan9_test_heritage` (24), optics-theorist (5).
  Priority: T20/T8 first (zero-coverage paths).
- **Notification-triggers-request** — deferred to Phase 2.
- **Messenger watch/unwatch** — stubs exist, need write_tx.
- **Agda formalization** — deferred until architecture stabilizes.

## Dev workflow

```
cargo test --workspace          # 401 regular tests
cargo test -- --ignored         # 48 stress/adversarial tests
cargo test --test bench_ipc --release -- --ignored --nocapture bench_all  # benchmarks
cargo fmt
cargo clippy --workspace        # zero warnings
cargo run -p pane-hello         # canonical app
```
