---
type: reference
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [par, Server, Proxy, Connection, coexponentials, scoping, deadlock]
extends: dependency/par/_hub
verified_against: par-0.3.10/src/server.rs
agents: [all]
---

# par Server — Multi-Client Sessions

## Types

```rust
pub struct Server<Connect, Resume, ConnectionData> { ... }
pub struct Proxy<Connect: Session> { ... }
pub struct Connection<Resume: Session> { ... }
pub enum Event<Connect, Resume, ConnectionData> {
    Connect { session: Connect },
    Resume { session: Resume, data: ConnectionData },
}
```

Server, Connection: `#[must_use]`. Proxy: NOT #[must_use] (droppable, cloneable).

**Coexponentials** (Kokke/Montesi/Peressotti, ICFP 2021) — not
standard !/?  but related structural rule for replicable connection
initiation.

## Three-part scoping discipline

**No two of Server, Proxy, Connection may coexist in same scope.**
Even two Proxies cannot see each other. Enforced by API design:

- `Server::start(f)` — creates Server, passes Proxy to closure. Never share scope.
- `Proxy::clone(f)` — passes new Proxy to closure. Original and clone separate.
- `Server::suspend(data, f)` — creates Connection in closure. Server not visible.
- `Server::poll(self) -> Option<(Self, Event<...>)>` — consumes self, returns new self + event. Drops internal sender for termination detection (None when no proxies/connections remain).

## Protocol parameters

- **Connect** — session type for connection initiation.
- **Resume** — session type for connection resumption.
- **ConnectionData** — server-side per-connection local data.

## Methods

Server: `start(f)`, `suspend(&mut self, data, f)`, `async poll(self) -> Option<(Self, Event)>`.

Proxy: `clone(&self, f)`, `connect(self) -> Connect` (consumes proxy).

Connection: `resume(self) -> Resume`.

## Internal mechanism

Uses `futures::channel::mpsc` (capacity 0). Server holds sender +
receiver. Proxy holds cloneable sender closure. Connection holds
FnOnce sender closure. Connection IDs via sequential allocator
with free list. Data in HashMap<usize, ConnectionData>.
