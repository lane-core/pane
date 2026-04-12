---
type: analysis
status: current
supersedes: [agent/session-type-consultant/revoke_interest_channel_analysis]
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [RevokeInterest, ServiceHandle, Drop, hybrid, leave, zapper, EAct, LinearActris, deferred_cancellation, affine_gap, two_cleanup_paths, batch_ordering, phase_4]
sources:
  - "[FH] §4 Discussion, lines 3879-3889 (leave construct)"
  - "[FH] Theorem 6 (Preservation under failure)"
  - "[FH] Theorem 8 (Global Progress under failure)"
  - "[FH] §4 E-RaiseS, E-CancelMsg, E-CancelH (cascading failure)"
  - "[JHK24] Theorem 1.2 (global progress adequacy)"
  - "[JHK24] cgraph.v:1192-1225 (exchange_dealloc)"
  - "[MostrousV18] Affine Sessions (affine drop legality)"
  - "[FowlerLMD19] Exceptional Asynchronous Session Types (async cancellation notification)"
verified_against: ["eventactors-extended.tex@2026-04-12", "2024-popl-dlfactris.pdf@2026-04-12", "iris-actris cgraph.v@2026-04-12"]
related: [decision/connection_source_design, architecture/looper, architecture/app, reference/papers/eact, reference/papers/dlfactris]
agents: [session-type-consultant]
---

# RevokeInterest hybrid evaluation (follow-up)

Evaluation of the synthesis Option 3 (hybrid) from the four-agent
roundtable on RevokeInterest channel routing. Supersedes the
initial analysis that endorsed the status quo.

## Verdict: endorse hybrid (Option 3), conditionally sound

The hybrid — Drop marks session locally, looper batches
RevokeInterest into next write flush — is strictly superior to
the status quo on ordering, progress, and formal correspondence.

## Formal correspondence

The hybrid maps directly to EAct's `leave(v)` construct
([FH] §4 Discussion, lines 3879-3889):

    actor{E[leave(v)], H, I, M} --> actor{idle(v), H, I, M} || zap(s.p)

- Local mark = actor transitions to idle(v) (no communication)
- Batched wire send = zapper thread zap(s.p) (async cancellation)

Preservation under failure (Theorem 6) and Global Progress
under failure (Theorem 8) both hold with zapper threads
present. The zapper need not fire atomically with the actor's
state transition — eventual execution suffices.

This is NOT buffered session types or asynchronous subtyping.
It's best called **deferred protocol termination**.

## Ordering advantage over status quo

Status quo: try_send from arbitrary thread context races with
looper's own sends. Hybrid: looper sends in phase 4 (ctl
writes), sequenced after phases 1-3 (reply/failed/teardown),
before phase 5 (new requests). Gives causal ordering:
protocol obligations resolved before revocation notification.

## Two-cleanup-paths analysis

Be's concern is valid but the pattern is standard. EAct has
both `raise`/`leave` (eager) and crash detection via E-Monitor
(backstop). Theorems 6-8 are proved for the combined system.

Risk: divergent effects. Mitigation: process_disconnect must
be idempotent w.r.t. prior RevokeInterest (skip already-
removed routes). Currently satisfied by routing table walk.

## Required invariants

**H1 (Looper liveness):** Looper runs another batch after local
mark. Guaranteed by calloop EventLoop + process_disconnect
backstop if looper dies.

**H2 (Idempotent cleanup):** process_disconnect skips sessions
already removed by RevokeInterest. Currently satisfied;
document as explicit invariant.

**H3 (Stale dispatch suppression):** After local revocation
mark, incoming frames for that session are dropped, not
dispatched. Requires revoked_sessions set in looper, checked
during phase-5 dispatch. New requirement the hybrid introduces.

**Batch phase:** RevokeInterest wire send goes in phase 4
(ctl writes), currently a stub in the six-phase batch ordering.
