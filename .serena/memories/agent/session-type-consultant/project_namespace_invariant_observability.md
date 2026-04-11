---
name: Namespace invariant observability analysis (2026-04-05)
description: Full mapping of I1-I13 + S1-S6 to namespace-observable vs internal-only, with minimal test set and publication-boundary analysis
type: project
---

Mapped all 19 invariants to namespace observability for pane-fs test planning.

**Namespace-observable (7):** I1, I4, I6, I8, I9, I13, S4
**Observable as degradation only (2):** I2, I3
**Not namespace-observable (8):** I5, I7, I10, I11, I12, S1, S3, S5
**Subsumed (2):** S2 (= I6), S6 (= I1)

**Minimal covering test set (4 tests, 7 invariants):**
1. Clean lifecycle: I1 + I9 + I13
2. Crash cleanup: I1-backstop + I9-crash + I8
3. Snapshot consistency: I6 + S2
4. Connection loss propagation: S4

**Unique namespace value (not coverable by unit tests):**
- I13: publication atomicity (pane appears with all attrs or not at all)
- I9: deregistration completeness (no zombie namespace entries)
- I6-through-snapshots: atomic snapshot updates under concurrent FUSE reads
- S4 publication path: fail_connection → handler → snapshot → namespace

**Key insight:** pane-fs tests verify the *publication boundary* — where internal looper state becomes externally observable. This is a separate failure surface from dispatch internals.

**Why:** Test plan basis for pane-fs namespace integration tests.
**How to apply:** Use the four minimal tests as the foundation. The three namespace-unique invariants (I9, I13, I6-snapshots) are where pane-fs tests add value that dispatch.rs cannot provide.
