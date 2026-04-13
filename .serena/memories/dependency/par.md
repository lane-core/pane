---
type: reference
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: critical
keywords: [par, session_types, linear_logic, CLL, Send, Recv, Dual, Enqueue, Dequeue, Server, Proxy, Connection, fork_sync, link, choose, handle, coexponentials]
verified_against: par 0.3.10 source (all five files read in full)
sources: [par-0.3.10/src/lib.rs, par-0.3.10/src/exchange.rs, par-0.3.10/src/queue.rs, par-0.3.10/src/server.rs, par-0.3.10/src/runtimes.rs, par Cargo.toml]
agents: [all]
---

# par 0.3.10 — Comprehensive Dependency Digest

par is a full implementation of propositional linear logic as
session types in Rust. Author: faiface (Michal Štrba). Repository:
https://github.com/faiface/par. Self-description: "Session types,
as an implementation of linear logic with MIX." Depends on
`futures 0.3` for oneshot channels and Streams. Optional:
`tokio` (feature `runtime-tokio`), `fastrand`, `tokio-tungstenite`.

pane depends on par 0.3.10 with `default-features = false` (no
tokio, no examples). Only pane-session depends on it directly.

## 1. Session Trait — The Core Abstraction

```rust
pub trait Session: Send + 'static {
    type Dual: Session<Dual = Self>;
    fn fork_sync(f: impl FnOnce(Self::Dual)) -> Self;
    fn link(self, dual: Self::Dual);
}
pub type Dual<S> = <S as Session>::Dual;
```

**Session** is the trait implemented by all session type endpoints.

- `Send + 'static` bound: endpoints can cross thread boundaries.
- `Dual` associated type: involutive — `Dual<Dual<S>> = S`.
  The type system enforces this via `Session<Dual = Self>`.
- `fork_sync(f)`: creates a dual pair in separate scopes. The
  closure gets `Self::Dual`; the caller gets `Self`. The closure
  is synchronous (not async) and runs to completion before
  fork_sync returns. This makes par runtime-agnostic — async
  forking is done by spawning inside the closure.
- `link(self, dual)`: wires two dual endpoints together.
  Non-blocking, non-async. Generalizes function application.

**() implements Session** as the empty/finished session. Self-dual:
`Dual<()> = ()`. fork_sync calls f(()), link does nothing. This
is the terminal object.

## 2. Exchange Module — Send and Recv

### Types

```rust
pub struct Recv<T, S: Session = ()> { rx: oneshot::Receiver<Exchange<T, S>> }
pub struct Send<T, S: Session = ()> { tx: oneshot::Sender<Exchange<T, S::Dual>> }
enum Exchange<T, S: Session> { Send((T, S)), Link(Recv<T, S>) }
```

Both are `#[must_use]`. Built on `futures::channel::oneshot`.

**Duality:**
- `Dual<Recv<T, S>> = Send<T, Dual<S>>`
- `Dual<Send<T, S>> = Recv<T, Dual<S>>`

### Linear logic correspondence

- `Recv<A, B>` = **A ⊗ B** (tensor / times)
- `Send<A, B>` = **A⊥ ⅋ B** (par)
- `Recv<Result<A, B>>` = **A ⊕ B** (plus / internal choice)
- `Send<Result<A, B>>` = **A⊥ & B⊥** (with / external choice)

### Methods on Recv<T, S>

- `async fn recv(self) -> (T, S)` — blocks until value arrives,
  returns value + continuation. `#[must_use]`.
- `fn poll_recv(self, cx) -> Result<(T, S), Self>` — non-blocking
  poll. Returns `Err(self)` if pending. `#[must_use]`.

When `S = ()`:
- `async fn recv1(self) -> T` — recv + discard empty continuation.
- `fn poll_recv1(self, cx) -> Result<T, Self>` — poll variant.

### Methods on Send<T, S>

- `fn send(self, value: T) -> S` — **non-blocking, non-async**.
  Supplies value, returns continuation. `#[must_use]`.

When `S = ()`:
- `fn send1(self, value: T)` — send + discard empty continuation.
  NOT #[must_use] (terminal).
