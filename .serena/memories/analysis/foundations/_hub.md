---
type: hub
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [foundations, cbpv, f_u_adjunction, data_vs_codata, error_duality, decision_procedure, levy, grokking, theoretical_vocabulary]
related: [analysis/duploid/_hub, analysis/optics/_hub, analysis/session_types/_hub, analysis/shell_sequent_calculus, reference/papers/grokking_sequent_calculus, reference/papers/duploids, policy/functoriality_principle]
agents: [optics-theorist, session-type-consultant, pane-architect, formal-verifier]
---

# Foundations cluster

## Motivation

Theoretical vocabulary anchors that pane adopts for design
governance. Each spoke is a keyword-shaped landing: short enough
to read in one pass, specific enough to settle a design
question, cited enough to trace to a primary source. The cluster
exists because pane needs the same disciplined vocabulary psh
built for its shell language — the two projects share a
theoretical substrate, and pane's architecture decisions lean
on the same duploid / CBPV / sequent-calculus framework.

These anchors are **not** tier-1 self-contained analyses. They
orient the reader, point at primary sources, and name a concept
so later memories (and code comments) can cite it by one-word
handle. When the question is "what type of feature should this
be?" or "where does X fit in the type theory?", these are the
first stop.

Ported from psh's concept-anchor layer on 2026-04-11. The psh
anchors themselves are at
`../psh/.serena/memories/analysis/<name>.md`; the ports here
strip psh-specific citations (rc, ksh93, sfio) and re-anchor
against pane's concrete types (`Handles<P>`, `ServiceFrame`,
`ReplyPort`, `MonadicLens`).

## Spokes

- [`analysis/foundations/decision_procedure`](decision_procedure.md)
  — three-way classifier (monadic / comonadic /
  boundary-crossing) for "should pane add feature X" questions.
  Grounded in duploid composition laws.
- [`analysis/foundations/cbpv_f_u_separation`](cbpv_f_u_separation.md)
  — Levy's call-by-push-value split: value types (positive)
  vs computation types (negative), bridged by F (returner) and
  U (thunk). Pane's `Handles<P>` / `Message` / `ReplyPort`
  instantiate the adjunction.
- [`analysis/foundations/data_vs_codata`](data_vs_codata.md) —
  constructors vs destructors, pattern vs copattern match. The
  duality that keeps `ServiceFrame` (data) and `Handles<P>`
  (codata) structurally distinct.
- [`analysis/foundations/error_handling_duality`](error_handling_duality.md)
  — ⊕ (status, positive) vs ⅋ (signal / callback, negative) as
  De Morgan duals from linear logic. `ReplyPort::Failed` vs
  `Handler::disconnected` compose orthogonally because they
  operate on different duploid subcategories.

## Open questions

- **CBPV vocabulary adoption in pane docs.** The foundations
  anchors introduce the F/U, data/codata, ⊕/⅋ vocabulary, but
  `docs/architecture.md` does not yet use these terms. Should
  pane's spec adopt them explicitly (as psh's spec does) or
  keep them in the analysis/ layer only? Cluster is agnostic;
  flag for Lane.
- **Linking to duploid and polarity clusters.** Some of these
  anchors (especially `error_handling_duality` and
  `data_vs_codata`) are as much about polarity discipline as
  about foundations. The duploid cluster's
  `polarity_and_composition` spoke is the cross-reference; see
  also `analysis/duploid/oblique_maps` and
  `analysis/duploid/cbv_focusing`.

## Cross-cluster references

- `analysis/duploid/_hub` — polarity, non-associativity, shift
  operators. The foundations anchors are the vocabulary the
  duploid cluster consumes.
- `analysis/shell_sequent_calculus` — applies three_sorts and
  cut_as_execution to pane-terminal / psh integration. Phase
  2+ work that will lean on these foundations.
- `reference/papers/grokking_sequent_calculus` — the
  programmer-facing introduction to sequent calculus, CBPV,
  focusing, and data/codata duality. Read this first if the
  vocabulary is new.
- `reference/papers/duploids` — Mangel-Melliès-Munch-Maccagnoni
  MMM25 + MM14b; the duploid grounding for the decision
  procedure and oblique maps.
- `policy/functoriality_principle` — Phase 1 types must be
  full-architecture types; the decision procedure is the
  operational check.

## Porting provenance

All four spokes were ported 2026-04-11 from psh's
`../psh/.serena/memories/analysis/*` anchors (commits
`3a2106a`, `9083b15`, `5ad39c4` on branch `redesign`). psh's
tier-1 audit on 2026-04-11 caught nine hallucinations across
the original draft; the ports inherit the corrected text. Each
port's `verified_against:` frontmatter records the psh source
path at port time. Tier-2 audit of the pane ports (per
`policy/agent_workflow` §"Tier-2 audit for theoretical anchors")
completed 2026-04-11: optics-theorist audited `decision_procedure`
and `data_vs_codata`; session-type-consultant audited
`cbpv_f_u_separation` and `error_handling_duality`;
formal-verifier audited hub structure and pointer graph. Four
MINOR findings folded (wording softenings, naming corrections,
line-number precision). All four foundation spokes plus the two
adjacent duploid spokes (`oblique_maps`, `cbv_focusing`) promoted
to `status: current` after fold.
