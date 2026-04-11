# EAct Invariant Verification (2026-04-03)

Full I1-I13 + S1-S6 verification against pane-app/pane-proto source.

**Verdict: Conditionally sound.** Type structure correctly encodes EAct. Looper (not yet built) is enforcement site for 11 of 19 invariants.

## Satisfied (type-level or implemented)

- **I4:** #[must_use] + Drop on Pane, PaneBuilder<H>, ServiceHandle<P>, TimerToken (6/8 obligation types (Pane, PaneBuilder, ServiceHandle, TimerToken, ReplyPort, CompletionReplyPort))
- **I5:** MessageFilter<M: Message> + Message: Clone excludes obligation handles at type level
- **I6/I7:** &mut H exclusivity in Handles<P>::receive, Handler methods, DispatchEntry closures
- **I13:** PaneBuilder consumed by run_with — no open_service after looper starts (structural)
- **S1:** monotonic u64 in Dispatch::insert
- **S2:** &mut H in fire_reply/fire_failed
- **S5:** HashMap::remove in cancel

## Convention only (no enforcement)

- **I2:** no blocking in handlers — halting-problem-adjacent, cannot detect
- **I3:** handler termination — process isolation is the backstop

## Needs work (depend on looper/server/build-config)

- **I1/S6:** panic=unwind not explicitly set in Cargo.toml profiles
- **I8:** send_and_wait thread-local check (looper)
- **I9:** destruction sequence ordering (looper)
- **I10:** Chan has no Drop impl, Transport has no non-blocking send
- **I11/I12:** framing layer (pane-server)
- **S3:** batch ordering (looper)
- **S4:** fail_connection ordering (looper)

## Key findings

- **Dispatch<H> correctly defunctionalizes EAct sigma:** insert = E-Suspend, fire_reply = E-React, fire_failed = affine gap compensation
- **State snapshot model safe under four conditions:** clone at quiescent point, clone never written back, clone used for reads only, only optic-accessible attributes snapshotted (not full H)

**How to apply:** Checklist during looper implementation. Each "needs work" item has a specific location and mechanism identified.