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

**Citation keys.** Each entry below is prefixed with its
canonical bibliography key (e.g. `[FH]`, `[JHK24]`). These are
the same keys used in pane's code doc comments (`//!` / `///`)
and in memory memos. The authoritative bibliography is
`docs/citations.md`; see `policy/code_citation_standard` for
the discipline.

## Session types (session-type-consultant primary)

- `[CMS]` [`reference/papers/forwarders`](forwarders.md) — Carbone, Marin, Schürmann. A logical interpretation of async multiparty compatibility (forwarders capture all multiparty-compatible compositions).
- `[DY]` [`reference/papers/multiparty_automata`](multiparty_automata.md) — Deniélou, Yoshida. Multiparty compatibility in communicating automata: characterisation and synthesis of global session types. (Old alias: `[MPA]`.)
- `[TCP11]` [`reference/papers/dependent_session_types`](dependent_session_types.md) — Toninho, Caires, Pfenning (PPDP 2011). Dependent session types via intuitionistic linear type theory.
- `[UD]` [`reference/papers/refinement_session_types`](refinement_session_types.md) — Ueno, Das. Practical refinement session type inference (extended version; likely ESOP 2025). (Old alias: `[RST]`.)
- `[MMSZ]` [`reference/papers/projections_mpst`](projections_mpst.md) — Majumdar, Mukund, Stutz, Zufferey. Generalising projection in asynchronous multiparty session types. (Old alias: `[ProjMPST]`.)
- `[AGP]` [`reference/papers/async_global_protocols`](async_global_protocols.md) — Pischke, Masters, Yoshida. Asynchronous global protocols, precisely.
- `[FH]` [`reference/papers/eact`](eact.md) — Fowler, Hu. Speak Now: Safe actor programming with multiparty session types (the EAct calculus / Maty). Theorem locator at [`reference/papers/eact_sections`](eact_sections.md).
- `[JHK24]` [`reference/papers/dlfactris`](dlfactris.md) — Jacobs, Hinrichsen, Krebbers (POPL 2024). Deadlock-Free Separation Logic: Linearity Yields Progress for Dependent Higher-Order Message Passing. Introduces LinearActris; Theorem 1.2 proves global progress via linearity + connectivity-graph acyclicity.
- `[DHMV]` [`reference/papers/interactive_complexity`](interactive_complexity.md) — Dal Lago, Heindel, Mazza, Varacca. Computational complexity of interactive behaviors. (Old alias: `[IC]`.)

## Profunctor optics (optics-theorist primary)

- `[BG]` [`reference/papers/dont_fear_optics`](dont_fear_optics.md) — Boisseau, Gibbons. Don't Fear the Profunctor Optics — accessible three-part introduction.
- `[CBG24]` [`reference/papers/profunctor_optics`](profunctor_optics.md) — Clarke, Boisseau, Gibbons (Compositionality 2024). Profunctor Optics: A Categorical Update — formal paper, Tambara modules, mixed optics, monadic lenses.

## VDC and duploids (theoretical foundations)

- `[MMM]` [`reference/papers/duploids`](duploids.md) — Mangel, Melliès, Munch-Maccagnoni (PACMPL Vol 10 POPL, Article 73, 2026); foundational definitions in Munch-Maccagnoni 2014b (MM14b, FoSSaCS 2014). Classical Notions of Computation and the Hasegawa-Thielecke Theorem. Duploid semantics, (+,−) non-associativity. (Old alias: `[MMM25]`.)
- `[CS10]` [`reference/papers/fcmonads`](fcmonads.md) — Cruttwell, Shulman (TAC Vol 24, No 21, 2010). A Unified Framework for Generalized Multicategories. §3 VDCs, §5 composites (Segal), §6 restrictions, §7 virtual equipments.
- `[FVDblTT]` [`reference/papers/logical_aspects_vdc`](logical_aspects_vdc.md) — *Authors and venue pending verification.* FVDblTT — type theory of VDCs.
- `[Hou07]` [`reference/papers/linear_logic_no_units`](linear_logic_no_units.md) — Houston (PhD thesis, University of Manchester, 2007). Linear Logic Without Units — promonoidal categories as models for unitless multiplicative linear logic. (Old alias: `[Hou]`.)
- `[KvR20]` [`reference/papers/squier_hott`](squier_hott.md) — Kraus, von Raumer (LICS 2020). Coherence via Well-Foundedness (conference); extended journal version titled "A Rewriting Coherence Theorem with Applications in HoTT". (Old alias: `[KvR]`.)

## Sequent calculus

- `[Spi14]` [`reference/papers/dissection_of_l`](dissection_of_l.md) — Arnaud Spiwack (2014). A Dissection of L — each System L connective introduced separately. (Old alias: `[Spi]`.)
- `[BTMO]` [`reference/papers/grokking_sequent_calculus`](grokking_sequent_calculus.md) — Binder, Tzschentke, Müller, Ostermann. Grokking the Sequent Calculus (Functional Pearl) — accessible introduction to λμμ̃.

## Knowledge management (this is the rulebook)

- `[Sun26]` [`reference/papers/memx`](memx.md) — Sun (March 2026). MemX: a local-first hybrid retrieval system whose principles are canonicalized for serena in [`policy/memory_discipline`](../../policy/memory_discipline.md).

## Unix history (context)

- `[Rit77]` [`reference/papers/unix_retrospective`](unix_retrospective.md) — Dennis M. Ritchie (1977 Hawaii conference presentation). The UNIX Time-sharing System — A Retrospective. (Old alias: `[Rit]`.)

## Where the rules live

- [`policy/memory_discipline`](../../policy/memory_discipline.md) — the canonical principles document for serena memory organization

## When to consult

- "What does the EAct calculus say about X?" → `reference/papers/eact` (full paper at `~/gist/safe-actor-programming-with-multiparty-session-types/`)
- "Is this composition multiparty-compatible?" → `reference/papers/forwarders`
- "What are the laws for monadic lenses?" → `reference/papers/profunctor_optics` Proposition 4.7
- "Why is (+,−) non-associative?" → `reference/papers/duploids`
- "How does Segal compositionality work in a VDC?" → `reference/papers/fcmonads` §5
