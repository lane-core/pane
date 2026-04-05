---
name: Namespace-as-test-surface design
description: Plan 9 synthetic filesystem testing patterns applied to pane-fs — synchronous ctl, snapshot consistency, shell test idioms
type: project
---

Design session on using /pane/ namespace as integration test oracle (2026-04-05).

## Key decisions and recommendations

1. **Synchronous ctl writes.** Recommended that `echo cmd > /pane/n/ctl` blocks until the looper processes the command and updates the snapshot. This is the critical design choice — without it, tests degrade to poll loops. Plan 9's /proc ctl was synchronous (write returns after command takes effect). Budget: filesystem tier is already 15-30us, adding looper round-trip is within budget.

2. **attrs.json as machine test oracle.** One FUSE read, one snapshot, all attributes. Individual attrs/ files are the human interface. attrs.json is how tests verify cross-attribute consistency. Plan 9 lacked this — /proc/pid/status was the closest (fixed format, not extensible).

3. **Three test levels.** Single-file sanity (FUSE wiring), write-then-read (ctl->state->read loop), lifecycle (create/exercise/close/verify cleanup). Most value in level 2.

4. **Snapshot boundary accepted.** Cross-file reads have no consistency guarantee (same as Plan 9 /proc). Within a single FUSE request, consistency is guaranteed by clone-based snapshot. Tests that need consistency use attrs.json or synchronous ctl as barrier.

5. **No plan9-style /proc formalized test suite existed.** Plan 9 relied on daily use (ps, acid, acme using /dev/wsys) as implicit integration tests. Pane must synthesize that pressure with explicit shell test scripts.

## Open question

Lane has not confirmed synchronous ctl. This is a design decision that affects pane-fs architecture (FUSE write handler must block on looper response channel).

**Why:** Lane asked how Plan 9 tested synthetic filesystems, specifically whether namespace properties could serve as integration test surface for pane-fs.

**How to apply:** Reference when implementing pane-fs FUSE handlers (especially ctl write path) and when writing the integration test harness. The shell test patterns are directly usable as test scripts.
