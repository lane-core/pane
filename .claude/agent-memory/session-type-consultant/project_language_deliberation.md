---
name: Language deliberation -- Haskell/OCaml vs Rust for session layer
description: Analysis of moving core pane subsystems to Haskell or OCaml for better session type support -- verdict is stay with Rust+par
type: project
---

Deliberation conducted 2026-04-05. Lane explored moving core (session layer, actor framework, dispatch) to Haskell or OCaml, keeping Rust for ecosystem-dependent parts.

**Verdict: Stay with Rust + par.** Migration cost exceeds formal/ergonomic gains for pane's binary-only protocol surface.

Key findings:

1. **OCaml CBV advantage is real but ergonomic, not foundational.** Gay/Vasconcelos (JFP 2010, S5) show session fidelity holds under both CBV and CBN. Haskell would need pervasive strictness annotations (NFData on all sent types, strict fields on obligation handles). OCaml avoids this.

2. **OCaml has no static session type library.** ocaml-mpst (Imai et al., ECOOP 2020) is runtime monitoring only. FuSe (Padovani, ECOOP 2017) is closest to static but unmaintained. You'd build from scratch.

3. **Haskell LinearTypes (%1 ->) is the one thing that would genuinely close the affine gap.** DLfActRiS Theorem 1.2 requires linearity for global progress. But pane's I6 (single-threaded dispatch) prevents the exact deadlock scenario DLfActRiS identifies (two-thread channel drop). Architectural compensation sufficient.

4. **OCaml 5 algebraic effects would eliminate bridge threads** and give direct-style send_request. But pane already plans calloop-based active phase (no bridge threads there). Handshake bridge is ~50 lines.

5. **FFI boundary (OCaml/Haskell <-> Rust) introduces NEW trust boundary** that doesn't exist today. Generated typestate clients could recover most compile-time safety for Rust side.

6. **par is the most mature session type library in any language** for practical use. Actively maintained, CLL-grounded, production-quality.

7. **The scenario that would change this answer:** multi-party session types (3+ participants, not bilateral). Not in pane's roadmap.

**Why:** Lane asked for honest assessment of language alternatives.
**How to apply:** If MPST is ever proposed, revisit this analysis. For binary-only protocols, Rust+par is the right choice.
