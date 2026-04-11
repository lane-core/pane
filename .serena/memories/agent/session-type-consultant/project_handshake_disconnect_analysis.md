---
name: Handshake disconnect analysis
description: Transport disconnect during par-driven handshake — panic (Option A) is correct CLL encoding, error-through-channel (Option D) violates three-channel separation
type: project
---

Decision: handshake transport disconnect = panic (Option A), not error-through-channel (Option D).

**Verdict: Option A conditionally sound. Option D formally incorrect.**

Key findings (2026-04-03):

1. **CLL has exactly two session outcomes**: complete or annihilated. Par encodes this via #[must_use] + no Drop impl + panic on peer's oneshot::Canceled. No third "error" outcome exists in the formalism (Wadler JFP 2014, Theorem 2: cut elimination).

2. **Option D violates three-channel separation** (architecture.md lines 96-101). Transport failure is a crash-channel event. Routing it through par's protocol channel conflates infrastructure failure with protocol outcome. Also doesn't eliminate the panic path (transport can die before error is sent through par).

3. **EAct E-Raise** (Fowler et al. §5, lines 3287-3293) handles actor failure at any lifecycle point, including init phase (before reactive loop). Handshake panic = E-Raise during spawn-init. The "supervisor" is the OS process supervisor.

4. **Par has NO Drop impl** on Send/Recv (verified: no `impl Drop` anywhere in par-0.3.10/src). Linearity enforced by #[must_use] + peer panic on oneshot::Canceled.

**Four conditions for soundness:**
- C1: App::connect() panics on handshake failure (already in spec, line 610)
- C2: Bridge thread detached is fine for finite handshake
- C3: I1 (panic=unwind) must hold — load-bearing for oneshot cancellation propagation
- C4: Transport read timeouts needed for liveness (network partition during handshake)

**Why:** Lane asked whether panic or error-through-channel is the right encoding for transport disconnect during par-driven handshake phase.
**How to apply:** Handshake failures are always panics, never protocol-level errors. The catch_unwind boundary is at App::connect / PaneBuilder, not inside the handshake. Active-phase disconnect uses calloop -> disconnected() -> Flow::Stop (different regime entirely).
