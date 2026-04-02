---
name: Handler trait debate (session-type vs Be engineer)
description: Analysis of monolithic Handler vs per-protocol handlers -- concessions, attack points, key theorems cited
type: project
---

Session-type consultant position: monolithic Handler trait is conditionally sound for current single-protocol codebase, unsound for target multi-protocol architecture.

**Conceded to Be engineer:**
- Phase 3 (calloop plumbing) and Phase 4 (API surface / trait boundaries) should be staged separately
- Builder ceremony / wall-of-trait-bounds is a real usability concern -- not what C1 proposes
- Dead code elimination argument was wrong; default methods inline trivially
- Single port's total ordering is preserved by calloop multi-source batch model -- not broken by Phase 3

**Maintained against Be engineer:**
- Per-protocol channels (C1) is dictated by EAct KP3 and the Progress theorem structure
- Clipboard events flow through same batch/FilterChain -- typed ingress, unified processing
- Unified Message enum with 4 panic branches in Clone is a type-level lie (linear vs plain messages)
- No protocol phase enforcement in monolithic Handler (Ferrite Theorem 4.3 contrast)
- Cross-session state contamination via shared &mut self
- Filter chain is type-unsafe (can transform message variants, breaking session correspondence)

**Key citations used:**
- EAct Progress Theorem 3.10 (no inter-process deadlock due to event-driven non-blocking)
- EAct Lemma 3.14 (idle configuration can invoke handler for any active session)
- EAct KP3 (actors must support multiple simultaneous sessions)
- EAct E-React rule (handler store indexed by session endpoint)
- DLfActRiS connectivity graph acyclicity (linear resource duplication violates invariant)
- Ferrite Theorem 4.3 (protocol fidelity)
- Honda/Yoshida/Carbone MPST Definition 3.4 (global type projection determines local types)

**How to apply:** When Phase 3 design discussions arise, use the "typed ingress, unified batch" formulation. The calloop source is per-protocol, the batch processing is unified. This satisfies both the session-type requirement (per-protocol typing) and the Be engineer's requirement (filter chain sees everything, total ordering preserved).
