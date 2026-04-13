---
type: analysis
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [shared-state, Observable, SharedLens, IntraMessenger, AppState, Transport, thread-per-pane, SH1-SH7, I2, fail_connection, backpressure, ArcSwap, RwLock, EAct, DLfActRiS]
related: [decision/thread_per_pane, architecture/shared_state, architecture/proto, architecture/app, architecture/session, decision/connection_source_design, reference/papers/eact, reference/papers/dlfactris]
sources: [FH eventactors-extended.tex lines 749-752 and 1047-1061, JHK24 Theorem 1.2, decision/connection_source_design D7/D9/D12]
verified_against: [architecture/shared_state 2026-04-12, obligation.rs current, dispatch.rs current, service_handle.rs current]
agents: [session-type-consultant]
---

# Shared-State Safety Analysis

VERDICT: Conditionally sound. Three must-fix issues, seven new
invariants (SH1-SH7).

## Verdicts by primitive

- **Observable<T>** — Sound. Non-blocking (ArcSwap), no wait
  edges, no Inv-RW contribution. Single-writer capability split
  (SH7) needed to prevent concurrent update contention.

- **SharedLens<S,A>** — Conditionally sound. RwLock write path
  violates I2 (cross-pane blocking). Must replace with ArcSwap
  or accept violation with watchdog detection. Lens laws hold
  per-operation (read-committed), not across operations under
  concurrency (SH2).

- **IntraMessenger<P>** — Conditionally sound. Two must-fix:
  (1) SH5: no fail_connection equivalent for direct channel —
  pane exit silently orphans pending requests, ReplyPort Drop
  compensation doesn't fire until sender also exits.
  (2) SH4: calloop::channel is unbounded — fast sender can
  overwhelm slow receiver without backpressure.

- **Transport<M>** — Sound. Direct path is type-safe by
  construction (same compilation unit). Move semantics ≡
  serialize+deserialize for value types (Message trait bounds).

- **AppState** — Conditionally sound. Must be write-once after
  setup (SH6). Mutable Observable stored in AppState is formally
  outside EAct ([FH] lines 749-752 warn about inter-actor shared
  state), but does not break any theorem because Observable
  creates no wait edges.

## Must-fix priority

1. SH5 (IntraMessenger fail_connection) — safety violation:
   orphaned DispatchEntries, no failure signal to sender
2. SH3 (SharedLens I2) — I2 violation: cross-pane write lock
   contention blocks dispatch
3. SH4 (IntraMessenger backpressure) — resource exhaustion:
   unbounded channel allows memory growth

## Invariants SH1-SH7

- SH1: Observable non-blocking reads (structural, ArcSwap)
- SH2: SharedLens per-operation lens laws (documented caveat)
- SH3: SharedLens write must not block cross-pane (ArcSwap fix)
- SH4: IntraMessenger backpressure parity (bounded channel)
- SH5: IntraMessenger fail_connection equivalence (destruction
  sequence integration)
- SH6: AppState write-once registry (typestate or runtime flag)
- SH7: Observable single-writer (capability split:
  ObservableWriter move-only, ObservableReader cloneable)

## Key theoretical grounding

- [FH] lines 749-752: "Introducing any method to synchronise
  shared state... means deadlock-freedom is no longer guaranteed."
  Observable/SharedLens are outside EAct formalism but don't
  create synchronisation (no wait edges), so the warning applies
  but the theorems still hold.
- [FH] lines 1047-1049: "shared state" in EAct means intra-actor
  state (handler struct H), not inter-actor. Observable is
  inter-actor, formally invisible to EAct's type system.
- [JHK24] Theorem 1.2: connectivity-graph acyclicity. Observable
  is not a channel, doesn't appear in connectivity graph.
  SharedLens RwLock contention is not a channel send/receive.
  Neither affects the theorem.
- Inv-RW (D3): Observable/SharedLens don't create request-wait
  edges. IntraMessenger preserves Inv-RW because install-before-
  wire (I4) and DispatchCtx cap check run identically regardless
  of transport.
