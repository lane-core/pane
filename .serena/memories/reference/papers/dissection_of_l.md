---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [dissection_of_l, spiwack, system_l, sequent_calculus, mu_mutilde, classical, structural_reference]
related: [reference/papers/_hub, reference/papers/grokking_sequent_calculus, reference/papers/duploids]
agents: [session-type-consultant]
---

# A dissection of System L

**Author:** Spiwack
**Path:** `~/gist/dissection-of-l.gist.txt`

## Summary

Dissects **System L** (a presentation of classical sequent
calculus with explicit terms / coterms / commands) into its
constituent parts. Each connective is introduced separately,
each rule justified, each polarity choice motivated.

Useful as a structural reference: when you need to know what
the typing rule for a particular sequent-calculus connective
looks like, this paper has it.

## Concepts informed

- The three-sort structure (terms / coterms / commands) that
  pane's analysis as a duploid uses
- λμμ̃ as the underlying calculus — both pane's polarity
  classifications and psh's design rest on this
- Why classical (vs intuitionistic) sequent calculus is the
  right setting for talking about both producers and consumers

## Used by pane

- `pane/shell_sequent_calculus_analysis` — referenced when
  pane is interpreted through the same sequent-calculus lens
  as psh
- `pane/polarity_classifications`
- The session-type consultant cites this when proposing
  typing rules for new pane primitives
