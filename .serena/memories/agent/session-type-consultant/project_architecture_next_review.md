---
name: architecture-next.md review (second pass, 2026-04-03)
description: Second-pass review — all 5 first-pass issues fixed, 1 moderate (Message not object-safe blocks filter chain), 2 minor (stream/suspend enforcement, send_and_wait cycle detection)
type: project
---

Second-pass review of docs/architecture-next.md after first-pass fixes applied.

**Verdict:** Conditionally sound. One moderate issue blocks filter chain implementation.

**First-pass issues — all fixed:**
1. E-Reset covers both Flow variants (line 47)
2. §4.3 corrected to §4.2.2 (line 73)
3. Destruction invariant gap documented — on_failed must NOT call send_request, abandoned entries cleared without callbacks (lines 757-765)
4. Token named in HashMap (line 380)
5. Display/Control bundling explicit (lines 974-978)

**New issues from second pass:**
1. **MODERATE (M3): Message trait not object-safe.** Clone supertrait prevents `&dyn Message` and `Box<dyn Message>`. Filter chain (lines 631-640) won't compile. Fix: typed filters `MessageFilter<P: Protocol>` (aligns with per-service registration already in spec) or clone-boxing pattern with DynMessage.
2. **Minor (M1):** Stream close before suspension — no enforcement mechanism specified. Who initiates close when server suspends mid-stream?
3. **Minor (M2):** send_and_wait same-Connection cycle deadlock — spec acknowledges the risk but doesn't say whether framework detects it.

**Implementation readiness:** Everything except filter chain can proceed. M3 must be resolved before filter chain coding.

**Why:** Tracks review state of the v2 architecture spec through iterative refinement.
**How to apply:** M3 resolution needed before filter chain implementation in Phase 1. M1/M2 can be deferred to Phase 3 (streaming/suspension).
