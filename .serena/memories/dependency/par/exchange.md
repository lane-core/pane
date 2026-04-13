---
type: reference
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: critical
keywords: [par, Send, Recv, exchange, oneshot, branching, recursion, choose, handle, link]
extends: dependency/par/_hub
verified_against: par-0.3.10/src/exchange.rs
agents: [all]
---

# par Exchange — Send and Recv

## Types

```rust
pub struct Recv<T, S: Session = ()> { rx: oneshot::Receiver<Exchange<T, S>> }
pub struct Send<T, S: Session = ()> { tx: oneshot::Sender<Exchange<T, S::Dual>> }
enum Exchange<T, S: Session> { Send((T, S)), Link(Recv<T, S>) }
```

Both `#[must_use]`. Built on `futures::channel::oneshot`.

**Duality:**
- `Dual<Recv<T, S>> = Send<T, Dual<S>>`
- `Dual<Send<T, S>> = Recv<T, Dual<S>>`

**Linear logic:**
- `Recv<A, B>` = **A ⊗ B** (tensor)
- `Send<A, B>` = **A⊥ ⅋ B** (par)
- `Recv<Result<A, B>>` = **A ⊕ B** (internal choice)
- `Send<Result<A, B>>` = **A⊥ & B⊥** (external choice)

## Recv methods

- `async fn recv(self) -> (T, S)` — blocks until value. `#[must_use]`.
- `fn poll_recv(self, cx) -> Result<(T, S), Self>` — non-blocking poll. `#[must_use]`.
- When `S = ()`: `async fn recv1(self) -> T`, `fn poll_recv1(self, cx) -> Result<T, Self>`.

## Send methods

- `fn send(self, value: T) -> S` — **non-blocking, non-async**. `#[must_use]`.
- When `S = ()`:
  - `fn send1(self, value: T)` — terminal. NOT #[must_use].
  - `fn choose<S2>(self, choice: impl FnOnce(S2) -> T) -> Dual<S2>` — pick branch from enum, get dual. `#[must_use]`.
  - `fn handle(self) -> Dual<S>` when `T = S: Session` — supply session, get dual. `#[must_use]`.

## Non-blocking send architecture

`send` calls `S::fork_sync(|dual| self.tx.send(Exchange::Send((value, dual))))`.
Creates continuation's dual pair inline, sends value AND dual through oneshot,
returns continuation. No awaiting.

## Exchange::Link — forwarding

`Link(Recv<T, S>)` variant used by `Send::link`. Redirects receiver to another
receiver. `Recv::recv` loops through Link indirections transparently. Cut-elimination
of linear logic — relay process compiled away into direct link.

## Panic on drop

Send dropped without sending → Recv panics: `.expect("sender dropped")`.
Recv dropped without receiving → Send panics on send: `.expect("receiver dropped")`.
**No graceful drop path.** Linearity enforced by panic, not type system.

## Branching — Via Native Enums

No dedicated branching types. Enums carry session endpoints:

```rust
enum Choice {
    Left(Recv<i64>),
    Right(Send<String>),
}
// Chooser: Send<Choice> → .choose(Choice::Left) → Send<i64>
// Offerer: Recv<Choice> → match → handle session
```

`Send::choose(Enum::Variant)` codifies
`fork_sync(|dual| self.send1(Choice::Left(dual)))`.

## Recursion — Via Rust Type Recursion

No dedicated fixpoint types. Native enum recursion:

```rust
enum Counting {
    More(Recv<i64, Recv<Counting>>),
    Done(Send<i64>),
}
```

No Box needed — oneshot channels inside Recv/Send provide memory indirection.
Implementation uses loops with reassignment. Queue module is standardized version.
