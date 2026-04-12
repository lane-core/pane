---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [pubsub, notification, blocking-read, plumber, provider-push, 9P, rio, event-file]
related: [reference/plan9/papers_insights, reference/plan9/man_pages_insights, reference/plan9/divergences]
agents: [plan9-systems-engineer]
sources: [plumb.ms, 8half.ms, rio(4), plumber(4), intro(5), devmnt.c]
---

# Analysis: pub/sub and provider-initiated push in pane

Lane asked (2026-04-11): pane's current model gives consumers a
ServiceHandle<P> to send to providers, but providers have no
handle to push back. How did Plan 9 handle server-to-client
notification? Is blocking-read the right model? Does pane need
server-initiated push at all?

## 1. 9P precedent: no server push

9P is strictly request-response. The server NEVER initiates a
message. Every 9P exchange is Trequest -> Rreply. The only
"server-initiated" behavior is completing a previously blocked
read. The server holds the Tread and responds when data is
available. From the wire's perspective, this is still a response
to the client's request.

Specific mechanism: client sends Tread on the fid. Server's
9P implementation (lib9p Srv, or kernel devmnt) holds the
request in a queue. When the server has data, it responds with
Rread containing the data. The client's read(2) call returns.
If the client wants more notifications, it reads again.

This is NOT polling. The read blocks. No busy-wait, no retry
interval. It's cooperative blocking: the client says "I'm
ready for the next event" and the server says "here it is"
when one exists.

## 2. The event file pattern (/dev/cons, rio wctl, mouse)

Plan 9 services that needed to push events used blocking reads
on synthesized files:

- **rio(4) wctl**: "A subsequent read will block until the
  window changes size, location, or state." Client opens wctl,
  reads geometry+state, reads again — blocks until change.
  
- **mouse(3) via rio(4)**: Reading the mouse file blocks until
  the mouse moves or a button changes.

- **proc(3) wait**: Read blocks until a child exits. The wait
  file is the death notification mechanism.

- **plumber(4) port files**: Read blocks until a plumb message
  arrives for that port.

The pattern is uniform: open file, read blocks, data arrives
when state changes, read again for next event. The file IS the
subscription. Opening it subscribes; clunking (closing) it
unsubscribes. No separate subscribe/unsubscribe messages needed.

## 3. Plumber specifics

The plumber was Plan 9's closest thing to pub/sub. At the 9P
level:

1. Client opens `/mnt/plumb/edit` (a port file).
2. Client sends Tread on that fid.
3. Plumber holds the Tread — no Rread yet.
4. Someone writes a plumb message to `/mnt/plumb/send`.
5. Plumber pattern-matches, finds `edit` port, responds with
   Rread containing the formatted plumb message.
6. Client processes it, sends another Tread. Blocks again.

Multicast: "A copy of each message is sent to each client that
has the corresponding port open." Multiple open fids on the
same port file each get a copy. The plumber tracks open state
via 9P open/clunk.

Fan-out cost: O(readers) per message. Each reader has an
independent fid and an independent blocked Tread. The plumber
iterates over all open fids for the port and responds to each.

## 4. How this maps to pane

### The gap restated

In pane's current model:
- Consumer gets ServiceHandle<P> with write_tx to send to provider
- Provider receives via Handles<P>::receive / HandlesRequest<P>
- Provider has NO write_tx back to the consumer
- The wire (SyncSender<(u16, Vec<u8>)>) IS bidirectional after
  the server assigns session_ids on both sides
- But the high-level API only surfaces consumer->provider

### Three options analyzed

**Option A: Blocking-read (Plan 9 model)**
Subscriber sends a request (e.g., Subscribe { topic }) via
send_request. Provider holds the ReplyPort, replies when an
update arrives. Subscriber's on_reply callback fires, subscriber
sends another request for the next update.

Pros: No new wire primitives. Works with existing
ServiceFrame::Request/Reply. Each "subscription" is a held
ReplyPort — natural flow control (provider can't flood).
Cons: One outstanding request = one buffered event. If events
arrive faster than the subscriber re-requests, the provider
must buffer or drop. ReplyPort is one-shot (reply consumes it),
so each event requires a new request/reply round-trip. Latency
of one round-trip per event.

**Option B: Provider gets a reverse ServiceHandle**
When InterestAccepted, the server creates routing pairs in both
directions. The provider receives a ServiceHandle<P::Reverse>
or similar. Provider can send_notification back to the consumer.

Pros: True push — provider sends when it has data. No round-trip
latency. Natural for pub/sub.
Cons: New wire mechanism. Provider needs to know about each
subscriber (fan-out management). Requires defining what the
reverse protocol is. Changes the service model significantly.

**Option C: Queue/Dequeue streaming (already in architecture)**
architecture.md already defines Queue<T, S> / Dequeue<T, S>
for session-typed streaming with backpressure. Provider opens
a stream, pushes items. Consumer reads items.

Pros: Already designed. Handles backpressure (push() returns
Err(Backpressure)). Session-typed — stream closure transitions
to continuation type S.
Cons: Not yet implemented (Phase 3). Stream is point-to-point,
not multicast. Each subscriber needs its own stream.

### Recommendation

The blocking-read model (Option A) works TODAY with existing
primitives and is the right model for the common case. But it
should be understood as "long-polling" — each event is a
request/reply pair. This is exactly what Plan 9 did.

For high-throughput pub/sub (many events, many subscribers),
Queue<T, S> (Option C) is the right answer once implemented.
Each subscriber gets a dedicated stream from the provider.

Option B (reverse handle) is unnecessary if Queue exists. The
Queue IS the reverse channel, with backpressure and session
typing that a raw reverse handle would lack.

### The provider's subscriber tracking problem

In all three options, the provider must track subscribers.
Plan 9 solved this implicitly: the plumber tracked open fids.
Open = subscribed, clunk = unsubscribed. pane's equivalent:

- Provider implements HandlesRequest<TopicProto>
- Consumer sends Subscribe { topic } request
- Provider stores the ReplyPort (Option A) or opens a Queue
  to the consumer (Option C)
- Consumer's RevokeInterest or disconnect = unsubscribe
  (provider's ServiceTeardown callback cleans up)

The server already sends ServiceTeardown when a consumer
disconnects. This is the clunk equivalent.
