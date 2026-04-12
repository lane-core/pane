---
type: architecture
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [pane-app, actor, handler, dispatch, messenger, service_handle, install-before-wire, I8, I9, catch_unwind]
related: [architecture/looper, status, decision/messenger_addressing, policy/beapi_naming_policy]
agents: [pane-architect, session-type-consultant, be-systems-engineer, formal-verifier]
---

# Architecture: pane-app (Actor Framework)

## Summary

pane-app is the runtime layer translating pane-proto's type contracts into a working actor system. It owns the Handler trait lifecycle, DispatchCtx for request-handler protocol fidelity, and the Messenger/ServiceHandle addressing primitives. The crate exports 103 tests (91 unit + 7 stress + 5 integration) because dispatch and handler semantics are the ground truth for invariant enforcement: I8 (send_and_wait panics from looper thread), I9 (catch_unwind on Reply/Failed with destruction sequence ordering), install-before-wire, and protocol-scoped typing at the Handler level. See architecture/looper for six-phase batch ordering (S3), watchdog (I2/I3 detection), and TimerToken — this digest covers the Handler framework, addressing, and dispatch routing that depends on those foundations.

## Components

### Modules (src/ top-level)

**Looper-related (see architecture/looper):**
- **looper.rs** — calloop EventLoop wrapper, Batch collection, six-phase ordering (S3), heartbeat watchdog, send_and_wait I8 ThreadId check
- **looper_core.rs** — dispatch logic, owns Dispatch<H> and ServiceDispatch<H>, catch_unwind boundary (I1), destruction sequence
- **watchdog.rs** — stall detection thread for I2/I3
- **timer.rs** — TimerToken obligation handle, calloop Timer source, set_pulse_rate

**Not looper (core handler framework):**
- **pane.rs** — Pane identity stub (connection + tag), Tag struct
- **builder.rs** — PaneBuilder<H>: typed setup phase, serve<P>() bounds check (H: Handles<P>), connect to ProtocolServer, open_service with interest negotiation, buffered message drain before run_with
- **dispatch_ctx.rs** — DispatchCtx<'a, H>: scoped context lifetime-bound to dispatch, insert() for dispatch entries before wire send, connection() accessor (PeerScope)
- **handles_request.rs** — HandlesRequest<P> trait, scoped to RequestProtocol, H = Self binding enforced at trait signature level (Ferrite protocol fidelity)
- **dispatch.rs** — Dispatch<H>: per-request token→closure map (EAct E-Suspend/E-React), insert() allocates tokens (monotonic per-Dispatch), fire_reply/fire_failed consume entries, fail_connection (S4), fail_session (S1 fix), cancel (S5)
- **service_dispatch.rs** — ServiceDispatch<H>: static table frozen at setup, two tiers (receivers for notifications, request_receivers for requests), type-erased closures capture concrete P::Message + protocol tag, dispatch_request returns Flow (totality — sends Failed for unregistered sessions)
- **messenger.rs** — Messenger: self-reference capability handle, cloneable, carries self_address (Address), timer_tx, sync_tx (for send_and_wait), looper_thread (ThreadId Arc<OnceLock>)
- **service_handle.rs** — ServiceHandle<P>: live service binding, protocol-scoped, send_request with DispatchCtx (install-before-wire), send_notification, send_and_wait (non-looper sync blocking), wire_reply_port constructor, Drop sends RevokeInterest
- **send_and_wait.rs** — SyncRequest struct, SendAndWaitError, SyncReplyResult, oneshot reply channel, error types (Timeout, Disconnected, Failed, SerializationError)
- **exit_reason.rs** — ExitReason re-export from pane-proto for wire transmission (Graceful, Disconnected, Failed, InfraError)

### Handler Framework

**Pane app definition:** A user writes a struct implementing Handler (lifecycle methods: ready, disconnected, pulse) and optionally Handles<P> (for service notifications) and/or HandlesRequest<P> (for service requests). Call Pane::setup::<MyHandler>() to enter PaneBuilder<MyHandler>, then serve<P>() to advertise services, connect() for the transport, open_service::<P>() to subscribe to remote services, finally run_with(handler_instance) to enter the looper. See builder.rs and example in tests/two_pane_echo.rs.

**BeAPI lineage:** Pane mirrors BApplication + BWindow setup. PaneBuilder follows Be's pre-Run() registration pattern (serve = Handler registration, open_service = Subscribe). Messenger ≈ BApplication::PostMessage thread-safe handle, scoped variant for dispatch context. ServiceHandle<P> ≈ BMessenger targeting a BHandler, but protocol-scoped (Handles<P> at compile time, no untyped messaging). DispatchCtx ≈ BHandler's access to BLooper via `Looper()` accessor — lifetime-scoped, cannot be stashed. HandlesRequest<P> ≈ separate trait (Be conflated Notification + Request in MessageReceived; pane splits them for ReplyPort obligation).