- `fn choose<S2: Session>(self, choice: impl FnOnce(S2) -> T) -> Dual<S2>`
  — picks a branch from an enum and returns the dual of the
  chosen session. Codifies the "outside" pattern: wraps
  `fork_sync(|dual| self.send1(Enum::Variant(dual)))`.
  `#[must_use]`.
- `fn handle(self) -> Dual<S>` when `T = S: Session` — supplies
  a session and returns its dual. Sugar for delegation.
  `#[must_use]`.

### Non-blocking send architecture

Send is always non-blocking. The `send` method calls
`S::fork_sync(|dual| self.tx.send(Exchange::Send((value, dual))))`.
It creates the continuation's dual pair inline, sends both the
value AND the continuation's dual through the oneshot, and returns
the continuation. No awaiting.

### Exchange::Link — the forwarding mechanism

The internal `Exchange` enum has a `Link(Recv<T, S>)` variant
used by `Send::link`. Instead of providing a value, it redirects
the receiver to another receiver. Recv::recv loops through Link
indirections transparently. This is the cut-elimination
(forwarding) of linear logic — a process that merely relays
between two endpoints is compiled away into a direct link.

### Panic on drop

If a Send endpoint is dropped without sending, the Recv side
panics: `recv` calls `.expect("sender dropped")`. If a Recv
endpoint is dropped without receiving, the Send side panics
on send: `.expect("receiver dropped")`. **There is no graceful
drop path.** Linearity is enforced by panic, not by the type
system. Rust's affine types allow drop; par compensates with
runtime panics.

## 3. Queue Module — Streaming Sequences

### Types

```rust
pub struct Dequeue<T, S: Session = ()> { deq: Recv<Queue<T, S>> }
pub struct Enqueue<T, S: Session = ()> { enq: Send<Queue<T, S::Dual>> }
pub enum Queue<T, S: Session = ()> { Item(T, Dequeue<T, S>), Closed(S) }
```

All `#[must_use]`. Standardized recursive pattern equivalent to:
```
type Dequeue<T, S> = Recv<Queue<T, S>>;
type Enqueue<T, S> = Send<Queue<T, Dual<S>>>;
```

**Duality:**
- `Dual<Dequeue<T, S>> = Enqueue<T, Dual<S>>`
- `Dual<Enqueue<T, S>> = Dequeue<T, Dual<S>>`

### Methods on Dequeue<T, S>

- `async fn pop(self) -> Queue<T, S>` — next item or Closed.
  `#[must_use]`.
- `async fn fold<A, F>(self, init, f) -> (A, S)` — async fold
  over all items, returns accumulator + continuation. `#[must_use]`.
- `async fn for_each<F>(self, f) -> S` — async for_each, returns
  continuation. `#[must_use]`.
- `fn into_stream(self) -> DequeueStream<T, S>` — converts to
  `futures::Stream<Item = Next<T, S>>`. `#[must_use]`.

When `S = ()`:
- `async fn fold1<A, F>(self, init, f) -> A`
- `async fn for_each1<F>(self, f)`
- `fn into_stream1(self) -> DequeueStream1<T>` — Stream<Item = T>,
  None on close. `#[must_use]`.

### Methods on Enqueue<T, S>

- `fn push(self, item: T) -> Self` — non-blocking push. Returns
  new Enqueue for the next push. NOT #[must_use] (can be chained
  or closed).
- `fn close(self) -> S` — signals no more items, returns
  continuation. `#[must_use]`.

When `S = ()`:
- `fn close1(self)` — close + discard empty continuation.

### Stream types

- `DequeueStream<T, S>` — Stream<Item = Next<T, S>>
- `Next<T, S>` — enum { Item(T), Closed(S) }
- `DequeueStream1<T>` — Stream<Item = T>, ends with None

### Bounded queues

There is NO built-in bounded queue or backpressure mechanism.
Enqueue::push is always non-blocking and unbounded. Each push
allocates a new oneshot channel internally (recursive unfolding
of the Queue enum). Backpressure must be built externally, e.g.
via a credit-based protocol using additional Send/Recv exchanges.

## 4. Server Module — Multi-Client Sessions

### Types

```rust
pub struct Server<Connect, Resume, ConnectionData> { ... }
pub struct Proxy<Connect: Session> { ... }
pub struct Connection<Resume: Session> { ... }
pub enum Event<Connect, Resume, ConnectionData> {
    Connect { session: Connect },
    Resume { session: Resume, data: ConnectionData },
}
```

