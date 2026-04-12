---
type: analysis
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [O2, O5, handshake_cap, cancel_scope, max_outstanding_requests, cancel_by_token, tflush, obligation_counting, send_and_wait, backpressure]
related: [decision/connection_source_design, agent/session-type-consultant/backpressure_tier_review, reference/papers/eact, reference/papers/dlfactris, reference/papers/forwarders]
agents: [session-type-consultant]
---

# O2 + O5 session-type analysis

## O2: Handshake-Negotiated Cap

**Verdict: Conditionally sound.** Three conditions.

### Q1: Separate counters for requests vs notifications

Separate. send_request creates DispatchEntry (E-Suspend continuation, pending
obligation). send_notification does not. Counting them together conflates two
distinct resource classes. D7 already gets this right: "send_notification
(byte cap only)."

### Q2: Reply-frees-slot vs handler-sends-next-request race

No race. Single-threaded dispatch + S3 six-phase batch ordering: Reply
processed in phase 1, new requests sent in phase 5. Handler Reply callback
signature (FnOnce(&amp;mut H, &amp;Messenger, R) -> Flow) lacks DispatchCtx, so
Reply callbacks cannot call send_request. Cap counter is settled between
phases.

### Q3: 0 = unlimited

Acceptable for Phase 1 (trusted local clients). Degrades to channel-capacity
bound (WRITE_CHANNEL_CAPACITY = 128). Phase 2 servers MUST be able to impose
a cap (Welcome may reduce). 9P msize enforcement: bridge stores Welcome value.

### Conditions

C1: Cap reads between phase 1 and phase 5 (S3 — by construction).
C2: Document 0 = unlimited as channel-capacity-limited.
C3: Effective cap = Welcome value, not Hello value. Bridge stores Welcome.

## O5: Universal Cancel Scope

**Verdict: Sound.**

### Q1: Cancel-by-token sufficient as primitive

Yes. [FH] §4 E-RaiseS mechanism is per-continuation. Session-level cancel is
a fold over tokens (Dispatch::fail_session already exists). [CMS] §5.1
forwarder composition preserves soundness of iterated cancels. [FH] Theorem 8
requires "fully cancelled" end state, not single-frame mechanism.

### Q2: Token reuse

Impossible. Dispatch::next_token is monotonic u64. After cancel, entry removed,
token never reassigned. Late Reply returns None from fire_reply (S5). Phase 2
tombstones: audit/recovery concern, not session-type concern.

### Q3: Cancel + send_and_wait

Sound. Cancel drops DispatchEntry → oneshot Sender dropped → Receiver returns
RecvError → external thread unblocks. Ordering: cancel on looper thread, wait
on external thread, oneshot provides synchronization. [FH] Lemma 1 permits
independence. Implementation note: SendAndWaitError mapping needs updating
(RecvError currently maps to LooperExited, should distinguish Cancelled).

### Cancel channel escalation (D7 interaction)

ctl channel has own small bound; hit cap = connection teardown. Teardown
triggers fail_connection on all entries. Strictly stronger: "can't cancel one"
→ "cancel all." Obligation resolution preserved either way.

## Formal citations used

- [FH] §3.2 E-Suspend, E-React, E-Send — dispatch entry lifecycle
- [FH] §3.3 Lemma 1 — independence of thread reductions
- [FH] §4 E-RaiseS, E-CancelH — cancel mechanics
- [FH] Theorem 8 — global progress under failure
- [JHK24] §1 — linearity enables progress; removing cap weakens progress
- [CMS] §5.1 — forwarder composition preserves cut-elimination
- Plan 9 flush(5) — cancel-if-present, unknown token as no-op
