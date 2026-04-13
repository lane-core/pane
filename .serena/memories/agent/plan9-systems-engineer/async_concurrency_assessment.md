---
type: analysis
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [async, sync, calloop, event_loop, devmnt, mountio, mountmux, rio, thread2, alt, concurrency, Path_A, Path_B, Path_C]
sources: [devmnt.c (mountio:772, mountmux:934), rio.c (mousethread:461, alt:490), xfid.c (xfidctl:102, xfidallocthread:41), 8½ paper, agent/session-type-consultant/rumpsteak_smol_translation]
verified_against: [pane-app/src/looper.rs, pane-app/src/connection_source.rs, reference/plan9/src devmnt.c + rio source]
related: [architecture/looper, decision/server_actor_model, policy/feedback_per_pane_threading]
agents: [plan9-systems-engineer]
---

# Async vs. Sync Assessment for pane's Actor Loop

## Verdict: Sync (calloop) is correct for Phase 1. Async deferred to Phase 2 evaluation.

Session-type-consultant's Path A confirmed from Plan 9 perspective.

## Key findings

### pane's calloop loop IS rio's mousethread
rio.c:461 mousethread used alt() over mouse+reshape channels
in a for(;;) loop — structurally identical to calloop's poll-
dispatch. pane is not departing from Plan 9; it's following
rio's multiplexer pattern, not devmnt's per-process blocking.

### devmnt.c mountio is the wrong comparison
mountio (772-826) is N-processes-blocking-on-rendezvous with a
rotating reader gate (m->rip). It multiplexes replies via
mountmux (tag matching). Async would not simplify it — the
implicit state (sleeping processes, stack frames) would become
explicit (Future state machines) for no benefit, since callers
block on reply anyway.

### What async would gain (Phase 2+ value)
- Handler suspension: .await sub-requests without blocking
  thread (enables notification-triggers-request)
- Rumpsteak Sink/Stream integration
- Eliminate forwarding thread in Looper::run()

### What async would cost
- Cooperative scheduling trust (same I2/I3 contract, harder
  to reason about — missing .await is silent stall)
- Six-phase batch ordering across suspended handlers requires
  phase-aware scheduler
- Rc<RefCell<FrameWriter>> !Send preserved by LocalExecutor
  but precludes work-stealing migration
- calloop integration glue (EventSource wrapping executor) or
  full reactor replacement
- Overhead on non-suspending handlers (900K msg/sec baseline)

### Plan 9 lesson applied
Plan 9 never built an async runtime because the kernel scheduler
WAS the async runtime (cheap processes = cheap concurrency).
When intra-process concurrency was needed, thread(2) + alt()
provided CSP. rio's xfid pool (thread per outstanding request)
was async task spawning without language support. pane's per-pane
threading gives cross-pane isolation (like Plan 9 per-process
blocking). Intra-pane async would be like thread(2) inside rio —
useful when handlers need to coordinate, not yet needed.

### Phase 2 trigger conditions
Revisit async when: (1) notification-triggers-request lands and
self-messaging becomes unwieldy, (2) forwarder handlers need
cross-connection coordination, (3) Rumpsteak integration moves
from verification-only to runtime.
