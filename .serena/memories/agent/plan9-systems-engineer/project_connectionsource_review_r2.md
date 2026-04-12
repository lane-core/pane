---
type: project
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [ConnectionSource, round2, handshake_ownership, backpressure, request_wait_graph, Inv-RW, Tflush, cancel, direct_pane_to_pane]
related: [agent/plan9-systems-engineer/project_connectionsource_review, agent/plan9-systems-engineer/dlfactris_reverification_flag, decision/server_actor_model, decision/messenger_addressing, architecture/session, architecture/looper]
agents: [plan9-systems-engineer, session-type-consultant, be-systems-engineer, pane-architect]
---

# ConnectionSource design review — Round 2 (2026-04-11)

Follow-up to `agent/plan9-systems-engineer/project_connectionsource_review`
(round 1). Lane requested detailed reasoning on Q2 (handshake
ownership), and reframings of Q1 (two-function backpressure) and
Q3 (acyclicity, Inv-RW, theorem citation).

## Q2 — handshake ownership: full Option A argument

**Position: ConnectionSource is born Active, handshake runs in a
short-lived bridge thread, handoff via `LooperMessage::NewConnection`
+ oneshot ack.**

### Core principle

ConnectionSource is pane's mounted session. Mounts in Plan 9 do
not exist until version negotiation succeeds. `devmnt.c`
(`mntversion` → `mntattach`) has no code path for a partially
mounted Mnt struct. Pane should inherit the pre-birth / post-birth
distinction: a ConnectionSource that exists is a session that
speaks the protocol.

### Three consequences

1. **Phase ordering stays load-bearing.** Making ConnectionSource
   Handshaking-able adds a mode bit to every dispatch branch in
   the six-phase batch. Mode bits accrete.

2. **Error semantics stay clean.** `ConnectError::Rejected` is
   pre-birth (no watchers, no routing). `ProtocolAbort` is
   post-birth (watchers fire, routing tears down). Under Option B
   these paths merge and every consumer disambiguates.

3. **Watchdog budget.** Hello/Welcome serialization and peer_cred
   syscall all happen off the looper under Option A. Under
   Option B they accrue to the watchdog budget. 16 MB
   `HANDSHAKE_MAX_MESSAGE_SIZE` is not bounded enough to ignore.

### The "ready but not registered" window doesn't exist

Session-type's concern assumed fd handoff requires multi-step
ceremony. Don't do that. The bridge thread sends a
`LooperMessage::NewConnection { welcome, transport, ack:
oneshot::Sender<Registered> }` through the existing mpsc channel.
The looper pops it in phase 3/4 (Lifecycle), constructs
ConnectionSource on the looper thread, registers it via
`LoopHandle::insert_source`, sends `Registered { service_handle }`
back through the oneshot. The bridge thread awaits the ack and
returns `Ok(ClientConnection { service_handle, ... })` to the
app. `PaneBuilder::connect()` stays synchronous-looking.

No handler exists during the unregistered window because no
`ServiceHandle<P>` has been handed out yet. Session-type's
"handler attempting to use not-yet-wired ServiceHandle" cannot
occur.

### 9P precedent (explicit)

`version(5)`: "Version must be the first message sent on the 9P
connection, and the client cannot issue any further requests
until it has received the Rversion reply." Tversion uses NOTAG
(~0) to mark itself as outside the ordinary request stream. The
kernel's devmnt does Tversion synchronously before `mntattach`
installs anything. rio's filsysproc reads Tversion as a
distinguished first message. plumber's fileserver follows the
same pattern. Every Plan 9 file server treats version as a boot
sequence, not a state in the main loop.

### fd handoff mechanism concretely

Not `sendmsg(SCM_RIGHTS)` — in-process move of the
`UnixStream` / `Transport` through the existing mpsc channel.
One new `LooperMessage` variant. One oneshot for registration
confirmation. That's it.

## Q1 — backpressure API shape (refined)

**Position: ship one function first (`send_request` infallible,
cap-and-abort), defer `try_send_request` until a concrete user
appears.**

9P had one wire shape but diverse client APIs. The two-function
proposal is not a 9P violation — it's orthogonal ergonomics. But
prefer one if one suffices. Adding functions is cheap; removing
them isn't.

### Handshake-negotiated cap

Add two fields to Hello/Welcome:

- `max_outstanding_bytes: u32` — write queue byte cap (hard
  backpressure signal)
- `max_outstanding_requests: u16` — pending-request count
  (fairness across services on one connection)

