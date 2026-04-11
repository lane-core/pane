---
type: hub
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [session_types, eact, ferrite, binary_sessions, optic_boundary, coprocess, principles, affinity, linearity, protocol_fidelity]
related: [reference/papers/eact, reference/papers/dlfactris, reference/papers/forwarders, reference/papers/refinement_session_types, reference/papers/dependent_session_types, analysis/eact/_hub, analysis/optics/_hub, architecture/app, architecture/session, agent/session-type-consultant/_hub]
agents: [session-type-consultant, pane-architect, formal-verifier]
---

# Session-types analysis cluster

## Motivation

Session types are the spec language for pane's IPC. The
cluster answers three distinct queries: (1) *how do we
design* a new protocol (principles C1–C6 from EAct), (2)
*where does session vocabulary meet optics* at the
pane-proto / pane-app membrane (rules R1–R10), and (3) *what
does a real concrete protocol look like* (the coprocess
per-tag binary sessions). Together they give a reader the
abstract rules, the concrete Rust boundary discipline, and
one worked example.

The hub is also the entry point for anyone asking "why
doesn't pane use multiparty session types on the wire?" —
the short answer is `analysis/session_types/principles` C3;
the long answer is in `analysis/eact/design_principles_not_adopted`.

## Spokes

- [`analysis/session_types/principles`](principles.md) — six
  design principles (C1 heterogeneous session loops, C2
  sub-protocol typestate, C3 binary sessions on wire, C4
  access-point discovery, C5 Handles<P> declarative
  dispatch, C6 looper / session separation). Derived from
  Fowler-Hu and adapted for pane's actor model.
- [`analysis/session_types/optic_boundary`](optic_boundary.md)
  — ten empirical rules (R1–R10) from a full crate scan:
  the `!Send` / `!Clone` firewall at pane-proto, obligation
  handles sitting in pane-app, `drop-sends-failure` as
  session discipline, the value / obligation split as the
  cartesian / linear split.
- [`analysis/session_types/coprocess_corrected`](coprocess_corrected.md)
  — per-tag binary sessions (not mailbox types). Corrects an
  earlier mailbox-typed proposal, grounds the wire
  multiplexing / protocol structure separation in 9P
  precedent, and proves deadlock-freedom via DLfActRiS.

## Open questions

- **Dynamic discovery and C2.** Sub-protocol typestate
  assumes static protocol knowledge; `DeclareInterest` +
  access-point discovery (C4) need a clearer end-to-end
  story. The `ActivePhase<T>` shift (see `analysis/duploid`)
  is part of the answer.
- **Affinity vs linearity at `ServiceHandle::Drop`.** Drop
  sends `RevokeInterest`; this is affine, not linear. When
  is the weakening OK? R9 in the boundary rules partially
  answers, but the wire-protocol side is still TODO.
- **Polarity in Setup vs Active phases.** Coprocess example
  is pure-alternating. The hub has not yet documented the
  full polarity-preservation story for non-alternating
  protocols.

## Cross-cluster references

- `analysis/eact/_hub` — EAct calculus, theorems, the
  specific divergences and non-adoptions. Read for formal
  grounding.
- `analysis/optics/_hub` — the optic boundary rules live
  where session vocabulary *stops*; the optics cluster
  covers what's on the other side.
- `analysis/duploid/_hub` — polarity vocabulary shared
  throughout; explains positive (wire) vs negative
  (handler) in session-type terms.
- `reference/papers/forwarders` — multiparty compatibility;
  justifies binary-on-wire as a sound subset.
- `reference/papers/refinement_session_types`,
  `reference/papers/dependent_session_types` — adjacent
  theory; not directly used by pane but informs the design
  space.
- `architecture/session` — the FrameCodec, ProtocolServer
  actor, handshake, watch/PaneExited — where the principles
  actually land in Rust.
- `architecture/app` — `HandlesRequest<P>` `H = Self`
  binding, install-before-wire, destruction sequence; the
  boundary in action.
- `agent/session-type-consultant/_hub` — institutional
  knowledge for the session-type agent.