### Actor Dispatch

**Message routing:** Transport frames arrive as LooperMessage (from pane-session bridge), decoded to service_id + payload + token (if request). ServiceDispatch routes by session_id to the pre-registered receiver closure (registered during PaneBuilder setup). The closure deserializes P::Message (or P::Message + ReplyPort<P::Reply> for requests) and calls Handles<P>::receive or HandlesRequest<P>::receive_request.

**Request/reply protocol:** send_request on ServiceHandle<P> does three things atomically (install-before-wire — Plan 9 devmnt.c:786-790): (1) construct DispatchEntry with on_reply + on_failed closures, (2) insert into Dispatch<H> via DispatchCtx.insert() to get Token, (3) serialize with the token and send to wire.

**Linearity condition (D1, `decision/connection_source_design`,
non-negotiable):** `try_send_request` MUST return
`Result<CancelHandle, (Req, Backpressure)>` — not just
`Result<CancelHandle, Backpressure>`. The request message must be
returned inside the error variant because the obligation handle
(I4 typestate) is consumed on the error path otherwise. If the
caller's `Req` is moved into `try_send_request` and the send
fails, the caller has lost ownership of the request with no way
to retry or drop it cleanly. Same semantics as
`std::sync::mpsc::SyncSender::try_send`. On the error path,
`try_send_request` must also call `Dispatch::cancel(conn, token)`
to remove the orphaned DispatchEntry that was installed in step
(2) above. Source: session-type-consultant analysis, D1/D7/L2. Reply/Failed frames arrive later, routed by (PeerScope, Token) to the installed entry. fire_reply/fire_failed consume the entry and invoke the callback. Sessions send Failed automatically for unregistered session_ids (totality: every Request gets a reply or failure on the wire, preventing orphaned dispatch entries).

