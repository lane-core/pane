---
type: hub
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [verification, invariants, I1-I13, S1-S6, audit, session_audit, spec_fidelity, test_coverage, fs_scripting, namespace_testing, deadlock]
related: [status, architecture/looper, architecture/proto, architecture/session, architecture/app, architecture/fs, analysis/eact/_hub, analysis/session_types/_hub, agent/formal-verifier/_hub]
agents: [formal-verifier, pane-architect, session-type-consultant]
---

# Verification analysis cluster

## Motivation

Ground truth for the invariant enumeration I1–I13 and S1–S6.
The session-type model is the specification; the tests are
the proof; the audits are where we make sure both are
current. Pane chases 19/19 invariant coverage (detection-
enforced for invariants the type system cannot rule out) and
this cluster records *how* that coverage was reached, *what*
audits turned up, and *which* gaps were resolved on which
date.

The cluster is the formal-verifier agent's home base. Any
claim of the form "this invariant holds because …" should
cite one of the spokes; any change to `architecture/looper`,
`architecture/session`, or the dispatch layer should re-
verify the relevant spoke.

## Spokes

- [`analysis/verification/session_audit_2026_04_06`](session_audit_2026_04_06.md)
  — the comprehensive 2026-04-06 verifier session: I1–I13
  / S1–S6 status table with movement notes, deadlock
  analysis (DAG topology, leaf-lock write handle),
  polarity audit, test gaps N1–N5, doc-drift report. The
  current institutional record.
- [`analysis/verification/spec_fidelity`](spec_fidelity.md) —
  architecture.md vs implementation divergences at the
  93-test baseline. Five critical findings. Baseline is
  stale (279 tests current) but the divergence list should
  be re-audited before citing.
- [`analysis/verification/test_coverage`](test_coverage.md)
  — mapping of the then-current test suite to I1–I13 /
  S1–S6 coverage. Infrastructure-blocked gaps called out.
  Re-run against 246 + 28 + 5 before citing gaps.
- [`analysis/verification/fs_scripting`](fs_scripting.md) —
  validation of pane-fs against 10 real BeOS scripting
  scenarios. 7/10 clean, 1/10 better than BeOS, 2/10
  needing attention. Informs the ctl syntax and
  per-signature index work.
- [`analysis/verification/namespace_testing`](namespace_testing.md)
  — Plan 9 test patterns applied to pane-fs: invariant
  observability mapping (7 filesystem-observable, 2
  degradation-only, 8 not-observable, 2 subsumed) and
  minimal covering test set.

- [`analysis/verification/invariants/inv_rw`](invariants/inv_rw.md)
  — defines Inv-RW (Request-Wait graph acyclicity), the
  load-bearing progress invariant. Three guarantees (I2, I8,
  protocol-scoped send_request). Distinguishes from [JHK24]
  Theorem 1.2 (local star topology) and [FH] EAct progress
  (whole-system). Created 2026-04-11 per O6 in
  `decision/connection_source_design`.

Archived (superseded, kept for provenance):

- `archive/analysis/verification/wiring_soundness_2026_04_06`
  — foundational PaneBuilder ↔ ProtocolServer soundness
  analysis; action items delivered post-04-06.
- `archive/analysis/verification/session_audit_2026_04_05`
  — one-day-older snapshot of the session audit;
  superseded by 2026-04-06.

## Open questions

- **Test counts.** Spec fidelity and test coverage spokes
  were written at 93 tests. Current `status` reports 246
  regular + 28 stress + 5 integration. A full re-baseline
  is pending.
- **I12 spec drift.** Permissive codec + looper soft-drop
  is the current implementation; architecture.md still
  describes a connection-level error. Candidate for the
  next spec pass.
- **Agda formalisation.** Four properties identified
  (ReplyPort exactly-once, Dispatch one-shot, destruction
  sequence ordering, install-before-wire). Deferred until
  architecture stabilises; flagged in `decision/server_actor_model`.

## Cross-cluster references

- `status` — current invariant status (19/19 verified or
  detection-enforced). The hub's spokes are the detail
  behind the tally.
- `architecture/looper` — implementation of S3 (six-phase
  batch ordering), I2/I3 watchdog, I8 ThreadId check, I9
  catch_unwind.
- `architecture/proto`, `architecture/session`,
  `architecture/app`, `architecture/fs` — per-crate
  invariant tables; this cluster is the cross-crate audit
  view.
- `analysis/eact/_hub` — theoretical grounding for the I /
  S numbering; the session audit spoke references EAct
  theorems directly.
- `analysis/session_types/_hub` — principles the audits
  measure against.
- `agent/formal-verifier/_hub` — the verifier agent's
  charter, including the doc-drift-report obligation from
  `policy/agent_workflow` Step 4.
