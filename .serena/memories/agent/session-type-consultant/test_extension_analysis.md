---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [test_suite, haiku_port, session_types, obligation_handles, affine_gap, cancel, deferred_revocation, cbor_handshake, provider_api, two_function_split, batch_ordering]
related: [agent/be-systems-engineer/haiku_test_audit, architecture/app, architecture/looper, architecture/proto, decision/connection_source_design, analysis/session_types/_hub, reference/papers/eact, reference/papers/dlfactris]
agents: [session-type-consultant]
sources: [service_handle.rs, obligation.rs, dispatch.rs, subscriber_sender.rs, handshake.rs, looper.rs, connection_source.rs]
verified_against: [pane crates 2026-04-11]
---

# Test Extension Analysis: Beyond Haiku

Session-type-consultant review of the be-systems-engineer's Haiku
test audit. Identifies 32 tests across 7 capability areas that
Haiku could not have written because the underlying mechanisms
did not exist. Each test traces to a specific decision, invariant,
or theorem.

## Methodology

For each area: (1) identify the session-type or protocol property
being tested, (2) cite the formal grounding (theorem, invariant,
decision), (3) name a concrete test with what it verifies, (4)
classify the test as static-guarantee-verification (confirming
the type system does its job), runtime-invariant-verification
(confirming runtime compensation is sufficient), or
adversarial-stress (confirming the system handles worst-case
interleaving).

## Area 1: Obligation Handles (ReplyPort, CancelHandle, TimerToken)

5 tests. These verify the affine-gap compensation strategy:
Rust's affine types + #[must_use] + Drop-sends-failure as a
stand-in for linear types. Formal grounding: [JHK24] §1
"The need for linearity" — affine drops create the deadlock
shape LinearActris rules out; pane compensates via Drop.

1. test_reply_port_drop_fires_on_failed — Drop a ReplyPort
   without calling .reply(). Verify the requester's on_failed
   callback fires. Invariant: I4 typestate + obligation.rs
   Drop impl. This is the core affine-gap compensation test.

2. test_reply_port_double_reply_compile_error — (compile-fail
   test) Attempt to call .reply() twice on the same ReplyPort.
   Verify: compilation fails because .reply() consumes self.
   Static guarantee: move semantics enforce exactly-once.

3. test_reply_port_drop_during_unwind — Panic inside a handler
   that holds a ReplyPort. Verify: Drop fires during stack
   unwinding, requester gets ReplyFailed. Invariant: I1
   (panic=unwind + catch_unwind) + obligation Drop.

4. test_cancel_handle_drop_is_noop — Drop CancelHandle without
   calling .cancel(). Verify: no Cancel frame sent, request
   proceeds normally. Tests inverted-polarity obligation
   (CancelHandle Drop is no-op, opposite of ReplyPort).

5. test_timer_token_drop_cancels_timer — Drop TimerToken.
   Verify: no further pulse() callbacks fire. Obligation
   handle linearity: Drop is the cancellation path. Tests
   that the calloop Timer source is actually removed.

## Area 2: Two-Function Send Split (D1/D7)

5 tests. These verify the infallible/fallible boundary, the
linearity condition (L2: message returned on error), and the
DispatchEntry rollback on try_send failure.

1. test_try_send_request_returns_message_on_cap_exceeded —
   Call try_send_request when outstanding >= cap. Verify:
   Err((original_msg, CapExceeded)) returned, original
   message is the SAME value (not a clone). Invariant: L2
   linearity condition from D1. No DispatchEntry installed.

2. test_try_send_request_rollback_on_channel_full — Fill the
   write channel, then call try_send_request. Verify:
   Err((msg, ChannelFull)) returned AND the DispatchEntry
   that was installed (step 1 of install-before-wire) is
   rolled back via Dispatch::cancel. Outstanding request
   counter must NOT be incremented. Tests the session-type
   requirement: orphaned entries must not persist.

3. test_send_request_panics_on_cap_exceeded — Call send_request
   (infallible variant) when outstanding >= cap. Verify:
   panic caught by LooperCore's catch_unwind, destruction
   sequence fires. This IS the cap-and-abort from D1.

