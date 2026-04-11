---
type: hub
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [optics, monadic_lens, profunctor, concrete_encoding, fp_library, writer_monad, panefs, attr_reader, linearity, boundary]
related: [reference/papers/dont_fear_optics, reference/papers/profunctor_optics, reference/fp_library, architecture/proto, architecture/fs, agent/optics-theorist/_hub, decision/clipboard_and_undo, analysis/duploid/_hub]
agents: [optics-theorist, pane-architect, session-type-consultant]
---

# Optics analysis cluster

## Motivation

Pane uses optics to model typed attribute access across two
boundaries: **pane-proto's effectful kit API** (handlers
expose state via `MonadicLens<S, A>` with a pure view and an
effectful set) and **pane-fs's FUSE read path** (synthesized
`AttrReader<S>` closures served from snapshots). The cluster
grounds both, names what is *not* an optic, and documents why
the concrete encoding is preferred over the profunctor
(`fp-library`) encoding.

Two-tier resolution (recorded here as well as in
`architecture/proto` and `architecture/fs`):

- **Tier 1 — pane-proto kit:** concrete `MonadicLens<S, A>`.
  Pure view + Kleisli-over-writer-monad set. No Send/Sync
  trait-object blocker. Laws verified by
  `assert_monadic_lens_laws`.
- **Tier 2 — pane-fs FUSE read:** concrete `AttrReader<S>` /
  `AttrSet<S>`, read-only, HashMap-backed. Deliberately
  *not* reusing `MonadicLens` — different query (snapshot
  read, no set path).

The historical concrete-vs-`fp-library` tension lived at Tier
1. `fp-library`'s profunctor-object lacked Send/Sync bounds
and the representation theorem did not buy runtime law
enforcement in Rust. Resolved by choosing concrete.

## Spokes

- [`analysis/optics/implementation_guidance`](implementation_guidance.md)
  — three profunctor insights validated by the kit:
  obligation handles as linear lenses, `AttrCapability`
  lattice, PutPut coalescing.
- [`analysis/optics/scope_boundaries`](scope_boundaries.md) —
  seven-dimension analysis + 9P composition. Why optics is
  a semantic criterion, not a protocol primitive.
- [`analysis/optics/panefs_taxonomy`](panefs_taxonomy.md) —
  per-entry optic classification for `/pane/<id>/`: Lens,
  Getter, AffineTraversal, NOT-an-optic (ctl, reload,
  close). Path composition examples.
- [`analysis/optics/writer_monad`](writer_monad.md) — writer
  monad `Ψ(A) = (A, Vec<Effect>)` for effectful set;
  thunkability criterion (MM14b Prop 6); ArcSwap as identity
  comonad on reads.
- [`analysis/optics/boundaries`](boundaries.md) — what
  doesn't fit: obligation handles (linear, outside optics),
  ctl commands (side-effecting, outside optics), event
  streams (temporal, outside optics), close / reload / copy
  (lifecycle, outside optics).

Agent-private companion: `agent/optics-theorist/linearity_gap`
— reference material on affinity + runtime recovery,
Ferrite-style CPS, connectivity graph. Kept private because
the LinearActris work is session-type-adjacent, not
optics-core.

## Open questions

- `property.rs` (outside pane-proto) still uses `fp-library`.
  This is a separate concern at a different abstraction
  layer; not a blocker for the Tier 1 / Tier 2 resolution
  above.
- PutPut law for lenses that mutate nested `Vec<Effect>` —
  the current harness checks via explicit state comparison;
  a future property-test generator would strengthen this.

## Cross-cluster references

- `analysis/duploid/_hub` — polarity grounding for why the
  set side is Kleisli (positive) and the view side is
  comonadic (negative) in MonadicLens.
- `reference/papers/dont_fear_optics` — Boisseau-Gibbons
  pedagogical introduction (read first if new to optics).
- `reference/papers/profunctor_optics` — Clarke-Boisseau-Gibbons
  formal paper; Proposition 4.7 grounds the mixed-optic
  characterization of MonadicLens.
- `reference/fp_library` — viability assessment of the
  Rust profunctor-optics crate, Send/Sync gap notes.
- `architecture/proto` — `MonadicLens` + `assert_monadic_lens_laws`
  test harness live here.
- `architecture/fs` — `AttrReader` / `AttrSet` live here.
- `decision/clipboard_and_undo` — clipboard is deliberately
  *not* an optic (it's a command-driven side channel).
