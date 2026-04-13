---
type: reference
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: critical
keywords: [par, pane, handshake, bridge, request_reply, pubsub, cancel, backpressure, correlator]
extends: dependency/par/_hub
agents: [all]
---

# par × pane Integration

## Current usage

### Handshake (pane-session/src/handshake.rs)

```rust
pub type ClientHandshake =
    par::exchange::Send<Hello, par::exchange::Recv<Result<Welcome, Rejection>>>;
pub type ServerHandshake = par::Dual<ClientHandshake>;
```

Two-step: client sends Hello, receives Result. Both branches
terminate par session. Branching at value level (Result).

### Bridge (pane-session/src/bridge.rs)

```
Handler ←→ par oneshot ←→ Bridge thread ←→ FrameCodec ←→ wire
```

`bridge_client_handshake`: `ParSend::fork_sync` + `std::thread::spawn`
+ `futures::executor::block_on` for par's async recv. Serializes
via FrameCodec + CBOR.

After handshake, par is done. Active phase uses flat enum dispatch
(LooperMessage) — no session typing.

### Phase 3: SubscriberSender (pane-app)

`Enqueue<Vec<u8>>` / `Dequeue<Vec<u8>>` pair for pub/sub.
Enqueue::push from handler, Dequeue as calloop StreamSource
with PingWaker.

### Phase 4: ReplyFuture (pane-app)

`par::exchange::Send` / `Recv` pair for async request/reply.
ReplySender wraps Send (resolve/reject/Drop), ReplyFuture wraps
Recv (async recv with typed downcast). In Dispatch's async_entries.

## Mapping to pane's needs

### Request/Reply — YES

```rust
type Request<Req, Resp> = Send<Req, Recv<Resp>>;
```

Continuation ensures exactly-once reply. Complication: pane
multiplexes many exchanges over one connection. Need par session
per in-flight request (fork_sync each, link to correlator), or
Server module with Connection per request.

### Notification — YES, trivially

`Send<Payload>` (one-shot) or `Enqueue<Payload>` (streaming).
Non-blocking send matches pane requirement.

### Pub/Sub — YES

```rust
type Subscription<T> = Dequeue<T, ()>;
```

Each subscriber gets own Enqueue/Dequeue pair. Provider holds
N Enqueues. Server module could manage lifecycle.

### Failure Cascade — PARTIALLY

par's failure = panic. pane needs controlled teardown. Model
failure as protocol branch:

```rust
enum ServiceEvent<T> {
    Message(T, Dequeue<ServiceEvent<T>>),
    Teardown,
}
```

Requires failure anticipated in protocol design.

### Token Correlation — NO, directly

par has no tokens/correlation IDs. Each session implicitly
correlated (continuation IS correlation). Correlator becomes
routing layer mapping wire tokens to par endpoints.

### Backpressure — NOT built-in

Credit-based protocol expressible but verbose:
```rust
type Backpressured<T> = Recv<Credit, Send<T, Backpressured<T>>>;
```

### Cancel — YES, as branch

```rust
enum RequestOutcome<Resp> { Reply(Resp), Failed, Cancelled }
```

Complication: client blocked on Recv cannot simultaneously send
cancel. Needs separate cancel session or enum choice before
blocking.

## Key design decisions

1. **par sound for in-process binary protocol segments.** Each
   handshake, request/reply, subscription stream.
2. **par cannot type multiplexing layer.** Wire routing by
   session_id/token is multi-party — outside par's type system.
3. **Panic-on-drop appropriate for bridge** (→ ProtocolAbort),
   not for app-level violations (need graceful reporting).
4. **Server module → pane connection lifecycle.** Three-part
   scoping prevents deadlocks DLfActRiS §5 addresses.
5. **Queue module → pub/sub.** Enqueue/Dequeue with close
   continuation = cleanly terminated subscription.
6. **Branching via enums → service dispatch.** Each service is
   a branch; session type is enum of per-service sub-protocols.
