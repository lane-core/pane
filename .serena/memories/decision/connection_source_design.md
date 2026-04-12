---
type: decision
status: decided
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [ConnectionSource, round2, Inv_RW, Inv_CS1, determinism, backpressure, two_function_split, universal_cancel, handshake_option_a, phase_1, tier_classification, RevokeInterest, hybrid_revocation, deferred_leave, CBOR, handshake_format, wire_extensibility]
related: [agent/plan9-systems-engineer/project_connectionsource_review, agent/plan9-systems-engineer/project_connectionsource_review_r2, agent/plan9-systems-engineer/dlfactris_reverification_flag, agent/session-type-consultant/project_connectionsource_review_r2, decision/server_actor_model, decision/messenger_addressing, architecture/looper, architecture/session, architecture/app, status]
agents: [pane-architect, formal-verifier, session-type-consultant, plan9-systems-engineer, be-systems-engineer, optics-theorist]
---

# ConnectionSource design (WIP)

Working decision log for pane's Phase 1 Core blocker: calloop
EventSource wrapping a Connection fd, designed for multi-connection
registration. Captures round-1 and round-2 roundtable outcomes and
Lane's decisions 2026-04-11. D7 (tier classification) and D8
(RevokeInterest hybrid) added same day after O1 roundtable and
targeted follow-up. All blocking items resolved (D1-D10). pane-architect can
dispatch ConnectionSource implementation. Non-blocking items
O3/O4/O6 and latent items L1/L2 remain for doc/audit.

## Decided (2026-04-11)

### D1 — Q1 backpressure API: two-function shape

Two functions per send site in the general case:

- `send_*` — infallible-return, cap-and-abort on overflow. 90% path.
- `try_send_*` — fallible, returns `(Req, Backpressure)` on error.

**Linearity condition (non-negotiable, session-type-consultant):**
the fallible variant **must** return the request inside the error
variant: `Result<CompletionToken, (Req, Backpressure)>`. Same
semantics as `std::sync::mpsc::SyncSender::try_send`. Without this,
the request obligation handle is consumed on the error path and I4
(typestate handles) linearity breaks at the call boundary.

**Theoretical status:** `send_* = panic_on_err ∘ try_send_*` is a
*pointed monad algebra*, not a Kleisli triple and **not** [CBG24]
MonadicLens (Def 4.6 / Prop 4.7) — MonadicLens is effectful set on
pure state, not totalization of a partial function. The split is
principled but not an optic construction.
(Source: `agent/optics-theorist/project_connectionsource_review`
§"Round 2", and CBG24 §4 verification.)

**Exact shape across every send site — UNRESOLVED (see O1).**
Lane asked to think through the whole system before committing.

### D2 — Q2 handshake ownership: Option A

ConnectionSource is born **Active**. Handshake runs synchronously
in a short-lived bridge thread. Handoff mechanism:

