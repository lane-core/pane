---
type: reference
status: current
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [papers, gist, anchors, theory, session_types, optics, vdc, sequent_calculus, hub]
related: [reference/haiku/_hub, reference/plan9/_hub]
agents: [all]
---

# Theoretical paper anchors

Index of vendored theoretical papers in `~/gist/`. Each anchor
is a retrieval pointer: paper name, path, summary, concepts
informed, agents that consult it. The actual paper content lives
in `~/gist/<paper>/` (or `.gist.txt` for plain-text excerpts).

## Session types (session-type-consultant primary)

- [`reference/papers/forwarders`](forwarders.md) — Carbone, Marin, Schürmann. A logical interpretation of async multiparty compatibility (forwarders capture all multiparty-compatible compositions).
- [`reference/papers/multiparty_automata`](multiparty_automata.md) — Multiparty compatibility in communicating automata.
- [`reference/papers/dependent_session_types`](dependent_session_types.md) — Toninho-Caires-Pfenning. Dependent session types via intuitionistic linear type theory.
- [`reference/papers/refinement_session_types`](refinement_session_types.md) — Practical refinement session-type inference.
- [`reference/papers/projections_mpst`](projections_mpst.md) — Generalizing projections in multiparty session types.
- [`reference/papers/async_global_protocols`](async_global_protocols.md) — Asynchronous global protocols.
- [`reference/papers/eact`](eact.md) — Fowler-Hu. Safe actor programming with multiparty session types (the EAct calculus).
- [`reference/papers/dlfactris`](dlfactris.md) — Hinrichsen-Krebbers-Birkedal. Deadlock-free async message reordering in Rust with multiparty session types.
- [`reference/papers/interactive_complexity`](interactive_complexity.md) — Computational complexity of interactive behaviors.

## Profunctor optics (optics-theorist primary)

- [`reference/papers/dont_fear_optics`](dont_fear_optics.md) — Boisseau-Gibbons. Don't Fear the Profunctor Optics — accessible three-part introduction.
- [`reference/papers/profunctor_optics`](profunctor_optics.md) — Clarke, Boisseau, Gibbons. Profunctor Optics, a Categorical Update — formal paper, Tambara modules, mixed optics, monadic lenses.

## VDC and duploids (theoretical foundations)

- [`reference/papers/duploids`](duploids.md) — Munch-Maccagnoni / Mangel / Melliès. Classical Notions of Computation and Hasegawa-Thielecke (MM14b / MMM25). Duploid semantics, (+,−) non-associativity.
- [`reference/papers/fcmonads`](fcmonads.md) — Cruttwell, Shulman. Mathematical foundation for VDCs. §3 VDCs, §5 composites (Segal), §6 restrictions, §7 virtual equipments.
- [`reference/papers/logical_aspects_vdc`](logical_aspects_vdc.md) — Hayashi, Das et al. FVDblTT — type theory of VDCs.
- [`reference/papers/linear_logic_no_units`](linear_logic_no_units.md) — Houston. Promonoidal categories as models for unitless multiplicative linear logic.
- [`reference/papers/squier_hott`](squier_hott.md) — Kraus, von Raumer. Squier's theorem in HoTT — rewriting-as-cells, local-to-global coherence.

## Sequent calculus

- [`reference/papers/dissection_of_l`](dissection_of_l.md) — Spiwack. Dissecting System L into constituent parts.
- [`reference/papers/grokking_sequent_calculus`](grokking_sequent_calculus.md) — Binder, Tzschentke, Müller, Ostermann. Accessible introduction to λμμ̃.

## Knowledge management (this is the rulebook)

- [`reference/papers/memx`](memx.md) — Sun (March 2026). MemX, the local-first hybrid retrieval system whose principles are canonicalized for serena in [`policy/memory_discipline`](../../policy/memory_discipline.md).

## Unix history (context)

- [`reference/papers/unix_retrospective`](unix_retrospective.md) — Ritchie. The Unix Time-Sharing System: A Retrospective.

## Where the rules live

- [`policy/memory_discipline`](../../policy/memory_discipline.md) — the canonical principles document for serena memory organization

## When to consult

- "What does the EAct calculus say about X?" → `reference/papers/eact` (full paper at `~/gist/safe-actor-programming-with-multiparty-session-types/`)
- "Is this composition multiparty-compatible?" → `reference/papers/forwarders`
- "What are the laws for monadic lenses?" → `reference/papers/profunctor_optics` Proposition 4.7
- "Why is (+,−) non-associative?" → `reference/papers/duploids`
- "How does Segal compositionality work in a VDC?" → `reference/papers/fcmonads` §5