**Destruction sequence:** When the looper exits (panic in dispatch or Flow::Stop), LooperCore::shutdown() (1) calls dispatch.fail_connection(primary) to fire all on_failed callbacks for pending requests, (2) dispatch.clear() drops remaining entries, (3) handler dropped (obligation handles' Drop compensation fires), (4) exit_tx notified. This sequence is load-bearing: fail_connection before clear ensures callbacks have a chance to clean up (S4, I9). Catch_unwind wraps each phase (I1, I9 fix commit 6e0130b).

**Dispatch phases:** See architecture/looper. S3 six-phase batch ordering is implemented in Looper (phases 1–2 are Reply/Failed → ServiceTeardown; phase 3 is lifecycle; phase 5 is Requests/Notifications). pane-app's role is the targeted dispatch methods in LooperCore (dispatch_reply, dispatch_failed, dispatch_request, dispatch_notification, dispatch_teardown, dispatch_pane_exited), each with its own catch_unwind.

### ProtocolHandler Derive

**Status: Not implemented.** Listed on Phase 1 (PLAN.md line 37). The macro would generate Handles<P>::receive match-statement from named handler methods. Deferred pending architecture stabilization and macro framework decisions.

### Address / Messenger / ServiceHandle (Distribution-Ready Primitives)

Per decision/messenger_addressing:

- **Address** — lightweight pane address (pane_id, server_id), copyable, serializable. Extracted from Messenger via .address(). Not tied to any particular connection — designed for direct pane-to-pane (diverges from all four agents' server-mediated assumption).
- **Messenger** — inbound self-reference, cloneable, carries self_address and framework APIs (set_pulse_rate, address(), set_content stub, watch/unwatch stub). Passed to dispatch callbacks for self-targeting.
- **ServiceHandle<P>** — outbound service binding, protocol-scoped, owns send_request. Not cloneable (once-bound handle semantics).

**Stub vs real:** Messenger.set_content, watch/unwatch are stubbed (TODO comments, crates/pane-app/src/messenger.rs:81–100). Real implementation requires write_tx on Messenger and ControlMessage wire send, deferred to Phase 1 (PLAN.md line 61). ServiceHandle.send_request and send_and_wait are real. ServiceHandle.Drop sends RevokeInterest via hybrid deferred
revocation (D8): local mark + looper-batched wire send. H1/H2/H3
invariants tested.

Address resolution (routing frames to the target handler) is stubbed at the looper level — all dispatch currently assumes primary_connection (the single pane's own connection). Phase 2 adds multi-connection routing.

### Integration Tests (5)

Each exercises end-to-end flows with real ProtocolServer:

1. **builder_integration.rs** — PaneBuilder connects to ProtocolServer, advertises services in Hello, opens a service via DeclareInterest, receives InterestAccepted + assigned session_id, gets a real ServiceHandle, reads Ready lifecycle message. Validates setup phase and service registration wiring.
2. **two_pane_echo.rs** — Two panes (provider + consumer) via ProtocolServer. Provider serves an EchoService (Ping → Pong). Consumer opens service, sends Ping, receives Pong. Tests inter-pane messaging, frame routing, session_id assignment, protocol tag byte, full happy path.
3. (5 total mentioned in status.md; only 2 visible in repo at read time; 3 others may be in progress or listed under different names)

### Stress Tests (7)

Test invariants under adversarial conditions:

- **destruction_sequence_survives_handler_panic** — panic in dispatch() is caught, Exit(Failed) returned, handler dropped during shutdown (I9 destruction ordering tested via LooperCore public API).
- **cross_protocol_gibberish_payload_dropped_by_tag** — wrong protocol tag byte causes frame drop with no panic (S8 tag checking).
- **reply_before_teardown, lifecycle_after_teardown, notifications_last** — batch ordering tests (S3), verify phases run in order.
- **send_and_wait_panic_from_looper_thread** — I8 enforcement, panics when called from looper thread.
- **dispatch_entry_token_uniqueness** — monotonic token allocation, no collisions (S1).

(Exact count = 7; all #[ignore], run with `cargo test -- --ignored`.)

### Known Gaps

**ConnectionSource (C1-C6 landed):** calloop EventSource for
post-handshake connections, two-function send API, deferred
revocation, CancelHandle wiring, Looper-side registration. New
modules: `connection_source.rs`, `subscriber_sender.rs`,
`backpressure.rs`. Remaining: bridge-side integration (replacing
bridge threads with ConnectionSource for real connections). See
`decision/connection_source_design`.

**Messenger:** set_content, watch/unwatch, post_app_message are stubs awaiting real ctl send wiring.

**ServiceHandle:** wire_reply_port constructor is stubbed (no real ReplyPort serialization on responses).

**ProtocolHandler derive:** macro not started.

**Address routing:** all dispatch assumes primary_connection; Phase 2 needed for multiple connections.

**ActivePhase<T>:** explicit ω_X operator carrying negotiated state (max_message_size, PeerAuth, known_services) not yet threaded through dispatch context.

**AppPayload marker trait:** Clone + Send + 'static marker, not yet used.

**Eager Hello.interests:** service interests should be included in initial Hello, not DeclareInterest late-binding. Deferred pending four-agent recommendation (PLAN.md line 64).

**DeclareInterest late-binding:** waiting for Eager Hello.interests design (PLAN.md line 35).

## Invariants

| Invariant | Status | Mechanism |
|-----------|--------|-----------|
| I1 (panic=unwind, Drop fires) | **Enforced** | catch_unwind on every dispatch_* call in LooperCore; obligation handles' Drop compensation |
| I8 (send_and_wait panics from looper thread) | **Enforced** | Runtime ThreadId check in ServiceHandle::send_and_wait; panics if caller ThreadId matches looper_thread (crates/pane-app/src/send_and_wait.rs, comment line 143–146) |
| I9 (dispatch cleared before handler drop) | **Enforced** | destruction_sequence (LooperCore::shutdown) — fail_connection (phase 1) before clear (phase 2) before handler drop (phase 3); catch_unwind on Reply/Failed branches (commit 6e0130b); tested by destruction_sequence_survives_handler_panic |
| Install-before-wire | **Enforced** | DispatchCtx.insert() called before serialization in ServiceHandle::send_request (crates/pane-app/src/service_handle.rs:81–116); comment cites Plan 9 devmnt.c:786-790 |
| S3 (six-phase batch ordering) | **Enforced** | Implemented in Looper, not pane-app (see architecture/looper); pane-app provides dispatch_* methods called in order |
| S4 (fail_connection scoped) | **Enforced** | Dispatch::fail_connection filters entries by connection before firing callbacks (crates/pane-app/src/dispatch.rs:118–135) |

Structurally enforced (types):
- Protocol-scoped typing: ServiceHandle<P>, send_request<H> with Handles<P> bound, HandlesRequest<P> H = Self binding (Ferrite protocol fidelity)
- DispatchCtx lifetime binding prevents stashing or cloning
- ReplyPort and CancelHandle #[must_use] prevents forgetting obligations

Runtime checked:
- I8 ThreadId check (panics)
- Watchdog detection of I2/I3 (see architecture/looper)

## See also

- `architecture/looper` — six-phase batch, watchdog, send_and_wait, I8, S3
- `decision/messenger_addressing` — Address/Messenger/ServiceHandle design rationale
- `policy/beapi_naming_policy` — faithful Be adaptation tier 1 (Handler, Messenger, etc. names)
- `status` — current test counts, Phase 1 roadmap, known gaps
- pane-app source: `crates/pane-app/src/`
- Recent commits: session 4 Looper rewrite (`bbc7026`), I9 fix (`6e0130b`), service registration wiring (`a3aedff`)
