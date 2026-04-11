---
name: EAct homeroll architecture analysis
description: Analysis of proposal to homeroll EAct framework using par for CLL, eliminating MPST layer — conditionally sound for binary-only sessions
type: project
---

Proposal: par (CLL binary) + homerolled EAct (actor layer) replaces par + telltale (MPST) + EAct.

**Verdict: conditionally sound for pane's binary-only architecture.**

Key findings from 2026-04-03 analysis:

1. **EAct uses MPST, does not replace it.** EAct's type system is parameterized by a compliance oracle (Definition in §4, extended paper lines 2557-2584). For binary sessions, CLL duality IS a valid compliance oracle (Wadler JFP 2014, Theorem 1). For multi-party (3+ participants in one session), you'd need Scribble/telltale/manual proof.

2. **Cross-session deadlock prevention via EAct Global Progress** (Corollary cor:global-progress) depends on: I6 (single-threaded dispatch), I2/I8 (no blocking in handlers), I3 (handlers terminate). These map exactly to pane's existing invariants.

3. **The architecture spec already IS an EAct implementation.** Handler=EAct actor, Handles<P>=handler store sigma, Dispatch<H>=E-Suspend/E-React, Looper=sequential dispatch, PaneBuilder<H>=pre-reactive sigma construction. Homerolling is pattern recognition, not greenfield.

4. **Trust surface is smaller** without telltale (pre-alpha). CLL duality sufficient for binary compliance. Trust surface = par's correctness + pane's invariant maintenance.

5. **Unsoundness trigger**: introducing multi-party protocols (3+ participants in single session) without a compliance oracle beyond CLL duality. Currently not in Phase 1/2/3.

**Why:** Lane proposed eliminating the MPST layer entirely.
**How to apply:** When evaluating future protocol additions, flag any multi-party (non-binary) sessions as requiring a compliance mechanism beyond HasDual. All current pane protocols are binary (pane <-> server).

Key EAct paper references used:
- Compliance: Definition §4 (lines 2557-2584)
- Progress: Theorem thm:progress (lines 2973-2989)
- Global Progress: Corollary cor:global-progress (lines 3139-3144)
- Inter-session deadlock argument: §4.2 (lines 2860-2931)
- Independence of Thread Reductions: Lemma lem:gp:independence (lines 3088-3100)
- Implementation uses Scribble global types: §6 (lines 3910-3918)
