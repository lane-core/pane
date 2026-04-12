---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [performance, single-thread, parallelism, direct-connections, write-batching, I6, Inv-CS1, Inv-RW, routing-hop, FrameWriter]
sources:
  - "[FH] §3.2 E-Send, §3.3 Lemma 1 (Independence of Thread Reductions), Definition 7 (Thread-Terminating), Theorems 5-8"
  - "[JHK24] Theorem 1.2 (connectivity-graph acyclicity → global progress)"
  - "[CMS] §5.1 (forwarder chain cut-elimination)"
  - "decision/connection_source_design D3 (Inv-RW), D4 (Inv-CS1), D7 (tier classification), D9 (cap), D12 (non-blocking writes)"
  - "policy/feedback_per_pane_threading"
verified_against: ["connection_source.rs FrameWriter (VecDeque<Vec<u8>>)", "server.rs process_service + try_enqueue", "server.rs WriteHandle::enqueue + spawn_writer_thread"]
related: [decision/connection_source_design, architecture/looper, architecture/session, agent/session-type-consultant/architecture_abc_analysis]
agents: [session-type-consultant]
---

# Performance triad analysis: dispatch parallelism, routing hop, write batching

Lane identified three performance concerns after 200K msg/sec benchmark. Analysis requested from session-type perspective.

## 1. Single-threaded dispatch (I6) — conditionally sound parallelism

Selective parallelism possible for notification-only services.
Request dispatch must remain single-threaded (obligation handles
are !Send, DispatchCtx is lifetime-bound).

**Key result:** [FH] Lemma 1 (Independence of Thread Reductions)
proves actor-level reductions don't interfere across threads.
Map to pane: notification handlers with cloned read-only state
+ effect channel back to looper = internal computation below
session-type granularity. [FH] Theorem 5 still holds.

**Three invariants:**
- P1 (Worker isolation): parallel workers never hold &mut H
- P2 (Effect ordering): effects collected and applied by looper in next tick
- P3 (Obligation exclusion): no ReplyPort/CancelHandle/DispatchCtx crosses thread boundary — compiler-enforced (!Send)

**Cost of current design:** CPU-heavy handler delays [FH] Corollary 1 (Global Progress) by delaying Thread-Terminating condition. Safety preserved, liveness degrades proportionally to compute time. Watchdog detects only 5s+ stalls.

## 2. ProtocolServer routing hop — sound to remove

Direct pane-to-pane connections preserve all session-type safety properties. [FH] Theorems 6-8 are topology-independent.

**Progress argument changes:** [JHK24] Theorem 1.2 (star topology acyclicity) no longer applies structurally. Replace with Inv-RW (D3) + I2 (no blocking). This is weaker: star acyclicity is structural, Inv-RW + I2 is runtime-convention. In practice, pane's handler model (I2: return Flow::Continue immediately) prevents wait-edge formation regardless of topology.

**Five requirements for direct connections:** DC1 (session negotiation handshake), DC2 (per-endpoint failure detection), DC3 (session ID agreement), DC4 (Inv-RW preservation under I2), DC5 (direct backpressure propagation via try_send_notification).

## 3. Write batching — sound, no ordering degradation

FIFO preserved by contiguous append + sequential flush. Batching is a wire optimization below session-type granularity.

**Recommended:** Replace VecDeque<Vec<u8>> with single contiguous Vec<u8> buffer. Eliminates per-frame allocation (100 damage rects: 100 allocs → 0 amortized). [FH] E-Send queue ordering preserved by append order. POSIX writev guarantees iovec ordering.

**Inv-CS1 interaction:** orthogonal. Inv-CS1 governs inbound dispatch ordering. Write batching governs outbound I/O granularity. No interference.

**Backpressure interaction:** cap counter increments at send_request time, not write() time. Batching doesn't change cap semantics.
