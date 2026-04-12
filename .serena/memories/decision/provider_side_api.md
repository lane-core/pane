---
type: decision
status: decided
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [pub_sub, provider, subscriber, reverse_handle, long_poll, SubscriberSender, WatcherSet, DeclareInterest, ReplyPort, push, pull]
related: [decision/connection_source_design, decision/server_actor_model, architecture/looper, architecture/session, status]
agents: [session-type-consultant, plan9-systems-engineer, be-systems-engineer, optics-theorist]
---

# Provider-side API: dual pub/sub pattern

Decided 2026-04-11 after roundtable consultation (all four design
agents) triggered by attempting to build a pub/sub app on the
current API. The experiment exposed a gap: pane's high-level API
is consumer-facing only. The provider has no convenient handle to
push messages back to subscribers.

## The gap

- Consumer gets `ServiceHandle<P>` → can send_request /
  send_notification TO provider.
- Provider gets `Handles<P>` → can RECEIVE from consumer, reply
  via `ReplyPort`.
- Provider does NOT get a handle to push back to subscriber.
- The wire supports bidirectional messaging after InterestAccepted
  (server.rs Route is symmetric). Gap is purely in the high-level
  API — no wire protocol changes needed.

## Two complementary patterns (not competing)

### Pattern 1 — Long-poll via held ReplyPort (Plan 9 model)

Subscriber sends a Request (e.g., `NextUpdate`). Provider holds
the `ReplyPort` and responds when an update is available. The
subscriber's `on_reply` callback fires, then the subscriber
immediately sends another Request.

```
Subscriber                          Provider
    |--- Request(NextUpdate) ------>|
    |                               |  (holds ReplyPort)
    |                               |  ... event occurs ...
    |<-- Reply(Update{data}) -------|  (consumes ReplyPort)
    |--- Request(NextUpdate) ------>|  (re-subscribe)
```

**Characteristics:**
- Works with today's API. Zero new types or wire primitives.
- Natural flow control — at most one outstanding ReplyPort per
  subscriber. Provider cannot flood.
- One round-trip per event (latency cost).
- Right for: low-frequency property changes, subscriber-paced
  updates, Phase 1 simplicity.

**9P precedent:** Every Plan 9 push service (plumber, rio wctl,
mouse, proc wait) used this pattern. Blocking `Tread` that the
server held until data was ready. The plumber multicast by
iterating pending `Tread`s from all fids on the same port file.

**DeclareInterest = subscribe.** If each topic is its own
ServiceId, the service binding lifecycle IS the subscription
lifecycle. No explicit Subscribe/Unsubscribe messages needed.
Open = subscribe, clunk (RevokeInterest/disconnect) = unsubscribe.
This is exactly how the plumber worked (`open("/mnt/plumb/edit")`
= subscribe, `clunk` = unsubscribe).

### Pattern 2 — Push via SubscriberSender (Be model)

Provider receives a `SubscriberSender<P>` (sending-only
capability) for each subscriber when the service binding is
established. Provider stores these, iterates to push.

**Characteristics:**
- Requires new API surface: `SubscriberSender<P>` type, provider
  callback on InterestAccepted, ServiceTeardown callback for
  cleanup.
- True push semantics — no round-trip latency per event.
- Provider has per-subscriber control: filtering, per-subscriber
  backpressure, different data for different subscribers.
- Right for: high-frequency updates (window invalidation,
  compositor events), server-initiated events (focus change,
  workspace switch).

**`SubscriberSender<P>` is asymmetric from `ServiceHandle<P>`:**
- Consumer's `ServiceHandle<P>`: owns interest lifecycle (Drop →
  RevokeInterest per D8).
- Provider's `SubscriberSender<P>`: sending-only, does NOT own
  lifecycle. Becomes invalid when consumer revokes/disconnects.
  Different ownership, different type.

**Haiku precedent:** `WatchingService` (registrar, clipboard, MIME
server) stored `BMessenger` per client in `map<BMessenger,
Watcher*>`. `NotifyWatchers(message, filter)` iterated the map,
applied filter predicate, sent to each matching watcher. Dead
watchers detected reactively on send failure and cleaned up.
(Source: `src/servers/registrar/WatchingService.cpp:66-228`,
`src/servers/registrar/Watcher.cpp:56-93`.)

app_server `ServerWindow` stored `fClientLooperPort` + `fClientToken`
for lifecycle events (BMessage) and `fLink` (PortLink) for draw
responses (binary link protocol). Two channels per client: BMessage
for lifecycle, link protocol for high-throughput draw. Pane's CBOR
handshake + postcard data plane echoes this split.
(Source: `src/servers/app/ServerWindow.cpp:SendMessageToClient:4399-4408`.)

**Disconnection: use ServiceTeardown, not pane_exited.**
ServiceTeardown is per-service-session (subscriber revoked one
service). pane_exited is per-pane (whole pane died). A subscriber
might revoke one service while keeping others alive.
(session-type-consultant.)

**Optional utility: `WatcherSet<P>`** — convenience wrapper for
the common pattern:
- `add(sender: SubscriberSender<P>)` — register
- `remove(addr: &Address)` — unregister
- `notify_all(msg: &P::Message)` — fan-out (clone + send)
- `notify_filtered(...)` — selective fan-out
- Automatic stale-sender cleanup on send failure
Mirrors Haiku's `WatchingService` as reusable component.

## When to use which

| Use case | Pattern | Why |
|---|---|---|
| Property change notification | Long-poll | Low frequency, subscriber paces |
| Topic subscription | Long-poll | Natural model, DeclareInterest = subscribe |
| Window invalidation / compositor | Push | High frequency, server-initiated |
| Focus / workspace change | Push | Server-initiated, all windows need it |
| Chat / messaging | Long-poll | Subscriber-paced, natural request/reply |
| Sensor / streaming data | Queue<T,S> (Phase 3) | Typed streaming with backpressure |

## What does NOT change

D1-D11 unchanged. Reverse sends from provider are Tier A
`send_notification` (D7 classification). No new request-wait
edges (Inv-RW preserved). No new connectivity-graph edges
([JHK24] star topology preserved). Affine gap identical to
consumer side.

Optics: same D6 structure. AffineTraversal for keyed subscriber
access, Traversal for fan-out broadcast. Handle direction is
opposite-category on morphisms, not profunctor variance reversal
(optics-theorist).

## Implementation phasing

- **Phase 1 (now):** Long-poll works with existing API. Build
  pub/sub experiments using held ReplyPort.
- **Phase 1 (later, with ConnectionSource):** Provider-side
  callback on InterestAccepted to deliver SubscriberSender<P>.
  ServiceTeardown callback for cleanup. This is part of
  "pane-server: service-aware routing, per-service wire dispatch"
  in the status memory.
- **Phase 3:** Queue<T,S> streaming for high-throughput push
  with typed backpressure.

## Provenance

Lane attempted to build a pub/sub app on the current API,
exposing the provider-side gap. Roundtable dispatched on the
design question. Plan9 recommended long-poll (held Tread/ReplyPort
pattern, DeclareInterest-as-subscription insight). Be recommended
per-subscriber SubscriberSender with WatcherSet utility, grounded
in WatchingService and app_server precedent. Session-type confirmed
wire supports bidirectional, recommended serve_with_interest API.
Lane decided to record both as complementary patterns.

(Sources: `agent/session-type-consultant/provider_side_api`,
`agent/be-systems-engineer/provider_side_pub_sub`,
`agent/plan9-systems-engineer/provider_side_pub_sub`,
optics-theorist inline analysis.)
