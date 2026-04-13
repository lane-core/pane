---
type: decision
status: decided
created: 2026-04-12
last_updated: 2026-04-12
importance: critical
keywords: [par, session_types, multiplexer, NonBlockingSend, Enqueue, Dequeue, ReplyPort, SubscriberSender, DeclareInterest, SessionMultiplexer]
related: [dependency/par, decision/pane_session_mpst_foundation, decision/mpst_extraction_plan, policy/intermediate_state_principle]
agents: [session-type-consultant, optics-theorist, plan9-systems-engineer, be-systems-engineer]
---

# par Integration Architecture

Decided 2026-04-12 after four-agent roundtable on how par's
session type runtime integrates with pane-session. Lane's
directive: use par at every possible opportunity. Bar for NOT
using par: really impractical or obviously wrong semantics.

## Principle

par's runtime (oneshot channels, Queue, Server) used where two
independent execution contexts exist. par's type algebra (Send,
Recv, Dual, Queue, branching) used as formal protocol specification
everywhere. Where par's runtime doesn't fit (same-thread
callback dispatch), pane-session implements par-equivalent
discipline with honest justification.

## par Runtime — Direct Use

### Subscriptions: Enqueue<T> / Dequeue<T>

SubscriberSender<P> holds `par::queue::Enqueue<P::Message>`
directly. Provider pushes via Enqueue::push (non-blocking).
Consumer pops via Dequeue::pop or into_stream1(). Close on
disconnect via Enqueue::close1(). Cleanest par fit — all four
agents unanimous.

Replaces: `SyncSender<(u16, Vec<u8>)>` in SubscriberSender.

### ReplyPort: Send<Result<T, ()>>

ReplyPort wraps `par::exchange::Send<Result<T, ()>>`. Calling
reply(value) calls send1(Ok(value)). Drop fires send1(Err(())).
Par's move semantics enforce exactly-once. Drop compensation
(send Failed) preserved for graceful degradation.

Replaces: current closure-based ReplyPort with par-grounded
linear obligation.

### Session establishment: Send/Recv exchange

DeclareInterest as par session:
`Send<DeclareInterest, Recv<Result<Accepted, Declined>>>`.
Bounded exchange like handshake. Two execution contexts (handler
thread, bridge thread). Par's proven pattern.

### Handshake: already shipped

ClientHandshake = Send<Hello, Recv<Result<Welcome, Rejection>>>.
Unchanged.

## par Runtime — Does Not Fit (with justification)

### Per-request token correlation

Both handler and multiplexer on same looper thread. Par's async
Recv doesn't compose with FnOnce(&mut H) -> Flow callbacks.
HashMap::remove is isomorphic to par's move-semantic consumption
— same exactly-once guarantee, compatible with synchronous
dispatch. SessionMultiplexer routing table manages virtual par
sessions without par's oneshot channels.

All four agents agree. Session-type agent's formal analysis:
par oneshots and DispatchEntry are isomorphic for one-shot
request/reply on a single thread.

### Server event loop

par Server uses async mpsc(0). ProtocolServer uses calloop.
Incompatible event models. par Server used as specification
reference for deadlock freedom analysis (Kokke/Montesi/Peressotti
2021 coexponentials). ProtocolServer stays.

### Wire codec

FrameReader/FrameWriter handle bytes, not typed values. Par
assumes in-process channels. Codec boundary (T ↔ Vec<u8>) is
outside par's model.

### Batch phase ordering

Scheduling policy, not protocol. Par has no message priority.
Sequential code in dispatch_batch IS the ordering guarantee.

## SessionMultiplexer (replaces ActiveSession + RequestCorrelator)

Single post-handshake state container with per-session tracking:

- sessions: HashMap<u16, SessionSlot> — per-session token sets
- next_token: u64, request_cap: u16 — global token alloc + cap
- peer: PeerScope, max_message_size: u32 — negotiated params
- SessionSlot: pending_tokens, revoked flag, version

Key improvement over prior design: per-session token tracking
enables cascade_session_failure to return REAL TeardownSets
(not empty stubs). Each (session_id, token) pair represents a
virtual par session of type Send<Req, Recv<Result<Resp, ()>>>.

## Ad-hoc type fates

| Type | Fate | Reason |
|---|---|---|
| NonBlockingSend | Stays | Wire-boundary N1 enforcement. Par validates design, can't replace across process boundary. |
| RequestCorrelator | Absorbed into SessionMultiplexer | Token alloc + per-session tracking unified. |
| ActiveSession | Absorbed into SessionMultiplexer | Richer version with per-session slots. |
| TeardownSet | Stays | Affine-gap compensation. Now populated correctly by multiplexer. |
| Backpressure | Stays | Wire-boundary error vocabulary. |

## SyncSender<(u16, Vec<u8>)> replacement plan

6 production sites holding raw channel type:

| Site | Replacement |
|---|---|
| SubscriberSender.write_tx | par::queue::Enqueue<P::Message> |
| ServiceDispatch::send_reply | par::exchange::Send<Result<T, ()>> via ReplyPort |
| ServiceDispatch::send_failed | Same Send endpoint, Err(()) path |
| ServiceHandle.write_tx | MuxSender to SessionMultiplexer |
| LooperCore.write_tx | MuxHandle to SessionMultiplexer |
| Messenger.write_tx | MuxSender (creates Enqueue endpoints for subscribers) |

## Provenance

Lane directed par integration after identifying that MPST
extraction created ad-hoc types instead of building on par as
explicitly instructed. Four-agent roundtable 2026-04-12 determined
where par runtime fits (subscriptions, ReplyPort, session
establishment) and where it doesn't (per-request correlation,
server event loop, wire codec, batch ordering). Lane accepted
this framing.

Session-type agent read all par 0.3.10 source, wrote
dependency/par digest. All agents read the digest + current
pane-session code. Convergence on hybrid architecture: par
runtime where two execution contexts exist, par-equivalent
discipline on same-thread paths. SessionMultiplexer replaces
ActiveSession + RequestCorrelator with per-session token tracking.
