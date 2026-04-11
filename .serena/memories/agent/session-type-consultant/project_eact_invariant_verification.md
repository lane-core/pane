---
name: EAct invariant verification (2026-04-03)
description: Full I1-I13 + S1-S6 verification against pane-app/pane-proto source — 9 satisfied, 2 convention-only, 8 need looper/server/build-config
type: project
---

Full EAct invariant verification against architecture.md and all pane-app + pane-proto source.

**Verdict: Conditionally sound.** Type structure correctly encodes EAct. Looper (not yet built) is enforcement site for 11 of 19 invariants.

**Satisfied (type-level or implemented):** I4, I5, I6, I7, I13, S1, S2, S5
- I4: #[must_use] + Drop on Pane, PaneBuilder<H>, ServiceHandle<P>, TimerToken (4/8 obligation types implemented)
- I5: MessageFilter<M: Message> + Message: Clone excludes obligation handles at type level
- I6/I7: &mut H exclusivity in Handles<P>::receive, Handler methods, DispatchEntry closures
- I13: PaneBuilder consumed by run_with — no open_service after looper starts (structural)
- S1: monotonic u64 in Dispatch::insert
- S2: &mut H in fire_reply/fire_failed
- S5: HashMap::remove in cancel

**Convention only (no enforcement path):** I2, I3
- I2: no blocking in handlers — EAct Global Progress, DLfActRiS receptive predicate. Cannot detect.
- I3: handler termination — halting problem. Process isolation is the backstop.

**Needs work (depend on looper/server/build-config):** I1, I8, I9, I10, I11, I12, S3, S4, S6
- I1/S6: panic=unwind not explicitly set in Cargo.toml profiles
- I8: send_and_wait thread-local check (looper)
- I9: destruction sequence ordering (looper)
- I10: Chan has no Drop impl, Transport has no non-blocking send
- I11/I12: framing layer (pane-server)
- S3: batch ordering (looper)
- S4: fail_connection ordering (looper)

**Key finding — Dispatch<H> correctly defunctionalizes EAct sigma:**
- insert = E-Suspend, fire_reply = E-React (remove + execute), fire_failed = affine gap compensation (pane-specific)
- Box<dyn Any> downcast pattern correct — closure captures R type at creation

**Key finding — state snapshot model safe under four conditions:**
1. Clone at quiescent point (between dispatch cycles)
2. Clone never written back to looper state
3. Clone used for reads only (pane-fs queries)
4. Only optic-accessible attributes snapshotted, not full H (H contains !Clone ServiceHandle)

**Why:** Rigorous verification before looper implementation.
**How to apply:** Use as checklist during looper implementation. Each "needs work" item has a specific location and mechanism identified.
