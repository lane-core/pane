# Server Must Be a Single-Threaded Actor

**Decision (2026-04-05):** ProtocolServer should be restructured from N-reader-threads-with-shared-mutex to a single-threaded actor with an mpsc ingress channel.

## Problem
The original design had N reader threads concurrently accessing `Arc<Mutex<ServerState>>` for routing decisions and writes. This violates EAct's single-mailbox invariant (Fowler/Hu, "Speak Now," Definition 3.1) and creates TOCTOU windows between lock acquisitions in the teardown path.

## Architecture
- Reader threads own read halves exclusively (no mutex), decode frames, send `ServerEvent { conn_id, frame }` to actor's mpsc channel
- Actor thread processes events sequentially: routing table, DeclareInterest, teardown notifications, all writes
- WriteHandles still need interior mutability (actor thread writes to multiple connections) but routing state becomes plain HashMaps
- Routing table invariant (consistent acyclic bipartite graph) enforced by sequential processing, not locks

## Session Type Scope
- Handshake: par governs `Send<Hello, Recv<Result<Welcome, Rejection>>>` -- correctly scoped
- Active phase: inherently runtime-dispatched via FrameCodec + service byte demux, equivalent to dependent session type `Sigma(s: ServiceId). Protocol(s)` which Rust cannot express statically
- Read/write transport split: transport-level concern, not protocol decomposition. No session type governs it.
- Server internals: not session-typed. Server is infrastructure (dynamic forwarder), not a protocol participant. No dual exists.

## DLfActRiS Relevance
Star topology (actor at center owning all write endpoints) is trivially acyclic per Jacobs/Hinrichsen/Krebbers POPL 2024 Theorem 1.2 (global progress). Dynamic forwarder topology changes are outside LinearActris's current scope but sequential processing makes them safe.