Server and Connection are `#[must_use]`. Proxy is NOT #[must_use]
(can be dropped at will, can be cloned).

### Linear logic correspondence

Proxy implements **coexponentials** — from Kokke, Montesi, Peressotti
"Client-server sessions in linear logic" (ICFP 2021). Not
standard linear logic !/? but a related structural rule for
replicable connection initiation.

### Three-part scoping discipline

To maintain deadlock freedom, **no two of Server, Proxy,
Connection may coexist in the same scope.** Even two Proxies
cannot see each other. This is enforced by API design:

- `Server::start(f)` — creates Server, passes Proxy to closure f.
  Server is returned; Proxy is in f's scope only.
- `Proxy::clone(f)` — passes a new Proxy to closure f. The
  original Proxy and the clone never share scope.
- `Server::suspend(data, f)` — creates Connection, passes to
  closure f. Server and Connection never share scope.
- `Server::poll(self) -> Option<(Self, Event<...>)>` — consumes
  self, returns new self + event. During poll, Server drops its
  internal sender, enabling termination detection (returns None
  when no proxies or connections remain).

### Protocol parameters

- **Connect** — session type for connection initiation. Client
  uses Proxy::connect() → Connect. Server sees Connect in
  Event::Connect.
- **Resume** — session type for connection resumption. Client
  uses Connection::resume() → Resume. Server sees Resume in
  Event::Resume.
- **ConnectionData** — server-side local data per connection.
  Passed to suspend(), returned in Resume events.

### Methods

Server<Connect, Resume, ConnectionData>:
- `fn start(f: impl FnOnce(Proxy<Dual<Connect>>)) -> Self` —
  create server + proxy. `#[must_use]`.
- `fn suspend(&mut self, data, f: impl FnOnce(Connection<Dual<Resume>>))` —
  create or maintain connection.
- `async fn poll(self) -> Option<(Self, Event<...>)>` — wait for
  next event. `#[must_use]`.

Proxy<Connect>:
- `fn clone(&self, f: impl FnOnce(Self))` — duplicate into
  separate scope.
- `fn connect(self) -> Connect` — initiate connection. Consumes
  proxy. `#[must_use]`.

Connection<Resume>:
- `fn resume(self) -> Resume` — resume interaction. `#[must_use]`.

### Internal mechanism

Uses `futures::channel::mpsc` (capacity 0) internally. Server
holds both sender and receiver. Proxy holds a cloneable sender
closure. Connection holds a FnOnce sender closure. Server::poll
drops its sender to enable termination detection (if no proxy/
connection senders remain, recv returns None).

Connection IDs are managed via a simple allocator (sequential
IDs with free list). Data stored in HashMap<usize, ConnectionData>.

## 5. Runtimes Module

Three async forking helpers:

```rust
// Requires feature "runtime-tokio"
pub mod tokio {
    pub fn fork<S: Session, F>(f: impl FnOnce(S::Dual) -> F) -> S
    where F: Future<Output = ()> + Send + 'static;
}

pub mod spawn {
    pub trait Fork {
        fn fork<S: Session, F>(&self, f: impl FnOnce(S::Dual) -> F) -> S
        where F: Future<Output = ()> + Send + 'static;
    }
    impl<Spawn: futures::task::Spawn> Fork for Spawn { ... }
}

pub mod local_spawn {
    pub trait Fork {
        fn fork<S: Session, F>(&self, f: impl FnOnce(S::Dual) -> F) -> S
        where F: Future<Output = ()> + Send + 'static;
    }
    impl<Spawn: futures::task::LocalSpawn> Fork for Spawn { ... }
}
```

All call `S::fork_sync` internally, spawning the future inside
the closure. pane does NOT use any of these — it uses
`fork_sync` directly with `std::thread::spawn` inside (bridge.rs).

## 6. Branching — Via Native Enums

par has NO dedicated branching/choice types. Branching is
modeled via Rust enums carrying session endpoints:

```rust
enum Choice {
    Left(Recv<i64>),   // branch with session continuation
    Right(Send<String>),
}
// Chooser: Send<Choice> → picks variant → gets dual of payload
// Offerer: Recv<Choice> → matches variant → handles session
```