1. Bridge runs `bridge_client_handshake` on the unsplit transport
   (today's code path).
2. On `Welcome`, bridge sends a new `LooperMessage::NewConnection
   { welcome, transport, ack: oneshot::Sender<Registered> }`
   variant through the existing mpsc channel.
3. Looper pops it in phase 3/4 (Lifecycle), constructs
   `ConnectionSource` on the looper thread, registers via
   `LoopHandle::insert_source`, and sends
   `Registered { service_handle }` back through the oneshot.
4. Bridge awaits the ack and returns
   `Ok(ClientConnection { service_handle, ... })` to the caller.
   `PaneBuilder::connect()` stays synchronous-looking.

**Why this closes session-type's "ready but not registered"
concern:** no `ServiceHandle<P>` has been handed out during the
window between step 2 and step 4, so no handler can attempt to use
a not-yet-wired ServiceHandle. The window is invisible to user
code.

**Why not Option B (pre-handshake ownership):**

- Adds a `Handshaking` mode bit to every dispatch branch; mode
  bits accrete.
- Merges `ConnectError::Rejected` (pre-birth, no watchers)
  and `ProtocolAbort` (post-birth, watchers fire) — every consumer
  would have to disambiguate.
- Hello/Welcome serialization + peer_cred syscall would accrue to
  the 5s watchdog budget; `HANDSHAKE_MAX_MESSAGE_SIZE = 16 MB` is
  not bounded enough to ignore on the looper thread.

**9P precedent (version(5)):** Tversion is a boot sequence, not a
state in the main loop. Every Plan 9 file server treats version
this way — `devmnt.c`'s `mntversion` → `mntattach` has no
partial-mount code path. ConnectionSource inherits: "a
ConnectionSource that exists is a session that speaks the
protocol."

(Source: `agent/plan9-systems-engineer/project_connectionsource_review_r2`
§"Q2".)

**fd transfer mechanism:** in-process move of
`UnixStream`/`Transport` through the existing mpsc — **not**
`sendmsg(SCM_RIGHTS)`. One new `LooperMessage` variant, one
oneshot for registration confirmation.

### D3 — Q3 Inv-RW is the load-bearing progress invariant

**Inv-RW (Request-Wait graph acyclicity):** at any moment, the
directed graph whose nodes are in-flight requests and whose edges
are `A → B` when A's reply cannot be produced until B's reply is
produced, is acyclic.

Guaranteed in pane by:

1. **I2** — handlers do not block, return `Flow::Continue`
   immediately; a handler cannot transitively create a wait edge
   to itself within a single dispatch invocation.
2. **I8** — synchronous waits confined to non-looper threads,
   which hold oneshot channels rather than `Dispatch` entries.
3. **Protocol-scoped `send_request`** — session types bound which
   panes a handler can address.

**Relationship to [JHK24] Theorem 1.2:** different theorem,
different graph. [JHK24] §1 argues *connectivity-graph* acyclicity
is needed because in LinearActris connectivity and wait coincide
— a thread blocked on `recv` literally waits on its peer endpoint.
Pane decouples them via [FH] `E-Suspend` / `E-React`: a connection
being open does **not** establish a wait edge. [JHK24] Theorem 1.2's
hypothesis, mapped onto pane's dispatch model, is about Inv-RW, not
connection topology.

**Citation fix needed (tier-2 audit trigger — see O3):**
`decision/server_actor_model`'s [JHK24] Theorem 1.2 citation
currently reads as covering whole-pane progress. It should be
restricted to "proves ProtocolServer's local star topology is
progress-safe" (still true). For whole-system progress (especially
Phase 2 direct pane-to-pane per `decision/messenger_addressing`),
cite **[FH] EAct progress** — per-actor, no topology requirement.

### D4 — Q3 Inv-CS1 spec now as ordering / determinism convention

**Restated from initial framing.** First pass labeled Inv-CS1 as
a "fairness policy (not safety)." Lane questioned whether this
implied a tradeoff between fairness and safety. It does not, and
the word "fairness" was wrong on examination.

**Actual content.** Inv-CS1 says: within a single batch tick, phase
5 (Requests/Notifications) drains source A's frames to completion
before moving to source B, and so on. Stronger than BLooper's
`Looper.cpp:1273-1276` pattern, which breaks out of the inner drain
on new input (favouring cross-source responsiveness).

**What Inv-CS1 actually provides:** determinism and debuggability.

- Wire FIFO + I6 (sequential dispatch) already carry per-session
  causal ordering, *without* Inv-CS1.
- Session types are per-session, so cross-session interleaving is
  neutral for session-type correctness, *without* Inv-CS1.
- Dispatch is sequential (I6), so interleaving is a sequence not
  a race, neutral for Inv-RW safety, *without* Inv-CS1.
- What Inv-CS1 *does* change: a batch tick with frames from
  multiple sources produces a deterministic sequence across runs,
  and one session's lifetime doesn't interleave with another's in
  trace logs.

**What Inv-CS1 does NOT provide:**

- Not safety — Inv-RW carries safety entirely. Shipping or
  dropping Inv-CS1 does not weaken any safety property.
- Not fairness — drain-to-completion is *anti-fairness* in the
  standard sense; source A can dominate a batch until drained.
  (Bounded by the per-source batch cap — one noisy source can't
  starve others indefinitely — but the default bias is toward
  throughput, not fairness.)
- Not causality beyond what wire FIFO already provides.

**No tradeoff with safety.** Inv-RW and Inv-CS1 live at different
levels and address different properties. Inv-RW is a progress
invariant over the request-wait graph; Inv-CS1 is a determinism
convention over batch-phase execution order.

**Justification for speccing it anyway:** deterministic test
output is worth an explicit invariant slot even when it is not
load-bearing for correctness. Annotating the convention as "we
drain per-source by design" makes test stability a property of the
spec rather than an implementation accident, and auditor traces
are easier to read when one session's frames are contiguous.

**Divergence from BLooper (document at implementation time):**
`src/kits/app/Looper.cpp:1273-1276` breaks the inner drain on new
port input — BLooper favours cross-source responsiveness. Pane
diverges because pane's sources carry session-typed protocol
frames where adjacency in trace output matters more than inner-loop
responsiveness, and because the per-source batch cap bounds the
fairness cost.

