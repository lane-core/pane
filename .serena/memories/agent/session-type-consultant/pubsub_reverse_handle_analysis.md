---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [pubsub, reverse_handle, provider_to_consumer, bidirectional_session, service_interest, obligation, affine_gap, ServiceTeardown]
related: [decision/connection_source_design, architecture/app, architecture/session, reference/papers/eact, reference/papers/dlfactris]
agents: [session-type-consultant]
---

# Pub/Sub reverse handle: session-type analysis

**Verdict: conditionally sound.** Three conditions.

## Core finding

The wire is already bidirectional after InterestAccepted.
server.rs allocates two session_ids and routes symmetrically.
The gap is API-only: the provider has no `ServiceHandle<P>`
for its half of the session.

## Session-type structure

Single bidirectional session, not two composed unidirectional.
Both directions share the same `Route` in the server's routing
table. [FH] §3.2 E-Send is direction-agnostic. Exposing the
reverse direction adds no connectivity-graph edge ([JHK24]
Theorem 1.2 still applies to star topology).

## Recommended API: serve_with_interest

Deliver `ServiceHandle<P>` to provider at InterestAccepted
time via a callback registered during PaneBuilder::serve.
Rationale: (1) matches [FH] E-Init — session establishment
is distinct from message exchange; (2) ServiceHandle is
!Clone, so delivering once at connection time is the only
ergonomic option; (3) zero-cost for non-pub/sub providers.

Reject: adding session_id or PeerScope to every receive call.
Bloats the hot path for 90% case.

## Obligation analysis

Reverse ServiceHandle::Drop sends RevokeInterest per D8.
Provider notified of subscriber disconnect via ServiceTeardown
(already sent by server on process_disconnect). Per-session
callback needed on provider side — pane_exited is per-pane,
not per-service-session.

Affine gap: same as consumer side. Drop terminates session
early, peer always notified via ServiceTeardown. [FH] Theorem
6 (Preservation under failure, §4) covers this.

## D1-D11 impact

No changes. Reverse handle send_notification is already
classified in D7 Tier A (two variants). D8 applies
symmetrically. D9 (request cap) unaffected — notifications
don't count. D10 (cancel) unaffected — notifications have
no tokens.

## Conditions

1. Reverse handle delivered at InterestAccepted, not
   constructed ad-hoc from raw session_id.
2. ServiceTeardown dispatched to provider as per-session
   callback (not only framework-internal).
3. Reverse handle uses same write channel as forward
   direction, with provider_session as discriminant.

No wire changes. No new ControlMessage variants.
