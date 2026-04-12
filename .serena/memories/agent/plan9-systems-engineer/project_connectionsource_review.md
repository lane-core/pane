---
type: project
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [ConnectionSource, calloop, unified_fd, cap_and_abort, handshake, backpressure, DLfActRiS, direct_pane_to_pane, dispatch_model]
related: [architecture/session, architecture/looper, decision/messenger_addressing, decision/server_actor_model, reference/plan9/divergences, reference/plan9/man_pages_insights]
agents: [plan9-systems-engineer]
---

# ConnectionSource design review (2026-04-11)

Phase 1 blocker. Design consultation for pane-app's calloop
EventSource wrapping a post-handshake Connection. Replaces the
bridge thread-per-half model in `pane-session/src/bridge.rs:266-375`.
Design rule: design for multi-connection (Phase 2), implement
single-connection first.

## Three recommendations

### Q1 — Unified fd source, not split

Use a single `calloop::generic::Generic<Fd>` registered with
`Interest::BOTH`. Toggle the WRITE bit based on write-queue
non-empty state via `LoopHandle::update()`. Kill the
`TransportSplit` trait at this seam: it was a Rust ownership
workaround for the multi-thread bridge (documented inline at
`transport.rs:181-184` as such), not a design preference.

**Plan 9 grounding.** `mount(2)` / devmnt
(`/sys/src/9/port/devmnt.c`) used one fd for both directions.
9P demultiplexes via tag field (16-bit), not via "which side
the byte came from." Closing the fd ends the conversation
atomically — there's no half-close.

**Implementation hazard:** forgetting to re-arm WRITE after
enqueuing causes silent write stalls. Add a test that fills
the kernel buffer and confirms re-arm.

**Confidence: high.**

### Q2 — Cap-and-abort backpressure, not fallible sends

Write queue has a cap. Overflow tears down the connection
(ProtocolAbort). Handlers see infallible `send_request` from
`ServiceHandle<P>` (per `decision/messenger_addressing §3`),
matching I2 (no blocking in handlers).

Refinement: negotiate the cap in Hello/Welcome as
`max_outstanding_bytes` (or frames), analogous to
`max_message_size` in `handshake.rs:10-11`. Both ends declare,
effective is minimum. This makes the cap auditable instead of a
hidden constant.

**Plan 9 grounding.** 9P has zero protocol-level flow control.
Tversion's only negotiated parameter is `msize`. Tflush cancels
an outstanding request by tag (intro(5), flush(5)) — there is
no Tslow. devmnt blocked writers in the *kernel* queue, which
pane cannot replicate in userspace without violating I2. When
9P got into bad states, devmnt tore down the whole mount. This
is exactly cap-and-abort.

**What the architecture doc already commits to:**
`docs/architecture.md:1233-1237` specifies Backpressure for
**streams** (Queue/Dequeue) — that's correct for streams
because they're producer-controlled. For ordinary send_request
on ServiceHandle, cap-and-abort is right.

**Confidence: high on cap-and-abort; medium on handshake
refinement (can land later without breakage).**

### Q3 — Handshake in bridge thread, ConnectionSource born Active

Option A. Short-lived bridge thread runs handshake synchronously
(`verify_transport` + write Hello + read Welcome), then hands
(fd, Welcome) to the looper via a `LooperMessage::NewConnection`
event. Looper constructs ConnectionSource in Active state and
registers it with calloop.

**Plan 9 grounding.** Tversion/Rversion in `version(5)`:
"Version must be the first message sent on the 9P connection,
and the client cannot issue any further requests until it has
received the Rversion reply." Tversion uses NOTAG (0xFFFF),
resets the session, wipes the fid table. The kernel's devmnt
does version negotiation synchronously *before* installing the
mount; if it fails, the mount never happens. There is no
"partially mounted" state. **ConnectionSource is pane's mounted
session** — it should be the post-handshake object.

**Anti-pattern avoided:** handshake-inside-dispatch means every
tick checks phase, partial Hello reads park in state, error
paths distinguish "never existed" from "active session aborted"
(different watch-table semantics per `server.rs:416-444`),
writers need pre-handshake guards. All of this goes away with
Option A.

**Cost:** short-lived thread per in-flight handshake.
Negligible at pane's scale. Matches current bridge.rs flow,
just trimmed (Phase 3 split+spawn replaced with channel send).

**Split consideration:** client-side connect (DNS/TLS/etc. can
take seconds) needs the bridge thread. Server-side accept might
eventually be inlined with heartbeat watchdog coverage. Start
uniform.

**Confidence: high on principle; medium on uniform
client/server treatment.**

## Bonus — DAG-acyclicity for direct pane-to-pane

**Don't enforce at ConnectionSource.** Acyclicity is a property
of the **request-wait graph**, not the **connection graph**.
The request-wait graph is guaranteed acyclic by:

1. I2 (handlers cannot block) — handlers issue send_request
   and return Flow::Continue; replies come back via phase 5
2. I8 (send_and_wait panics from looper thread) — synchronous
   waits only on non-looper threads, which don't own a
   ConnectionSource
3. Protocol-scoped send_request (`decision/messenger_addressing
   §3`) — session types constrain legal messages per state

Connection cycles are fine (9P had them routinely: A exports
/dev to B, B exports /bin to A). What matters is that no
handler waits for a reply while holding the dispatch thread.

**Flag for session-type-consultant:** the DLfActRiS Theorem 1.2
citation in `decision/server_actor_model` was made in a
star-topology context. When Phase 2 lands direct pane-to-pane,
the specific theorem no longer transitively covers the whole
system. Re-verify that either (a) a different theorem applies,
or (b) the I2+I8+session-type dispatch model is itself
sufficient. I lean (b); they have the final word.

**Confidence: high on "don't enforce at ConnectionSource";
medium on the dispatch-model-suffices argument.**

## Spirit-of-Plan-9 check

No conflicts with "everything is a file" — ConnectionSource
lives in the protocol tier, orthogonal to pane-fs. A future
`/pane/connections/<id>/status` following the proc(3) ctl
pattern would be a pane-fs-tier addition, not a ConnectionSource
concern. Namespace transparency preserved:
ConnectionSource is blind to peer locality (local unix vs
remote TLS), consistent with
`decision/host_as_contingent_server`.

## Files consulted

- `crates/pane-session/src/transport.rs` — Transport trait,
  TransportSplit (transport.rs:181-188), UnixStream impl
- `crates/pane-session/src/bridge.rs:266-375` — current
  handshake + split + reader/writer thread flow
- `docs/architecture.md:1220-1237` — Streams + Backpressure
  section, stream-specific Err(Backpressure) semantics
- `architecture/session` — bridge, ProtocolServer, FrameCodec
  self-poisoning
- `architecture/looper` — six-phase batch, I2/I6/I8,
  single-thread by construction
- `decision/messenger_addressing` — protocol-scoped
  send_request, direct pane-to-pane
- `decision/server_actor_model` — DLfActRiS citation context
- `reference/plan9/man_pages_insights §1` (proc ctl pattern,
  rio wctl blocking reads)
- `reference/plan9/divergences` (mount(2) one-fd note, 9P
  clunk-on-abandon)