(Agent-level sources on what Inv-CS1 provides varied:
session-type-consultant round 2 said "deterministic test output
+ auditor clarity"; plan9 round 2 said "necessary for fairness"
using the word loosely; be round 2 said "per-source causal
ordering"; optics round 2 confirmed it is a runtime scheduling
property, not a standard traversal property. The determinism
framing in this decision is the honest synthesis.)

### D5 — Universal Cancel (Plan 9 Tflush analog)

Commit that `Cancel { token }` is a first-class universal operation
that crosses any ConnectionSource regardless of protocol state.
This is Plan 9's `Tflush(5)` analog — the escape hatch pane
inherits from Plan 9 to cover deadlock classes that I2 and I8 don't
catch.

**Exact scope — UNRESOLVED (see O5).** Plan 9 Tflush was by-tag
(cancel one request). Pane could scope narrower (cancel-by-token)
or wider (cancel-by-service, cancel-by-connection,
cancel-by-selector). The round-2 plan9 recommendation was
"first-class universal," which covers shape but not scope.

**Interaction with D1:** Cancel is itself a send (of a `Tcancel`
frame), so it participates in the two-function discipline.
Backpressure-on-Cancel semantics are part of the whole-system
design pass (O1).

### D6 — Optics framing: AffineTraversal for HashMap access

`HashMap<ConnectionId, ConnectionSource>` single-entry access is an
**AffineTraversal** (genuine composable optic, partial target — key
may be absent). Insert and remove are plain HashMap operations,
not optics. Iteration is a `Traversal` but the phase-5 drain
ordering (D4 Inv-CS1) is a runtime scheduling property, not an
optic property — traversals over `Applicative F` allow any
applicative-legal interleaving, which is exactly what Inv-CS1
forbids.

The two different graphs under discussion (connection graph vs
request-wait graph) are two distinct `Getter` projections onto
`Graph<_>`; acyclicity holds on the request-wait projection (D3
Inv-RW), not on the connection projection. This is the one
optic-theoretic contribution to Q3: it names the pattern
"different invariant on different projection of the same
underlying state."

(Source: `agent/optics-theorist/project_connectionsource_review`
§"Round 2", and pane's cross-cutting `analysis/optics/boundaries`
position that ConnectionSource is on the runtime-I/O side of the
optic scope boundary.)

### D7 — Whole-system two-function tier classification

Three-tier classification of all send sites, decided 2026-04-11
after O1 roundtable (all four design agents) plus targeted
RevokeInterest follow-up.

**Tier A — Handler-context sends (looper thread, inside dispatch,
cannot retry, cannot block):**

| Site | Shape | Rationale |
|---|---|---|
| `send_request` | TWO variants: `send_request` (infallible, cap-and-abort) + `try_send_request` → `Result<CancelHandle, (Msg, Backpressure)>` | Obligation linearity (L2): must return Msg on error. DispatchEntry rollback on try_ error path via `Dispatch::cancel(conn, token)`. |
| `send_notification` | TWO variants: `send_notification` (infallible) + `try_send_notification` → `Result<(), (Msg, Backpressure)>` | No obligation handle, but message recovery useful. Avoids API asymmetry with send_request. |
| `cancel` | INFALLIBLE ONLY. Dedicated ctl slot or separate channel. | Escape hatch (D5 Tflush). If cancel fails on backpressure, recovery is lost. Cancel-if-present semantics: server ignores unknown tokens. |
| `set_content` | INFALLIBLE ONLY. Coalesced (last-writer-wins). | Idempotent overwrite. Intermediate values meaningless. Optics: lawful Setter satisfying PutPut. |
| `watch` / `unwatch` | INFALLIBLE ONLY. | Ctl-plane, rare, small. try_watch returning Address is useless. |

**Tier B — External-thread sends (non-looper, can retry/block):**

| Site | Shape | Rationale |
|---|---|---|
| `send_and_wait` | ALREADY FALLIBLE (`Result<Reply, SendAndWaitError>`). No change. | Caller blocks. Timeout is backpressure signal. |
| `post_app_message` | FALLIBLE ONLY: `try_post_app_message` → `Result<(), (Msg, ChannelFull)>`. | External thread can retry/wait/give up. Infallible would block (violating expectations) or panic (hostile). |

**Tier C — Looper-internal sends (framework, not user-facing):**

All infallible. Reply/Failed wire send, ServiceTeardown,
PaneExited delivery, phase 4 ctl writes. Framework-controlled,
no user API.

