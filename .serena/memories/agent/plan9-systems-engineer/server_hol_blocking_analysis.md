---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [head-of-line-blocking, server, write-path, 9P, devmnt, per-connection-queue, backpressure, builder-ordering]
sources: [decision/connection_source_design, architecture/session, architecture/looper, reference/plan9/foundational]
verified_against: [crates/pane-session/src/server.rs:412-424, crates/pane-app/src/connection_source.rs:307-418]
related: [decision/connection_source_design, decision/server_actor_model, agent/plan9-systems-engineer/project_connectionsource_review_r2]
agents: [plan9-systems-engineer]
---

# Server head-of-line blocking analysis

Lane asked four questions about pane's server write path, builder
ordering, and Plan 9 precedent. Stress test confirmed: publisher
stalls at 774ms (vs <2ms with MemoryTransport) because
ProtocolServer's `process_service` (server.rs:419-420) synchronously
calls `wh.write_frame()` on the target connection. `write_frame`
takes `writer.lock()` and does a blocking write on the socket fd.
When the target's kernel buffer is full (slow reader), the actor
thread blocks, stalling ALL routing for ALL connections.

## Q1 — 9P server routing and slow clients

Plan 9's 9P multiplexer in `devmnt.c` (the kernel mount driver)
had a different topology: it was a client-side mux, not a
server-side router. `mountmux` (devmnt.c:803-870) read Rmsgs from
the mount fd and woke the specific process waiting on each tag.
Writes went the other direction: `mountrpc` (devmnt.c:660-800)
wrote Tmsgs to the mount fd. The mount fd was a kernel pipe to the
file server; the write could block if the pipe buffer was full.

For **file servers** (e.g., `ramfs`, `exportfs`, `fossil`),
lib9p's dispatch model was: one reader thread reads Tmsg, spawns
or dispatches a worker, worker produces Rmsg, worker writes Rmsg
to the connection fd. The write was synchronous. A slow client's
pipe buffer filling up would block that worker — but not other
workers serving other requests or other connections, because each
worker was independent.

**The key difference:** Plan 9 file servers either (a) used one
process per request (`exportfs` forked per request in early
versions) or (b) used a thread pool where each thread handled one
request and wrote its reply independently (`lib9p`). The actor
thread never did the routing AND the writing — those were separate
concerns in separate threads.

Plan 9 did NOT solve the slow-client problem at the protocol
level. It solved it structurally: the writer of a reply was the
handler of that request, and blocking the writer only blocked that
one request's completion. This is not possible when a single actor
thread does all routing.

**What Plan 9 actually did about slow clients:**

- `exportfs` eventually moved to a fixed thread pool
  (exportfs.c:rpc → srvwork). Each worker held a Fcall, processed
  it, wrote the reply. Slow client = that worker blocked = other
  workers continued.
- The kernel's `devmnt.c` write path (`mountio`) held a qlock on
  the mount channel for the write. A slow server blocked the
  client process that made the 9P call — but only that process.
  Other processes using different mounts (or even the same mount
  with different tags) could still proceed because they ran in
  separate threads.
- There was no per-connection output queue in the protocol.
  The kernel trusted that the pipe/network would absorb writes
  quickly enough. When it didn't, the calling process blocked
  in `mountrpc`. This was considered acceptable because the
  alternative (non-blocking writes with retry) conflicted with
  Plan 9's model of simple, blocking I/O.

## Q2 — Per-connection non-blocking write queues

Recommendation: the server actor should enqueue frames into a
per-connection write queue instead of synchronously writing.
This is the same pattern ConnectionSource already implements on
the client side (FrameWriter + VecDeque).

**Queue bounding:** Three options for when a per-connection queue
fills:

1. **Drop connection (recommended for Phase 1).** When the queue
   exceeds a highwater mark (e.g., 64 frames), the server tears
   down the connection: sends ProtocolAbort, closes the fd, fires
   PaneExited to watchers. This is what HTTP/2 servers do when a
   stream window is exhausted and the client isn't reading. It's
   what Plan 9 would have done if the pipe broke — process gets
   an error, file server cleans up.

2. **Drop frames (bad).** Violates session-type obligations.
   Request/Reply pairs become dangling. Only viable for
   notifications where loss is tolerable, but distinguishing at
   the routing layer requires inspecting frame content (breaks
   opacity).

3. **Signal backpressure to sender (Phase 2).** The server would
   need to send a flow-control frame back to the source
   connection, telling it to slow down sends to a specific
   session. This is TCP window scaling at the application layer.
   Complex, and Plan 9 never did it — 9P has no flow control
   beyond the pipe buffer.

