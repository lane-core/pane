---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [Inv-RW, request-wait, acyclicity, progress, deadlock, I2, I8, send_request, EAct, DLfActRiS]
related: [architecture/looper, decision/server_actor_model, decision/connection_source_design, analysis/verification/_hub, analysis/eact/_hub]
agents: [pane-architect, session-type-consultant, plan9-systems-engineer, formal-verifier]
---

# Inv-RW: Request-Wait graph acyclicity

## Definition

At any moment, the directed graph whose nodes are in-flight
requests and whose edges are A → B (when A's reply cannot be
produced until B's reply is produced) is acyclic.

Inv-RW is the load-bearing progress invariant for the pane
system. It prevents deadlock among in-flight requests.

## Three guarantees

1. **I2 (handlers do not block):** Handlers return `Flow::Continue`
   immediately. A handler cannot transitively create a wait edge
   to itself within a single dispatch invocation. Convention,
   detection-enforced via heartbeat watchdog.

2. **I8 (synchronous waits confined to non-looper threads):**
   `send_and_wait` panics from looper thread (ThreadId check).
   Non-looper threads hold oneshot channels rather than Dispatch
   entries — they cannot create cycles in the dispatch graph.

3. **Protocol-scoped `send_request`:** Session types bound which
   panes a handler can address. ServiceHandle<P> is the only
   send_request call site, and it requires Handles<P> at compile
   time. This limits the connectivity of the request-wait graph
   to declared protocol relationships.

## Relationship to [JHK24] Theorem 1.2

Different theorem, different graph. [JHK24] §1 argues
*connectivity-graph* acyclicity is needed because in LinearActris
connectivity and wait coincide — a thread blocked on `recv`
literally waits on its peer endpoint. Pane decouples them via
[FH] `E-Suspend` / `E-React`: a connection being open does
**not** establish a wait edge.

[JHK24] Theorem 1.2's hypothesis, mapped onto pane's dispatch
model, is about Inv-RW, not connection topology. For
ProtocolServer's local star topology, [JHK24] Theorem 1.2 is
directly applicable (star is trivially acyclic). For whole-system
progress (Phase 2 multi-connection), cite [FH] EAct progress
(Theorems 6 + 8) — per-actor, no topology requirement.

## Relationship to Inv-CS1

Inv-RW and Inv-CS1 live at different levels:

- **Inv-RW** is a progress invariant over the request-wait graph.
  Load-bearing for safety.
- **Inv-CS1** is a determinism convention over batch-phase
  execution order. Provides deterministic test output and
  auditor-friendly traces. Not safety-relevant.

No tradeoff between them. Dropping Inv-CS1 does not weaken any
safety property. See D4 in `decision/connection_source_design`.

## Provenance

Defined in D3 of `decision/connection_source_design` (2026-04-11).
Session-type-consultant conceded Inv-CS1 to implementation detail
and identified Inv-RW as the load-bearing invariant. Plan9-systems-
engineer contributed the Tflush (D5/D10) escape hatch for deadlock
classes I2/I8 don't catch. Optics-theorist framed the two graphs
(connection vs request-wait) as distinct Getter projections (D6).