`Send::choose(Enum::Variant)` is the ergonomic API: given
`Send<Choice>`, calling `.choose(Choice::Left)` returns
`Dual<Recv<i64>> = Send<i64>`. Internally codifies
`fork_sync(|dual| self.send1(Choice::Left(dual)))`.

Linear logic mapping:
- `Recv<Choice>` where Choice has variants A, B = **A ⊕ B**
  (receiver must handle all branches = internal choice received)
- `Send<Choice>` = **A⊥ & B⊥** (sender picks one branch =
  external choice offered)

Result<A, B> is the standard two-branch choice enum. par's docs
use it extensively.

## 7. Recursion — Via Rust's Type Recursion

par has NO dedicated recursion/fixpoint types. Recursion uses
Rust's native enum recursion:

```rust
enum Counting {
    More(Recv<i64, Recv<Counting>>),
    Done(Send<i64>),
}
// Session endpoint: Recv<Counting> or Send<Counting>
```

No Box needed — the memory indirection is provided by the oneshot
channels inside Recv/Send.

Implementation uses loops with reassignment:
```rust
let mut session: Recv<Counting> = ...;
loop {
    match session.recv1().await {
        Counting::More(inner) => {
            let (value, next) = inner.recv().await;
            session = next; // rebind for next iteration
        }
        Counting::Done(report) => break report.send1(total),
    }
}
```

The Queue module is just a standardized version of this pattern.

## 8. What par CANNOT Express

### No subtyping or session subtyping
par's types are exact. There is no `S <: S'` that allows a
session offering more choices to be used where fewer are expected.
Each side must implement exactly the protocol specified.

### No dependent types or refinement
Cannot express "receive an i64 that is positive" or "send N items
where N was received earlier." Value constraints are runtime only.

### No timeouts or failure modes
par panics on endpoint drop. There is no `Try`, `Timeout`, or
`Error` session combinator. A session that might fail must encode
failure as a branch (e.g., Result<Success, Failure>) BEFORE the
failure point, which requires the failure to be anticipated in the
protocol design. Unanticipated transport failures (IPC disconnect)
are unrecoverable — they propagate as panics.

### No delegation / session passing across processes
par's session endpoints are in-process (oneshot channels). They
cannot be serialized or sent over IPC. Delegation (passing a
session endpoint through another session endpoint) works within
a process but not across process boundaries.

### No multiparty session types
par is binary. Every session has exactly two sides. Multi-party
coordination requires a coordinator process that holds multiple
binary sessions simultaneously (demonstrated in par's "multiple
participants" example). Deadlock freedom of such compositions
depends on the topology not having cycles — par's scoping
discipline (fork_sync) helps but doesn't fully prevent cycles
when multiple sessions are juggled manually.

### No channel mobility across async boundaries
Session endpoints are not `Sync`. They can be `Send` (moved to
another thread) but not shared. This is correct for linear
resources but means you cannot hold a session endpoint in shared
state without Mutex (which defeats linearity).

### No backpressure or flow control
Enqueue::push is always non-blocking and unbounded. Send::send
is always non-blocking. The receiver can fall behind with no
protocol-level mechanism to slow the sender.

### No runtime introspection
No way to query session state, count outstanding messages, or
inspect the protocol type at runtime.

### No graceful shutdown
Dropping an endpoint panics the peer. There is no "cancel" or
"abort" session type that allows orderly shutdown without
completing the full protocol.

## 9. Linear Discipline — Enforcement Mechanism

par relies on three mechanisms:

1. **Move semantics:** Session endpoints are consumed by their
   methods (recv, send, pop, push, etc.). After calling send(),
   the Send endpoint is gone. The continuation is a new endpoint.
   Rust's ownership prevents use-after-move.

2. **#[must_use]:** All session types and all methods returning
   continuations are #[must_use]. Compiler warns if a continuation
   is ignored.

3. **Panic on drop:** If either side of a channel is dropped
   without completing the protocol, the other side panics when it
   tries to communicate. oneshot::Receiver::await panics with
   "sender dropped"; oneshot::Sender::send panics with "receiver
   dropped". This is the ONLY runtime compensation for the affine
   gap — Rust allows drop, par panics on it.

**What this means:** par provides *affine* safety (use at most
once) via Rust's move semantics, plus *runtime linear* safety
(must use exactly once) via panic-on-drop. The type system
prevents *misuse* (wrong message, wrong order). The runtime
prevents *non-use* (protocol abandonment), but only via panic
(not graceful error handling).

The panic compensation is sufficient for in-process sessions
where panic = thread abort. For cross-process sessions (IPC),
the panic only affects the bridge thread — the other process
needs separate detection (which pane handles via ProtocolAbort
framing and ServiceTeardown).

## 10. Linear Logic Mapping (Complete)

| par Type | Linear Logic | Name |
|----------|-------------|------|
| `Recv<A, B>` | A ⊗ B | Tensor (times) |
| `Send<A, B>` | A⊥ ⅋ B | Par |
| `Recv<A>` (= Recv<A, ()>) | A ⊗ 1 ≅ A | |
| `Send<A>` (= Send<A, ()>) | A⊥ ⅋ 1 ≅ A⊥ | |
| `()` | 1 (and ⊥) | Unit (self-dual) |
| `Recv<Result<A, B>>` | A ⊕ B | Plus (internal choice) |
| `Send<Result<A, B>>` | A⊥ & B⊥ | With (external choice) |
| `Dequeue<T, S>` | !T ⊗ S (sort of) | Recursive tensor |
| `Enqueue<T, S>` | ?T⊥ ⅋ S | Recursive par |
| `Proxy<C>` | Coexponential | Per Kokke/Montesi/Peressotti 2021 |
| `Server` | Coexponential server | |
| `Session::link` | Cut | Cut elimination / forwarding |
| `Session::fork_sync` | Cut (introduction) | Spawn dual pair |

Note: par collapses 1 and ⊥ into the single type `()`. The
crate description says "linear logic with MIX" — the MIX rule
allows identifying 1 with ⊥, which is what `() : Dual = ()`
implements.

## 11. pane's Current par Usage

### Handshake types (pane-session/src/handshake.rs)

```rust
pub type ClientHandshake =
    par::exchange::Send<Hello, par::exchange::Recv<Result<Welcome, Rejection>>>;
