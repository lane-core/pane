---
name: Three-crate implementation review (updated 2026-04-03)
description: Review of pane-proto/pane-session/pane-app against architecture.md — conditionally sound, 2 moderate (Messenger crate boundary, Chan no Drop), 5 minor
type: project
---

Review of three-crate implementation against docs/architecture.md (par + EAct, three error channels).

**Verdict: Conditionally sound.** Skeleton accurately reflects spec structure. Two moderate issues block looper implementation.

**EAct correspondence:** Correct. Handler=Actor, Handles<P>=sigma static, Dispatch<H>=sigma dynamic (E-Suspend/E-React), Flow=E-Reset. send_request user-facing API not yet implemented (only raw Dispatch::insert). Box<dyn Any> downcast pattern is correct — closure captures R type at creation, framework-internal.

**CLL/Chan correctness:** Type-level encoding sound. PhantomData<par::exchange::Send/Recv> correctly uses par types as phantom state markers — runtime oneshot channels inside par types are correctly ignored. Duality checked for handshake (Dual<ClientHandshake>), but NOT checked for active-phase protocols across process boundary. Inherent to IPC — compensated by panic-on-deserialize-failure (I1).

**Moderate issues:**
1. Handles<P>::receive missing &Messenger parameter (spec requires it). pane-proto can't reference Messenger from pane-app. Resolution: Proxy trait in pane-proto, or restructure dispatch.
2. Chan<S,T> has no Drop impl — ProtocolAbort [0xFF][0xFF] not sent. Transport drop does close fd (backstop works), but peer can't distinguish protocol-abandoned from connection-broken.

**Minor issues:**
3. ServiceId missing UUID (spec requires { uuid, name })
4. fail_connection doesn't prevent send_request during destruction (runtime invariant, not enforced)
5. LifecycleMessage missing PaneExited and handler missing pane_exited/supported_properties
6. CLL additives (branching) still not expressible — Chan has no choose/offer
7. ServiceId Deserialize leaks strings — bounded-set assumption needs enforcement against attacker-sent IDs

**Confirmed sound:**
- Blanket Message impl — no orphan issues, obligation handles correctly excluded (!Clone, !Serialize)
- Dispatch<H> internal consistency — no circular deps, type erasure correct
- Message not object-safe — acknowledged in spec, filter chain uses typed MessageFilter<M>
- Par phantom-only usage — heavyweight dep for type-level only, but semantically correct

**Why:** Implementation review against unified architecture spec.
**How to apply:** Items 1-2 must be resolved before looper implementation. Items 3-7 during implementation.
