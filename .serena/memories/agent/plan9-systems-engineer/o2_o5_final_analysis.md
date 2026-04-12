---
type: project
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [O2, O5, backpressure, cancel, Tflush, flush5, max_outstanding_requests, fire_and_forget, cancel_if_present, tag_reuse, msize, handshake]
related: [decision/connection_source_design, agent/plan9-systems-engineer/o1_backpressure_review, agent/plan9-systems-engineer/project_connectionsource_review_r2, reference/plan9/man_pages_insights]
agents: [plan9-systems-engineer]
---

# O2 + O5 final design analysis (plan9-systems-engineer)

Analysis of simplified O2 (request count only) and O5 (cancel
scope) proposals against 9P/Plan 9 precedent.

## O2: max_outstanding_requests (u16) only — accepted

### Byte cap dropped: acceptable for Phase 1

Original proposal had max_outstanding_bytes as primary, count as
additive. Simplification drops byte cap entirely.

Key insight: pane already negotiates max_message_size in
Hello/Welcome (the msize analog). The derived worst-case byte
budget is max_outstanding_requests * max_message_size. For Phase 1
(local unix socket, trusted peers), this derived budget being
loose is acceptable — the channel capacity is the real enforcement.

Phase 2 risk: remote adversary sending max-size messages to force
OOM. Restore max_outstanding_bytes for Phase 2.

### Effective cap looseness: acceptable

Purpose of cap in Phase 1 is signal visibility and deadlock
prevention, not resource metering. 9P msize was similarly a
worst-case bound — nobody routinely sent max-msize Treads.

### Default 128: reasonable

Plan 9 NMSG was 32 per mount point. Pane's 128 is per connection
multiplexing ~10 services = ~13 per service. Tighter than Plan 9
per service. Must align with WRITE_CHANNEL_CAPACITY (also 128) to
avoid config-drift bugs.

### Ctl-plane exempt: unchanged

Tflush was never subject to flow control. Cancel as escape hatch
cannot be gated by congestion it's meant to relieve. Ctl ops cheap
by construction (small, fixed-size). Design bug if ctl op is
expensive — catch in review.

## O5: Cancel { token: u64 } — accepted

### Cancel-by-token = faithful Tflush(oldtag) translation

1:1 mapping. Tflush took one oldtag, Cancel takes one token.
Wider scopes (CancelSession, CancelAll) are compositions, not
primitives. Plan 9 never had "flush all" — you clunked the fid
or dropped the connection.

### Advisory cancel: acceptable weakening

flush(5) said "should" not "must." Real Plan 9 servers frequently
completed work before Rflush arrived — client was required to
handle both reply-before-flush and flush-before-reply. Pane's
"advisory" makes this reality explicit.

Pane's additional constraint: user-authored handlers may have
kicked off side effects. Cancel means "I no longer want the reply"
not "undo the request."

Critical preservation: cancel-if-present semantics. Unknown or
already-completed tokens silently succeed. Without this, timing-
dependent errors make the escape hatch unreliable.

### Fire-and-forget: correct for Phase 1

Why Rflush existed: tag reuse. 9P tags were u16, could exhaust.
Client must not reuse oldtag until Rflush. Pane tokens are u64 —
585 years at 1B/sec. No reuse hazard, no synchronization needed.

Late replies handled by token tombstone set (O1 recommendation 5a).
Connection teardown is the backstop.

Phase 2 consideration: optional cancel-ack for resource accounting
over remote connections. Not blocking for Phase 1.

## Verdicts

| Item | Verdict | Confidence |
|---|---|---|
| Drop byte cap (Phase 1) | Accept | High |
| Loose effective cap | Acceptable | High |
| Default 128 | Reasonable | High |
| Ctl-plane exempt | Unchanged | High |
| Cancel-by-token | Faithful | High |
| Advisory cancel | Acceptable | High |
| Fire-and-forget | Correct (Phase 1) | High |
