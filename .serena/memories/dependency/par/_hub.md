---
type: reference
status: current
supersedes: []
created: 2026-04-12
last_updated: 2026-04-12
importance: critical
keywords: [par, session_types, linear_logic, CLL, overview]
verified_against: par 0.3.10 source (all five files read in full)
agents: [all]
---

# par 0.3.10 — Dependency Hub

par: full implementation of propositional linear logic as session
types in Rust. Author: faiface (Michal Štrba).
https://github.com/faiface/par. "Session types, as an
implementation of linear logic with MIX."

Depends on `futures 0.3` (oneshot channels + Streams). Optional:
`tokio` (feature `runtime-tokio`), `fastrand`, `tokio-tungstenite`.

pane depends on par 0.3.10 with `default-features = false`. Only
pane-session depends on it directly.

## Session Trait — Core Abstraction

```rust
pub trait Session: Send + 'static {
    type Dual: Session<Dual = Self>;
    fn fork_sync(f: impl FnOnce(Self::Dual)) -> Self;
    fn link(self, dual: Self::Dual);
}
pub type Dual<S> = <S as Session>::Dual;
```

- `Send + 'static`: endpoints cross thread boundaries.
- `Dual`: involutive — `Dual<Dual<S>> = S`.
- `fork_sync(f)`: creates dual pair. Closure gets `Self::Dual`,
  caller gets `Self`. Synchronous — async forking done by spawning
  inside closure. Runtime-agnostic.
- `link(self, dual)`: wires two dual endpoints. Non-blocking.
  Generalizes function application.

`()` implements Session as empty/finished session. Self-dual.
Terminal object.

## Runtimes (section 5)

Three async forking helpers: `tokio::fork`, `spawn::Fork`,
`local_spawn::Fork`. All call `fork_sync` internally, spawning
future inside closure. **pane does NOT use these** — uses
`fork_sync` directly with `std::thread::spawn` (bridge.rs).

## Spokes

- [dependency/par/exchange](dependency/par/exchange) — Send, Recv, branching via enums, recursion patterns
- [dependency/par/queue](dependency/par/queue) — Dequeue, Enqueue, streaming sequences
- [dependency/par/server](dependency/par/server) — Server, Proxy, Connection, coexponentials
- [dependency/par/linear_discipline](dependency/par/linear_discipline) — enforcement mechanism, limitations, linear logic mapping table
- [dependency/par/pane_integration](dependency/par/pane_integration) — current pane usage + mapping to pane's needs
