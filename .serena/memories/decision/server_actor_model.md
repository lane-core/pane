---
type: decision
status: current
supersedes: [pane/server_actor_model_decision]
sources: [pane/server_actor_model_decision]
created: 2026-04-05
last_updated: 2026-04-11
importance: high
keywords: [protocol_server, actor, single_threaded, mpsc, EAct, dlfactris, star_topology, write_handle]
related: [decision/messenger_addressing, reference/papers/eact, reference/papers/dlfactris, architecture/looper]
agents: [pane-architect, session-type-consultant, plan9-systems-engineer]
---

# Server must be a single-threaded actor

**Decision (2026-04-05):** `ProtocolServer` should be restructured
from N-reader-threads-with-shared-mutex to a single-threaded
actor with an mpsc ingress channel.

## Problem

The original design had N reader threads concurrently accessing
`Arc<Mutex<ServerState>>` for routing decisions and writes. This
violates EAct's single-mailbox invariant (Fowler / Hu, "Speak
Now," Definition 3.1) and creates TOCTOU windows between lock
acquisitions in the teardown path.

## Architecture

- Reader threads own read halves exclusively (no mutex), decode
  frames, send `ServerEvent { conn_id, frame }` to actor's mpsc
  channel
- Actor thread processes events sequentially: routing table,
  DeclareInterest, teardown notifications, all writes
- WriteHandles still need interior mutability (actor thread
  writes to multiple connections) but routing state becomes
  plain HashMaps
- Routing table invariant (consistent acyclic bipartite graph)
  enforced by sequential processing, not locks

## Session type scope

- **Handshake:** par governs `Send<Hello, Recv<Result<Welcome, Rejection>>>`
  — correctly scoped
- **Active phase:** inherently runtime-dispatched via FrameCodec
  + service byte demux, equivalent to dependent session type
  `Sigma(s: ServiceId). Protocol(s)` which Rust cannot express
  statically
- **Read / write transport split:** transport-level concern,
  not protocol decomposition. No session type governs it.
- **Server internals:** not session-typed. Server is
  infrastructure (dynamic forwarder), not a protocol
  participant. No dual exists.

## DLfActRiS relevance

**[JHK24] Theorem 1.2 (scoped):** Star topology (actor at center
owning all write endpoints) is trivially acyclic per Jacobs /
Hinrichsen / Krebbers POPL 2024 Theorem 1.2 — this proves
ProtocolServer's **local** star topology is progress-safe. The
theorem's hypothesis is connectivity-graph acyclicity; star
topology satisfies it trivially. Dynamic forwarder topology
changes are outside LinearActris's current scope but sequential
processing makes them safe.

**Whole-system progress:** For progress across the full pane
system (especially Phase 2 direct pane-to-pane per
`decision/messenger_addressing`), cite **[FH] EAct progress**
(Fowler / Hu, "Speak Now," Theorems 6 + 8) — per-actor, no
topology requirement. The load-bearing invariant for whole-system
progress is Inv-RW (Request-Wait graph acyclicity), defined in
`analysis/verification/invariants/inv_rw`. [JHK24] Theorem 1.2
and [FH] EAct progress address different graphs and different
scopes; neither subsumes the other.

**Citation correction (D3, `decision/connection_source_design`):**
The original text read [JHK24] Theorem 1.2 as covering whole-pane
progress. Restricted 2026-04-11 to local star topology per D3
analysis.

See `reference/papers/dlfactris` and `reference/papers/eact`
for the formal grounding.
