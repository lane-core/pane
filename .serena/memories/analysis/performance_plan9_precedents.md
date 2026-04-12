---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [performance, dispatch, routing_hop, batching, coalescing, vectored_io, plan9, rio, draw, exportfs, devmnt, flush]
sources: [8half.ms, plumb.ms, names.ms, draw(3), devmnt.c, exportfs(4), thread(2), acme.ms]
verified_against: [pane-session/src/server.rs actor_loop, pane-app/src/connection_source.rs FrameWriter/try_flush, pane-app/src/looper.rs dispatch_batch]
related: [decision/server_actor_model, architecture/looper, architecture/session, reference/plan9/papers_insights]
agents: [plan9-systems-engineer]
---

# Performance analysis: Plan 9 precedents for pane

Roundtable analysis on three performance concerns. Baseline:
200K msg/sec over Unix sockets, 20μs P50 latency.

## 1. Single-threaded dispatch and CPU-intensive handlers

**Plan 9 patterns:**

- **Pattern A (rio):** Sequential server loop, bounded per-request
  work. No blocking allowed. Server responsiveness comes from fast
  handlers, not concurrency.
- **Pattern B (acme):** Process-per-request. Each I/O request gets
  its own proc/thread. State implicitly encodes request state.
  Pike: "the code worked the first time."
- **Pattern C (exportfs):** Worker pool. Main thread dispatches,
  workers handle blocking operations (network I/O to real fs).

**Recommendation:** pane's looper is Pattern A. Expensive work
should go to background `thread::spawn`, results return via mpsc
to the looper. Phase 6 post-batch can amortize across batch
(compute once, reply N). Worker pool (Pattern C) only needed if
Phase 2 ProtocolServer proxies to blocking backends.

**Key insight:** The watchdog (I2/I3 detection) is the correct
mechanism for a convention-based model. Plan 9 had the same
convention with no enforcement mechanism at all.

## 2. Server routing hop

**Plan 9 paths:**

- **Kernel device (bind #s /srv):** No 9P overhead, direct
  function call to Dev struct methods. ~10-50x faster than mount.
- **Mounted file server (devmnt):** Full 9P serialize → write →
  context switch → read → deserialize round trip per operation.
  ~2-4μs on contemporary hardware.

**draw as kernel device:** devdraw existed specifically because
draw performance demanded elimination of the mount hop. When rio
moved draw to userspace, the draw protocol was redesigned to
amortize the hop cost (batch commands + flush).

**pane's hop:** Two Unix socket transits (sender→server→target)
plus actor routing. At 200K msg/sec, ~5μs per message. Routing
logic itself is trivial (HashMap lookup + try_enqueue).

**Recommendation:** Don't eliminate hop in Phase 1. 200K msg/sec
exceeds windowing needs. In Phase 2, broker direct socketpairs
for high-traffic pairs (server becomes control plane, not data
plane). This matches cpu(1) pattern — exportfs brokers namespace,
then client talks directly to remote file servers.

**Direct connection tradeoff:** Eliminates one socket transit
(~2x throughput, ~5μs P50 reduction) but loses server
visibility into message flow and complicates teardown
coordination.

## 3. Write batching and coalescing

**Plan 9 mechanisms:**

- **Kernel pipe buffer:** Implicit coalescing. T-messages
  accumulated in 8KB pipe buffer; server could read multiple in
  one read(2). Incidental, not designed.
- **rio draw protocol (explicit flush):** libdraw accumulated
  draw commands in display->buf (grew as needed, flushed at 8KB
  or on explicit flushimage). Single write(2) for many commands.
  **Flush boundary was semantic** (coherent visual update), not
  transport-level.

**What pane already has:**

1. calloop batch tick — read-side batching via dispatch_batch
   six-phase ordering
2. FrameWriter queue — write-side accumulation in VecDeque,
   try_flush drains without returning to poll
3. before_sleep drain — synthetic writable events for cross-thread
   writes

**What's missing:** Vectored I/O (writev). Current try_flush
writes one frame per write(2) syscall. writev via IoSlice would
collapse N frames into one syscall (~300-500ns saved per frame).
Estimated gain: ~10x reduction in write syscalls during bursts,
potentially 50% throughput increase.

**Phase 6 post-batch (semantic flush):** The right place for
rio-style semantic coalescing. Coalesce redundant attribute
writes, group frames by destination connection, signal
display-level "frame complete" to compositor. Recommended only
for display protocol, not general messaging.

**Recommendation:** Vectored I/O is ~20 lines of code, real but
not urgent. Profile first. Phase 6 semantic batching is more
architecturally important than syscall savings.

## Confidence levels

- Single-threaded dispatch model: **high** — well-validated by
  Plan 9's 20-year experience.
- Routing hop analysis: **high on analysis, medium on numbers** —
  need profiling to confirm where 20μs P50 is actually spent.
- Write batching: **medium** — vectored I/O gain is real but may
  not be the bottleneck. Unix socket buffers already coalesce at
  kernel level for small frames.
