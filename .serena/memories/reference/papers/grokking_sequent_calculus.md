---
type: reference
status: current
citation_key: BTMO
aliases: [Grokking]
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [grokking, sequent_calculus, binder, tzschentke, müller, ostermann, lambda_mu_mutilde, fun, core, accessible_introduction, data_codata, error_duality]
related: [reference/papers/_hub, reference/papers/dissection_of_l, reference/papers/duploids]
agents: [session-type-consultant, pane-architect]
---

# Grokking the sequent calculus

**Authors:** Binder, Tzschentke, Müller, Ostermann
**Path:** `~/gist/grokking-the-sequent-calculus.gist.txt`

## Summary

Programmer-facing introduction to **λμμ̃**. Compiles a small
functional language (Fun) to a sequent-calculus core (Core).
Shows the practical benefits: data / codata duality is explicit,
direct vs indirect consumers are typed differently, and the
⊕/⅋ error duality has a clean operational semantics.

The structure of the paper:

1. Why classical sequent calculus is good for compilers
2. λμμ̃ syntax and operational semantics
3. Type system
4. Compilation from Fun to Core
5. Worked examples

## Concepts informed

- The data / codata distinction (pane's MonadicLens uses codata
  observers conceptually)
- ⊕/⅋ error duality — pane has Protocol vs Crash error
  channels, which mirror this duality
- Direct vs indirect consumers — relevant to how pane's
  obligation handles work

## Used by pane

- `analysis/shell_sequent_calculus` — uses the Fun→Core
  compilation framing
- The session-type consultant cites this as the accessible
  starting point before going to `dissection_of_l`
