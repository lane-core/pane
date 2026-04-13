---
type: analysis
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: critical
keywords: [async, sync, bridge, calloop, par, Dequeue, Enqueue, LocalPool, noop_waker, poll_recv, backpressure, credit, session_types, actor, event_loop]
sources:
  - "[FH] §6 Implementation — Java NIO event loops for EAct actors"
  - "[JHK24] §2 LinearActris — recv is busy-loop, send is non-blocking; §5-6 connectivity graph acyclicity"
  - "[Rumpsteak] Cutner/Yoshida/Vassor — Tokio-based async executor for MPST endpoints"
  - "[AGP] Scalas/Barbanera/Yoshida — asynchronous global protocols, queue types for buffered messages"
  - "[LI-AMC] Atkey et al. — asynchronous forwarders as typed queue processes"
  - "[Actris] Hinrichsen et al. — channel = pair of lock-protected buffers, recv = busy-loop"
  - "par 0.3.10 exchange.rs — Recv::poll_recv(cx) returns Result<(T,S), Self>; oneshot::Receiver::try_recv() exists but inaccessible through par's API"
  - "futures-executor 0.3.32 LocalPool — try_run_one(), run_until_stalled(), already in dep tree via par → futures"
  - "futures-task 0.3.32 noop_waker() — zero-cost waker for polling ready futures"
verified_against:
  - "par-0.3.10/src/exchange.rs:135 (poll_recv signature)"
  - "par-0.3.10/src/queue.rs:104 (Dequeue::pop = deq.recv1().await)"
  - "futures-channel-0.3.32/src/oneshot.rs:450 (try_recv method)"
  - "futures-executor-0.3.32/src/local_pool.rs:202 (try_run_one)"
  - "eventactors-extended.tex:4093-4094 (Java NIO runtime)"
  - "iris-actris/actris/channel/channel.v:62-67 (recv = busy-loop)"
related: [dependency/par, decision/server_actor_model, architecture/looper, agent/session-type-consultant/eact_mpst_pane_session_analysis]
agents: [session-type-consultant]
---

# Async Session Bridge — Literature Survey + Architecture Recommendation

## 1. What the literature says

**No paper addresses calloop/mio/epoll integration with session-typed channels.** This is a gap — the closest results are:

### 1a. EAct [FH] §6
Uses Java NIO (non-blocking I/O) to run actor event loops. The runtime generates CFSM state types; non-blocking states (sends, suspends) execute directly; blocking states (receives) install a handler and yield to the event loop. The event loop calls the handler when a message arrives. **This is exactly pane's model** — the gap is that EAct's implementation is Scala/JVM where NIO channels integrate natively with the event loop selector, whereas pane must bridge par's futures-based oneshot channels into calloop's fd-based poll loop.

### 1b. LinearActris / DLfActRiS [JHK24] §2
ChanLang's `recv` is a **busy-loop**: spin on try_recv until data arrives (iris-actris/actris/channel/channel.v:62-67). `send` is non-blocking (append to lock-protected buffer). This is a theoretical model (Coq mechanization), not a practical runtime. Key insight: the paper models recv as blocking precisely because the global progress theorem (Theorem 1.2) requires that blocked threads eventually unblock. For pane, the equivalent is: if a Dequeue::pop is pending, the system must guarantee the Enqueue::push will eventually happen, or the subscription must be cleanly closed.

### 1c. Rumpsteak (Cutner/Yoshida/Vassor)
Uses **Tokio** as the async runtime. Session type endpoints are async — send/recv are async methods driven by Tokio's executor. The paper doesn't discuss bridging async endpoints into synchronous event loops; it assumes a full async runtime is available. Case study (HTTP cache) integrates with Hyper, Fred, all Tokio-based.

### 1d. Asynchronous Global Protocols [AGP]
Models asynchronous communication via **queue types** (FIFO buffers between participants). Proves that balanced global protocols have bounded en-route transmissions (Lemma 5.11). Does not address runtime integration — this is purely a type-theoretic result about when async reordering preserves safety. Relevant to pane: the balancedness condition is automatically satisfied by pane's star topology (server forwards, no reordering).

### 1e. Logical Interpretation of Async Multiparty Compatibility
Models forwarders as typed queue processes. Queues store boxed messages (in-transit). Forwarders can input arbitrarily many messages without blocking output. This is the theoretical model for pane's ProtocolServer as a forwarder — but says nothing about runtime integration.

