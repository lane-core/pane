---
type: agent
status: current
sources: []
created: 2026-04-07
last_updated: 2026-04-11
importance: high
keywords: [mailbox_types, session_types, retraction, tag_multiplexing, per_tag_binary, 9P_fid, coprocess, BTreeMap, runtime_state]
related: [analysis/session_types/coprocess_corrected, reference/papers/dependent_session_types, reference/papers/forwarders, reference/papers/dlfactris, agent/session-type-consultant/_hub]
agents: [session-type-consultant]
---

# Mailbox type move was wrong — per-tag binary sessions are correct

**Lane corrected my move from binary session types to mailbox
types for tagged coprocesses.** Tags multiplex independent
sessions, not asynchronous untyped channels.

## The rule

Do not propose mailbox types (de'Liguoro / Padovani, ECOOP 2018)
for tag-multiplexed protocols. Tags index independent binary
sessions; the correct model is per-tag session types with a
runtime multiplexer. The 9P fid model is the precedent.

## Why

The whole value proposition of session types is that you never
ask "what state am I in?" — the type constrains exactly one
legal action. A `BTreeMap`-based runtime state tracker
undermines this. Wire multiplexing is orthogonal to protocol
structure.

## How to apply

When analyzing any tag / fid / correlation-ID multiplexed
protocol, type each independent conversation as its own binary
session. The multiplexer is infrastructure, not protocol.
PendingReply handles (typestate witnesses) track per-tag state,
matching pane's existing obligation handle pattern.

See `analysis/session_types/coprocess_corrected` for the full
formal treatment that came out of this correction.
