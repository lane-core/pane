---
name: Three-tier session type architecture assessment (2026-04-03)
description: telltale (MPST) + par (CLL) + pane-session (IPC bridge) feasibility — conditionally sound, composition gap closed, two projection steps needed, vendor recommended
type: project
---

Assessed three-tier architecture: telltale (MPST global types, Lean-verified) + par (CLL binary, in-process) + pane-session (IPC bridge).

**Verdict:** Conditionally sound. The architecture is feasible and theoretically grounded.

**Key findings:**

1. **CLL-to-MPST bridge requires TWO projection steps**, not one. GlobalType -> LocalTypeR (telltale provides) -> per-partner binary session type (pane must implement). LocalTypeR retains partner annotations; CLL session types do not. The second step is the Scalas/Yoshida "Less is More" (POPL 2019) decomposition.

2. **Expressiveness gap:** pane-session has no recursion combinator (Mu/Var). telltale's LocalTypeR does. Recursive protocols from global specs have no codegen target in pane-session currently. Must be added.

3. **EAct + MPST + CLL compose as layers, not product.** MPST validates multi-party protocol design; CLL validates binary channels; EAct validates actor-level interleaving. Adding MPST strictly strengthens EAct by providing mechanized compliance verification (EAct Theorem 3.10's precondition).

4. **Gap closure:** Composition gap CLOSED for MPST-specified protocols (Harmony + coherence + progress theorems in Lean). Compliance gap PARTIALLY closed (only for globally-specified protocols). Conditional fidelity gap UNCHANGED (affine drop is intrinsic).

5. **telltale stability risk:** v10.0.0 in 5 weeks = 9 breaking changes. Recommend vendoring telltale-types (~200 lines) and implementing own projection. Lean proofs remain valid as reference.

6. **par dependency wrong:** par is async, in-process, panics on disconnect. pane-session already adapted patterns (Queue, Server, enum branching) but runtime models are incompatible. Continue adapting patterns, not code.

7. **Message trait unchanged:** Global spec reinforces service_id-on-Protocol (not Message). Adds exhaustiveness verification layer above the trait, doesn't modify trait design.

8. **Recommendation:** Defer telltale integration to Phase 2 (multi-server composition). Phase 1 binary sessions + EAct sufficient. When Phase 2 arrives: vendor types, implement projection, add recursion to pane-session, build codegen.

**Why:** Foundational architectural decision about session type layering.
**How to apply:** Reference when Phase 2 design begins. The two-step projection requirement and recursion gap are prerequisites for any MPST integration.