For Phase 1, option 1 is correct. A connection that can't drain
its write queue within a reasonable budget is broken from the
server's perspective. The connection-level teardown is the
escalation path, and it's exactly what Plan 9's error model
would produce — the write fails, the connection is cleaned up.

**Highwater mark sizing:** 64 frames × max_message_size gives
the worst-case memory. With 4KB max_message_size, that's 256KB
per slow connection — bounded and acceptable. The mark should be
configurable per-connection, negotiated alongside
max_outstanding_requests (or derived from it).

## Q3 — Connection lifecycle and builder ordering

Plan 9's `mount(2)` was synchronous: `mount(fd, afd, old, flags,
aname)` ran the version/attach sequence (9P's handshake), then
modified the calling process's namespace. The mount call returned
only after the connection was fully established. There was no
"register an event source" step because Plan 9's I/O model was
blocking — you read from /dev/whatever and the kernel did the 9P
walk/read/write on your behalf, synchronously.

**Mapping to pane:** D2 (Option A) already solves this correctly.
The handshake runs in a bridge thread, and `NewConnection` hands
the live fd to the Looper via the existing mpsc channel. The
Looper constructs and registers ConnectionSource when it processes
the `NewConnection` message. The key insight is the same as
`mount(2)`: the connection must be fully established (handshake
complete) before anything in the event loop touches it.

The remaining builder-ordering problem — "Looper must be running
before connections register" — is solved by the oneshot ack in
D2's handoff. `connect()` blocks on the ack, which the Looper
sends after `insert_source`. The Looper doesn't need to be
running when `connect()` is called; it just needs to be running
before the ack is consumed. This is fine because PaneBuilder
typically calls `connect()` before `run()`, and `run()` processes
the queued `NewConnection` message on its first iteration.

**But there's a subtlety Lane is pointing at:** if `connect()`
is called before the Looper thread exists (before `run()`), the
`NewConnection` message sits in the mpsc channel and the oneshot
ack never fires. The bridge thread blocks forever.

**Fix:** `connect()` should be callable before `run()`. The mpsc
channel already exists at PaneBuilder construction time. The
`NewConnection` message queues. `run()` drains the queue on first
iteration. The bridge thread blocks on the oneshot ack, which is
fine — it's a bridge thread, not the looper thread. This is
exactly how Plan 9's `mount(2)` worked: the mount could happen
before any reads, because mount was a kernel call that ran to
completion before returning.

## Q4 — Simplifying the write path

Current client-side write path: ServiceHandle → mpsc(128) →
ConnectionSource VecDeque(highwater 8) → FrameWriter → fd.

Three buffer layers:
1. mpsc(128) — bounded channel from handler to ConnectionSource
2. VecDeque (highwater 8) — internal queue in FrameWriter
3. Kernel socket buffer — OS-level

Plan 9's `devmnt.c` had one buffer layer: the kernel pipe buffer.
`mountio` wrote directly to the mount channel. There was no
application-level queuing — the kernel pipe was the queue.

**Can pane simplify?** The mpsc channel exists because
ServiceHandle (on the looper thread) and ConnectionSource (also
on the looper thread) are the same thread. The channel is
intra-thread. This is the key waste: an mpsc channel is a
cross-thread synchronization primitive being used for
same-thread communication.

**Proposed simplification (single queue):**

If ServiceHandle can write directly to ConnectionSource's
FrameWriter queue (they're on the same thread), eliminate the
mpsc channel entirely for looper-thread sends. FrameWriter's
VecDeque becomes the only application-level buffer. The path
becomes: ServiceHandle → FrameWriter VecDeque → fd.

This requires ConnectionSource to be accessible from the looper
thread's dispatch context. The Looper already owns
ConnectionSource (it's a calloop EventSource). The dispatch
context (DispatchCtx / Messenger) needs a reference or handle to
the connection's FrameWriter.

**For cross-thread sends (send_and_wait, SubscriberSender from
non-looper threads):** keep a bounded channel. These are genuinely
cross-thread and need synchronization. But looper-thread sends
(the 90% path) skip the channel.

This matches Plan 9 exactly: writes from the same address space
went directly to the buffer (mountio on the calling process's
stack). Cross-machine writes went through the network pipe
(inherently queued by the kernel).

**What you pay:** FrameWriter must be shared between
ConnectionSource and the dispatch context. On the looper thread,
this is safe (single-threaded, I6). The type system will want
proof — either interior mutability (RefCell) or the dispatch
context borrowing ConnectionSource mutably during dispatch
(which calloop's EventSource doesn't naturally support; you'd
need a split borrow or an indirection).

**Confidence:** High on the direction (eliminate intra-thread
channel), medium on the mechanism (RefCell vs split borrow vs
extracting FrameWriter into shared ownership).
