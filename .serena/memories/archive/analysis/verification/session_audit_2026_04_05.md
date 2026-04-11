---
type: analysis
status: archived
archived: 2026-04-11
superseded_by: analysis/verification/session_audit_2026_04_06
created: 2026-04-05
last_updated: 2026-04-05
importance: low
keywords: [formal_verifier, session_audit, invariants, 2026-04-05, doc_drift, structural_issue]
related: [analysis/verification/_hub]
---

# Formal-Verifier Session-End Report (2026-04-05)

## Architecture conformance: GOOD
All 15 new types/flows match the spec. Provider model amendment (commit 3299bb6) is consistent.

## Invariant updates
- I6 (sequential dispatch): partial → LooperCore.run() sequential on mpsc
- I13 (open_service blocks): partial → server-side DeclareInterest works, PaneBuilder stub
- S4: server-side tested via remove_connection + ServiceTeardown synthesis (but delivery to peer not tested)

## Doc drift
- architecture.md:79 — stale `Recv<Welcome>` (should be `Recv<Result<Welcome, Rejection>>`)
- architecture.md:1092 — closure form takes one arg, code takes two
- PLAN.md:78-80 — pane-server section lists ProtocolServer unchecked but it's in pane-session

## Structural issue
ConnectionId defined in TWO places: server.rs:42 and dispatch.rs:16. Must reconcile when PaneBuilder wires to ProtocolServer.

## High-priority test gaps
1. Connection drop → ServiceTeardown delivery to peer (not just state cleanup)
2. Self-provide rejection (consumer_conn == provider_conn)
3. Session_id overflow boundary
4. ServiceHandle Drop → RevokeInterest (currently TODO)

## Heritage gaps
dispatch.rs should cite EAct handler store sigma (section 3.2, E-Suspend/E-React).

## Duploid analysis: VERIFIED
Server single-threaded actor, ServiceFrame all-positive, ReplyPort as ↑(continuation), MonadicLens mixed optic — all verified against code. ActivePhase<T> deferred (not implemented yet).
