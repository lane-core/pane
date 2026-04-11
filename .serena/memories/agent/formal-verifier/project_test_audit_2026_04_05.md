---
name: Test suite audit 2026-04-05
description: Comprehensive audit of 77 tests against I1-I13, S1-S6 invariants. 3 critical gaps resolved 2026-04-05. 8 important gaps remain (infrastructure-blocked).
type: project
---

Audit of 77 tests across pane workspace against architecture spec invariants.

**Sufficient coverage:** I4 (implemented types), I9, S4, S5
**Partial:** I1, S1

**Critical gaps — ALL RESOLVED (2026-04-05):**
1. property.rs PutPut now compares full EditorState (not focused field). monadic_lens.rs assert_monadic_lens_laws also compares full S. claim_13 demonstrates GetPut as catch for idempotent dirty-flag bugs.
2. Float Display/FromStr roundtrip added (claim_7_display_fromstr_roundtrip_f64). Tests exact values, documents lossy values, demonstrates text-level PutGet risk ("0.10" -> "0.1").
3. Post-exit dispatch guard (exited flag) added to LooperCore. dispatch_after_exit_returns_exit_immediately verifies callback does NOT execute. handler_not_reused_after_panic and multiple_flow_stop_does_not_double_destruct updated.

**Former false-confidence cases — status:**
- property.rs PutPut: RESOLVED (full state comparison)
- handler_not_reused_after_panic: RESOLVED (exited guard, callback provably blocked)
- phase1_catches_bad_transport: RESOLVED (renamed to phase1_accepts_valid_transport — honest name, no longer misleading)
- I5 (filter bypass for obligations): still untested, infrastructure-blocked

**Important gaps (blocked on infrastructure):**
ProtocolAbort (I10/I11), concurrent snapshot (I6), filter bypass for obligations (I5/B7), send_and_wait looper check (I8), batch coalescing (S3), ServiceHandle Drop, transport death mid-handshake

**Also noted:** claim_11 renamed from claim_11_close_violates_getput to claim_11_close_inexpressible_as_monadic_lens (more accurate).

**Why:** Establishes baseline for what's verified vs. assumed. Prioritizes work when implementation catches up.

**How to apply:** Before adding new tests, check this audit for whether the invariant is already partially covered. When implementing blocked infrastructure, add the corresponding test from the "important gaps" list.
