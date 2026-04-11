---
type: analysis
status: current
audited_by: [session-type-consultant@2026-04-11]
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [error_duality, oplus, par, status, callback, de_morgan, linear_logic, reply_port_failed, handler_disconnected, orthogonal_composition]
related: [analysis/foundations/_hub, analysis/foundations/data_vs_codata, analysis/duploid/polarity_and_composition, analysis/session_types/optic_boundary, reference/papers/grokking_sequent_calculus, reference/papers/linear_logic_no_units]
sources: [../psh/.serena/memories/analysis/error_duality_oplus_par]
verified_against: [../psh/docs/specification.md@2026-04-11, /Users/lane/gist/grokking-the-sequent-calculus.gist.txt@HEAD]
agents: [session-type-consultant, optics-theorist, formal-verifier]
---

# ⊕ / ⅋ error-handling duality

## Problem

Pane has two co-existing error-handling mechanisms that look
superficially different but turn out to be the same pattern
seen from two sides: request/reply failures via `ReplyPort`
(positive, status-like) and connection / handler failures via
lifecycle callbacks (negative, signal-like). The sequent
calculus names them as a De Morgan dual pair, which is why
they compose orthogonally — which is why the dispatch layer
can treat them independently without coordination.

## Resolution

Linear logic has two disjunctions, ⊕ ("plus", additive) and
⅋ ("par", multiplicative). They are **De Morgan duals** —
connected by the same involutive negation that swaps CBV and
CBN, caller-chooses and callee-chooses, data and codata. The
programmer-facing consequence is two legitimate,
complementary error conventions:

- **⊕ (positive — caller inspects).** The operation returns a
  result the caller pattern-matches on. In pane, `ReplyPort`
  failures (the `Err(ReplyFailed)` arm fired by
  `DispatchEntry::on_failed`, see `crates/pane-app/src/dispatch.rs`
  and the install-before-wire discipline at `architecture/app`)
  are the ⊕ realization: the caller asked, the result is
  either Reply or Failed, the caller inspects. This is a **data type** (see
  `analysis/foundations/data_vs_codata`): producer chooses a
  variant, consumer pattern-matches.
- **⅋ (negative — callee invokes a continuation).** The
  operation hands off a continuation the callee invokes when
  things go wrong. In pane, `Handler::disconnected` and
  `Handler::pane_exited` are the callee-chosen destructors: the
  transport decides when the handler should observe a
  connection loss; the handler must respond to the observation
  but does not "inspect" anything. This is a **codata type**:
  consumer chooses a destructor, producer responds.

Both conventions are present because both are legitimate. In
the sequent calculus they are dual, not different; the
negation swaps one into the other. The quote psh cites from
`../psh/refs/ksh93/ksh93-analysis.md` lines 225–226 names the
duality directly: "⊕ and ⅋ are connected by the same
involutive negation that swaps CBV and CBN." Grokking the
Sequent Calculus (`reference/papers/grokking_sequent_calculus`)
presents this as "the duality between the two different ways
of handling exceptions" (gist line 11552).

The **orthogonal-composition** property follows: because ⊕ and
⅋ operate on different duploid subcategories, a ⊕-handler
(`ReplyPort` failure path) and a ⅋-handler
(`Handler::disconnected`) do not need to coordinate. Pane's
current design already reflects this — the dispatch layer
treats them as independent, with different storage (request
table vs handler trait) and different lifecycles (one-shot
completion vs handler-wide observation). The duality explains
why this separation is sound, not coincidental.

**Linear-logic notation caveat.** ⊕ is *additive* disjunction;
⅋ is *multiplicative* disjunction. Grokking introduces them
as "two different kinds of disjunction" (gist lines 11509–11552).
The error-handling reading (status vs callback) is the
programmer-facing instance of the logical duality, not a
deviation from standard linear logic.

## Status

Adopted from psh 2026-04-11. Treat ⊕ / ⅋ as vocabulary for
distinguishing `ReplyPort::Failed` semantics from handler
lifecycle callbacks. The orthogonal-composition property is
already realized by pane's dispatch architecture; the anchor
names it rather than introducing it. Not yet formally proved
within pane's codebase that the two mechanisms cannot
interfere across connection boundaries — the structural
analogy with linear logic's two disjunctions is what the
anchor consumes.

## Source

- `reference/papers/grokking_sequent_calculus` — Binder,
  Tzschentke, Müller, Ostermann. Presents ⊕ / ⅋ as dual
  error-handling strategies in the functional pearl. The
  accessible first-read.
- `../psh/docs/specification.md` §"⊕ and ⅋" line 946, §"try /
  catch — scoped ErrorT (⊕ discipline)" line 957, §"trap —
  unified signal handling (⅋ discipline)" line 969, §
  orthogonal composition lines 1040–1044. psh's authoritative
  framing; not vendored into pane.
- `../psh/refs/ksh93/ksh93-analysis.md` §"⊕ / ⅋ error-handling
  duality" lines 206–226 — the table of conventions and the
  De Morgan duality framing, citing grokking `[7]`.
- `/Users/lane/gist/grokking-the-sequent-calculus.gist.txt`
  lines 11509–11552 — the two-disjunctions introduction.
- `reference/papers/linear_logic_no_units` — Houston. Unit-free
  MLL as the substrate; background context, not the direct
  source of the error-handling interpretation.
