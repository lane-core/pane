---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [eact, fowler, hu, safe_actor, multiparty, session_types, sigma, omega, e_self, e_monitor, preservation, progress, global_progress]
related: [reference/papers/_hub, reference/papers/dlfactris, reference/papers/forwarders]
agents: [session-type-consultant, pane-architect, formal-verifier]
---

# Safe actor programming with multiparty session types (EAct)

**Authors:** Fowler, Hu
**Path:** `~/gist/safe-actor-programming-with-multiparty-session-types/`

## Summary

Defines the EAct calculus — a typed actor language with
multiparty session types. Proves three safety theorems:

- **Preservation** (Theorem 4.7 / 4.10) — well-typed
  configurations stay well-typed under reduction
- **Progress** (Lemma 4.5 / 4.12) — well-typed actors don't
  get stuck
- **Global progress** (Corollary 4.8 / Theorem 4.13) — the
  whole system progresses, not just individual actors

§4.2.2 covers monitoring (E-Monitor / E-InvokeM): monitoring is
"orthogonal" to safety — adding it doesn't break the theorems.

§5 discusses ibecome (which pane chooses NOT to adopt).

## Concepts informed

- pane's actor model is grounded in EAct
- The 19 invariants in pane's architecture spec map to EAct's
  safety conditions
- E-Self (self-messaging) is mentioned once in §894 informally;
  pane defers it (notification-triggers-request open question)
- E-Monitor justifies pane's WatchPane / PaneExited control
  messages as orthogonal to session safety
- ibecome rejection: the paper itself notes systems without
  ibecome enjoy global progress (§5)

## Used by pane

- `reference/plan9/divergences` — repeatedly cited
- `architecture/looper` — six-phase batch ordering preserves
  the EAct E-Send / E-Receive interleaving discipline
- The whole `analysis/eact/` cluster — hub at
  `analysis/eact/_hub` with spokes for audit, divergences,
  gaps, invariants, and design principles not adopted.
