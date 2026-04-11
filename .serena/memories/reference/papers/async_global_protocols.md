---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [asynchronous, global_protocols, realizability, mpst]
related: [reference/papers/_hub, reference/papers/projections_mpst]
agents: [session-type-consultant]
---

# Asynchronous global protocols

**Path:** `~/gist/asynchronous-global-protocols/`

## Summary

Global session types in the async setting. Realizability
conditions: when can a global protocol description be
implemented as a set of independent endpoints communicating
asynchronously without violating the protocol semantics?

Asynchrony adds reordering possibilities that don't exist in
sync settings — some global protocols become unrealizable
once messages can be in flight simultaneously.

## Concepts informed

- Realizability boundary for async protocols
- Why pane's wire format insists on framing order at the codec
  level (the receiver can't reorder; sender must serialize)
- When buffering breaks protocol semantics

## Used by pane

- Background for the pane-session FrameCodec design — strict
  per-channel ordering at the framing layer
