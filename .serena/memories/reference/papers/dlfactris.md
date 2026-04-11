---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [dlfactris, hinrichsen, krebbers, birkedal, deadlock_freedom, rust, multiparty_session_types, actor, message_reordering]
related: [reference/papers/_hub, reference/papers/eact, reference/papers/forwarders]
agents: [session-type-consultant, pane-architect]
---

# Deadlock-free async message reordering in Rust with multiparty session types (DLfActRiS)

**Authors:** Hinrichsen, Krebbers, Birkedal (POPL 2024)
**Path:** `~/gist/deadlock-free-asynchronous-message reordering-in-Rust-with-multiparty-session-types/`

(Note the space in the directory name — quote when listing.)

## Summary

A deadlock-freedom result for actor systems with async message
reordering, formalized in Iris (Rust-style ownership) on top of
multiparty session types. Uses a connectivity-graph acyclicity
condition to rule out the deadlock cases.

The connectivity graph idea — explicit DAG of who-can-talk-to-whom
— is structural, not behavioral. If the graph is acyclic, the
system can be proven deadlock-free regardless of message ordering.

## Concepts informed

- pane's `agent/optics-theorist/linearity_gap` mentions a "connectivity
  graph debug tool" derived from this paper
- The acyclicity invariant for pane's WatchPane / PaneExited
  (one-shot, unidirectional server→watcher, no response
  obligation)
- The Rust-typed actor model — proves what Rust's affine types
  give us at the static level

## Used by pane

- `agent/optics-theorist/linearity_gap` — agent-private
  reference material on LinearActris and connectivity.
- `architecture/looper` — implicitly relies on connectivity
  acyclicity for I9 destruction-sequence ordering
- `reference/papers/eact` — cross-reference (EAct + DLfActRiS
  cover complementary safety properties)
