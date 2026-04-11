---
name: EAct vs MPST architectural decision
description: Assessment of bottom-up (par CLL + EAct) vs top-down (telltale MPST) composition for pane — Plan 9 perspective, 2026-04-03
type: project
---

Lane proposed dropping telltale (MPST) dependency in favor of homerolling EAct framework using par (CLL) for binary channels. Plan 9 engineer assessed five questions.

## Core position: EAct + par is the Plan 9 approach

Plan 9 had no global protocol specification. Each file server spoke 9P correctly on each channel; composition emerged from namespace assembly. This is structurally identical to EAct's Progress theorem: correct binary protocols + correct actor discipline = system progress.

## Key findings

1. **Bottom-up vs top-down:** Plan 9 was definitively bottom-up. No choreography, no global type. Mount/bind assembled composition; each server spoke 9P independently.

2. **MPST gap analysis:** MPST's global type would theoretically catch cross-process circular dependencies (A→B→A deadlock), but pane has dynamic topology and open participant sets — classical MPST assumes fixed roles. The cases MPST can't handle (partial failure) matter more than the cases it catches. I8 runtime check + fail-at-use-site (Ehangup) is the practical defense.

3. **Namespace independence:** pane-fs projects state, not protocol structure. Session type formalism is beneath the namespace, not visible through it. Clean separation — same pattern as Plan 9's /proc not exposing kernel scheduling state machines.

4. **Global spec rejection:** Plan 9 team would reject global choreography as "single point of specification brittleness." Adding a service is additive with binary protocols, multiplicative with a global spec.

5. **Recursion:** par's Server type constructor closes pane-session's Rec/Var gap. Manual fixpoint (finish → reuse transport) is fine for Phase 1-2; type-level recursion matters in Phase 3 (suspension/resumption).

## Flagged risk

Multiple protocol types (vs Plan 9's single 9P) make debugging harder. Recommended: invest in protocol-level tracing (9pcon equivalent) early.

**Why:** Foundational architectural decision about dependency graph and composition model.

**How to apply:** Reference when session type / protocol composition questions arise. The decision aligns pane with Plan 9's proven bottom-up model. Watch for the debugging gap as protocol count grows.
