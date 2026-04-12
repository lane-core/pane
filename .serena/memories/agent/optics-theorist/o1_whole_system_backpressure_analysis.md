---
type: agent
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [O1, backpressure, two_function_split, send_sites, Setter, pointed_monad_algebra, coalescing, PutPut, no_trait]
sources: [decision/connection_source_design, reference/papers/profunctor_optics, reference/papers/dont_fear_optics, agent/optics-theorist/project_connectionsource_review]
verified_against: [CBG24 Def setter (arxivmain.tex:1771), CBG24 Def monadiclens (arxivmain.tex:1054), DontFear Optics.md:187 (affine laws)]
related: [decision/connection_source_design, analysis/optics/scope_boundaries, agent/optics-theorist/project_connectionsource_review]
agents: [optics-theorist]
---

# O1 whole-system backpressure: optics-theorist analysis

Analysis of the three-tier send-site classification for the
ConnectionSource two-function API (O1 from decision/connection_source_design).

## 1. Profunctor structure over tiers: irreducible heterogeneity

The three tiers (A: both variants, B: infallible-only, C: fallible-only)
differ in *codomain category*, not focus. Tier A is in Kl(Result<_, (Req, E)>),
Tier B is in Set, Tier C is in Kl(Result<_, E'>). This is morphism-level
heterogeneity. No profunctor optic captures it because:
- Send sites don't compose (send_request ∘ cancel is not meaningful)
- No universal quantification over profunctor P is useful
- CBG24 mixed optics address asymmetric read/write in ONE optic, not
  classification of a FAMILY of unrelated morphisms

Consistent with round-2 result: sends are Kleisli arrows, not optics.
Gate 1 from `feedback_not_every_access_is_a_lens` — no get, no lens.

## 2. Pointed monad algebra: degenerate, not worth naming

With only 2 of ~10 sites having both variants, and those 2 having
*different* error monads (one returns obligation handle, other doesn't),
there's no family to abstract over. The theoretical classification
(round-2) correctly rules out MonadicLens but the "pointed monad algebra"
label earns no design payoff.

Recommendation: keep in decision log for provenance, don't propagate
to architecture or code comments.

## 3. set_content as Setter: coalescing = PutPut

set_content is a lawful Setter (CBG24 Def setter, line 1771):
Setter((A,B),(S,T)) := V([A,B],[S,T]). Write-only, no get.

Coalescing (only latest write matters) IS the PutPut law:
set(a2, set(a1, s)) = set(a2, s). Every lawful Setter satisfies this.
Write-queue optimization (eliding intermediates) is provably correct
if set_content is a lawful Setter. No exotic optic needed.

set_content is the ONE send site on the optic side of the scope boundary.
All others are runtime-I/O side per analysis/optics/scope_boundaries D4.

## 4. No-trait decision: correct

Sites differ on 4 independent axes (error type, return type, obligation
structure, cancellation semantics). A trait parameterized over all 4 is
more complex than the methods it abstracts. The 2-site default impl
(send = unwrap ∘ try_send) doesn't justify a trait. No profunctor
composability payoff since sites don't compose with each other.

## 5. Additional observations

5a. set_content composes with other optics on content field if they
    exist (Setter composition). Not needed Phase 1, worth documenting.

5b. Cancel is a natural transformation from in-flight-requests to
    the trivial category (forgets content, keeps token). Naturality
    is automatic (cancel doesn't inspect request). Supports
    cancel-by-token as robust scope choice for O5.

5c. Queue::push is a partial monoid action, NOT a Setter.
    push accumulates (monoidal), set_content overwrites (idempotent).
    Document to prevent conflating streaming-push with coalescing.