### 1f. Dependent Session Types [TCP11]
No content on backpressure, bounded queues, or flow control. The paper is about type-level dependence on message values, not resource bounding.

**Summary: the async-session-type literature assumes either a full async runtime (Rumpsteak/Tokio), a native NIO event loop (EAct/Java), or a theoretical busy-loop (LinearActris/Coq). No paper addresses the specific problem of bridging oneshot-based session channels into a synchronous poll-based event loop. This is novel engineering, not novel theory.**

## 2. Backpressure in session type literature

**No established encoding for bounded queues exists in the session type papers surveyed.** par's Enqueue::push is always non-blocking and unbounded. The papers treat channels as unbounded FIFO queues.

The closest theoretical result is [AGP] Lemma 5.11 (en-route transmissions are bounded for balanced protocols), but this bounds the number of in-flight messages by the protocol structure, not by explicit flow control.

A credit-based encoding IS expressible in par:
```
type Backpressured<T> = Recv<Credit, Send<T, Backpressured<T>>>;
```
But this is ad-hoc — no paper provides soundness guarantees for it, and it changes the protocol shape fundamentally. pane's existing backpressure (outstanding request counter, write channel capacity) is more practical.

**Recommendation:** Do not encode backpressure in par's type system. Keep N2 (per-session credit tracking) as a pane-session runtime invariant, which is what the eact_mpst analysis already specifies.

## 3. Bridging architecture recommendation

### The problem precisely stated

par's `Dequeue<T>::pop()` returns `impl Future`. The future resolves when `Enqueue::push()` deposits a value into the underlying `futures::channel::oneshot`. pane's looper runs calloop (epoll/kqueue-based). calloop has no native async executor — it polls fds and fires callbacks synchronously.

The subscription pattern: Handler pushes `P::Message` via `Enqueue::push` (non-blocking, on looper thread). Consumer drains via `Dequeue::pop` (async), serializes, writes to wire via FrameWriter. Both sides are on the looper thread.

### Option A: Manual poll with noop_waker (RECOMMENDED)

After `Enqueue::push()`, the oneshot is immediately ready. `Recv::poll_recv(cx)` with a noop waker context will return `Ok(value)` on the first poll. No executor needed.

**Mechanism:**
```rust
use futures_task::noop_waker_ref;
use std::task::Context;

fn drain_dequeue<T: Send + 'static>(mut deq: Dequeue<T>) -> Vec<T> {
    let waker = noop_waker_ref();
    let mut cx = Context::from_waker(waker);
    let mut items = Vec::new();
    loop {
        // Dequeue::pop() returns a future wrapping deq.recv1()
        // We can't poll pop() directly — it's async.
        // But Recv::poll_recv exists and takes a Context.
        // Problem: Dequeue wraps Recv<Queue<T, S>>, and .deq is private.
        // We need to use DequeueStream which implements Stream::poll_next.
        break;
    }
    items
}
```

**CRITICAL ISSUE:** `Dequeue::pop()` is an async fn that returns a future. The future internally calls `self.deq.recv1().await`. We cannot call `poll_recv` directly because `Dequeue.deq` is private. However, `Dequeue::into_stream()` returns a `DequeueStream` that implements `futures::Stream`, and `Stream::poll_next` takes `&mut Context`. This IS pollable with a noop waker.

**Revised mechanism:**
```rust
use futures::StreamExt; // for poll_next
use futures_task::noop_waker_ref;
use std::task::Context;
use std::pin::Pin;

fn try_drain_stream<T: Send + 'static>(
    stream: &mut Pin<Box<DequeueStream<T, ()>>>
) -> Vec<T> {
    let waker = noop_waker_ref();
    let mut cx = Context::from_waker(waker);
    let mut items = Vec::new();
    loop {
        match stream.as_mut().poll_next(&mut cx) {
            Poll::Ready(Some(Next::Item(value))) => items.push(value),
            Poll::Ready(Some(Next::Closed(()))) => break, // stream done
            Poll::Ready(None) => break,
            Poll::Pending => break, // no more ready items
        }
    }
    items
}
```

