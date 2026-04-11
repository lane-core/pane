---
name: Architecture spec full review (2026-04-03)
description: Comprehensive review of docs/architecture.md — conditionally sound, 2 moderate issues (KP2 misattribution, unknown discriminant gap), 9 minor, 4 new invariants proposed
type: project
---

Full review of architecture spec against session-type theory, cross-referenced with pane-session implementation.

**Verdict:** Conditionally sound. No critical issues.

**Two moderate issues:**
1. EAct KP2 cited for obligation/value split — wrong. KP2 is about API style (no explicit channels), not about separating obligations from values. The principle is correct but should cite affine/linear type distinction or EAct's TH-Handler type/value separation.
2. Unknown service discriminant rejection not specified. Frames on undeclared service IDs need explicit reject behavior, not silent processing. New invariant I12 proposed.

**Key positive findings:**
- EAct sigma -> Dispatch mapping is correct defunctionalization (TH-Handler Fig. 8)
- E-Suspend/E-React -> send_request/reply lifecycle is accurate
- CLL correspondence in types.rs/dual.rs is standard and correct
- Ferrite Theorem 4.3 citation correctly scoped ("in the sense of")
- Handshake-to-active-phase transition is the right architecture for multiplexed extensible protocols

**Four new invariants proposed (I10-I13):**
- I10: Chan Drop must not block (from Phase 1 review, not yet in spec)
- I11: [0xFF,0xFF] not valid postcard (from Phase 1 review, not yet in spec)
- I12: Unknown service discriminant must be rejected (new)
- I13: DeclareInterest handle must not be usable before InterestAccepted (new)

**Why:** Establishes review baseline for the v2 architecture spec before Phase 1 implementation.
**How to apply:** Reference when implementing Phase 1 items from PLAN.md. The two moderate issues should be fixed in the spec. The four new invariants should be added to the Linear Discipline section.
