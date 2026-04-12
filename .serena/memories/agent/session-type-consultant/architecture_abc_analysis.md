---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [architecture, non-blocking, looper-first, head-of-line, write-path, session-safety, frame-drop, connection-teardown, D1-D11, Inv-RW]
sources:
  - "[FH] §3.2 E-Send, §3.3 Lemma 1 (Independence), §4 E-RaiseS/E-CancelMsg, Theorems 6-8"
  - "[JHK24] Theorem 1.2 (star topology progress)"
  - "[FH] §1.3 KP1 (Reactivity)"
  - "[MostrousV18] Affine Sessions"
  - "decision/connection_source_design D1-D11"
  - "policy/feedback_per_pane_threading"
verified_against: ["server.rs:412-424 (process_service synchronous write)", "connection_source.rs:321-340 (ConnectionSource struct)", "pub_sub_stress.rs:739-855 (backpressure test)"]
related: [decision/connection_source_design, decision/server_actor_model, architecture/looper, architecture/session]
agents: [session-type-consultant]
---

# Architecture A/B/C analysis for PaneBuilder ordering + head-of-line blocking + write path

Lane's design question: three candidate architectures for three
related problems (builder ordering, server head-of-line blocking,
write path complexity). Explicitly rejected phased approach.

## Verdict

**Architecture C (Looper-first + non-blocking server routing) is
sound. Only D2 requires amendment.** A is unsound (doesn't fix
head-of-line). B is conditionally sound but requires amending
both D2 and D8.

## Key session-type findings

### Frame drop policy (Q1)

Frame-level drops are NOT safe for Reply/Failed or Request
frames — they create partial session failures outside [FH]
Theorems 6-8 scope (which assume whole-actor failure). The only
sound drop policy is connection teardown on queue overflow, which
maps to [FH] E-RaiseS cascading failure.

Notification drops are safe (no obligation, [FH] E-CancelMsg
covers discarded messages).

### Handshake on looper thread (Q2)

Does NOT violate I2. Handshake is [FH] E-Init (configuration
setup), not handler dispatch. [FH] Lemma 1 scopes progress to
actors in the E-React/E-Suspend cycle. Phase 1 local unix
sockets: handshake completes in microseconds. Phase 2 remote
connections need bridge-thread handshake (D2 amendment scope).

### Write path simplification (Q3)

Architecture C eliminates mpsc for Tier A sends. Minimum correct
path: ServiceHandle → DispatchCtx::write_to_connection →
ConnectionSource::enqueue_frame → fd. Two buffer boundaries
(VecDeque + kernel). Install-before-wire (I4) enforced by
same-thread execution. FIFO per session by I6.

### D1-D11 compatibility (Q4)

A: breaks D3 (Inv-RW — server hub blocks, star collapses).
B: amends D2 + D8. C: amends D2 only.

## Server write path under C

Server acquires per-connection non-blocking write queues (bounded
VecDeque). Actor routes to queue. Overflow → connection teardown
(S4 fail_connection). Actor thread never blocks. [JHK24] Theorem
1.2 star topology progress preserved.