This works because:
1. After `Enqueue::push(item)`, the oneshot for that item is immediately resolved
2. `DequeueStream::poll_next` calls `self.future.poll_unpin(cx)` which hits the resolved oneshot
3. With a noop waker, Poll::Pending means "genuinely not ready" (no push has happened yet)
4. No async runtime needed — pure synchronous polling

**Where to call it:** In the calloop event callback for ConnectionSource, or in the batch dispatch phase. When a handler calls `enqueue.push(msg)`, the looper can drain the corresponding DequeueStream before returning to the calloop poll loop.

**Session-type soundness:** The noop waker does not violate any session property. par's protocol guarantees are about message ordering and type safety, not about the waking mechanism. The oneshot channel's SeqCst ordering ensures visibility. The only risk is that if push hasn't happened yet, poll returns Pending — the noop waker means we won't be woken when push happens later. This is fine IF we always poll after push (synchronous calloop model guarantees this — push and drain are on the same thread in the same batch).

### Option B: futures::executor::LocalPool

`LocalPool::try_run_one()` runs one ready future without blocking. It IS already in pane's dep tree (par depends on futures). Could be used to spawn Dequeue::for_each as a local task and tick it from calloop.

**Pros:** Handles genuinely async scenarios (push from another thread). `run_until_stalled()` drains all ready work.
**Cons:** Heavier than needed. LocalPool manages a FuturesUnordered internally. Spawning requires a `LocalSpawner`. The indirection adds complexity for a use case where the value is always immediately ready.

### Option C: smol / async-executor

smol is ~1000 lines but adds a dependency. async-executor (which smol uses) can be created as a local executor. Both would work but are unnecessary — futures::executor is already available and Option A doesn't even need an executor.

### Recommendation: Option A (noop_waker + DequeueStream)

**Zero new dependencies.** Uses only futures-task::noop_waker (already in dep tree). No executor overhead. Sound because push-before-drain ordering is guaranteed by the single-thread calloop model.

**Fallback:** If a future use case requires genuinely async draining (push from another thread, delayed delivery), upgrade to Option B (LocalPool). The DequeueStream abstraction works with both — LocalPool just provides a real waker that schedules re-polling.

## 4. Integration pattern for pane-session

```
Subscription lifecycle:
1. Provider handler receives subscriber_connected
2. Provider gets Enqueue<P::Message> (par session endpoint)
3. Provider pushes messages: enqueue = enqueue.push(msg)
4. Corresponding DequeueStream lives on same looper thread
5. After each batch dispatch, drain all DequeueStreams:
   - Items → serialize → FrameWriter::enqueue → wire
   - Closed → subscription ended, clean up
6. Provider drops Enqueue (or calls close) → subscriber sees Closed
```

The DequeueStream is held as looper-local state (Rc, not Send — same as SharedWriter). The drain happens in the existing batch post-processing or as a calloop idle callback.

## 5. Session-type analysis of the bridge

**Static guarantees preserved:**
- Enqueue/Dequeue duality: par's type system ensures push/pop type agreement
- Protocol fidelity: Queue<T,S> enum forces handler to process both Item and Closed branches
- #[must_use] on Dequeue prevents ignoring the consumer side

**Affine gap:**
- Enqueue can be dropped without closing → Dequeue::pop will eventually panic ("sender dropped")
- Compensation: same as all par endpoints — panic propagation. For pane, the SubscriberSender wrapper should catch the panic (or use Dequeue's DequeueStream which returns None on sender drop, NOT panic — need to verify)
- DequeueStream1::poll_next returns Poll::Ready(None) when Closed(()) is received, but on sender drop the underlying oneshot panics. This is a real gap — the DequeueStream will panic if the Enqueue is dropped without close().

**Mitigation:** pane-session must ensure Enqueue::close() is called on subscription teardown (ServiceTeardown path). The TeardownSet obligation (C2) should include Enqueue endpoints. If the process crashes, the bridge thread catches the panic via catch_unwind (existing pattern).

**Deadlock freedom:** The noop_waker polling pattern cannot deadlock because:
1. Push is non-blocking (par invariant)
2. Drain polls only ready values (noop waker, no blocking)
3. Single thread — no cross-thread wait cycle
4. Star topology preserved — [JHK24] Theorem 1.2 applies

This is novel engineering grounded in established theory. The async/sync boundary is resolved by exploiting par's non-blocking send guarantee and the calloop single-thread model, not by introducing an async runtime.
