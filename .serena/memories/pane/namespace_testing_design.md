# Namespace Testing Design

Merged from Plan 9 systems engineer (test patterns) and session-type consultant (invariant observability mapping). 2026-04-05.

## Plan 9 test patterns applied to pane-fs

1. **Synchronous ctl writes (recommended, not yet confirmed by Lane).** `echo cmd > /pane/n/ctl` blocks until looper processes the command and updates the snapshot. Without this, tests degrade to poll loops. Plan 9's /proc ctl was synchronous. Budget: filesystem tier 15-30us, looper round-trip within budget. Design decision: FUSE write handler must block on looper response channel.

2. **attrs.json as machine test oracle.** One FUSE read, one snapshot, all attributes. Individual attrs/ files are the human interface. attrs.json is how tests verify cross-attribute consistency.

3. **Three test levels.** Single-file sanity (FUSE wiring), write-then-read (ctl→state→read loop), lifecycle (create/exercise/close/verify cleanup). Most value in level 2.

4. **Snapshot boundary accepted.** Cross-file reads have no consistency guarantee (same as Plan 9 /proc). Within a single FUSE request, consistency guaranteed by clone-based snapshot. Tests needing consistency use attrs.json or synchronous ctl as barrier.

5. **No Plan 9-style implicit test suite existed.** Plan 9 relied on daily use (ps, acid, acme using /dev/wsys). Pane must synthesize that pressure with explicit shell test scripts.

## Invariant observability mapping

All 19 invariants classified by namespace observability:

- **Namespace-observable (7):** I1, I4, I6, I8, I9, I13, S4
- **Observable as degradation only (2):** I2, I3
- **Not namespace-observable (8):** I5, I7, I10, I11, I12, S1, S3, S5
- **Subsumed (2):** S2 (= I6), S6 (= I1)

## Minimal covering test set (4 tests, 7 invariants)

1. **Clean lifecycle:** I1 + I9 + I13
2. **Crash cleanup:** I1-backstop + I9-crash + I8
3. **Snapshot consistency:** I6 + S2
4. **Connection loss propagation:** S4

## Unique namespace value (not coverable by unit tests)

- I13: publication atomicity (pane appears with all attrs or not at all)
- I9: deregistration completeness (no zombie namespace entries)
- I6-through-snapshots: atomic snapshot updates under concurrent FUSE reads
- S4 publication path: fail_connection → handler → snapshot → namespace

**Key insight:** pane-fs tests verify the *publication boundary* — where internal looper state becomes externally observable. This is a separate failure surface from dispatch internals.

**How to apply:** Use the four minimal tests as foundation. The three namespace-unique invariants (I9, I13, I6-snapshots) are where pane-fs tests add value that dispatch.rs cannot provide.