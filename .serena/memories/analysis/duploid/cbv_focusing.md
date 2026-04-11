---
type: analysis
status: current
audited_by: [optics-theorist@2026-04-11]
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [cbv_focusing, static_focusing, critical_pair, reentrancy, focused_sequent_calculus, mu_mu_tilde, fire_once_per_dispatch, downen]
related: [analysis/duploid/_hub, analysis/duploid/polarity_and_composition, analysis/optics/writer_monad, reference/papers/grokking_sequent_calculus, reference/papers/dissection_of_l, reference/papers/duploids]
sources: [../psh/.serena/memories/analysis/polarity/cbv_focusing]
verified_against: [../psh/docs/specification.md@2026-04-11, ../psh/docs/vdc-framework.md@2026-04-11]
agents: [optics-theorist, session-type-consultant]
---

# CBV focusing as reentrancy discipline

## Problem

Pane's handler methods fire in a six-phase batched loop per
`architecture/looper`. Within a single dispatch call, state
observed via a `MonadicLens` view is effectively cached —
re-reading the same attribute in the same handler method
yields the same value. Across dispatch calls the cache is
invalidated. This is not memoization-as-optimization; it is
focusing discipline as the operational answer to "when do
effects happen relative to observations." Naming it gives
future optic design questions a handle.

## Resolution

**Focusing** is a structural discipline on the sequent calculus
that resolves the critical pair between μ (continuation-binding)
and μ̃ (value-binding) by mandating one evaluation order.
**CBV (call-by-value) focusing** picks the order where the
value side wins: when both rules apply, μ̃ fires first, the
value lands in the variable, and that value is reused at every
subsequent consumption site within the same focused scope.

In pane, CBV focusing is the operational answer to "when does
a `MonadicLens` view re-read state on a handler method?" Once
per dispatch call, at first use; subsequent reads within the
same call see the cached value. The scope boundary is the
dispatch method — not the handler lifetime, not the optic
itself. Across dispatch calls (next message, next lifecycle
event), the cache is invalidated and the next view re-reads
fresh.

This is **not** memoization-as-optimization, even though it
looks like it from outside. It is focusing discipline realized
at the polarity boundary — Downen et al.'s static focusing
made operational. Memoization is what it looks like; focusing
is what it is. The operational consequence is that
`MonadicLens` laws (GetPut, PutGet, PutPut) remain meaningful
in the presence of effects because effects are bounded to the
focused scope: you cannot observe a partial mutation from
within a single dispatch, only at the boundary to the next.

Pane's relevant load-bearing sites:

- **`Handles<P>::receive`** — treat a single dispatch call as
  the focused scope for `MonadicLens` law-checking purposes.
  Whether pane's current implementation caches repeated views
  within one `receive` call is not verified here; the anchor
  is governance vocabulary for law-checking, not a claim about
  existing runtime behavior. Writes produce a `Vec<Effect>`
  residue the handler applies at scope exit.
- **`ctl` dispatch (future work)** — per `analysis/optics/panefs_taxonomy`,
  ctl writes are synchronous and block until the looper has
  processed them. Each ctl write is its own focused scope.
- **`AttrReader<S>`** — reads from a `Clone`d state snapshot
  (per `architecture/fs`). The snapshot is the frozen
  focused-scope state; concurrent FUSE reads see the same
  value within the same snapshot generation.

Preserved caveat from psh's original anchor: cross-variable
consistency across separate focused scopes is **not**
guaranteed. Two `MonadicLens` reads of different attributes
across two separate dispatch calls can see inconsistent
states (the underlying state moved between the calls). Psh
documents this as a known caveat at `docs/specification.md`
line 677; pane inherits it — `status`'s "Known open questions"
section may need an entry when the ctl path lands.

## Status

Adopted from psh 2026-04-11 as design vocabulary for the
"effects once per focused scope" discipline. Treat every
`Handles<P>::receive` call as a focused scope for the
purposes of `MonadicLens` law checking. Not formally verified
that pane's dispatch loop realizes the Downen-style static
focusing in the technical sense — the structural analogy is
what the anchor consumes.

## Source

- `reference/papers/grokking_sequent_calculus` — Binder,
  Tzschentke, Müller, Ostermann. Introduces focusing as
  critical-pair resolution in the functional pearl. Cleanest
  first read.
- `reference/papers/dissection_of_l` — Spiwack. Focusing as a
  structural phase of System L.
- `reference/papers/duploids` — focusing at the categorical
  level as critical-pair resolution.
- `../psh/docs/specification.md` §"CBV focusing as the
  reentrancy semantics" line 556, §"Known caveat:
  cross-variable consistency" line 677. psh's authoritative
  framing; not vendored into pane.
- `../psh/docs/vdc-framework.md` §6.2 "The Sequent Calculus as
  the Type Theory of Shell" line 694 — psh's commitment to
  Downen-style static focusing. Not vendored.
