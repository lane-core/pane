---
type: architecture
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [looper, calloop, six-phase, batch, watchdog, send_and_wait, I8, S3, dispatch, ThreadId]
related: [status, analysis/eact/_hub, analysis/session_types/principles]
agents: [pane-architect, formal-verifier, be-systems-engineer, session-type-consultant]
---

# Architecture: Calloop Looper

## Summary

The Looper is pane-app's per-pane event loop. It owns the dispatch
thread for a single pane, processes incoming frames in strict
six-phase batch order, and serializes all handler invocations on
one thread by construction (I6, S2). Backed by calloop's
EventLoop. Replaced the prototype's hand-rolled event loop in
session 4.

The looper IS a BLooper in the BeOS sense: one thread, sequential
dispatch, owns its handlers, panic = unwind = Drop. The calloop
substrate is an implementation detail; the BeOS semantics are the
contract.

## Components

### LooperCore<H>

The dispatch logic. Owns Dispatch<H>. Services the request/reply
protocol (Reply, Failed routed by token through `fire_reply` /
`fire_failed`), handles ServiceTeardown and PaneExited, calls user
handlers via `Handles<P>::receive`, dispatches lifecycle methods on
Handler. `catch_unwind` boundary on every dispatch_* method (I1).

dispatch_service was factored into targeted methods after session 3
review: dispatch_reply, dispatch_failed, dispatch_request,
dispatch_notification, dispatch_teardown, dispatch_pane_exited.
Each is independently testable. Bug surface: each adds its own
catch_unwind. The session 3 I9 regression (commit `6e0130b`) was a
missing catch_unwind on Reply/Failed branches; the factoring made
the fix surgical.

### Six-phase batch ordering (S3)

Per dispatch tick, phases run in order, each draining its queue
before the next:

1. **Reply / Failed** — drain pending replies and failures
   (consumer-side request/reply routing through Dispatch by token)
2. **ServiceTeardown** — server-initiated service drops, clean up
   DeclareInterest entries
3. **PaneExited / Lifecycle** — death notification from watched
   panes, lifecycle transitions
4. **ctl writes** (stub) — pane-fs ctl dispatch when wired
5. **Requests / Notifications** — incoming service traffic
6. **post-batch** (stub) — placeholder for batch coalescing
   optimizations (Phase 4)

The order is load-bearing:

- Reply / Failed must drain before ServiceTeardown (a teardown
  invalidates pending replies; if teardown ran first, the reply
  would arrive at a torn-down dispatch and be dropped silently).
- ServiceTeardown before PaneExited (a pane death implies all its
  services are torn down first; running PaneExited first leaves
  orphaned ServiceTeardowns to process against a dead pane).
- ctl writes before request processing (state mutations must be
  visible to dispatched requests).

Tested: `batch_ordering_reply_before_teardown`,
`batch_ordering_lifecycle_after_teardown`,
`batch_ordering_notifications_last`.

### Forwarding thread

The bridge's reader loop spawns a thread that forwards transport
frames into the Looper's input channel. The Looper itself runs on
the calloop event loop in the main thread of its pane. This is the
only place pane-app uses an extra OS thread. The forwarding thread
is unconditional — calloop has no async I/O for arbitrary
file-descriptor-backed transports, so the forwarding is the
adapter.

### TimerToken / set_pulse_rate

Calloop Timer source backs `Messenger::set_pulse_rate`. TimerToken
is the obligation handle: Drop cancels the timer. One outstanding
timer per token. (3 TimerToken tests.)

### Heartbeat watchdog

Separate thread. Sends a heartbeat to the looper every 100ms,
expects an ack within configurable timeout (default 5s). If the
looper thread doesn't ack, logs and diagnoses. Detects I2 (no
blocking in handlers) and I3 (handlers terminate) violations
**after the fact**. Cannot preempt Rust code — this is detection,
not prevention. The verifier's reasoning: I2 and I3 are
conventions enforceable only at runtime; the watchdog converts
silent stalls into observable signals. (4 watchdog tests.)

### send_and_wait

Synchronous blocking request from non-looper threads. Two pieces:

1. ThreadId stored at `Looper::run()` entry (the looper's own
   thread).
2. `ServiceHandle::send_and_wait` checks the calling ThreadId
   against the stored looper ThreadId. **Panics** if they match
   (I8 enforcement: a looper-thread send_and_wait would deadlock
   itself waiting for a reply that only its own dispatch loop can
   produce).

Implementation: oneshot reply channel, `SendAndWaitError` for
non-panic failures (e.g., service not registered, send queue
full). (6 send_and_wait tests including the I8 panic test.)

This is the one place where the looper's threading model is
explicitly observable to user code. Every other dispatch path
goes through async ReplyPort.

## Invariants

| Invariant | Mechanism |
|---|---|
| I1 (panic=unwind, Drop fires) | catch_unwind on every dispatch_* method; Drop on obligation handles |
| I2 (no blocking in handlers) | Convention; detection via heartbeat watchdog |
| I3 (handlers terminate) | Same mechanism as I2 |
| I6 (sequential single-thread dispatch) | By construction: one calloop EventLoop, one stored ThreadId at run() |
| I7 (service dispatch fn pointers sequential) | ServiceDispatch and RequestReceiver closures called within dispatch_service, same thread as I6 |
| I8 (send_and_wait panics from looper thread) | ThreadId check in ServiceHandle::send_and_wait |
| I9 (dispatch cleared before handler drop) | destruction_sequence_ordering test; catch_unwind on Reply/Failed branches |
| S2 (sequential dispatch) | Follows from I6 |
| S3 (six-phase batch ordering) | Implemented in Looper; three batch ordering tests |

## See also

- `status` — current state, test counts, what's next
- `analysis/eact/_hub` (when written) — EAct invariant analysis
  context, gap-by-gap resolutions
- `analysis/session_types/principles` — C1–C6 principles the
  looper enforces at dispatch
- pane-app source: `crates/pane-app/src/looper/`
- Recent commits: `bbc7026` (calloop substrate), `0bd0dab` (timer),
  `60567ab` (send_and_wait + I8), `a3aedff` (six-phase + watchdog
  + session-end updates), `e5cd130` (death notification),
  `6e0130b` (I9 fix)
