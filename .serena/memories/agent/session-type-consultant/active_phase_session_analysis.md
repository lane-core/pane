---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: critical
keywords: [active-phase, session-types, par, multiplexing, bugs, affine-gap, flow-control, request-correlation, non-blocking, refactoring]
sources:
  - "[FH] §3.2 E-Send, §3.3 Lemma 1 (Independence), §4 E-RaiseS, Theorems 6-8"
  - "[JHK24] Theorem 1.2 (star topology progress)"
  - "[Gay & Hole 2005] Session subtyping"
  - "[MostrousV18] Affine sessions"
  - "par 0.3.10 source: exchange.rs, server.rs, queue.rs, lib.rs"
  - "decision/connection_source_design D1-D12"
verified_against:
  - "par/exchange.rs:169 (Send::send non-blocking)"
  - "par/exchange.rs:23 (oneshot transport)"
  - "par/server.rs:128 (mpsc::channel(0) rendezvous)"
  - "service_handle.rs:284 (blocking SyncSender::send — the bug)"
  - "connection_source.rs:120 (FrameReader WouldBlock state machine — the fix)"
  - "server.rs:119 (WriteHandle::enqueue try_send — the fix)"
related: [decision/connection_source_design, agent/session-type-consultant/architecture_abc_analysis, reference/papers/eact, reference/papers/dlfactris]
agents: [session-type-consultant]
---

# Active-Phase Session Type Analysis

Lane identified three adversarial bugs as symptoms of one root
cause: pane abandoned session type discipline after the handshake.
This analysis evaluates whether par session types can extend into
the active phase, and what pane-session patterns should replace
the current pane-app implementations.

## Core finding

Par's binary session types are the WRONG tool for the active
phase. The active phase is genuinely concurrent (both sides
send/receive simultaneously), multiplexed (N sessions per
connection), and multi-outstanding (M requests per session).
Par's types are sequential binary — Send<A, Recv<B>> enforces
alternation, which is the opposite of what the active phase
needs.

The correct linear logic reading of the active phase is par (⅋),
not tensor (⊗) — the optics-theorist's diagnosis of Bug 2 is
exactly right. But par the library enforces sequencing via
continuation types, not concurrency.

## What should move to pane-session

1. **FlowControl** — cap negotiation + tracking + enforcement.
   Currently pane-app/backpressure.rs, but negotiated in
   pane-session's handshake.
2. **RequestCorrelator** — token allocation, wire framing,
   timeout tracking. Protocol-level half of Dispatch.
3. **ActiveSession** — post-handshake state: FlowControl +
   negotiated params + connection identity. The planned
   ActivePhase<T> from PLAN.md.
4. **NonBlockingSend trait** — makes blocking sends from the
   looper thread unrepresentable at the type level.
5. **Pub-sub fan-out** — reusable communication pattern, not
   application logic.

## What stays in pane-app

- DispatchEntry<H> (handler-specific closures)
- ServiceHandle<P> typed API surface
- CancelHandle semantics
- Handler dispatch logic

## Par's runtime limitations for IPC

- Channels are unbounded (oneshot per continuation via fork_sync)
- No bounded backpressure mechanism
- In-process only (oneshot::channel, not unix sockets)
- Server uses mpsc::channel(0) for coordination, not data flow
- Dropping Send/Recv panics the peer ("sender/receiver dropped")
  — no graceful failure path like ReplyPort's Drop compensation

## Three bugs — precise session type diagnosis

Bug 1 (partial-frame): read_exact introduces "partially received
value" state that session types don't model. Fixed by
FrameReader WouldBlock state machine — restores atomic value
delivery guarantee.

Bug 2 (bidirectional buffer): SyncSender::send blocks (⊗
semantics) where the protocol requires non-blocking send (⅋
semantics). Fixed by D12 SharedWriter / try_send.

Bug 3 (head-of-line): Server actor thread blocked on one
connection's write, violating [FH] Lemma 1 (Independence of
Thread Reductions). Fixed by D12 per-connection writer threads.

## Verdict

Conditionally sound. Par stays for handshake (correct, valuable).
Active phase uses protocol-level type constraints (Protocol,
RequestProtocol traits) + linear obligation handles (ReplyPort,
CancelHandle) + runtime enforcement (FlowControl,
NonBlockingSend). Extracting FlowControl, RequestCorrelator,
and ActiveSession to pane-session is the right refactoring.