**Key structural decisions:**

1. Only 2 of ~10 sites get both variants (send_request,
   send_notification). Two-function split is not universal.
2. Cancel is privileged — separate channel, guaranteed delivery.
   Cancel-if-present protocol invariant: server must treat
   `Cancel { unknown_token }` as no-op (Plan 9 flush(5) contract).
3. `set_content` coalesces — provably correct by PutPut law
   (optics-theorist confirmation).
4. No `Sendable` trait — sites differ on error type, return type,
   obligation structure, cancellation semantics. Irreducible
   heterogeneity confirmed by all four agents.
5. `Queue<T,S>::push` (streaming) is a partial monoid action, NOT
   a Setter — do not conflate with set_content. Classify in
   Phase 3.

**New implementation requirements from roundtable:**

- DispatchEntry rollback: `try_send_request` error path must call
  `Dispatch::cancel(conn, token)` to remove orphaned entry
  (session-type-consultant).
- CancelHandle closure must capture ctl channel sender, not data
  channel (session-type-consultant).
- Cancel channel needs its own small bound; "cancel hit cap =
  connection teardown" (be-systems-engineer).
- Ctl-plane sends assumed cheap by construction — document
  (plan9-systems-engineer).
- Cancel/watch/unwatch/set_content exempt from O2 byte+request
  caps. Only send_request (both caps) and send_notification
  (byte cap only) are counted (plan9-systems-engineer).

**Phase 2 items surfaced:**

- Token tombstones for recently-cancelled tokens (plan9).
- Reply-after-cancel for state-mutating requests: mark entry
  "cancel-requested" instead of removing (plan9).
- set_content generation counter (u64) for network coalescing
  (plan9).

(Sources: `agent/session-type-consultant/revoke_interest_channel_analysis`,
`agent/plan9-systems-engineer/o1_backpressure_review`,
`agent/optics-theorist/o1_whole_system_backpressure_analysis`,
be-systems-engineer O1 inline analysis.)

### D8 — RevokeInterest: hybrid deferred revocation

RevokeInterest (sent on ServiceHandle::Drop) uses a hybrid
pattern: local mark + looper-batched wire send. Decided 2026-04-11
after targeted roundtable (all four agents on RevokeInterest
channel routing).

**The pattern:**

1. `ServiceHandle::Drop` sets `write_tx = None` (already done)
   and posts `LooperMessage::LocalRevoke { session_id }` to the
   looper's input channel (new variant).
2. Looper's `Batch::collect` adds session_id to
   `revoked_sessions: HashSet<u16>` and queues a RevokeInterest
   wire frame for phase 4.
3. Phase 4 (ctl writes) sends all queued RevokeInterest frames
   in-band on the data channel — FIFO-ordered relative to all
   preceding frames.
4. Phase 5 checks `revoked_sessions` before dispatching
   requests/notifications — frames for revoked sessions are
   silently dropped.
5. `process_disconnect` (connection close) is the backstop,
   cleaning up any sessions not yet revoked.

**Why not the status quo (try_send from Drop):**

- try_send from Drop races with looper dispatch — unsequenced
  relative to batch structure.
- Sends from arbitrary threads (ServiceHandle can be dropped
  from non-looper threads).
- Lossy: if channel full, revocation silently lost.

