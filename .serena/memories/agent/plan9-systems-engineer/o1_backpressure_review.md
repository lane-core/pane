---
type: project
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [ConnectionSource, O1, backpressure, three_tier, cancel, Tflush, flush5, ordering, token_tombstone, set_content_generation, post_app_message]
related: [decision/connection_source_design, agent/plan9-systems-engineer/project_connectionsource_review_r2, reference/plan9/man_pages_insights, reference/plan9/decisions]
agents: [plan9-systems-engineer]
---

# O1 three-tier backpressure review (plan9-systems-engineer)

Analysis of the three-tier (A: handler-context, B: external-thread,
C: looper-internal) backpressure API proposal against 9P/Plan 9
semantics.

## 1. 9P mapping verdict: aligns well

- send_request/send_notification → Tread/Twrite (data-plane). Both
  variants (infallible + fallible) correct.
- cancel → Tflush. Correct as privileged.
- set_content → Twstat. 9P had no special treatment for wstat; pane
  adds coalescing, which is reasonable.
- watch/unwatch → No 9P analog (Plan 9 used blocking reads). Ctl-
  plane infallible classification is sound.
- post_app_message → Closest to write(2) to a 9P ctl file from
  external process. Fallible-only correct.

**Key insight:** 9P had no data/ctl distinction at wire level; pane
bakes it into the API. The "ctl exempt from backpressure" rule
depends on the assumption that ctl ops are cheap by construction.
Must be documented.

## 2. Cancel channel: separate is sound with caveats

Plan 9 flush(5) depended on in-order delivery: "The semantics of
flush depends on messages arriving in order." Separate channel
creates ordering hazard: cancel can arrive at server before the
request it targets.

Three approaches analyzed:
- (a) In-band: ordering correct, but cancel blocked by backlog
- (b) Separate + sequence numbers: correct, complex server state
- (c) Separate + cancel-if-present: advisory, simple, best-effort

**Recommendation: (c)**, matching pane's existing "advisory request
cancellation" semantics. Client-local dispatch entry removal is
synchronous (no race). Wire Cancel is optimization hint.

## 3. O2 cap interaction

Exempt from both caps: Cancel, watch/unwatch, set_content (with
coalescing). Only send_request and send_notification count.
Rationale: can't let data-plane congestion prevent cancellation or
ctl-plane setup. set_content coalescing bounds memory to O(1).

## 4. post_app_message: non-blocking fallible preferred

External thread should get Err(msg) immediately on full channel,
not block. Blocking external threads risk priority inversion if they
hold locks.

## 5. Three items the proposal misses

### 5a. Token tombstones after cancel

flush(5): client must not reuse oldtag until Rflush. pane uses u64
tokens (no exhaustion), but late-arriving Reply for cancelled token
could misdispatch. Add small tombstone set; discard replies for
tombstoned tokens; GC on connection close.

### 5b. Reply-before-cancel semantics for state-mutating requests

flush(5): "If a response to the flushed request is received before
the Rflush, the client must honor the response as if it had not
been flushed, since the completed request may signify a state change
in the server."

cancel() should mark-then-discard (fire on_reply if Reply arrives
before confirmation of cancellation), not remove-immediately. For
generic send_request that can carry state-mutating operations,
immediate removal loses state-change confirmations.

Phase 1 mitigation: acceptable to use simpler model if all Phase 1
requests are side-effect-free. Must be revisited for Phase 2.

### 5c. set_content generation counter

Coalescing across network (Phase 2) creates last-writer-wins races.
Generation counter (u64 per message) lets server detect gaps. Not
needed Phase 1 if set_content is full snapshot. Cheap insurance.

## Summary verdicts

| Item | Verdict | Confidence |
|---|---|---|
| Tier classification | Sound | High |
| Cancel channel separation | Sound with advisory semantics | High |
| No Sendable trait | Correct — premature abstraction | High |
| O2 cap exemptions | Cancel + ctl exempt | High |
| post_app_message fallible | Correct, prefer non-blocking | High |
| Token tombstones | Missing, should add | High |
| Reply-before-cancel | Missing for Phase 2 | High (need), Medium (Phase 1 priority) |
| set_content generation | Missing for Phase 2 | Medium |
