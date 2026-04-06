# Wiring Soundness Analysis (2026-04-06)

Session-type analysis of connecting PaneBuilder/LooperCore stubs to the real ProtocolServer. Five questions evaluated.

## Setup-to-Active Phase Transition: Conditionally Sound

Three-phase lifecycle (Handshake → Setup → Active) shares one mpsc channel for phases 2 and 3. Sound under invariant:

**S-SETUP: Server MUST resolve all initial interests (from Hello.interests) before sending Lifecycle::Ready.**

Ready is the omega_X signal (positive shift from setup to active). PaneBuilder is consumed by run_with (ownership transfer enforces no late open_service calls). Without S-SETUP, PaneBuilder's blocking recv() during open_service could receive Lifecycle::Ready instead of InterestAccepted, requiring client-side message filtering.

If S-SETUP is adopted, any non-InterestAccepted/Declined message during setup is a protocol violation (assert and disconnect).

## ServiceHandle Affine Gap: Sufficient for Safety

- Drop sends RevokeInterest — at-most-once revocation (affine, not linear).
- Connection-loss before RevokeInterest: server's process_disconnect() cleans up all routes for dead connections, subsumes missing RevokeInterest. Safety preserved, liveness gap in timing window (frames to dead connection fail silently).
- session_id uniqueness: monotonic counter + !Clone + pub(crate) constructor. Module-boundary guarantee, not type-level.
- Server should explicitly handle RevokeInterest for unknown session_ids as no-op (defensive).

## Dispatch Table Type Erasure: Correct Duploid Move

Type erasure at dispatch table = shift cycle (↑P at serialize, ↓P at deserialize). compile-time checking deferred to closure capture at open_service time, where P is known. session_id mismatch prevented by single-threaded actor (DLfActRiS Theorem 5.4). Deserialization failure is runtime safety net for protocol violations.

## Setup-Phase Deadlock: None

Topology is DAG: PaneBuilder → writer → transport → server actor → transport → reader → PaneBuilder. All intermediate sends non-blocking (unbounded mpsc). Socket buffers drained by reader threads. No directed cycle in wait-for relation.

## Polarity: Clean

Setup phase is all-positive (wire negotiation + dispatch table construction). Active phase has exactly one polarity crossing per dispatch (positive frame into negative handler). Dispatch table is positive structure holding negative values. ServiceHandle construction is ↑(negative) shift, not a crossing.

## Round 2 Refinements (2026-04-06)

### Q1: S-SETUP via Hello.interests (Option a)

S-SETUP is enforced by construction: initial interests are in Hello, resolved by the server in accept(), returned as Welcome.bindings. No intermediate setup phase needed for initial services. DeclareInterest reserved for dynamic post-run_with service opening.

Par session Send<Hello, Recv<Result<Welcome, Rejection>>> contains all setup data. The three-phase lifecycle collapses to two (Handshake then Active) for initial services.

Server accept() change: read Hello, post to actor, block on oneshot for Welcome. Actor registers provides, resolves interests, builds Welcome with bindings, writes to wire. All routing state stays in actor thread.

PaneBuilder change: open_service records interest plus dispatch fn pointer but does not block. run_with performs handshake, matches bindings, constructs ServiceHandles.

### Q2: Writer Thread (not Arc<Mutex>)

Arc<Mutex<Writer>> creates deadlock risk: looper holds mutex, write blocks (kernel buffer full), reader can't drain to looper, peer can't write to us, circular wait. Writer thread eliminates this: looper to mpsc to writer is acyclic DAG. DLfActRiS Theorem 5.4 (acyclic actor topology implies deadlock-free).

Writer thread accepts (u8, Vec<u8>) with no service knowledge needed. Permissive codec simplification applies only to reader side.

### Q3: Transport Split After Handshake

Handshake par session requires both directions (multiplicative conjunction). Active phase is two independent streams (par). Split corresponds to decomposition of the par connective. Bridge thread does handshake with full transport, then splits: read half to reader loop, write half to writer thread. connect_and_run returns (Welcome, Receiver<LooperMessage>, Sender<(u8, Vec<u8>)>).

## Action Items

1. Adopt S-SETUP via Hello.interests (option a) -- no separate setup phase for initial services
2. Move interest resolution into actor thread (accept posts to actor, blocks on oneshot for Welcome)
3. Writer thread topology for client connections
4. connect_and_run returns writer channel alongside reader channel
5. Explicit RevokeInterest match arm in ServerState::process_control()
6. DeclareInterest for dynamic service opening only (post-run_with)
