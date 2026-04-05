---
name: Optics x session types deep deliberation
description: Full analysis of profunctor optics as foundational state access composed with par session types — uncharted territory, eight specific findings
type: project
---

Deep deliberation from 2026-04-05 on whether profunctor optics could be the foundational state access mechanism composed with par's session types and EAct actors.

**Verdict: No unified calculus exists. Three orthogonal algebras that compose at the engineering level.**

Key findings:

1. **Session types CAN carry optics** but only in concrete representation (rank-2 profunctor form can't serialize/type-erase — confirmed by optics-theorist feasibility analysis + Clarke et al. Theorem th:profrep). Session fidelity/progress preserved because optics are passive values.

2. **Session duality and optic directionality are unrelated.** Session duality (send<->receive) operates on communication structure. Optic "duality" (lens<->prism via opposite categories, Clarke et al. after Definition def:prism) operates on data access patterns. They don't compose into a single duality notion.

3. **Linear lenses (Clarke et al. Definition def:linearlens, Riley 2018 §4.8) map directly to pane's obligation handles.** `S -> ([B,T] . A)` = decompose state into focused value + one-shot setter. ClipboardWriteLock IS a linear lens continuation. Current Message/obligation split = cartesian/linear optic split.

4. **Dependent session types needed for handshake-negotiated optics.** Par is non-dependent — can't express "continuation type depends on negotiated capabilities." TLL+C (Fu/Xi/Das) has the theory. Pane's runtime dispatch (enum-based active phase) is the pragmatic workaround.

5. **Remote optic laws don't hold without atomicity.** Session types provide communication correctness, not state consistency. PutGet violated by staleness, PutPut violated by reordering. ClipboardWriteLock mechanism (atomic lock) is the correct fix — session types govern the lock protocol, not the optic laws.

6. **PutPut justifies stream coalescing.** Queued lens updates coalesce by PutPut (last write wins). Valid for lenses; invalid for prisms, traversals, linear optics. Per-optic verification required.

7. **Optic subtyping lattice is the right structure for capability negotiation.** Client requests Lens, server may grant Getter (less powerful). No formal treatment of optic capability negotiation protocols exists.

8. **Current architecture already embodies the correct separation.** Session types = protocol. Optics = state decomposition. Actor model = concurrency. They compose at engineering level without unified formalism.

**Why:** Lane exploring radical "optics-first" redesign as thought experiment.
**How to apply:** When evaluating future designs, use this as the reference for what optics + session types can and cannot provide. The three-concern separation is validated, not accidental.