**Why not remove wire send entirely (be's Option 2):**

- Phase 2 long-lived connections: leaked routes persist for
  connection lifetime (potentially hours).
- Eager cleanup matters: routes are heavier than Plan 9 fids
  (routing table entries, dispatch entries, watch subscriptions).

**Why not ctl channel:**

- Separate channel breaks FIFO ordering — RevokeInterest can
  overtake pending replies, orphaning DispatchEntries
  (session-type-consultant).
- Creates ordering anomaly: server removes route before
  processing last Reply queued on data channel.

**Session-type grounding:**

- Maps to [FH] §4 `leave(v)` construct in EAct: actor transitions
  to `idle(v)` (local mark), zapper thread `zap(s.p)` fires
  eventually (looper batch). [FH] Theorems 6 + 8 proved for this
  pattern.
- Consistent with [MostrousV18] affine sessions (drop at any
  state) and [FowlerLMD19] asynchronous exception propagation.
- [JHK24] `exchange_dealloc` (cgraph.v:1192-1225): local graph
  mutation, star topology stays acyclic.

**Three invariants:**

- **H1 (Looper liveness):** Looper eventually runs another batch
  after local mark. Self-sustaining: calloop dispatch guarantees
  this while looper is alive; if looper exits, process_disconnect
  fires.
- **H2 (Idempotent cleanup):** `process_disconnect` must skip
  sessions already removed by RevokeInterest. Currently satisfied
  by routing table walk. Needs explicit documentation (candidate:
  I14 or S7).
- **H3 (Stale dispatch suppression):** After local mark, incoming
  frames for revoked sessions must be dropped, not dispatched.
  Requires `revoked_sessions: HashSet<u16>` checked in phase 5.

**Ordering strength:** Strictly stronger than status quo. Phase 4
placement guarantees RevokeInterest is sent after Reply/Failed
(phase 1), ServiceTeardown (phase 2), PaneExited/Lifecycle
(phase 3) — all obligations resolved before revocation goes out.

(Sources: `agent/session-type-consultant/revoke_interest_channel_analysis`,
`agent/plan9-systems-engineer/o1_revoke_interest_analysis`,
be-systems-engineer RevokeInterest inline analysis,
optics-theorist confirmation that removal is collection mutation
per D6.)

### D9 — Handshake-negotiated backpressure cap

Add `max_outstanding_requests: u16` to Hello and Welcome. Decided
2026-04-11 after O2 roundtable (all four agents).

**Hello (client proposes):**

```rust
pub struct Hello {
    pub version: u32,
    pub max_message_size: u32,
    pub max_outstanding_requests: u16,  // NEW
    pub interests: Vec<ServiceInterest>,
    pub provides: Vec<ServiceProvision>,
}
```

**Welcome (server may reduce, never increase):**

```rust
pub struct Welcome {
    pub version: u32,
    pub instance_id: String,
    pub max_message_size: u32,
    pub max_outstanding_requests: u16,  // NEW — effective cap
    pub bindings: Vec<ServiceBinding>,
}
```

**Semantics:**

- Client proposes a cap (default: 128).
- Server responds with effective cap (≤ client's proposal). 9P
  msize negotiation semantics.
- **Counts `send_request` only.** `send_notification` creates no
  DispatchEntry / obligation and must not pollute the request
  budget (session-type-consultant correction: shared counting
  lets notification-heavy handlers starve their own request
  budget). Field name `max_outstanding_requests` is
  self-documenting.
- Ctl-plane sends exempt per D7: cancel, watch, unwatch,
  set_content, RevokeInterest.
- Exceeding cap → `try_send_request` returns `Backpressure`;
  `send_request` triggers cap-and-abort.
- Default 0 = unlimited (backwards-compatible via serde default).
  Phase 2 servers MUST reject or reduce 0.

**Why request count only, not bytes:**

- `max_message_size` already bounds per-frame bytes.
- `max_outstanding_requests × max_message_size` = worst-case byte
  budget. Derived, not negotiated separately.
- One knob. 9P's msize was one number.
- Socket kernel buffer provides real byte-level backpressure
  (be-systems-engineer observation).

**No race within batch tick** (session-type-consultant
verification): Phase 1 processes replies (counter decreases);
phase 5 sends new requests (counter increases). Reply callback
signature `FnOnce(&mut H, &Messenger, R) -> Flow` lacks
`DispatchCtx`, so on_reply *cannot call send_request*. Cap
counter is monotonically decreasing through phases 1-4 and
monotonically non-decreasing in phase 5.

**Implementation requirement:** `WRITE_CHANNEL_CAPACITY` (currently
hardcoded 128 in `bridge.rs:70`) must derive from the negotiated
`Welcome.max_outstanding_requests`, not be a separate constant.
One knob, not two (plan9-systems-engineer).

**Haiku precedent:** Port capacity (`B_LOOPER_PORT_DEFAULT_CAPACITY
= 200`) was never negotiable — receiver decided unilaterally at
construction time. pane's negotiation is novel, justified by
cross-process IPC where both sides need to agree on flow control.
Haiku learned the hard way that some messages must bypass the cap
(BDirectMessageTarget for same-team messages) — pane's ctl-plane
exemption follows the same principle more explicitly.

(Sources: `agent/session-type-consultant/o2_o5_analysis`,
`agent/plan9-systems-engineer/o2_o5_final_analysis`,
`agent/be-systems-engineer/o2_o5_haiku_precedent`.)

### D10 — Universal Cancel scope: cancel-by-token

`Cancel { token: u64 }` is the sole cancel primitive. 1:1 with
Plan 9 `Tflush(oldtag)`. Decided 2026-04-11 after O5 roundtable
(all four agents, unanimous).

**Wire frame:**

```rust
// ControlMessage variant
Cancel { token: u64 }
```

**Semantics:**

- **Cancel-if-present:** Server treats `Cancel { unknown_token }`
  as no-op. The request may still be in transit on the data
  channel, or may have been completed before Cancel arrived.
- **Advisory:** Server MAY ignore Cancel. Weaker than Plan 9's
  Tflush ("server should abort"). Acceptable because pane
  handlers don't block on replies (I2) — no livelock from
  ignored cancel. Plan 9 needed mandatory flush because
  processes blocked on recv.
- **No ack (no Rflush):** Plan 9's Rflush was needed for 16-bit
  tag reuse safety. pane's u64 tokens never wrap — ack would be
  pure overhead.
- **Fire-and-forget:** Cancel is sent on the ctl channel
  (privileged, D7). Client removes DispatchEntry locally via
  `CancelHandle::cancel()`. Wire Cancel is a hint to the server.

**Wider scopes are compositions, not primitives:**

- cancel-by-service: iterate DispatchEntries for session_id,
  call cancel on each. `Dispatch::fail_session` already exists.
- cancel-by-connection: `fail_connection` already exists.
- cancel-by-selector: skip entirely — implies query language
  over in-flight state for questionable gain.
- Phase 2 can add `CancelSession { session_id }` or `CancelAll`
  as transport optimizations (one frame instead of N) without
  semantic extension.

**Implementation requirements:**

- `SendAndWaitError::Cancelled` variant needed
  (session-type-consultant). Cancel drops DispatchEntry → drops
  oneshot Sender → external thread unblocks with RecvError.
  Currently maps to `LooperExited` — needs distinct variant so
  callers can distinguish "looper died" from "request cancelled."
- CancelHandle closure must capture ctl channel sender (from D7).
- Cancel naturality (optics-theorist): Cancel is protocol-agnostic
  by construction — `Cancel { token }` doesn't mention the
  protocol type. This is a natural transformation (forgets
  request content, keeps token). Falls out automatically from wire
  format. Phase 2 benefit: cancel composes across routing hops
  without additional proof.

**Haiku precedent:** Haiku had no cancel mechanism at all.
`B_CANCEL` in `AppDefs.h` is a UI message code for file panel
dismiss, not a protocol cancel. `RemoveHandler` silently dropped
pending messages without notifying the sender. Cancel-by-token
is pure upside over Be's design.

(Sources: `agent/session-type-consultant/o2_o5_analysis`,
`agent/plan9-systems-engineer/o2_o5_final_analysis`,
`agent/be-systems-engineer/o2_o5_haiku_precedent`,
`agent/optics-theorist/o1_whole_system_backpressure_analysis`.)

### D11 — Handshake format: CBOR for Hello/Welcome, postcard for data plane

Self-describing serialization for handshake messages only. Decided
2026-04-11 after targeted roundtable (all four agents) plus
empirical verification of postcard behavior.

**The problem:** postcard is positional binary. Adding a field to
Hello/Welcome (e.g., `max_outstanding_requests` from D9) is a
breaking wire change. `#[serde(default)]` is dead code with
postcard — empirically verified:

```
V1→V2 (old client, new server): FAIL
  "Hit the end of buffer, expected more data"
V2→V1 (new client, old server): OK
  (trailing bytes silently ignored)
```

The annotation only works in one direction, and it's the wrong
direction (the server being newer than the client is the common
upgrade path).

**The decision:** Use CBOR (RFC 8949) for Hello and Welcome
payloads. Use postcard for all data-plane frames (ServiceFrame,
ControlMessage after handshake). The format boundary is the
handshake completion point — after Welcome is received,
everything switches to postcard.

**Why CBOR, not JSON or msgpack:**

- Binary framing (no need for separate length-prefix layer)
- Deterministic canonical form (RFC 8949 §4.2)
- Still self-describing (tagged field types, map keys)
- Diagnostic notation is human-readable for debugging
- `ciborium` crate: mature, serde-compatible — existing
  `#[derive(Serialize, Deserialize)]` on Hello/Welcome works
  unchanged
- Smaller on the wire than JSON (not that it matters for
  handshake, but free)

**Why not Option A (version-gated postcard):**

- Empirically falsified: `#[serde(default)]` does not work on
  short postcard input. Plan9 agent's claim that trailing
  defaulted fields would deserialize from shorter input was
  tested and failed.
- Version-gated schema requires `HelloV1`/`HelloV2`/... struct
  proliferation or raw-bytes dispatch. Scales poorly.
- 9P's Tversion worked because its version field was a
  human-readable string in a fixed-layout message — not
  positional binary with varint encoding.

**Why not Option C (extension map):**

- `HashMap<String, Vec<u8>>` inside postcard is fighting the
  format. Double encoding (extension values need their own
  format per key). Must-be-last policy not type-enforced.
- Builds a worse BMessage on top of a format optimized for not
  being BMessage (be-systems-engineer).

**Session-type grounding (session-type-consultant):**

- Payload encoding is invisible to session types — [FH] Remark 1
  (Session Fidelity) is agnostic to byte layout.
- The handshake→data-plane format transition is a phase boundary
  between two sequential sessions sharing a transport. [FH]
  Theorem 4 covers sequential composition with different
  value-level encodings.
- Self-describing format restores **session subtyping** ([Gay &
  Hole 2005]) at the handshake: a newer Hello with additional
  `#[serde(default)]` fields is a width subtype of an older
  Hello. Positional encoding destroys this.

**Optics grounding (optics-theorist):**

- Named-field access = row-polymorphic lens (parametric over
  extensions). Positional = rigid product projection. CBOR gives
  handshake fields the row-polymorphic access semantics that
  make evolution compositional.

**Haiku precedent (be-systems-engineer):**

- Haiku already converged on this split: BMessage
  (self-describing) for `AS_GET_DESKTOP` handshake, link protocol
  (positional binary) for data plane. The two-phase format split
  was the empirical answer to the same problem.

**Implementation:**

- Add `ciborium` dependency to pane-session.
- In `bridge.rs`: handshake serialization/deserialization uses
  `ciborium::ser::into_writer` / `ciborium::de::from_reader`
  instead of `postcard::to_allocvec` / `postcard::from_bytes`
  for Hello/Welcome only.
- `FrameCodec` (post-handshake) stays postcard.
- `#[serde(default)]` on `max_outstanding_requests` becomes
  functional.

**Version range (be-systems-engineer suggestion):** Consider
changing `version: u32` to `min_version: u32, max_version: u32`
in Hello. Server picks highest supported version within range.
Enables graceful downgrade when v2 client connects to v1 server.
Not blocking Phase 1 (only one version exists) but worth adding
now while touching the handshake format. Deferred to Lane's call.

(Sources: `agent/session-type-consultant/handshake_wire_extensibility_analysis`,
`agent/be-systems-engineer/handshake_wire_extensibility`,
`agent/plan9-systems-engineer/handshake_wire_extensibility_analysis`,
optics-theorist inline analysis. Empirical postcard test at
`/tmp/pc_test`.)

## Open items (blocking or near-blocking Phase 1)

### O1 — Whole-system two-function design (RESOLVED → D7)

Lane asked to think through every send site and find a general
solution, not just `send_request` / `try_send_request`. Candidate
sites:

- `ServiceHandle::send_request` (request/response)
- `ServiceHandle::send_notification` (fire-and-forget)
- `Messenger::set_content` (ctl, idempotent)
- `Messenger::watch` / `unwatch` (ctl, relationship setup)
- `Messenger::post_app_message` (fire-and-forget app payload)
- `Queue<T,S>::push` (streaming, already fallible)
- Looper-internal: Reply, Failed, Lifecycle, ServiceTeardown,
  PaneExited
- `CancelHandle::cancel` (from D5)
- `send_and_wait` (non-looper threads — already has its own
  blocking timeout semantics)

Driving questions:

1. Is "fallible vs infallible" the only axis, or is there also a
   control-plane / data-plane axis?
2. Does every site get both variants, or do some want only one?
3. Does `try_*` always return `(Req, Backpressure)`, or only where
   obligation linearity bites? (Fire-and-forget has nothing to
   return.)
4. Streaming already has the fallible behavior under the name
   `push` — rename to `try_push` and add `push` as cap-and-abort,
   or declare streaming an exception?
5. Universal Cancel: Cancel is a send. Backpressure-on-Cancel
   semantics — recursive cap-and-abort, or special case?
6. Looper-internal sends: do they even have a caller who'd want a
   `try_` variant, or are they always infallible?
7. Does a `Sendable` trait with default-method `send =
   unwrap_or_abort ∘ try_send` clean up the pattern, or is that
   over-engineering?

Status: RESOLVED. See D7.

### O2 — Handshake-negotiated backpressure cap (RESOLVED → D9)

### O3 — Tier-2 citation audit for `decision/server_actor_model`

Per `policy/agent_workflow` §"Tier-2 audit for theoretical
anchors," the citation fix in D3 (scope [JHK24] Theorem 1.2 to
local star topology, add [FH] EAct progress for whole-pane) is a
formal audit trigger. Dispatch session-type-consultant on the
amended citation.

Status: can run now or fold into Phase 1 implementation commit.

### O4 — S3 refinement: per-source phase 4 ctl drain

be round-2: **defer to Phase 2.** The scenario it addresses (C₁
frame dispatch triggering a ctl write to C₂ in the same iteration)
does not exist in Phase 1 star topology. Spec'ing before the first
callsite means writing a rule that will likely be wrong when Phase
2 is real.

plan9 round-2: ship as fairness doc.

session-type round-2: ergonomics only, [FH] Lemma 1 permits any
cross-actor ordering.

Recommended: defer per be. Add TODO in `architecture/looper`
pointing at this decision log. **Not confirmed by Lane.**

### O5 — Universal Cancel scope (RESOLVED → D10)

### O6 — Where does Inv-RW live?

Options:

- New spoke `analysis/verification/invariants/inv_rw` with
  crosslinks from `architecture/looper` and
  `decision/server_actor_model`
- Appended to `architecture/looper`
- Added to `decision/server_actor_model` as part of the O3
  citation fix

Recommended: new spoke + crosslinks. Inv-RW is load-bearing enough
to get its own identity, and the citation fix can reference the
spoke rather than duplicating the definition.

## Latent items (not blocking)

### L1 — `beapi_divergences` entry for two-function split

be round-2 flagged the two-function split as a deliberate Be
divergence (Be's pattern was one-function-with-timeout: `write_port_etc`, `BMessenger::SendMessage(..., timeout)`,
`LinkSender::Flush(timeout)`). Document at implementation time in
whichever memory holds Be-API divergences.

### L2 — Linearity condition for `try_send_*`

`Result<CompletionToken, (Req, Backpressure)>` — must return req
on error, or I4 (typestate handles) linearity breaks. Document
explicitly in the architecture spec so it can't be lost during
implementation.

## Round-2 agent memories

- `agent/session-type-consultant/project_connectionsource_review_r2`
  — Q1 two-function Kleisli lift, Q3 concedes to Inv-RW,
  demotes Inv-CS1 to implementation detail
- `agent/plan9-systems-engineer/project_connectionsource_review_r2`
  — Q2 full Option A reasoning with `LooperMessage::NewConnection`
  handoff, Q1 minimalism + handshake-negotiated cap, Q3 Inv-RW
  framing and universal Cancel as Tflush analog
- `agent/plan9-systems-engineer/dlfactris_reverification_flag` —
  flag for Phase 2 re-audit of the DLfActRiS citation scope
- be-systems-engineer round-2 analysis returned inline only (no
  memory saved); covers Haiku precedent for send_request both
  variants, S3 refinement defer recommendation, Inv-CS1 divergence
  from BLooper rationale
- optics-theorist round-2 analysis: AffineTraversal for
  HashMap access, projection framing for Inv-RW vs
  connection-graph acyclicity, pointed-monad-algebra analysis of
  two-function split, NOT CBG24 MonadicLens

## Provenance

Lane initiated this design consultation 2026-04-11 after the
ConnectionSource blocker was identified as the Phase 1 Core hot
path. Two roundtables dispatched: round 1 (all four design agents
on Q1/Q2/Q3), round 2 (all four on Q1 reframed + Q3 expanded,
plan9 on Q2 expansion). Lane made D1–D6 decisions in this session,
with D4 restated after questioning the fairness framing.

O1 roundtable (all four agents on three-tier classification) ran
same day; Lane approved → D7. RevokeInterest follow-up (all four
agents on channel routing) surfaced a three-way disagreement
(session-type + plan9: status quo; be: remove wire send; synthesis:
hybrid). Session-type follow-up grounded hybrid in [FH] §4 leave(v),
[MostrousV18], [FowlerLMD19]. Lane approved → D8.

O2 roundtable (all four agents on simplified cap proposal) and O5
roundtable (all four on cancel-by-token) ran same day. Session-type
corrected O2 to count send_request only, not notifications.
Unanimous on both. Lane approved → D9, D10.

D11 (CBOR handshake format) added same day after empirical
verification that postcard `#[serde(default)]` fails on short
input. All four agents consulted; plan9's Option A falsified by
test. Lane approved.

**All blocking opens resolved (D1-D11).** O3/O4/O6 are non-blocking
doc/audit tasks. pane-architect can dispatch ConnectionSource
implementation.
