---
type: decision
status: decided
created: 2026-04-12
last_updated: 2026-04-12
importance: critical
keywords: [async, calloop, executor, Scheduler, StreamSource, adapt_io, par, Rumpsteak, Smithay, EAct, spawn_local, futures, Path_C]
related: [decision/par_integration_architecture, dependency/par, decision/pane_session_mpst_foundation, research/async_session_bridge, agent/session-type-consultant/rumpsteak_smol_translation]
supersedes: [research/async_session_bridge]
agents: [session-type-consultant, optics-theorist, plan9-systems-engineer, be-systems-engineer]
---

# Async Migration: calloop's Built-in Executor

Decided 2026-04-12 after two-round four-agent consultation.
Lane directed investigation into principled async architecture
for pane, motivated by: (1) par's async Recv impedance mismatch,
(2) Smithay compositor integration (Smithay uses calloop),
(3) theoretical alignment with EAct's operational semantics.

## The Decision

Enable calloop 0.14's `executor` + `futures-io` features on the
existing calloop dependency. Zero new crate dependencies.
calloop's built-in async executor becomes pane's async runtime.

calloop's executor is `!Send` by construction (Rc<State<T>>,
spawn_local via async_task). I6 (sequential single-thread
dispatch) becomes a type-system guarantee, not convention.

## Architecture

```
event_loop.dispatch()           ← calloop polls ALL EventSources
  ├── ConnectionSource fires    ← socket bytes → FrameReader → Batch
  ├── Channel source fires      ← bridge messages → Batch
  ├── Timer source fires        ← timers → Batch
  ├── Executor fires            ← completed par futures → Batch
  └── StreamSource fires        ← par Dequeue items → Batch

dispatch_batch(&mut state)      ← synchronous, sequential phases 0-5
```

dispatch_batch STAYS SYNCHRONOUS. Executor fires during event
collection (calloop's poll cycle), not during batch dispatch.
S3 (six-phase ordering) preserved trivially.

## Handler API: sync with async escape hatch

Handlers stay synchronous:
```rust
fn receive(&mut self, msg: P::Message, ctx: &mut DispatchCtx) -> Flow
```

Composition case uses explicit scheduling:
```rust
fn receive(&mut self, msg: P::Message, ctx: &mut DispatchCtx) -> Flow {
    ctx.schedule(async move {
        let reply = some_async_op().await;
        // ...
    });
    Flow::Continue
}
```

Rationale (Be): "Futures compose external operations; handlers
own internal state mutations." Make common case easy (90% of
handlers never need async), make composition explicitly opt-in.

Categorical description (Optics): Set + Kl(Fut) coproduct,
normalized via monad unit η. Sync handlers embed trivially.
Queue-and-wait per handler preserves I6.

## NonBlockingSend stays fn (not async fn)

N1 is structural — send path must not yield. Executor is
receive-side only. NonBlockingSend::try_send_frame remains
a synchronous function. In async context, this constrains
sends to "no yield point during effect emission."

All four agents unanimous on this.

## par Integration Points (direct runtime use)

### Subscriptions: StreamSource wrapping par Dequeue

```rust
let stream = dequeue.into_stream1();
let source = StreamSource::new(stream)?;
handle.insert_source(source, |event, _, state| {
    match event {
        Some(msg) => { /* serialize, write to wire */ }
        None => { /* stream closed, cleanup */ }
    }
});
```

StreamSource wraps futures::Stream as calloop EventSource with
real Waker backed by PingWaker. When Enqueue::push resolves
the internal oneshot, StreamSource fires, callback delivers
item to Batch. First-class event source, proper lifecycle.

Eliminates noop-waker hack entirely.

### Handshake: adapt_io + executor

```rust
let async_stream = handle.adapt_io(unix_stream)?;
scheduler.schedule(async move {
    // par handshake as async future
    let hello_bytes = /* serialize Hello */;
    async_stream.write_all(&hello_bytes).await?;
    let welcome = /* read Welcome from async_stream */;
    // deliver Welcome via callback
})?;
```

Eliminates blocking bridge thread per connection. Sequential
two-message exchange is natural async fit.

