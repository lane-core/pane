---
type: decision
status: decided
created: 2026-04-12
last_updated: 2026-04-12
importance: critical
keywords: [pane_session, extraction, MPST, ordering, NonBlockingSend, FlowControl, RequestCorrelator, ActiveSession, FrameReader, failure_cascade, N1, N2, N3, N4]
related: [decision/pane_session_mpst_foundation, decision/connection_source_design, policy/intermediate_state_principle, policy/agent_workflow, status]
agents: [session-type-consultant, optics-theorist, plan9-systems-engineer, be-systems-engineer, pane-architect]
---

# MPST Extraction Plan

Execution plan for extracting protocol-layer abstractions from
pane-app to pane-session, grounded in Fowler-Hu EAct formalism.
Makes three adversarial bugs (bidirectional deadlock, partial-frame
hang, HoL blocking) impossible by construction through N1-N4
invariants encoded as Rust types.

Derived from two-round four-agent consultation 2026-04-12. All
ordering decisions unanimous after follow-up rounds.

## Phase A — Foundation (parallel, one commit)

All independent. Zero dispatch-loop impact. LOW risk. ~18 tests
move.

### A1: FrameReader + FrameWriter → pane-session/frame.rs [N4]

Move from `pane-app/src/connection_source.rs` to
`pane-session/src/frame.rs` alongside existing blocking FrameCodec.

**What moves:** `FrameReader`, `ReadState`, `ReadProgress`,
`read_into`, `FrameWriter`, `WRITE_HIGHWATER_BYTES`.

**What stays:** `SharedWriter`, `ConnectionSource`,
`ConnectionError`, `classify_frame`, `drain_write_channel`,
`before_sleep` (calloop integration).

**Tests that move:** ~15 unit tests (`frame_reader_*`,
`frame_writer_*`). They test with `Cursor<Vec<u8>>` mocks — no
calloop dependency.

**ConnectionError handling:** FrameReader should return
`FrameError` (already in pane-session) rather than
`ConnectionError`. ConnectionSource maps `FrameError` to its own
error type.

After extraction, pane-session exports both:
- `FrameCodec` — blocking reads for bridge/server threads
- `FrameReader` / `FrameWriter` — non-blocking for calloop

### A2: Backpressure → pane-session/backpressure.rs

Move `pane-app/src/backpressure.rs` verbatim. Standalone error
enum (CapExceeded, ChannelFull, ConnectionClosing). 3 tests move.

pane-app re-exports for downstream API stability.

### A3: Token + PeerScope → pane-session

Move from `pane-app/src/dispatch.rs`. Copy + Eq + Hash newtypes
with no handler dependency. Trivial.

### A4: NonBlockingSend trait → pane-session [N1]

New trait. Codifies what SharedWriter::enqueue() and mpsc
try_send already do. Constraint trait — no implementation yet.

```rust
pub trait NonBlockingSend {
    fn try_send_frame(&self, service: u16, payload: &[u8])
        -> Result<(), Backpressure>;
}
```

No Plan 9 precedent (novel constraint from cooperative
scheduling). Two implementors added later: SharedWriter
(looper-thread) and mpsc wrapper (cross-thread).

## Phase B — RequestCorrelator (depends on A)

Single extraction combining token allocation + FlowControl. ~11
tests move. MEDIUM risk.

### B1: RequestCorrelator → pane-session/correlator.rs [N1+N2]

**Why combined (unanimous):** The outstanding request counter is a
Fold over key liveness — a derived property of which tokens
exist. Separating them creates a cross-crate coherence obligation
where the intermediate state (cap in pane-session, counter in
pane-app) is something nobody would design on purpose. See
`policy/intermediate_state_principle`.

```rust
pub struct RequestCorrelator {
    next_token: u64,
    outstanding_requests: u64,
    request_cap: u16,
}

impl RequestCorrelator {
    pub fn allocate_token(&mut self) -> Token {
        // atomically: allocate token + increment counter
    }
    pub fn record_resolution(&mut self) {
        // decrement counter
    }
    pub fn would_exceed_cap(&self) -> bool { ... }
    pub fn set_cap(&mut self, cap: u16) { ... }
    pub fn outstanding_requests(&self) -> u64 { ... }
}
```

**After extraction, Dispatch<H> becomes:**

```rust
pub struct Dispatch<H> {
    correlator: RequestCorrelator,
    entries: HashMap<(PeerScope, Token), DispatchEntry<H>>,
}
```

Two fields. Every Dispatch method delegates counter operations
to correlator. allocate_token() atomically does both —
"insert without increment" failure mode eliminated.

**Tests that move:** ~11 (counter arithmetic, cap enforcement,
token monotonicity). Tests exercising closure-firing
(fire_reply, fire_failed, etc.) stay in pane-app.

