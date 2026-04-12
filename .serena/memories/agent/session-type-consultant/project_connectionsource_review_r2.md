---
type: project
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [ConnectionSource, round2, backpressure, two_function_split, Inv_RW, request_wait_graph, DLfActRiS, phase2_reverification]
related: [agent/plan9-systems-engineer/project_connectionsource_review, reference/papers/eact_sections, reference/papers/dlfactris, decision/server_actor_model, architecture/looper]
agents: [session-type-consultant, plan9-systems-engineer, pane-architect]
---

# ConnectionSource design review round 2 (2026-04-11)

Follow-up to round 1. Revisits Q1 (backpressure API) and Q3 (multi-connection acyclicity).

## Q1 — Two-function backpressure API

**Verdict: conditionally sound.** The split `send_request` (infallible, cap-and-abort) + `try_send_request` (fallible, returns `(Req, Backpressure)` on error) is principled.

### Why it works

- `try_send_request` is the Kleisli lift of `send_request` under the `Result<_, Backpressure>` exception monad. `send_request = unwrap_or_abort ∘ try_send_request`. Not ad-hoc — this is the standard algebraic-effect treatment of session operations with a default handler.
- On `Err(Backpressure)`, `E-Send` ([FH] §3.2) simply doesn't fire — session typestate is unchanged. The failure is *not* a session failure, just a non-advancement.
- On cap-and-abort, `E-RaiseS` ([FH] §4) fires at the connection level, producing zapper threads for every session on the poisoned ConnectionSource. `E-CancelH` ([FH] §4) then discharges every pending handler via the existing `fail_connection` path.
- The two paths never observe conflicting state because they're mutually exclusive per call: a given attempted send either succeeds, returns `Err` (no state change), or aborts the connection.

### Non-negotiable condition

**Fallible signature must return the request:** `try_send_request(req) -> Result<CompletionToken, (Req, Backpressure)>`. Otherwise the request obligation is consumed on the error path and linearity is broken at the call boundary. This matches `std::sync::mpsc::SyncSender::try_send`.

### `send_and_wait` composability (non-looper threads)

```rust
// Permitted: non-looper threads are I2-exempt
fn send_and_wait(&self, mut req) -> Result<Reply, SendAndWaitError> {
    loop {
        match self.try_send_request(req) {
            Ok(token) => return self.wait_for_reply(token),
            Err((returned_req, Backpressure)) => {
                req = returned_req;
                park_with_timeout()?;
            }
        }
    }
}
```

Sound because: (a) [FH] Lemma 1 (Independence of Thread Reductions, §3.3) — retries in one thread don't inhibit others; (b) each retry is a non-advance at the session level, so no state change to undo.

### Invariants affected

- **I4 (typestate handles):** preserved iff `(Req, Backpressure)` return pattern is used.
- **I2 (no blocking in handlers):** preserved — both functions non-blocking.
- **I8 (send_and_wait panics from looper thread):** preserved.
- **S4 (fail_connection scoped):** preserved — `E-CancelH` fires on aborted ConnectionSource's sessions.
- **I10/I11 (ProtocolAbort):** preserved via cap-and-abort path.

## Q3 — Inv-RW replaces connection-graph acyclicity

**Verdict: plan9's framing is sharper and I concede.** The load-bearing invariant is request-wait-graph acyclicity, not connection-graph acyclicity.

### Inv-RW (one sentence)

**Inv-RW:** At any tick boundary, the transitive closure of pending request-wait edges (pane A has a `Dispatch` entry waiting on a reply from pane B) forms a DAG.

### Why Inv-RW is the right formulation

[JHK24] §1 "The need for acyclicity" argues connectivity-graph acyclicity is needed because in LinearActris, connectivity and wait coincide — a thread blocked on `recv` literally waits on its peer endpoint. Pane decouples them: handlers call `send_request` and return `Flow::Continue` immediately ([FH] `E-Suspend`), with replies arriving via phase 5 ([FH] `E-React`). A connection being open does *not* establish a wait edge.

The [JHK24] Theorem 1.2 hypothesis maps, in pane's dispatch model, onto Inv-RW — not onto the connection topology.

### Why Inv-RW holds in pane

1. **I2** — handlers cannot transitively create a wait edge to themselves within a single dispatch invocation (dispatch entry exists only after the handler returns).
2. **I8** — synchronous waits confined to non-looper threads, which hold oneshot channels rather than `Dispatch` entries.
3. **Protocol-scoped send_request** — session types bound which panes a handler can address; restricts topology but is orthogonal to acyclicity.

### Inv-CS1 status

**Demoted.** Inv-CS1 (phase 5 per-source drain) is not required for deadlock freedom under Inv-RW. It's an S3 batch-ordering implementation detail — useful for deterministic test output, but not load-bearing. Session-type analysis doesn't need it.

### S3 refinement (ctl writes per-source drain)

**Implementation ergonomics only.** Closest formal result is [FH] Lemma 1 (Independence of Thread Reductions) — reordering between actors is always safe, so any ordering is permissive. Be's proposal is fine, but session types don't require it.

## Phase 2 re-verification sketch

`decision/server_actor_model`'s [JHK24] Theorem 1.2 citation is scoped to star topology. Phase 2 direct pane-to-pane breaks the star. The re-verification, when Phase 2 design lands:

1. **Inv-RW must still hold.** Risk: if Phase 2 adds any mechanism by which a handler on pane A blocks synchronously on a reply from pane B, Inv-RW is violated. Must be precluded — no "handler-local await" primitives.
2. **`fail_connection` scoping (S4) correctness under multi-connection.** `fail_connection(conn_id)` must fire `on_failed` for exactly the entries bound to `conn_id`. Selector-correctness property.
3. **`E-RaiseS` scope under multi-connection.** Abort on connection X must not cascade to connection Y absent a protocol dependency. [JHK24] §1 warns that forwarding creates transitive dependencies; if pane Phase 2 supports forwarding, request-wait acyclicity is a real proof obligation.

**Prediction:** Phase 2's first cut won't forward. Re-verification will be straightforward. Second cut (forwarding) needs actual work.

## Deltas from round 1

- Upgraded Q1 verdict from "infallible only" to "two-function split, both principled."
- Conceded Q3 to plan9: Inv-RW is the right invariant, connection-graph acyclicity is the wrong level.
- Demoted Inv-CS1 from invariant to implementation detail.
- S3 refinement is ergonomics, not a theoretical requirement.