### Reply obligations: par Send in executor futures

Request/reply futures spawned on Scheduler. par's Recv resolves
when wire reply arrives and correlator calls Send::send1().
Waker chain: oneshot.send() → Waker::wake() → Ping::ping() →
calloop wakes → Executor polls → future advances → callback
delivers result to Batch.

## What Stays Synchronous (do NOT asyncify)

### ConnectionSource + FrameReader

Plan9: "ConnectionSource IS mountmux. Tight loop, small state,
900K msg/sec. Don't fragment it." FrameReader's 4-state machine
is too small to justify async. Splitting read+write into async
futures loses batch-read efficiency and fragments interest
management.

### Batch dispatch

dispatch_batch stays fn, not async fn. Sequential phase
processing. Executor fires during collection, not dispatch.

### NonBlockingSend / SharedWriter

Send path is synchronous by N1 invariant. SharedWriter::enqueue
is Vec append — always non-blocking, no async needed.

## Smithay Compositor Integration

Smithay uses calloop 0.14 (same version as pane). pane's
EventSources sit alongside Smithay's (Wayland display, DRM,
libinput) in one calloop loop. No separate threads needed.

Be: "One loop, better than separate threads. What Be needed
threads for, calloop provides cooperatively. Eliminate cross-
thread sync. Batch limit + cooperative yielding replaces
preemptive isolation."

niri (production Smithay compositor) already uses calloop's
executor for async D-Bus and IPC operations. Proven pattern.

## Theoretical Grounding

calloop's executor is a concrete realization of EAct's Java NIO
runtime pattern ([FH] §6, l.4093-4094):
- Future .await = E-Suspend (yield to event loop)
- Executor callback = E-React (event loop fires handler)
- par Send = E-Send (non-blocking, synchronous)

The theory stack becomes executable:
- EAct (Fowler-Hu) → pane's actor model
- par → session types for handshake, subscriptions, replies
- calloop executor → runtime bridging par's async to dispatch
- Rumpsteak → MPST verification + runtime (Sink/Stream over
  adapt_io, try_session on Scheduler)

## Invariant Impact

| Invariant | Impact |
|---|---|
| I6 (sequential dispatch) | Strengthened. !Send executor, Rc-based |
| S3 (batch ordering) | Preserved. Executor fires during collection |
| N1 (non-blocking sends) | Unchanged. Send path stays synchronous |
| N2 (per-session credits) | Unchanged. Runtime invariant |
| N3 (failure cascade) | Improved. catch_unwind in Executor::Drop |
| N4 (frame atomicity) | Unchanged. Below executor layer |

## Supersedes

- research/async_session_bridge — noop-waker, LocalPool, smol
  options all unnecessary
- "pane doesn't use async" diagnosis — closed by calloop
  features already in dep tree
- Rumpsteak Path A (verification only) — upgraded to Path B/C
  feasibility

## Migration Phases

Phase 1: Enable calloop executor + futures-io features.
Phase 2: Async handshake (eliminate bridge threads).
Phase 3: StreamSource for par Dequeue (subscriptions).
Phase 4: Executor-driven par Recv (request/reply futures).
Phase 5: ctx.schedule() API for handler composition.

Each phase is a natural resting point per
policy/intermediate_state_principle.

## Provenance

Lane identified that ad-hoc types in pane-session (NonBlockingSend,
RequestCorrelator, ActiveSession, TeardownSet) were not built on
par as explicitly directed. Lane directed comprehensive overhaul
using par at every opportunity. Four-agent roundtable found par's
async Recv doesn't compose with sync calloop without bridging.
Smithay research revealed calloop has built-in executor (same
calloop version pane uses). Second roundtable assessed revised
Path C with calloop executor — unanimous approval. Session-type
agent: "strict upgrade on every session-type axis." Plan9:
"scoped async for handshake and par bridge, sync for
ConnectionSource." Be: "sync handlers with ctx.schedule() escape
hatch." Optics: "NonBlockingSend stays fn, mixed dispatch is
Set + Kl(Fut) coproduct."
