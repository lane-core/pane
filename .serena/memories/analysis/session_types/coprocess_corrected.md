---
type: analysis
status: current
sources: [.serena/memories/pane/coprocess_session_type_correction]
created: 2026-04-07
last_updated: 2026-04-11
importance: high
keywords: [coprocess, session_types, per_tag, binary_sessions, mailbox_types, retracted, 9P_fid, PendingReply, deadlock_freedom, dlfactris]
related: [agent/session-type-consultant/feedback_mailbox_type_retraction, reference/papers/dlfactris, reference/papers/forwarders, reference/papers/dependent_session_types]
agents: [session-type-consultant, pane-architect]
---

# Coprocess session types: per-tag binary sessions (corrected)

**Date:** 2026-04-07.
**Corrects:** Prior mailbox type proposal (retracted — see
`agent/session-type-consultant/feedback_mailbox_type_retraction`).

## The error

Mailbox types (de'Liguoro / Padovani, ECOOP 2018) were proposed
for tagged coprocess channels. **This was wrong.** Tags don't
change the protocol structure — they multiplex independent
binary sessions over one wire. Each tag has its own session
type; the multiplexer routes messages by tag. The programmer
never needs to ask "what state am I in?" because each per-tag
session has exactly one legal next action.

## Correct formalization

Per-tag session type: `S_t = Send<Req, Recv<Resp, End>>`

The multiplexer is a runtime map `Map<Tag, ExistentialSession>`,
**not** a session type. In Rust, per-tag state is tracked via
typestate handles (`PendingReply { tag: u16 }` —
`#[must_use]`, consumed by `read -p`, Drop sends cancel /
releases tag).

## 9P precedent

Exactly the 9P model: each fid had an independent state machine
(`walk → open → read/write → clunk`). Tags multiplexed fids
on one connection. Nobody argued 9P needed mailbox types.

## Deadlock freedom

Per-tag strict alternation (Send then Recv) + asymmetric
initiator / responder topology → DLfActRiS acyclicity
condition holds (Jacobs / Hinrichsen / Krebbers, POPL 2024,
Theorem 5.4). The shell never waits for the coprocess to
initiate. Buffer fill is backpressure, not deadlock.

## Implementation

- `print -p`: allocates tag, sends `{tag}\t{payload}\n`,
  returns `PendingReply` handle
- `read -p`: FIFO consumption of next `PendingReply`, blocks
  until tagged response arrives
- `read -p -t N`: explicit tag selection
- Single-coprocess degenerates to tag 0 (current prototype's
  `Option<Coproc>`)

## Key insight

**Wire multiplexing is orthogonal to protocol structure.**
Session types describe per-interaction protocols, not wire
scheduling. This is the structural lesson that generalizes
beyond coprocesses to any tag / fid / correlation-ID
multiplexed protocol.
