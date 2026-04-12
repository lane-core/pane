---
type: project
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [RevokeInterest, Tclunk, clunk5, backpressure, data_channel, ctl_channel, process_disconnect, freefidpool, Phase2, fid_audit]
related: [decision/connection_source_design, agent/plan9-systems-engineer/o1_backpressure_review, agent/plan9-systems-engineer/project_connectionsource_review_r2]
agents: [plan9-systems-engineer]
---

# RevokeInterest channel analysis (plan9-systems-engineer)

Lane asked 2026-04-11 whether RevokeInterest (sent on ServiceHandle
drop, analog of Plan 9 Tclunk) should move from the bounded data
channel to a guaranteed ctl channel. Analysis grounded in Plan 9
Tclunk semantics and lib9p connection lifecycle.

## Recommendation: keep status quo (data channel, try_send)

RevokeInterest stays on the data channel with try_send (best-effort).
process_disconnect is the authoritative cleanup (same role as
freefidpool in lib9p/srv.c:735). RevokeInterest is the eager
optimization, not the correctness mechanism.

## Plan 9 precedent

- Tclunk went through the same fd as all other 9P messages. No
  separate channel. No special congestion handling.
- Kernel mount driver (devmnt.c) used blocking writes to kernel
  pipe — "try_send dropping Tclunk" scenario didn't arise because
  pipe writes blocked the process, not dropped.
- If connection died before Tclunk sent: fids leaked until
  connection close. freefidpool (srv.c:735) walked entire fid hash
  at EOF, calling destroyfid on every entry.
- Acceptable because: fid namespace per-connection, fids lightweight,
  connection close is the authoritative GC. Designed around the
  invariant: connection close is the ultimate garbage collector for
  per-connection state.

## Why not ctl channel

1. No correctness gain — process_disconnect already walks all routes
   for dead connection and sends ServiceTeardown to peers
   (server.rs:462-474). Same as freefidpool.
2. Ordering hazard — separate channel lets RevokeInterest overtake
   pending data frames for the same session. Server tears down route,
   then late data frames arrive for nonexistent route. In-band
   delivery preserves FIFO. flush(5) man page: "The semantics of
   flush depends on messages arriving in order."
3. Unbounded channel = new failure mode (memory growth). Bounded
   channel = same drop behavior as status quo but with added
   complexity and ordering hazard.

## Why not looper-local only (skip wire send)

Eager cleanup matters for resource budgets. Routes carry routing
table entries, pending dispatch entries, watch subscriptions — heavier
than Plan 9 fids. Dropping 7 of 10 ServiceHandles should free those
routes immediately, not hold them until connection close.

Phase 2 makes this critical: connection outlives individual
ServiceHandle lifetimes. Without wire RevokeInterest, routes leak
for pane lifetime, not connection lifetime.

## Phase 2 concern: periodic fid audit

In Phase 2 (multi-service long-lived connections), the
process_disconnect backstop fires later. Consider periodic server-side
audit of routing table for orphan routes — known pattern from NFS
stale file handles and long-lived 9P2000.L mounts. Not needed Phase 1.

## Key source locations

- ServiceHandle Drop: pane-app/src/service_handle.rs:270-283
- process_disconnect: pane-session/src/server.rs:416-475
- Plan 9 freefidpool: reference/plan9/src/sys/src/lib9p/srv.c:735
- Plan 9 sclunk: reference/plan9/src/sys/src/lib9p/srv.c:544-551
- WRITE_CHANNEL_CAPACITY: pane-session/src/bridge.rs:70 (128)
