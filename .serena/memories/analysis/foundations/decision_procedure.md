---
type: analysis
status: current
audited_by: [optics-theorist@2026-04-11]
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [decision-procedure, monadic, comonadic, boundary-crossing, feature-classification, kleisli, cokleisli, oblique, vdc-framework-8-5]
related: [analysis/foundations/_hub, analysis/duploid/oblique_maps, analysis/duploid/polarity_and_composition, reference/papers/duploids, policy/functoriality_principle]
sources: [../psh/.serena/memories/analysis/decision_procedure_8_5]
verified_against: [../psh/docs/vdc-framework.md@2026-04-11]
agents: [optics-theorist, formal-verifier, pane-architect]
---

# The decision procedure (monadic / comonadic / boundary-crossing)

## Problem

When a new feature is proposed for pane, how is it classified
against the existing polarity discipline? Naming a feature's
polarity class after implementation is too late — by then the
composition laws have already been fought. The decision
procedure is a governance tool: classify first, implement
second.

## Resolution

psh's `docs/vdc-framework.md` §8.5 ("Decision procedure for new
features," line 865 in psh's tree) articulates a three-way
classifier pane adopts unchanged:

- **Monadic (Kleisli, `•`, positive side).** The feature is
  producer-side: it composes with other producers by Kleisli
  composition and associates cleanly. Effects flow forward
  through value composition. In pane: `ServiceFrame`
  construction, `Message` serialization, outbound wire writes,
  any operation that builds a value for consumption.
- **Comonadic (co-Kleisli, `○`, negative side).** The feature
  is consumer-side: it composes with other consumers by
  co-Kleisli composition. Context flows backward through
  demand-driven reads. In pane: `Handles<P>::receive` dispatch,
  `AttrReader` view functions, any operation observing state.
- **Boundary-crossing (oblique, P → N).** The feature is a
  producer-consumer interaction site — neither a pure producer
  nor a pure consumer, but the place where a value meets a
  demand. In pane: `ServiceHandle::send_request` (positive
  frame handed to negative handler), `ReplyPort::reply`
  (negative continuation fired with positive value), `ctl`
  command writes (positive text becomes negative state
  mutation). These are the oblique maps
  (`analysis/duploid/oblique_maps`), and per the (+,−)
  non-associativity (`analysis/duploid/polarity_and_composition`)
  every boundary crossing needs a polarity frame.

The procedure applied: identify the feature's interaction with
`•`, `○`, and cut. If it associates cleanly with one of the two
same-polarity compositions, classify there. If it's a
producer-consumer interaction, classify as boundary-crossing —
and the non-associative cross-polarity composition tells you
you need a frame. Pane's single-threaded actor (see
`decision/server_actor_model`) is the runtime analog of the
frame discipline: every polarity crossing is serialized, so
the bracketing ambiguity cannot be realized concurrently. The
frame discipline is implicit in the type system because the
looper serializes all polarity crossings; this is a structural
correspondence, not a formal proof that pane's actor realizes
Downen-style focusing in the technical sense.

## Status

Adopted from psh 2026-04-11. Authoritative for "should pane add
feature X" design questions once tier-2 audited. Whether the
full duploid composition laws carry over from psh's shell
setting to pane's actor / filesystem setting is not formally
verified — the structural correspondence is what pane's
governance consumes.

## Source

- `../psh/docs/vdc-framework.md` §8.5 "Decision procedure for
  new features" (line 865 in psh's tree) — the canonical source.
  Not vendored into pane; read from the psh checkout.
- `../psh/docs/vdc-framework.md` §8.1–8.4 (lines 779–865) — the
  composition law machinery the procedure consumes.
- `reference/papers/duploids` — theoretical justification of
  the three-way classification (MMM25 composition laws).
