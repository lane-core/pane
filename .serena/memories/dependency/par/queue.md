---
type: reference
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [par, Dequeue, Enqueue, Queue, Stream, push, pop, close, subscription]
extends: dependency/par/_hub
verified_against: par-0.3.10/src/queue.rs
agents: [all]
---

# par Queue — Streaming Sequences

## Types

```rust
pub struct Dequeue<T, S: Session = ()> { deq: Recv<Queue<T, S>> }
pub struct Enqueue<T, S: Session = ()> { enq: Send<Queue<T, S::Dual>> }
pub enum Queue<T, S: Session = ()> { Item(T, Dequeue<T, S>), Closed(S) }
```

All `#[must_use]`. Standardized recursive pattern.

**Duality:**
- `Dual<Dequeue<T, S>> = Enqueue<T, Dual<S>>`
- `Dual<Enqueue<T, S>> = Dequeue<T, Dual<S>>`

## Dequeue methods

- `async fn pop(self) -> Queue<T, S>` — next item or Closed. `#[must_use]`.
- `async fn fold<A, F>(self, init, f) -> (A, S)` — async fold. `#[must_use]`.
- `async fn for_each<F>(self, f) -> S` — async iteration. `#[must_use]`.
- `fn into_stream(self) -> DequeueStream<T, S>` — `futures::Stream<Item = Next<T, S>>`. `#[must_use]`.
- When `S = ()`: `fold1`, `for_each1`, `into_stream1` (Stream<Item = T>, None on close).

## Enqueue methods

- `fn push(self, item: T) -> Self` — non-blocking push. Returns new Enqueue. NOT #[must_use].
- `fn close(self) -> S` — signals end, returns continuation. `#[must_use]`.
- When `S = ()`: `fn close1(self)` — close + discard.

## Stream types

- `DequeueStream<T, S>` — Stream<Item = Next<T, S>>
- `Next<T, S>` — enum { Item(T), Closed(S) }
- `DequeueStream1<T>` — Stream<Item = T>, ends with None

## Bounded queues

**NO built-in bounded queue or backpressure.** push is always
non-blocking, unbounded. Each push allocates new oneshot
internally. Backpressure must be external (e.g., credit-based
protocol using additional Send/Recv exchanges).
