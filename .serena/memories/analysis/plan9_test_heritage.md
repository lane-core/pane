---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [plan9, test_suite, flush, version, fid, freefidpool, walk, tflush, tversion, cancel, handshake, service_handle, disconnect, declare_interest]
related: [agent/be-systems-engineer/haiku_test_audit, reference/plan9/divergences, reference/plan9/man_pages_insights, analysis/verification/_hub]
sources: [flush(5), version(5), walk(5), open(5), clunk(5), attach(5), intro(5), lib9p/srv.c, devmnt.c, server.rs, service_handle.rs, obligation.rs, control.rs, handshake.rs]
verified_against: [pane source 2026-04-11 (349 regular + 36 stress tests)]
agents: [plan9-systems-engineer]
---

# Plan 9 heritage test proposals for pane

Companion to `agent/be-systems-engineer/haiku_test_audit`.
Where that audit covers Be heritage (lifecycle, messaging,
watch/unwatch, timers, concurrency), this analysis covers
Plan 9 heritage: Tflush/Cancel, Tversion/handshake, fid/
ServiceHandle binding, freefidpool/disconnect cleanup, and
walk+open/DeclareInterest.

24 tests proposed (T1–T24), organized by Plan 9 concept.

## 1. Cancel as Tflush (D10) — T1–T5

Plan 9 flush(5) specifies three race conditions:
(a) Tflush before Trequest arrives, (b) Tflush after Rreply
sent, (c) Tflush of Tflush. Plus: unknown oldtag = silent Rflush.

Existing coverage: cancel_storm (adversarial.rs:695) — 500 requests,
immediate cancel-all, statistical race test. Phase 1 server
treats Cancel as no-op (server.rs:426-438).

- **T1** Cancel before request arrives (Tflush races Trequest).
  Two threads: Cancel sent before Request. Server must not panic
  on orphaned Cancel; Request still processes normally.
- **T2** Cancel after reply sent (Tflush races Rreply).
  Single request, deterministic ordering. Publisher sends Request,
  subscriber replies, publisher cancels. Exactly-once dispatch.
- **T3** Cancel for unknown token. Send Cancel{token: 0xDEAD}
  on a connection that never used that token. No error, no disconnect.
- **T4** Cancel after service teardown. Open service, send request,
  RevokeInterest, then Cancel for pending token. No leaked entries.
- **T5** Double cancel. Two Cancel{token: 42} for same token.
  No panic, no double on_failed. Type-level prevention (CancelHandle
  consumes self) doesn't protect against malicious wire messages.

Gap severity: Medium. cancel_storm provides statistical coverage
but misses deterministic race conditions and edge cases.

## 2. Handshake as Tversion (D2) — T6–T10

Plan 9 version(5): client proposes msize + version; server may
reduce msize, never increase; unknown version → "unknown" response;
Tversion mid-session aborts all state.

Existing coverage: CBOR roundtrip, forward-compat (missing fields
default to 0), basic accept. No msize reduction, no version
validation, no rejection tests.

- **T6** msize reduction. Client proposes max_message_size: 64MB,
  server enforces its own limit. Subsequent oversized frames rejected.
  **Exposes gap:** server currently echoes client's max_message_size.
- **T7** max_outstanding_requests reduction. Client proposes 1000,
  server caps to 64. Request #65 hits cap.
- **T8** Version mismatch → Rejection. Client sends version: 999.
  Server responds Err(Rejection{reason: VersionMismatch}).
  **Exposes gap:** server doesn't validate version field.
- **T9** Second handshake mid-session. After active-phase exchange,
  send another Hello on service 0. Server should reject (protocol
  violation, not reset).
- **T10** Empty interests + empty provides. Observer-only connection.
  Usable for Watch/Unwatch but no service frames.

Gap severity: High. No version validation or msize negotiation
exists in the server.

## 3. ServiceHandle as fid (binding stability) — T11–T14

