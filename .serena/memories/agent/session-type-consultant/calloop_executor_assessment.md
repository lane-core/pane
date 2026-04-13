---
type: analysis
status: current
supersedes: [research/async_session_bridge, agent/session-type-consultant/async_looper_assessment]
created: 2026-04-12
last_updated: 2026-04-12
importance: critical
keywords: [calloop, executor, async, par, session_types, batch_ordering, noop_waker, StreamSource, adapt_io, Rumpsteak, spawn_local, !Send, EAct, I6, S3, N1]
sources:
  - "[FH] §3.2 (E-Send, E-React, E-Suspend), §3.3 (Remark 1 session fidelity), §6 (Java NIO runtime)"
  - "[JHK24] §1 Theorem 1.2 (linearity + acyclicity => global progress)"
  - "calloop 0.14.4 src/sources/futures.rs (Executor, Scheduler, spawn_local, 1024-cap)"
  - "calloop 0.14.4 src/sources/stream.rs (StreamSource, PingWaker)"
  - "calloop 0.14.4 src/io.rs (adapt_io, Async<F>)"
  - "calloop 0.14.4 src/loop_logic.rs:296 (adapt_io signature)"
  - "par 0.3.10 exchange.rs (Send non-blocking, Recv async)"
  - "par 0.3.10 queue.rs (Dequeue::into_stream1)"
  - "crates/pane-app/src/looper.rs (dispatch_batch, event_loop.dispatch)"
verified_against:
  - "calloop-0.14.4/src/sources/futures.rs:180 (spawn_local = !Send)"
  - "calloop-0.14.4/src/sources/futures.rs:322 (1024 runnable cap)"
  - "calloop-0.14.4/src/sources/futures.rs:351 (callback(result, &mut ()))"
  - "calloop-0.14.4/src/sources/stream.rs:23-33 (PingWaker)"
  - "calloop-0.14.4/src/sources/futures.rs:46 (Rc<State<T>> = !Send)"
  - "looper.rs:497 (event_loop.dispatch before dispatch_batch)"
related: [dependency/par, decision/par_integration_architecture, decision/pane_session_mpst_foundation, architecture/looper, agent/session-type-consultant/rumpsteak_smol_translation]
agents: [session-type-consultant]
---

# Path C Revised: calloop's Built-in Async Executor — Session Type Assessment

## Verdict

Conditionally sound. Strict upgrade over sync status quo AND
previously-proposed smol/async-io Path C. All prior conditions
met with tighter guarantees. Zero new dependencies.

## Conditions

1. **Single-thread by construction.** calloop Executor is Rc-based,
   spawn_local produces !Send futures. Cannot migrate to another
   thread. I6 preserved by type system, not convention.

2. **Batch ordering preserved.** Executor runs during
   event_loop.dispatch() (event collection), not during
   dispatch_batch() (phase-ordered processing). Completed
   futures deposit results into Batch buckets via callback.
   S3 trivially maintained.

## Seven findings

### 1. par composes with calloop executor via oneshot waking

par Recv::recv() is async. Spawned on Scheduler, the future
parks until par's oneshot resolves. Oneshot resolution calls
Waker::wake() which calls Ping::ping() which wakes calloop.
Next dispatch, Executor polls future, callback fires with result.
Chain: oneshot.send() -> Waker::wake() -> Ping::ping() ->
calloop wakes -> Executor polls -> callback(T, &mut State).

### 2. Noop-waker eliminated

research/async_session_bridge Option A was a workaround for
missing waker integration. calloop executor provides real wakers
backed by Ping. The entire noop-waker approach, LocalPool option,
and smol option are unnecessary.

### 3. Subscriptions best served by StreamSource

calloop::sources::stream::StreamSource wraps futures::Stream
as EventSource. par Dequeue::into_stream1() -> DequeueStream1.
StreamSource(DequeueStream1) is a first-class calloop source.
Items delivered to Batch via callback. PostAction::Remove on
stream close. Superior to spawned for_each1 future.

### 4. Rumpsteak try_session now feasible (Path B/C)

spawn_local supports !Send futures. adapt_io wraps fds for
AsyncRead/AsyncWrite. Sink/Stream impls over Async<UnixStream>
provide Rumpsteak-compatible channels. try_session can be
spawned on Scheduler. The "pane doesn't use async at all"
diagnosis from rumpsteak_smol_translation is obsolete.

### 5. CPS callback maps to EAct E-Suspend/E-React

Executor callback = E-React (handler fires on message).
Future .await = E-Suspend (yield to event loop). calloop
executor is a concrete realization of EAct's Java NIO runtime
pattern ([FH] §6, l.4093-4094).

### 6. Executor's 1024-cap is batch-limit compatible

Executor processes at most 1024 runnables per dispatch call.
If more complete, re-pings itself. Matches pane's Phase 5
batch limit (64 msgs / 8ms). No completions lost.

### 7. Protocol sessions should be single async tasks

Entire protocol interaction (request/reply, subscription setup)
should be one spawned async task. Executor callback fires once
on session completion. Preserves par's type discipline end-to-end.
Fragmenting into per-step futures loses session-type tracking
at callback boundaries.

## Invariant impact

I6: strengthened (Rc + !Send). S3: preserved (executor fires
during collection, not dispatch). N1: unchanged. N2: unchanged.
N3: improved (catch_unwind in Executor::Drop prevents abort on
par panic-on-drop). N4: unchanged. I2/I3: unchanged (blocking
executor thread = same violation). I8: unchanged.

## What this supersedes

- research/async_session_bridge — entirely
- async_looper_assessment finding that "dispatch_batch becomes
  async fn" — wrong for calloop model; dispatch_batch stays sync
- rumpsteak_smol_translation Path A recommendation — upgraded
  to Path B/C feasibility
