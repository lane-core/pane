# Test Coverage Audit (2026-04-05)

Audit of 93 tests across pane workspace against architecture spec invariants I1-I13, S1-S6.

## Coverage summary

- **Sufficient:** I4 (implemented types), I9, S4, S5
- **Partial:** I1, S1

## Critical gaps — ALL RESOLVED (2026-04-05)

1. property.rs PutPut now compares full EditorState (not focused field). monadic_lens.rs assert_monadic_lens_laws also compares full S. claim_13 demonstrates GetPut as catch for idempotent dirty-flag bugs.
2. Float Display/FromStr roundtrip added (claim_7_display_fromstr_roundtrip_f64). Tests exact values, documents lossy values, demonstrates text-level PutGet risk ("0.10" -> "0.1").
3. Post-exit dispatch guard (exited flag) added to LooperCore. dispatch_after_exit_returns_exit_immediately verifies callback does NOT execute.

## Former false-confidence cases — resolved

- property.rs PutPut: full state comparison
- handler_not_reused_after_panic: exited guard, callback provably blocked
- phase1_catches_bad_transport: renamed to phase1_accepts_valid_transport (honest name)
- I5 (filter bypass for obligations): still untested, infrastructure-blocked

## Important gaps (blocked on infrastructure)

ProtocolAbort (I10/I11), concurrent snapshot (I6), filter bypass for obligations (I5/B7), send_and_wait looper check (I8), batch coalescing (S3), ServiceHandle Drop, transport death mid-handshake

**How to apply:** Before adding new tests, check this audit for existing partial coverage. When implementing blocked infrastructure, add corresponding tests from the gaps list.