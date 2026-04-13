---
type: reference
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [par, linear, affine, panic, drop, must_use, limitations, mapping, CLL]
extends: dependency/par/_hub
verified_against: par-0.3.10 source
agents: [all]
---

# par Linear Discipline + Limitations + Logic Mapping

## Enforcement mechanism

Three mechanisms:

1. **Move semantics:** Methods consume endpoints. After send(),
   Send is gone. Continuation is new endpoint. Ownership prevents
   use-after-move.
2. **#[must_use]:** All session types + continuation-returning
   methods. Compiler warns on ignored continuation.
3. **Panic on drop:** Endpoint dropped without completing protocol →
   peer panics. oneshot::Receiver::await panics "sender dropped";
   oneshot::Sender::send panics "receiver dropped". ONLY runtime
   compensation for affine gap.

**Summary:** Affine safety (use at most once) via move semantics.
Runtime linear safety (must use exactly once) via panic-on-drop.
Type system prevents *misuse*; runtime prevents *non-use* (via
panic only, not graceful error).

Panic compensation sufficient for in-process sessions (panic =
thread abort). For cross-process (IPC), panic only affects bridge
thread — other process needs separate detection (pane: ProtocolAbort
+ ServiceTeardown).

## What par CANNOT express

- **No subtyping.** Types exact. No S <: S'.
- **No dependent/refinement types.** Cannot constrain values.
- **No timeouts/failure modes.** Panic on drop. Failure must be
  encoded as protocol branch (Result) before failure point.
  Unanticipated transport failures → panics.
- **No delegation across processes.** Endpoints are in-process
  oneshot channels. Cannot serialize.
- **No multiparty.** Binary only. Multi-party needs coordinator
  holding multiple binary sessions. Deadlock freedom depends on
  acyclic topology.
- **No channel mobility across async.** Not Sync. Can Send (move
  to thread) but not share. Mutex defeats linearity.
- **No backpressure/flow control.** push/send always non-blocking,
  unbounded.
- **No runtime introspection.** Cannot query session state.
- **No graceful shutdown.** Dropping panics peer. No cancel/abort
  session type.

## Linear logic mapping (complete)

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
| `Proxy<C>` | Coexponential | Kokke/Montesi/Peressotti 2021 |
| `Server` | Coexponential server | |
| `Session::link` | Cut | Cut elimination / forwarding |
| `Session::fork_sync` | Cut (introduction) | Spawn dual pair |

par collapses 1 and ⊥ into `()`. "Linear logic with MIX" — MIX
rule allows identifying 1 with ⊥, which `() : Dual = ()` implements.
