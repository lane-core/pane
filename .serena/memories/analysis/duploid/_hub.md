---
type: hub
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [duploid, polarity, non_associative, mangel, melliees, munch_maccagnoni, writer_monad, mixed_optic, shift_operator, active_phase, thunkable, classical_notions]
related: [reference/papers/duploids, reference/papers/fcmonads, reference/papers/logical_aspects_vdc, reference/papers/linear_logic_no_units, analysis/optics/_hub, analysis/session_types/_hub, decision/server_actor_model, policy/functoriality_principle]
agents: [optics-theorist, session-type-consultant, pane-architect]
---

# Duploid analysis cluster

## Motivation

Pane's architecture is a *duploid* — a non-associative
polarized category — and naming the polarity explicitly
makes three things visible at once: (1) why the pre-actor
dispatch design produced a deadlock (non-associative bracket
realized concurrently), (2) why the handshake and active
phases must not compose (different duploid structures), and
(3) why `MonadicLens` is a mixed optic and therefore bridges
the two subcategories without unifying them.

The cluster is small but load-bearing: every dispatch
refactor, every new ω_X shift (handshake → active),  and
every protocol boundary needs a polarity story. Without it,
the temptation to fold handlers, replies, and filesystem
writes into a single uniform dispatch layer is hard to
refuse.

## Spokes

- [`analysis/duploid/polarity_and_composition`](polarity_and_composition.md)
  — the core framework: positive (Kleisli / CBV) vs
  negative (co-Kleisli / CBN), cross-polarity is
  non-associative, and pane's concrete polarity assignments
  (`ServiceFrame::*`, `Handles<P>::receive`, `ReplyPort`,
  `ActivePhase<T>`). Derived from the 2026-04-05 four-agent
  roundtable.
- [`analysis/duploid/writer_monad_and_optics`](writer_monad_and_optics.md)
  — second-round refinement: the writer monad `Ψ(A) = (A,
  Vec<Effect>)`, ArcSwap as identity comonad, mixed-optic
  characterization of MonadicLens (Clarke et al. Prop 4.7),
  the shift operator ω_X as forgetful transition. BeOS
  scripting polarity map appears here.
- [`analysis/duploid/polarity_classifications`](polarity_classifications.md)
  — lookup tables for Plan 9 (9P, fids, co-Kleisli), BeOS
  (already a duploid), and pane (ServiceFrame positive,
  ReplyPort ↑(...), dispatch CBV). Use this before
  classifying a new type.

## Open questions

- **ActivePhase<T> as explicit ω_X.** Designed, not yet
  threaded through the dispatch context. Status memo Phase
  1 item. Requires touching `DispatchCtx` and the looper.
- **Effect reordering rules.** Writer-monad effect lists
  are a free monoid; commutativity is not free. Current
  rule: reorder only thunkable operations (MM14b Prop 6).
  Needs an explicit Mazurkiewicz interference relation if
  we ever want automatic batching beyond the six-phase
  ordering.
- **Namespace composition.** Currently treated as set
  intersection (commutative, idempotent). Union mounts or
  precedence chains would reintroduce non-commutativity
  and require a polarity-aware composition law.

## Cross-cluster references

- `reference/papers/duploids` — Munch-Maccagnoni / Mangel /
  Melliès. MMM25 + MM14b; primary source.
- `reference/papers/fcmonads` — Cruttwell / Shulman.
  Virtual double categories as the abstract setting the
  duploid intuition sits inside.
- `reference/papers/logical_aspects_vdc` — FVDblTT, the
  type theory of VDCs.
- `reference/papers/linear_logic_no_units` — Houston.
  Unitless MLL is the closest "no units" antecedent for
  the non-associative collages on the filesystem side.
- `analysis/optics/_hub` — MonadicLens laws + implementation;
  polarity discipline justifies the mixed-optic choice.
- `analysis/session_types/_hub` — protocol design uses the
  same polarity vocabulary (handlers negative, wire
  positive).
- `decision/server_actor_model` — single-mailbox actor is
  the concrete enforcement mechanism preventing
  non-associative brackets.
- `policy/functoriality_principle` — "Phase 1 types must be
  full-architecture types" is a direct consequence of
  polarity discipline: shift operators must be identifiable
  before implementation.