**Linearity requirement:** Every allocate_token must pair with
exactly one resolution (fire_reply, fire_failed, cancel,
fail_session, fail_connection, clear). Enforced by Dispatch<H>
method discipline — entries is private, all removal paths call
record_resolution(). clear() resets counter to 0.

**Doc comment must state:** RequestCorrelator manages both token
allocation AND outstanding request bounds (plan9 recommendation).

## Phase C — ActiveSession + Failure Cascade (depends on B)

One conceptual phase, two reviewable commits. MEDIUM risk.

### C1: ActiveSession → pane-session/active_session.rs

Post-handshake state container. Consolidates scattered fields
from LooperCore<H> — everything not generic over H.

```rust
pub struct ActiveSession {
    pub(crate) correlator: RequestCorrelator,
    pub(crate) sessions: HashMap<u16, SessionState>,
    pub(crate) revoked_sessions: HashSet<u16>,
    pub(crate) primary_connection: PeerScope,
    pub(crate) max_message_size: u32,
    pub(crate) max_outstanding_requests: u16,
}
```

**Precedent:** Plan 9 `Mnt` struct in devmnt.c (per-mount,
post-handshake state container). EAct handler store σ (§3.2).

**What it consolidates:**
- revoked_sessions: currently homeless on Batch (transient)
- session→connection map: not tracked on looper side today
- per-session request_cap: currently global on Dispatch
- negotiated params: consumed from Welcome, scattered

**Why before failure cascade:** EAct defines σ in §3.2, failure
handling in §4. Mnt exists before muxclose(). Haiku ServerApp
was container-first, teardown-against-container. The cascade is a
consumer of session state, not a contributor to its shape. Shape
already determined after Phase B: "everything in LooperCore that
isn't generic over H."

### C2: Failure cascade policy → method on ActiveSession [N3]

**What moves to pane-session (cascade policy):**
- "Which sessions are affected by this transport error?"
- "Enumerate affected tokens for teardown"
- TeardownSet obligation type (#[must_use], Drop-panics if
  tokens not fully consumed)

```rust
#[must_use = "all teardown tokens must be resolved"]
pub struct TeardownSet(Vec<(PeerScope, Token)>);
impl Drop for TeardownSet {
    fn drop(&mut self) {
        if !self.0.is_empty() { /* panic or log */ }
    }
}

impl ActiveSession {
    pub fn cascade_connection_failure(&mut self, peer: PeerScope)
        -> TeardownSet { ... }
    pub fn cascade_session_failure(&mut self, session_id: u16)
        -> TeardownSet { ... }
}
```

**What stays in pane-app (cascade execution + orchestration):**
- Fire H-typed on_failed closures (needs &mut H)
- Batch phase 2 sequencing (run_destruction, connection_lost)
- Handler Drop semantics, catch_unwind boundaries
- calloop lifecycle integration

**Cascade split rationale (be-systems-engineer):** Policy is
session-layer (pane-session knows sessions). Execution is
handler-layer (pane-app knows closures). Orchestration is
framework-layer (pane-app knows batch phases).

## Post-extraction API surface

**pane-session exports (new):**
- FrameReader, FrameWriter (non-blocking codec)
- Backpressure (send failure signal)
- Token, PeerScope (correlation identifiers)
- NonBlockingSend (send contract trait)
- RequestCorrelator (token alloc + credit tracking)
- ActiveSession (post-handshake state container)
- TeardownSet (failure cascade obligation)

**pane-app keeps:**
- Dispatch<H> (closure storage + firing)
- DispatchEntry<H>, DispatchCtx<H> (handler-typed dispatch)
- ServiceHandle<P>, ServiceDispatch<H> (protocol API)
- LooperCore<H>, Looper<H> (event loop)
- SharedWriter, ConnectionSource (calloop integration)
- Cascade orchestration (batch phases, destruction sequence)

## Provenance

Lane directed: extract protocol-layer abstractions from pane-app
to pane-session, grounded in Fowler-Hu EAct. Two-round four-agent
consultation 2026-04-12.

Round 1: all four agents produced extraction plans independently.
Strong convergence on Phase A. Two disagreements: (1)
FlowControl + RequestCorrelator together or separate, (2)
ActiveSession before or after failure cascade.

Disagreement 1 resolved by forwarding optics-theorist's combined
extraction proposal to other three. All accepted unanimously.
Key insight: the counter is a Fold over key liveness. Separated
intermediate state violates intermediate state principle
(be-systems-engineer, enshrined as
`policy/intermediate_state_principle`).

Disagreement 2 resolved by targeted second round. Session-type
grounded ordering in EAct §3.2/§4 structure. Plan9 identified Mnt
as strong precedent. Be reversed position after applying own
intermediate state principle. Unanimous: ActiveSession first.