Client proposes, server may reduce (same semantics as `msize` in
version(5)). Byte cap is the primary mechanism; request count is
speculative and can be added later if fairness problems appear.

### rio and plumber

rio used kernel-buffered blocking pipes (~4 KB pipe buffer as
implicit flow control). Does NOT translate to calloop — the
looper cannot block. Pane's analog is the bounded mpsc queue
(currently `WRITE_CHANNEL_CAPACITY = 128` in `bridge.rs:351`).

plumber had no backpressure because the rules engine was dirt
cheap by design — microsecond rule evaluation, no slow ops in
dispatch. Pane gets the same property from I2 (handlers can't
block), plus cap-and-abort as the backstop for adversarial
handlers.

## Q3 — Inv-RW replaces DLfActRiS as the progress argument

### Inv-RW stated

**Request-wait graph acyclicity:** At any moment, consider the
directed graph whose nodes are in-flight requests and whose
edges are A → B when A's reply cannot be produced until B's
reply is produced. This graph is acyclic.

Guaranteed by I2 (handlers don't block, return Flow::Continue) +
I8 (send_and_wait panics from looper thread) + protocol-scoped
send_request. There is no "held while waiting" state to cycle.

### Relationship to JHK24 Theorem 1.2

**Different theorem, different graph.** JHK24 Theorem 1.2 is
about the channel-endpoint connectivity graph. Inv-RW is about
the dynamic request-wait graph inside dispatch state. They are
not comparable.

Phase 1 recommendation: restrict the `decision/server_actor_model`
DLfActRiS citation to "proves ProtocolServer's local star
topology is progress-safe" (still true). For whole-system
progress in Phase 2 (direct pane-to-pane), cite **EAct progress**
(Fowler/Hu) — per-actor, no topology requirement. Each pane is
a sequential mailbox actor; EAct applies per-actor; whole-system
progress follows if the inter-pane protocol is session-type
deadlock-free.

Session-type-consultant has the final word on the citation.

### Inv-CS1 and S3 refinement under Inv-RW

- **Session-type's Inv-CS1 (phase 5 per-source drain):** redundant
  for safety (Inv-RW holds regardless), necessary for fairness.
  Ship it, document as fairness.
- **Be's S3 refinement (ctl writes per-source):** same verdict.
  Preserves per-source causality (correct); cross-source order
  is a fairness choice. Ship it, document as fairness.

Neither is load-bearing for correctness under Inv-RW.

### Plan 9's actual deadlock-freedom mechanism

Plan 9 did not prove deadlock freedom. It relied on:

1. **Per-process namespaces that don't share state across
   boundaries.** Cycles in the namespace graph are not cycles
   in the request-wait graph. Pane gets this from per-pane
   loopers.
2. **Tflush(5) as universal escape hatch.** Any client can flush
   any outstanding request by tag; the server must complete or
   abandon. No deadlock is permanent.
3. **Clunk-on-abandon.** Kernel clunks fids on process death.
   Pane gets this from Drop.
4. **auth(5) separated from operational phase.** No auth-state
   cycles.

**Recommendation:** make Cancel a first-class universal operation
that crosses any ConnectionSource regardless of protocol state.
This is the one Plan 9 mechanism pane should inherit to cover
deadlock classes that I2/I8 don't catch. Currently
`decision/messenger_addressing` does not commit to Cancel being
universal — it should.

Confidence: high on "Plan 9 relied on isolation + Tflush"; high
on "pane needs a universal cancel"; medium on ConnectionSource
level as the right implementation point (could also live at
Connection or Transport level).

## Dispositions summary

| Question | Round 1 | Round 2 refinement |
|---|---|---|
| Q2 handshake | Option A | Option A, detailed fd handoff via LooperMessage + oneshot ack |
| Q1 backpressure | Cap-and-abort, handshake-negotiated cap | Ship one function; defer try_send; negotiate bytes first, count later |
| Q3 acyclicity | Don't enforce at ConnectionSource | Inv-RW is the invariant; cite EAct for Phase 2; universal Cancel as Tflush analog |

## Files consulted (round 2)

- `crates/pane-session/src/bridge.rs:260-376` (run_client_bridge)
- `crates/pane-session/src/handshake.rs` (Hello/Welcome)
- `crates/pane-session/src/server.rs:416-444` (watch table)
- Plan 9 sources: devmnt.c, rio/filsys.c, plumber.c
- Plan 9 man pages: intro(5), version(5), flush(5), auth(5), mount(2)