Plan 9: after walk+open, fid is bound. Mount table changes don't
affect existing fids (namec consulted during walk only).
ServiceHandle doc (service_handle.rs:4) claims this semantic.

Existing coverage: DeclareInterest works, Drop sends RevokeInterest.
No test mutates provider index after handle obtained.

- **T11** Provider dies, new provider appears. Existing handle gets
  ServiceTeardown, NOT transparent rebind. New DeclareInterest
  routes to new provider.
- **T12** New provider for same service doesn't affect existing handle.
  A's route to B survives C's arrival.
- **T13** Drop sends RevokeInterest (clunk). Provider receives
  ServiceTeardown. Session_id becomes reclaimable.
- **T14** Handle isolation between connections. Two consumers,
  same service. Independent session_ids. Dropping one doesn't
  affect the other.

Gap severity: Medium. The core fid semantic is claimed but untested.

## 4. Connection close as freefidpool — T15–T19

Plan 9: lib9p srv_close walks entire fid pool. devmnt.c mntclose
answers all pending Mntrpc with error. closepgrp walks mount table.
Property: zero residual references after close.

Existing coverage: rapid-connect-disconnect stress, teardown
cascade (8-connection barrier). Good stress coverage but no
post-disconnect state verification.

- **T15** Disconnect cleans up all routing state. Connect, provide,
  declare interest, watch, then disconnect. New DeclareInterest
  for dead provider's service → ServiceUnknown.
- **T16** Disconnect with multiple pending requests. All dispatch
  entries fire on_failed via ServiceTeardown.
- **T17** Watcher disconnect cleans up reverse index. A watches B,
  A disconnects, B disconnects later — no panic, no send to dead A.
- **T18** Post-teardown verification. 8-connection full mesh
  disconnect, then verify: new connections work, new DeclareInterest
  works, no ghost routes.
- **T19** Disconnect during DeclareInterest processing. Consumer
  sends DeclareInterest then immediately disconnects. No crash
  sending InterestAccepted to dead writer.

Gap severity: Low-medium. Stress coverage is good. Main gap is
deterministic post-disconnect state verification.

## 5. DeclareInterest as open/walk — T20–T24

Plan 9: walk(5) resolves name to fid. Errors: ENOENT, permission
denied. attach(5) establishes root fid. pane's Hello.provides
is the attach analog.

Existing coverage: DeclareInterest happy path is heavily tested.
Rejection paths have zero coverage (SelfProvide, ServiceUnknown,
SessionExhausted all implemented but untested).

- **T20** Self-provide rejection. Pane provides S, then DeclareInterest
  for S → InterestDeclined{reason: SelfProvide}. **Zero test coverage
  of implemented code.**
- **T21** ServiceUnknown rejection. DeclareInterest for service
  nobody provides → InterestDeclined{reason: ServiceUnknown}.
- **T22** SessionExhausted rejection. Set next_session near 0xFFFF
  boundary. Verify 0xFFFF (ProtocolAbort reserved) triggers
  SessionExhausted.
- **T23** DeclareInterest after provider disconnect. Provider dies,
  consumer declares interest → ServiceUnknown (provider_index cleaned).
- **T24** Multiple providers, first-match routing. Two providers for
  same service. Verify which wins. After winner disconnects, new
  DeclareInterest routes to survivor.

Gap severity: High. SelfProvide at zero coverage is the single
most important test to add.

## Priority ordering

1. **T20** (SelfProvide) — implemented code, zero tests
2. **T8** (VersionMismatch) — exposes missing server validation
3. **T21** (ServiceUnknown) — basic rejection path
4. **T11** (binding stability) — core fid semantic
5. **T3** (unknown token cancel) — protocol robustness
6. **T13** (Drop/clunk) — obligation handle wire behavior
7. **T6** (msize reduction) — exposes missing negotiation
8. **T15** (disconnect cleanup verification) — freefidpool completeness
9. Remaining tests in numerical order