4. test_try_send_notification_returns_message_on_full — Same
   L2 test for notifications. Fill channel, call
   try_send_notification, verify original message returned.
   No obligation handle involved (notifications are
   fire-and-forget), but message recovery still matters.

5. test_infallible_fallible_equivalence — Send N requests
   via send_request and N via try_send_request (both under
   cap). Verify: identical wire frames produced, same
   CancelHandles returned. Tests that send_request =
   panic_on_err . try_send_request (D1 pointed monad
   algebra).

## Area 3: Six-Phase Batch Ordering (S3)

4 tests. These go beyond the existing 3 ordering tests by
creating adversarial multi-type batches. Formal grounding:
[FH] §4 E-Send / E-Receive interleaving discipline — the
phase ordering preserves the EAct safety theorems.

1. test_all_phases_single_batch — Inject Reply + Failed +
   ServiceTeardown + PaneExited + LifecycleMessage +
   Request + Notification all into one batch tick. Verify:
   callbacks fire in strict phase order (1→2→3→5). No
   phase may execute out of order. This is the adversarial
   superset of the existing three tests.

2. test_reply_before_teardown_prevents_orphan — Send a Reply
   and a ServiceTeardown for the SAME session in the same
   batch. Verify: Reply fires first (phase 1), then
   ServiceTeardown fires (phase 2). If order reversed,
   the reply callback would target a torn-down session.
   Invariant: S3 + the ordering rationale from
   architecture/looper.

3. test_revoke_interest_phase4_after_reply — Inject
   LocalRevoke + Reply for the same session. Verify:
   Reply fires (phase 1), then RevokeInterest wire frame
   sent (phase 4). Tests D8 ordering strength: all
   obligations resolved before revocation goes out.

4. test_stale_request_after_revoke_dropped — Inject
   LocalRevoke in the same batch as an incoming Request
   for the revoked session. Verify: Request is silently
   dropped in phase 5 (H3 stale dispatch suppression).
   The handler's receive() must NOT be called for the
   revoked session.

## Area 4: Deferred Revocation (D8)

5 tests. H1/H2/H3 invariants. Formal grounding: [FH] §4
leave(v) → idle(v) → zap(s.p) pattern, [FH] Theorems 6+8.

1. test_service_handle_drop_sends_local_revoke — Drop a
   ServiceHandle. Verify: LooperMessage::LocalRevoke
   posted to the looper's input channel. write_tx set to
   None (local mark). No wire frame sent yet — the looper
   hasn't run. Tests the first half of the hybrid pattern.

2. test_looper_batches_revoke_to_wire — Drop a
   ServiceHandle, then run one looper batch tick. Verify:
   RevokeInterest frame appears on the write channel in
   phase 4. Tests H1 (looper liveness after local mark).

3. test_process_disconnect_skips_already_revoked — Revoke
   session S via local mark + looper batch, then trigger
   process_disconnect for the connection. Verify:
   process_disconnect does NOT attempt to clean up session
   S again. Tests H2 (idempotent cleanup).

4. test_concurrent_revoke_and_incoming_request — On the
   looper thread: post LocalRevoke for session S, then
   inject a Request for session S in the same batch.
   Verify: Request dropped, handler not called. Tests H3
   (stale dispatch suppression via revoked_sessions set).

5. test_revoke_from_non_looper_thread — Drop
   ServiceHandle from a non-looper thread (e.g., the
   thread that called send_and_wait). Verify: LocalRevoke
   still posted correctly via the mpsc channel, looper
   processes it on next tick. Tests that the hybrid
   pattern works across thread boundaries.

## Area 5: CBOR Handshake Extensibility (D11)

4 tests. Formal grounding: session subtyping [Gay & Hole 2005]
— a newer Hello with additional #[serde(default)] fields is a
width subtype of an older Hello. CBOR restores this property
that postcard destroys.

1. test_hello_extra_field_ignored_by_old_server — Serialize
   a HelloV2 with a field the V1 deserializer doesn't know.
   Deserialize as HelloV1. Verify: succeeds, extra field
   silently ignored. Tests backward compatibility (new
   client → old server).

