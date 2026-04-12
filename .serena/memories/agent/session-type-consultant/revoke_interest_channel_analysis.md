---
type: project
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [RevokeInterest, ServiceHandle, Drop, try_send, ctl_channel, data_channel, affine_gap, stale_binding, session_cleanup]
related: [decision/connection_source_design, agent/session-type-consultant/project_connectionsource_review_r2, reference/papers/eact, reference/papers/dlfactris]
agents: [session-type-consultant]
---

# RevokeInterest channel routing analysis

## Question

Should `ServiceHandle::Drop`'s `RevokeInterest` go through a ctl
channel, the data channel, or be handled looper-locally?

## Verdict: Option 4 — keep status quo (single shared channel, try_send), no change needed.

### Critical factual correction

The question as posed assumes separate "data" and "ctl" channels
exist in the client-side write path. They do not. There is ONE
`SyncSender<(u16, Vec<u8>)>` per connection (capacity
`WRITE_CHANNEL_CAPACITY = 128`), shared by all ServiceHandle
instances on that connection. `service_id == 0` is the control
wire-level demux, not a separate in-process channel. Data frames
and control frames compete for the same 128 slots.

This means "routing RevokeInterest through ctl instead of data"
would require creating a second in-process channel that does not
currently exist. The cost-benefit is unfavorable.

### Analysis of each option

**Option 1 — Separate ctl channel (new infra):**

Would require:
- A second `SyncSender` held by every ServiceHandle
- Writer thread selecting on two receivers (or merging)
- ServiceHandle grows from one cloned sender to two

Session-type implication: no safety gain. RevokeInterest is
not a session-type operation — it's a resource cleanup signal.
[FH] §4 `E-RaiseS` covers connection-level abort, not
per-session revocation. Per-session revocation is a server
routing-table operation; it does not change the session
typestate of any active protocol.

Ordering concern: if ctl and data are separate channels, a
RevokeInterest(S) could arrive at the server before the last
Reply(S, token) that's still in the data channel. This is
NOT a safety issue — the server's RevokeInterest handler
removes the route, and the orphaned reply frame hits
`process_service`'s "missing route = Cancel/Reply race"
no-op path (server.rs:413). But it IS a liveness issue: the
consumer's `DispatchEntry` for that token gets orphaned
(no `on_reply`, no `on_failed` fires). The existing
process_disconnect cleanup catches this eventually, but
the window is wider than necessary.

**Option 2 — Status quo (shared channel, try_send):**

try_send failure window: `WRITE_CHANNEL_CAPACITY = 128`.
For the channel to be full, 128 frames must be queued and
the writer thread must not have drained any. Under normal
load this is astronomically unlikely. Under pathological
load (writer thread stalled on transport), the connection
is already effectively dead and process_disconnect will
fire when the transport closes.

Session-type analysis:
- RevokeInterest is not a session obligation — it's cleanup.
  The affine gap (Rust allows Drop without completing the
  protocol) is already covered by process_disconnect.
- try_send failure means the server sees the session torn
  down via Disconnected rather than via RevokeInterest.
  The effect is identical: route removed, peer gets
  ServiceTeardown(ConnectionLost) instead of
  ServiceTeardown(ServiceRevoked). Different reason enum,
  same cleanup.
- Stale binding window: bounded by connection lifetime
  (transport close triggers process_disconnect). This is
  a liveness/ergonomics property, not a safety property.
  No session-type invariant is violated during the window.

**Option 3 — Looper-local revocation:**

Mark session as revoked in local state, suppress further
sends, let connection close handle wire notification.
Problem: the server's routing table holds the route until
either RevokeInterest or Disconnected. During the window,
the provider side still sees the session as active and may
continue sending frames. Those frames arrive at the
consumer's looper and get dispatched to... nothing (the
ServiceHandle is dropped, the ServiceDispatch entry for
that session_id may or may not still be registered).

This is worse than the status quo: it widens the stale
binding window from "try_send failure duration" (nearly
zero) to "remainder of connection lifetime." The provider
allocates resources (ReplyPorts, DispatchEntries) for a
session the consumer has already abandoned.

### Why the status quo is sound

1. **Safety (session fidelity):** RevokeInterest is outside
   the session protocol. It's a routing-table mutation at
   the server. No session-type guarantee ([FH] Theorem 4.7
   preservation, Theorem 4.10 progress) depends on its
   delivery. Sessions are already affine (can be dropped);
   the compensation is process_disconnect, not
   RevokeInterest.

2. **Liveness (stale binding):** Window is bounded by
   transport lifetime. try_send on a 128-slot channel fails
   only when the connection is effectively dead. At that
   point process_disconnect is imminent (writer_loop breaks
   on write failure, reader_loop breaks on read failure,
   both post Disconnected to the server).

3. **No ordering anomaly:** RevokeInterest and data frames
   share the same FIFO channel. The server processes them
   in wire order. No reordering is possible because there is
   only one channel. This is strictly better than Option 1.

4. **[JHK24] Theorem 1.2 not affected:** Connectivity-graph
   acyclicity is a connection-level property. Per-session
   revocation does not add or remove edges in the
   connectivity graph.

### Condition for revisiting

If Phase 2 introduces a separate ctl channel for other
reasons (e.g., Cancel needs guaranteed delivery per D5, or
handshake-negotiated backpressure per O2 creates separate
ctl/data budgets), then RevokeInterest should migrate to
the ctl channel as part of that work. But creating a ctl
channel solely for RevokeInterest is not justified.
