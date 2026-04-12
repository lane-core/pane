---
type: project
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [backpressure, two_function_split, tier_classification, O1, linearity, cancel_channel, obligation_handles, try_send_request, dispatch_rollback]
related: [decision/connection_source_design, agent/session-type-consultant/project_connectionsource_review_r2, architecture/app, architecture/proto]
agents: [session-type-consultant]
---

# Backpressure tier classification review (O1)

Session-type analysis of the whole-system two-function backpressure API.

## Verdict: conditionally sound

Three conditions:

1. `try_send_request` must roll back the DispatchEntry on wire-send failure (use existing `Dispatch::cancel` method). The CancelHandle only exists when a request is genuinely in flight.
2. Server must treat `Cancel { unknown_token }` as a no-op (handles cancel-before-request ordering inversion from separate ctl channel).
3. `CancelHandle::cancel()` closure must send on the ctl channel, not the data channel.

## Key findings

**Linearity (Q1):** Tier classification correctly identifies all obligation-carrying sites. Only `send_request` carries obligations (CancelHandle + DispatchEntry). All infallible-only sites have no obligation handles. The install-before-wire pattern means `try_send_request` must do: insert entry → try wire send → on failure, remove entry via `Dispatch::cancel()` → return `Err((msg, Backpressure))`.

**Cancel channel (Q2):** Separation is correct — cancel must be deliverable when data channel is full, per [FH] §4 E-RaiseS/E-CancelH. Ordering inversion (cancel arrives before request) is the Plan 9 Tflush race; solved by treating unknown-token cancel as no-op. Late reply after cancel already handled: `fire_reply` returns `None` for consumed/cancelled tokens.

**send_notification (Q3):** Two-function split not required by session types (no obligation handle, message is Clone). Pragmatically fine — avoids surprising asymmetry with try_send_request. Neutral verdict from session types.

**post_app_message (Q4):** Fallible-only correct. [FH] Lemma 1 (Independence of Thread Reductions) permits retries. Infallible would require blocking (Inv-RW violation risk) or cap-and-abort (disproportionate).

**Additional finding:** ServiceHandle::Drop sends RevokeInterest via try_send on data channel. If ctl channel is separate, consider routing RevokeInterest through it too — prevents silent loss when data channel is full.

## Formal grounding

- [FH] §4 E-RaiseS, E-CancelH — cancel must be deliverable for session failure to resolve
- [FH] §3.3 Lemma 1 — thread reductions independent, external thread retries safe
- [FH] §3.2 E-Send — backpressure failure is non-advancement (session typestate unchanged)
- Plan 9 flush(5) — server responds to Tflush(unknown_tag) without error
- I4 (typestate handles) — preserved iff try_send_request returns message on error AND rolls back DispatchEntry