2. test_welcome_extra_field_ignored_by_old_client — Same
   test in the reverse direction for Welcome. Tests the
   upgrade path where the server is newer.

3. test_hello_field_reordering_stable — Serialize Hello
   with fields in a non-canonical order (by constructing
   CBOR manually). Deserialize. Verify: all fields
   populated correctly. Tests that CBOR map access is
   key-based, not positional.

4. test_unknown_reject_reason_deserializes — Serialize a
   Rejection with a RejectReason variant the current
   deserializer doesn't recognize (simulating a future
   reason). Verify: deserialization either succeeds with
   a default/unknown variant or fails gracefully with a
   typed error (not a panic). Tests #[non_exhaustive] +
   CBOR interaction.

## Area 6: Cancel (D10)

4 tests. Formal grounding: Plan 9 Tflush(5) analog. Advisory,
fire-and-forget, cancel-if-present. [FH] E-CancelMsg: session
safety preserved when outstanding messages are cancelled.

1. test_cancel_if_present_unknown_token_noop — Send
   Cancel { token: 99999 } to the server. Verify: server
   treats it as no-op, no error, no panic, connection
   stays alive. Tests the D10 cancel-if-present contract.

2. test_cancel_before_request_arrives — Send a Request,
   immediately cancel via CancelHandle. The cancel wire
   frame (on ctl channel) may arrive before the request
   frame (on data channel). Verify: server handles
   gracefully regardless of arrival order. If cancel
   arrives first: server stores tombstone or ignores
   subsequent request. If request arrives first: server
   processes normally, cancel is belated no-op.

3. test_cancel_after_reply_sent — Send a Request, let the
   server reply, then cancel. Verify: cancel is a no-op
   (request already resolved). The DispatchEntry was
   already consumed by fire_reply, so the cancel closure
   sends a Cancel frame that the server ignores. No
   double-fire, no crash.

4. test_cancel_fires_send_and_wait_cancelled — From a
   non-looper thread, call send_and_wait. From another
   thread, cancel the same request via CancelHandle.
   Verify: send_and_wait returns
   SendAndWaitError::Cancelled (not LooperExited). Tests
   the D10 requirement for distinct error variants.

## Area 7: Provider-Side API (SubscriberSender)

5 tests. These exercise the provider-side pub/sub pattern
that Haiku's WatchingService was server-internal (never
exposed to app developers).

1. test_subscriber_connected_fires_on_interest_accepted —
   Provider pane serves a protocol. Consumer pane opens
   the service. Verify: provider's
   subscriber_connected(session_id, subscriber_sender)
   callback fires with a valid SubscriberSender. Tests
   the InterestAccepted → subscriber_connected routing
   (phase 3 batch).

2. test_subscriber_disconnected_fires_on_revoke — Consumer
   drops ServiceHandle (RevokeInterest). Verify: provider's
   subscriber_disconnected(session_id) callback fires.
   Tests the ServiceTeardown → subscriber_disconnected
   routing (phase 2 batch).

3. test_subscriber_sender_no_revoke_on_drop — Drop a
   SubscriberSender without sending anything. Verify:
   no RevokeInterest or ServiceTeardown generated. The
   CONSUMER owns the lifecycle — provider dropping its
   sender is a local decision, not a protocol event.
   Already tested in unit tests; integration version
   verifies through real ProtocolServer.

4. test_fan_out_notification_to_all_subscribers — Provider
   has 3 subscribers. Provider iterates its
   Vec<SubscriberSender> and calls send_notification on
   each. Verify: all 3 consumers receive the notification.
   Tests the pattern that replaced Haiku's
   NotifyWatchers iteration.

5. test_subscriber_churn_during_fan_out — Provider has 3
   subscribers. During a fan-out loop, subscriber 2
   disconnects (RevokeInterest arrives). Verify: provider
   can still send to subscribers 1 and 3, the send to
   subscriber 2's stale SubscriberSender is silently
   dropped (write_tx closed), and the
   subscriber_disconnected callback fires on the next
   batch tick. Tests that the provider's fan-out is
   resilient to concurrent churn.