pub type ServerHandshake = par::Dual<ClientHandshake>;
// ServerHandshake = Recv<Hello, Send<Result<Welcome, Rejection>>>
```

Two-step protocol: client sends Hello, receives Result. Both
branches (Welcome and Rejection) terminate the par session.
Branching is at the value level (Result), not the session level.

### Bridge (pane-session/src/bridge.rs)

The bridge connects par's in-process oneshot channels to IPC:

```
Handler ←→ par oneshot ←→ Bridge thread ←→ FrameCodec ←→ wire
```

- `bridge_client_handshake(transport) -> ClientHandshake`:
  Uses `ParSend::fork_sync` to create the client endpoint.
  Inside the closure, spawns a std::thread that uses
  `futures::executor::block_on` to drive par's async recv,
  then serializes/deserializes via FrameCodec + CBOR.

- `bridge_server_handshake(transport) -> ServerHandshake`:
  Same pattern with ParRecv::fork_sync.

After the handshake, par is DONE. The active phase uses flat
enum dispatch (LooperMessage) with no session typing.

### What's NOT using par

Everything after the handshake: request/reply correlation,
notifications, pub/sub, cancel, revocation, failure cascade,
backpressure. All implemented ad-hoc in pane-app with mpsc
channels, DispatchEntry closures, and runtime checks.

## 12. Mapping to pane's Needs

### Request/Reply

**Can par express it?** YES. A request/reply is:
```rust
type Request<Req, Resp> = Send<Req, Recv<Resp>>;
```
The dual (server side) receives the request, sends the response.
The continuation ensures exactly-once reply. ReplyPort's current
Drop-based enforcement becomes unnecessary — linearity enforced
by par's type system + panic-on-drop.

**Complication:** pane multiplexes many request/reply exchanges
over one connection. par's binary sessions are point-to-point.
To use par for request/reply, you'd need either:
(a) a par session per in-flight request (spawn via fork_sync
for each request, link to a correlator), or
(b) use Server module where each request is a Connection resume.

### Notification (fire-and-forget)

**Can par express it?** YES, trivially: `Send<Payload>` (one-shot)
or `Enqueue<Payload>` (streaming). The non-blocking send semantics
match pane's requirement.

### Pub/Sub (subscription with streaming)

**Can par express it?** YES. A subscription is:
```rust
type Subscription<T> = Dequeue<T, ()>;
// Provider side: Enqueue<T, ()>
```
Provider pushes items via Enqueue::push, closes via close1.
Subscriber pops via Dequeue::pop or for_each1.

**Complication:** pane's pub/sub fans out to multiple subscribers.
Each subscriber would need its own Enqueue/Dequeue pair (which is
correct — each subscription IS a separate session). The provider
holds N Enqueue endpoints. The Server module could manage
subscriber lifecycle.

### Failure Cascade

**Can par express it?** PARTIALLY. par's failure mode is panic
(endpoint dropped → peer panics). This is a blunt instrument.
pane needs controlled teardown: ServiceTeardown message to all
affected sessions, not a panic.

**Approach:** Model failure as a branch in the protocol:
```rust
enum ServiceEvent<T> {
    Message(T, Dequeue<ServiceEvent<T>>), // recursive
    Teardown,
}
```
Teardown is an explicit protocol step, not a crash. But this
requires the failure to be part of the protocol design, not a
transport-level accident.

### Token Correlation

**Can par express it?** NO, directly. par has no concept of
request tokens or correlation IDs. Each par session is
implicitly correlated (the continuation IS the correlation).
But pane's wire protocol multiplexes multiple logical sessions
over one transport with explicit token matching.

**Approach:** Use par to type each logical session separately.
The correlator becomes a routing layer that maps wire tokens to
par endpoints. Each in-flight request is a separate par session
(Send<Req, Recv<Resp>>), spawned via fork_sync, with the dual
held by the correlator. The correlator receives wire responses
and forwards them to the correct par endpoint.

### Backpressure

**Can par express it?** NOT BUILT-IN. par's sends and pushes
are always non-blocking and unbounded. Backpressure requires
a credit-based protocol:
```rust
type Backpressured<T> = Recv<Credit, Send<T, Backpressured<T>>>;
```
The sender must receive a credit before sending each message.
This is expressible but verbose. pane's current backpressure
(outstanding request counter, write channel capacity) would need
to be re-encoded as explicit protocol steps.

### Cancel

**Can par express it?** YES, as a branch:
```rust
enum RequestOutcome<Resp> {
    Reply(Resp),
    Failed,
    Cancelled, // server acknowledges cancel
}
// Client can cancel by dropping... but that panics.
```

**Complication:** pane's cancel is initiated by the client while
a request is in-flight. In par, the client is blocked on
Recv<Resp> — it cannot simultaneously send a cancel. This
requires either:
(a) A separate cancel session (spawned alongside the request
session), or
(b) Model the client side as an enum choice before blocking:
```rust
enum ClientAction {
    WaitForReply(Recv<Resp>),
    Cancel,
}
```
But this changes the protocol shape fundamentally.

## Key Design Decisions for pane-session Integration

1. **par is sound for in-process binary protocol segments.**
   Each handshake, each request/reply exchange, each subscription
   stream can be a par session.

2. **par cannot type the multiplexing layer.** The wire protocol
   routes by session_id and token — this is inherently a
   multi-party coordination problem. The correlator/router is
   OUTSIDE par's type system.

3. **par's panic-on-drop is appropriate for the bridge** (bridge
   thread panic → transport failure → ProtocolAbort) but NOT for
   application-level protocol violations (those need graceful
   error reporting).

4. **Server module maps to pane's connection lifecycle.** A pane
   server manages dynamic clients with connect/resume/disconnect
   — exactly Server's design. The three-part scoping discipline
   (Server/Proxy/Connection never in same scope) prevents the
   deadlocks that DLfActRiS §5 addresses for actors.

5. **Queue module maps to pub/sub and streaming.** Enqueue/Dequeue
   with continuation after close is exactly the pattern for a
   subscription that can be cleanly terminated.

6. **Branching via enums maps to pane's service dispatch.** Each
   service is a branch; the session type is an enum of per-service
   sub-protocols.
