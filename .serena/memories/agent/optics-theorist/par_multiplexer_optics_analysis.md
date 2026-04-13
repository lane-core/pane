---
type: analysis
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: critical
keywords: [par, multiplexer, routing_table, AffineTraversal, linear, mixed_optic, Server, ConnectionData, two_map, boundary]
sources:
  - "CBG24 Definition 4.6 (AffineTraversal)"
  - "CBG24 §4 (mixed optics)"
  - "par 0.3.10 exchange.rs, server.rs"
  - "dependency/par digest"
  - "analysis/optics/scope_boundaries"
  - "analysis/session_types/optic_boundary"
  - "decision/mpst_extraction_plan"
  - "decision/pane_session_mpst_foundation"
verified_against:
  - "crates/pane-app/src/dispatch.rs (Dispatch<H> entries HashMap)"
  - "crates/pane-session/src/correlator.rs (RequestCorrelator)"
  - "crates/pane-session/src/active_session.rs (ActiveSession)"
  - "crates/pane-app/src/service_handle.rs (send_request flow)"
related: [decision/pane_session_mpst_foundation, decision/mpst_extraction_plan, analysis/optics/scope_boundaries, agent/optics-theorist/linearity_gap, dependency/par]
agents: [optics-theorist]
---

# Par-based Multiplexer: Optic Analysis

Lane directed: pane-session MUST use par's session types as
foundation. This analysis classifies state access patterns for
the par-based session multiplexer.

## 1. Routing table optic

`HashMap<(PeerScope, Token), Send<ResponseBytes>>` is an
**AffineTraversal with linear codomain** — a mixed affine optic
per CBG24 §4:

- View side in Set (cartesian): key lookup is pure
- Update side in Lin (linear): par endpoint consumed by use
- `HashMap::remove` IS the concrete encoding
- 0-or-1 focus (key may be absent) = affine, not lens

Gate tests from `feedback_not_every_access_is_a_lens`:
- Gate 1 (get does something): Yes, remove reads+mutates
- Gate 2 (thunkable): No, par endpoint is obligation handle
- Gate 3 (focus count): 0-or-1, correct for AffineTraversal

NOT a MonadicLens — Proposition 4.7 does not apply. The optic's
job ends at extraction; endpoint consumption is linear protocol.

## 2. Two-map design (Option C recommended)

Three options evaluated:
- (A) Two maps same key, shared mutable state — coherence smell
- (B) One map with pair — H leaks into routing, violates boundary
- (C) Separated by lifecycle, connected by LooperMessage — clean

Option C: pane-session owns par endpoints + routing table.
pane-app owns DispatchEntry<H> closures. Connected by existing
LooperMessage channel. Each side has independent AffineTraversal
on its own map. No shared mutable state across crate boundary.

Flow: wire bytes → pane-session removes Send endpoint → send()
delivers to Recv side → Recv posts LooperMessage → pane-app
removes DispatchEntry → fires typed callback.

## 3. Server as ConnectionData — rejected

par Server manages connection lifecycle (event-driven poll).
ActiveSession needs continuous access for message routing.
Server access is NOT an optic — it's an event source. Gate 1
fails: async poll is not a synchronous get.

Use Server for connection lifecycle. Hold multiplexing state
independently, indexed by Server-provided connection IDs.

## 4. Boundary after par integration

The H-independence line does NOT shift. Par endpoints are
H-independent (carry bytes, not closures). Par integration
SHARPENS the boundary: routing cleanly in pane-session,
dispatch cleanly in pane-app. Currently interleaved in
Dispatch<H>; par separates them.

## Optic type summary

| Structure | Optic | Category | Crate |
|---|---|---|---|
| Routing table (par endpoints) | AffineTraversal | Set × Lin | pane-session |
| Closure table (DispatchEntry) | AffineTraversal | Set | pane-app |
| RequestCorrelator fields | Lens (product) | Set | pane-session |
| Revoked sessions set | AffineTraversal | Set | pane-session |
| Server connection lifecycle | NOT optic (event source) | N/A | pane-session |
| ServiceHandle send path | Kleisli arrow | N/A | pane-app |
