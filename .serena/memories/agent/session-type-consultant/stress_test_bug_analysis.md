---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [stress-test, bugs, partial-frame, bidirectional-deadlock, head-of-line, blocking, bounded-buffer, credit-flow, transport-session-boundary, D12, D9]
sources:
  - "[FH] §3.2 E-Send (non-blocking queue append), §3.3 Lemma 1 (Independence), §4 E-RaiseS, Theorems 6-8"
  - "[JHK24] §4.1 send1/recv1, p.47:6 (only recv/wait block), Theorem 1.2 (connectivity-graph acyclicity)"
  - "Gay & Vasconcelos 2010 §5 (bounded-buffer session types)"
  - "decision/connection_source_design D9, D12"
verified_against: ["server.rs reader_loop (blocking read_frame)", "server.rs WriteHandle::enqueue (try_send)", "frame.rs:191-210 (read_frame_inner blocking read_exact)", "stress.rs no_backpressure_unbounded_channel_fill"]
related: [decision/connection_source_design, agent/session-type-consultant/architecture_abc_analysis, decision/server_actor_model]
agents: [session-type-consultant]
---

# Stress test bug analysis: three bugs from adversarial testing

## Root cause (shared)

Implementation uses blocking I/O on bounded buffers. Formal models
([FH] E-Send, [JHK24] send1) assume non-blocking I/O on unbounded
buffers. The three bugs are three facets of this model-implementation gap.

## Bug 1: Partial frame blocks reader forever

NOT fixable by session-type construction. Session types model
complete-message delivery ([FH] E-Send is atomic append to queue).
No concept of "half a message" exists in any session calculus.
Transport-level concern.

Fix: read timeout on fd (SO_RCVTIMEO). Timeout fires → poison codec
→ ServerEvent::Disconnected → maps to [FH] E-RaiseS.

## Bug 2: Bidirectional buffer deadlock

FIXABLE by construction. The construction is non-blocking writes on
both sides (server AND client). D12 Part 1 fixes the server side.
Client needs the same architecture: actor → bounded queue → writer
thread → fd. Neither actor ever blocks on I/O. Writer threads are
independent ([FH] Lemma 1). Bounded queue overflow → connection
teardown (E-RaiseS).

D9 (max_outstanding_requests) is credit-based flow control — prevents
unbounded frame bursts. But D9 alone doesn't prevent the deadlock
because socket buffer saturation can occur below the credit cap. The
essential fix is non-blocking writes, not just credit caps.

[JHK24] Theorem 1.2 does NOT cover this — it operates on channel
ownership graphs, not socket buffer contention. Transport-level
cycles are outside the theorem's scope.

## Bug 3: HoL blocking during enqueue

NOT a session-type concern. [FH] E-Send models queue append as
instantaneous. Implementation cost (mpsc mutex contention ~200ns)
is the model-reality gap. D12 Part 1 (try_send) already eliminates
blocking. Remaining contention is nanosecond-level mutex acquisition.

Optional fix: SPSC lock-free queue per connection. Implementation
swap, not design change.

## Key formal citations

- [FH] E-Send §3.2: "message is appended to the session queue and
  the operation reduces to effreturn{()}" — non-blocking by definition
- [JHK24] p.47:6: "The c.recv() and c.wait() operations are blocking"
  — send conspicuously absent, confirming non-blocking send
- [FH] Lemma 1 §3.3: thread reduction in one actor doesn't inhibit
  another — justifies independent writer threads
- [FH] Theorems 6-8 §4: preservation/progress under failure —
  connection teardown on overflow maps to E-RaiseS
- Gay & Vasconcelos 2010 §5: bounded-buffer encodable in session
  types via credit tokens (receiver sends credits, sender must read
  credits before continuing to send)
