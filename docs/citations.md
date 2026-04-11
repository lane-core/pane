# Pane Citations

Authoritative bibliography for code-level citations in pane. Every
`[Key]` token in `//!` / `///` doc comments across `crates/*/src/`
must resolve to an entry in this file. `just cite-lint` enforces
resolution mechanically; semantic correctness is the reviewer's
and formal-verifier's responsibility (see `STYLEGUIDE.md`
§"Code Documentation and Citation Standard").

Entries are formatted in ACM citation style. Entries marked
`NEEDS BACKFILL` are awaiting verification of venue, year, or
DOI against the primary source; pane code should not cite those
keys until the entry is resolved.

## How to cite

**Canonical form:** `[AuthorYY]` — author initials + two-digit
year where known, e.g. `[JHK24]` for Jacobs, Hinrichsen, and
Krebbers 2024. For single-author papers, use a short author
key like `[Hou]` or `[Rit]`. Entries without resolved author
metadata use a descriptive acronym flagged `NEEDS BACKFILL`.

**Aliases:** an entry may list short-name aliases for places
where the short name reads more clearly in context.
`[LinearActris]` and `[dlfactris]` both resolve to `[JHK24]`.
The author+year form is the recommended default in code doc
comments; use aliases sparingly.

**In code doc comments:** write the key inline with the claim
it supports, not as an ornament:

```rust
/// Realizes the E-Suspend rule from EAct §3.2: installs a
/// one-shot continuation keyed on (connection, token),
/// consumed by `fire_reply`. Drop compensation on the
/// installed entry fires the failure continuation when the
/// connection dies. Reference: [FH] §3.2; [JHK24] §1 on the
/// affine-plus-terminal-token encoding of linearity.
pub fn insert(...) { ... }
```

## Bibliography index

| Key | Authors | Year | Short title |
|---|---|---|---|
| `[AGP]` | Pischke, Masters, and Yoshida | — | Asynchronous Global Protocols, Precisely |
| `[BG]` | Nordvall Forsberg and Gibbons | — | Don't Fear the Profunctor Optics |
| `[BTMO]` | Binder, Tzschentke, Müller, and Ostermann | — | Grokking the Sequent Calculus |
| `[CBG24]` | Clarke, Elkins, Gibbons, Loregian, Milewski, Pillmore, and Román | 2024 | Profunctor Optics: A Categorical Update |
| `[CMS]` | Carbone, Marin, and Schürmann | — | A Logical Interpretation of Asynchronous Multiparty Compatibility |
| `[CS10]` | Cruttwell and Shulman | 2010 | A Unified Framework for Generalized Multicategories |
| `[DHMV]` | Dal Lago, Heindel, Mazza, and Varacca | — | Computational Complexity of Interactive Behaviors |
| `[DY]` | Deniélou and Yoshida | — | Multiparty Compatibility in Communicating Automata |
| `[FH]` | Fowler and Hu | — | Safe Actor Programming with Multiparty Session Types |
| `[FVDblTT]` | *NEEDS BACKFILL* | — | Logical Aspects of Virtual Double Categories |
| `[Hou07]` | Houston | 2007 | Linear Logic Without Units |
| `[JHK24]` | Jacobs, Hinrichsen, and Krebbers | 2024 | Deadlock-Free Separation Logic |
| `[KvR20]` | Kraus and von Raumer | 2020 | Coherence via Well-Foundedness (conf.) / A Rewriting Coherence Theorem with Applications in HoTT (ext.) |
| `[MMM]` | Mangel, Melliès, and Munch-Maccagnoni | 2026 | Classical Notions of Computation and the Hasegawa-Thielecke Theorem |
| `[MMSZ]` | Majumdar, Mukund, Stutz, and Zufferey | — | Generalising Projection in Asynchronous Multiparty Session Types |
| `[Rit77]` | Ritchie | 1977 | The UNIX Time-sharing System — A Retrospective |
| `[Spi14]` | Spiwack | 2014 | A Dissection of L |
| `[Sun26]` | Sun | 2026 | MemX: A Local-First Long-Term Memory System |
| `[TCP11]` | Toninho, Caires, and Pfenning | 2011 | Dependent Session Types via Intuitionistic Linear Type Theory |
| `[UD]` | Ueno and Das | — | Practical Refinement Session Type Inference |

**Key renames folded in from 2026-04-11 backfill pass:**

- `[AGP]` — kept (descriptive key; authors confirmed but year
  still unverified)
- `[DHMV]` — replaces `[IC]` (author initials now known)
- `[DY]` — replaces `[MPA]` (author initials now known; old
  anchor text speculated Lange and Tuosto — wrong, actual
  authors are Deniélou and Yoshida)
- `[Hou07]` — replaces `[Hou]` (year now confirmed)
- `[KvR20]` — replaces `[KvR]` (year now confirmed)
- `[MMM]` — replaces `[MMM25]` (the published PACMPL version
  is 2026, not 2025; key dropped the suffixed year since the
  paper has both a 2025 preprint and a 2026 publication and
  the authors' own short form is "MMM")
- `[MMSZ]` — replaces `[ProjMPST]` (author initials now known)
- `[Rit77]` — replaces `[Rit]` (year now confirmed)
- `[Spi14]` — replaces `[Spi]` (year now confirmed)
- `[UD]` — replaces `[RST]` (author initials now known; year
  still unconfirmed)

Because no pane code cites any of these keys yet, the rename
is free. Serena `citation_key:` frontmatter fields will be
updated alongside.

## Entries

Each entry gives the full citation in ACM style followed by an
annotation describing what the reference contributes to pane.
The annotation is the substance of the citation — it answers
"why is this reference in pane's bibliography?" An entry whose
annotation does not name a specific thing pane draws from the
reference is dead weight and should be removed.

---

### `[AGP]`

Kai Pischke, Jake Masters, and Nobuko Yoshida. *Asynchronous
Global Protocols, Precisely.* *NEEDS BACKFILL: venue, year,
DOI.* (LaTeX source uses Elsevier CAS class — may be a journal
submission in preparation.)

**Aliases:** *(none)*

Global session types in the asynchronous setting with
realizability conditions for when a global protocol
description can be implemented as a set of independent
endpoints. Asynchrony introduces reordering possibilities that
do not exist in synchronous settings; some global protocols
become unrealizable once messages can be in flight
simultaneously. Pane uses this as background for its
`FrameCodec` design — strict per-channel ordering at the
framing layer is what keeps the realizability story honest
under the six-phase batch discipline.

---

### `[BG]`

Fredrik Nordvall Forsberg and Jeremy Gibbons. *Don't Fear the
Profunctor Optics.* *NEEDS BACKFILL: venue, year, URL.*
Three-part tutorial series distributed as markdown in the
`DontFearTheProfunctorOptics` repository; no formal
publication venue.

**Aliases:** `[DontFear]`

Accessible three-part introduction to profunctor optics. Part
1 covers monomorphic / polymorphic / van Laarhoven lenses;
part 2 introduces profunctors and the four typeclasses
(`Strong`, `Choice`, `Closed`, `Traversing`); part 3 unifies
them under the profunctor encoding. Pane's `MonadicLens` lives
in the cartesian-decomposition corner of this hierarchy
(affine-traversal shape; concrete encoding via function
pointers rather than rank-2 polymorphism). The
optics-theorist reads this first when consulted, before
descending into the formal treatment in `[CBG24]`.

---

### `[BTMO]`

David Binder, Marco Tzschentke, Marius Müller, and Klaus
Ostermann. *Grokking the Sequent Calculus (Functional Pearl).*
*NEEDS BACKFILL: venue, year, DOI.*

**Aliases:** `[Grokking]`

Programmer-facing introduction to λμμ̃. The paper compiles a
small functional language (Fun) to a sequent-calculus core
(Core), surfacing the data/codata duality, direct vs indirect
consumers, and the ⊕/⅋ error duality with clean operational
semantics. Pane touchpoints: data (`ServiceFrame` variants) vs
codata (`Handles<P>` destructors); error-handling conventions
as De Morgan duals; the sequent-calculus framing of the shell
integration work planned for Phase 2+.

---

### `[CBG24]`

Bryce Clarke, Derek Elkins, Jeremy Gibbons, Fosco Loregian,
Bartosz Milewski, Emily Pillmore, and Mario Román. 2024.
*Profunctor Optics, a Categorical Update.* *Compositionality*
6, 1 (2024). *NEEDS BACKFILL: exact article DOI or URL at
compositionality-journal.org.* Accepted 2023-03-06; arXiv
preprint <https://arxiv.org/abs/2001.07488>.

**Aliases:** `[CBG]`, `[MixedOptics]`

The formal paper on profunctor optics. Defines the optic
hierarchy via Tambara modules, proves the representation
theorem (concrete optics are isomorphic to profunctor
optics), introduces mixed optics (different categories on the
view side and the set side), and characterises monadic
lenses. Pane cites **Definition 4.6** for `MonadicLens`,
**Proposition 4.7** for the mixed-optic characterisation
`MndLens_Ψ ≅ Optic_(×, ⋊)` (view side in the cartesian
category W; set side in the Kleisli category Kl(Ψ) over a
writer monad), and the representation theorem justifying the
concrete encoding in `pane-proto/src/monadic_lens.rs`.

---

### `[CMS]`

Marco Carbone, Sonia Marin, and Carsten Schürmann. *A Logical
Interpretation of Asynchronous Multiparty Compatibility.*
*NEEDS BACKFILL: venue, year, DOI.* LaTeX source uses Springer
LLNCS class — likely a conference proceedings paper but venue
and year not confirmed from the examined source.

**Aliases:** `[Forwarders]`

Frames multiparty compatibility through linear logic. The
**forwarder** construction — a process `fwd a b` that links
two channels — captures all multiparty-compatible compositions
and preserves cut-elimination. The central theorem (§5.1)
proves that cut on forwarder chains reduces cleanly: a chain
of intermediaries between a sender and ultimate recipient
does not deadlock if each link is forwarder-correct. Pane's
`ProtocolServer` is an operational dynamic forwarder for
chained request/reply routing; the theorem licenses the
chained regime but does not cover any-to-any addressing.

---

### `[CS10]`

G. S. H. Cruttwell and Michael A. Shulman. 2010. A Unified
Framework for Generalized Multicategories. *Theory and
Applications of Categories* 24, 21 (2010), 580–655.
<http://www.tac.mta.ca/tac/volumes/24/21/24-21abs.html>

**Aliases:** `[fcmonads]`, `[VDC]`

Mathematical foundation for **virtual double categories
(VDCs)** and their generalised multicategories. Defines VDCs
as objects with vertical and horizontal arrows plus cells
(§3), composites via the Segal condition (§5), restrictions
as interface transformations (§6), and virtual equipments
(§7). Pane's session-typed channels are horizontal arrows;
handlers are vertical arrows; dispatch events are cells. The
Segal condition is the compositionality property that the
six-phase batch ordering in `architecture/looper` implicitly
checks.

---

### `[DHMV]`

Ugo Dal Lago, Tobias Heindel, Damiano Mazza, and Daniele
Varacca. *Computational Complexity of Interactive Behaviors.*
*NEEDS BACKFILL: venue, year, DOI.* Appears to be a preprint
or unsubmitted version in the examined source.

**Aliases:** `[IC]`

Classifies the computational complexity of session-typed
interactive behaviours. Some compositions are PTIME-checkable;
others are EXPTIME or undecidable. Background for keeping
pane's protocols in the PTIME-checkable subset and the formal
excuse on I2/I3 (the halting-problem hedge on handler
termination).

---

### `[DY]`

Pierre-Malo Deniélou and Nobuko Yoshida. *Multiparty
Compatibility in Communicating Automata: Characterisation and
Synthesis of Global Session Types.* *NEEDS BACKFILL: venue,
year, DOI.* LaTeX source uses an ICALP class template —
likely ICALP proceedings, year unconfirmed.

**Aliases:** `[MPA]`

Automata-theoretic perspective on multiparty session-type
compatibility. Defines compatibility as a property of the
synchronous product of communicating automata and
characterises when asynchronous behaviour preserves it.
Complements `[CMS]` (the linear-logic perspective) with a
different formal apparatus for the same property. Background
for the session-type consultant when asked whether a proposed
protocol is decidably compatible, and for the state-explosion
characterisation when verifying multi-party protocols
mechanically.

---

### `[FH]`

Simon Fowler and Raymond Hu. *Speak Now: Safe Actor Programming
with Multiparty Session Types* (Extended Version). The
underlying calculi are named **Maty** (base) and **Maty_zap**
(with failure handling). *NEEDS BACKFILL: published venue —
check ICFP / PACMPL / OOPSLA.* Artifact DOI:
<https://doi.org/10.5281/zenodo.18792000> (2026).

**Aliases:** `[EAct]`, `[Maty]`

Defines the EAct calculus — a typed actor language with
multiparty session types. Proves preservation (Theorem 4 §3.3),
progress (Theorem 5 §3.3), and Corollary 1 *Global Progress*
§3.3 under thread-terminating configurations. Under failure,
the extended calculus proves preservation (Theorem 6),
progress (Theorem 7), and global progress (Theorem 8) in §4.
Key reduction rules: E-Send, E-React, E-Suspend, E-Init
(§3.2); E-RaiseS, E-CancelMsg, E-CancelH, E-InvokeM (§4). Key
principles KP1–KP5 (§1.3), including "no explicit channels"
(KP2) — which pane deliberately diverges from via
`ServiceHandle<P>`. Pane's actor model is grounded in EAct;
the nineteen invariants in `status` map to EAct safety
conditions. Use `[FH]` for general citations; the serena
memory `reference/papers/eact_sections` is the theorem locator
for pointing at specific rules and theorems.

---

### `[FVDblTT]`

*NEEDS BACKFILL: authors, title, venue, year.* The serena
memory (`reference/papers/logical_aspects_vdc`) tentatively
lists "Hayashi, Das, et al." but the 2026-04-11 backfill pass
could not verify any bibliographic metadata from the primary
source (gist is technical content without a clear document
header). The paper defines the type theory of virtual double
categories, FVDblTT.

**Aliases:** *(none)*

Defines FVDblTT — the type theory of VDCs. Introduces protypes
as channel types (horizontal arrows), restrictions as
interface transformations on channel types (the
type-theoretic counterpart of `[CS10]` §6), and comprehension
types as observation of protocol state. Background reference
for the session-type consultant when reasoning about pane's
channel composition rules at the type-theoretic level rather
than the categorical level in `[CS10]`.

---

### `[Hou07]`

Robin Houston. 2007. *Linear Logic Without Units.* PhD
thesis. University of Manchester, submitted 30 September 2007.

**Aliases:** `[Hou]`

PhD thesis on promonoidal categories as models for unitless
multiplicative linear logic. The motivation: full MLL's
multiplicative units (1 and ⊥) complicate the categorical
semantics; removing them yields a cleaner theory for systems
where the unit is not load-bearing. Promonoidal structure
generalises monoidal structure by allowing the tensor to be a
profunctor rather than a functor — composing two things
becomes a relation rather than necessarily a function. Pane's
session types are unitless (there is no "do nothing" protocol
step); background for the session-type consultant when
proposals involve "the empty session" or unit-like channel
states.

---

### `[JHK24]`

Jules Jacobs, Jonas Kastberg Hinrichsen, and Robbert Krebbers.
2024. Deadlock-Free Separation Logic: Linearity Yields
Progress for Dependent Higher-Order Message Passing. *Proc.
ACM Program. Lang.* 8, POPL, Article 47 (January 2024), 33
pages. <https://doi.org/10.1145/3632889>

**Aliases:** `[LinearActris]`, `[dlfactris]`

Introduces **LinearActris**, a *linear* concurrent separation
logic for message-passing concurrency. LinearActris amends
Actris (which is affine) with linearity restrictions
sufficient to prove a global-progress adequacy theorem
(**Theorem 1.2**): a proof of `{Emp} e {Emp}` in LinearActris
implies `e` enjoys global progress — every reachable
configuration either has all threads as values with an empty
heap, or the configuration can step as a whole. Two
ingredients are necessary, and both fail in Actris alone:
(1) **linearity**, ruling out the case where a thread abandons
a send/receive obligation and leaves its peer waiting forever;
(2) **acyclicity of the connectivity graph**, ruling out
cross-wait cycles formed by linear threads holding endpoints
of different channels in opposite order. Pane cites this for
the star-topology acyclicity argument in
`decision/server_actor_model`, for the
affine-plus-closure-capability encoding of linearity that
`ReplyPort::drop` realizes, and for the design margin on
Watch/PaneExited as a one-shot terminal reverse edge that does
not violate acyclicity. **Not to be confused with** Rumpsteak
(Cutner, Yoshida, Vassor; PACMPL 2022), which is a different
paper about asynchronous subtyping for message reordering via
k-multiparty-compatibility model checking.

---

### `[KvR20]`

Nicolai Kraus and Jakob von Raumer. 2020. Coherence via
Well-Foundedness. In *35th Annual ACM/IEEE Symposium on Logic
in Computer Science (LICS '20)*. Association for Computing
Machinery, New York, NY, USA, 662–675. *NEEDS BACKFILL: exact
conference-paper DOI.* The extended journal version is titled
*A Rewriting Coherence Theorem with Applications in Homotopy
Type Theory* (venue and DOI unverified; examined source is
the extended version).

**Aliases:** `[KvR]`

Proves Squier's theorem (rewriting systems and coherence) in
homotopy type theory. The key insight: rewriting steps are
cells; local confluence at the cell level generates higher
coherence cells; global coherence follows from local
confluence plus termination. Pane's looper has multiple
resolution mechanisms (polarity frames, CBV focusing, signal
precedence); local confluence at each pair would make the
whole dispatch coherent. The six-phase batch ordering is the
termination side of the theorem. Not currently load-bearing
for a specific pane claim; kept as reference for future
reasoning about compositional coherence.

---

### `[MMM]`

Éléonore Mangel, Paul-André Melliès, and Guillaume
Munch-Maccagnoni. 2026. Classical Notions of Computation and
the Hasegawa-Thielecke Theorem. *Proc. ACM Program. Lang.* 10,
POPL, Article 73 (January 2026).
<https://doi.org/10.1145/3776715>

A foundational companion paper is Munch-Maccagnoni 2014b,
*Models of a Non-Associative Composition*, FoSSaCS 2014,
<https://doi.org/10.1007/978-3-642-54830-7_26>, cited in this
bibliography as **MM14b** when the foundational duploid
definitions are the specific reference. The MMM paper uses
MM14b definitions; both contribute to pane's polarity
discipline.

**Aliases:** `[MMM25]`, `[duploids]`, `[MM14b]`

Defines **duploids** as non-associative polarised categories
that integrate call-by-value and call-by-name computation.
Three of four associativity equations hold; the **(+,−)**
equation fails, capturing the CBV/CBN distinction. Key results
pane cites: Proposition 6 (MM14b) — thunkable implies central
in any duploid; Hasegawa-Thielecke (MMM) — in a dialogue
duploid, central equals thunkable (the converse holds); the
shift operator ω_X mediating between positive (CBV) and
negative (CBN) subcategories; composition laws ((+,+) and
(−,−) associate; (+,−) does not). Pane's analysis as a
duploid: positive subcategory contains the wire types
(`ServiceFrame`, `ControlMessage`, `Message`); negative
subcategory contains handlers and demand-driven reads. The
server deadlock was a non-associative bracket realised
concurrently; the actor model prevents this by serialising
polarity crossings. `ActivePhase<T>` is the explicit shift
operator carrying negotiated state.

---

### `[MMSZ]`

Rupak Majumdar, Madhavan Mukund, Felix Stutz, and Damien
Zufferey. *Generalising Projection in Asynchronous Multiparty
Session Types.* *NEEDS BACKFILL: venue, year, DOI.* LaTeX
source uses Springer LLNCS template — likely a conference
proceedings paper but venue and year not confirmed.

**Aliases:** `[ProjMPST]`

Refines the projection operator from global session types to
local (per-endpoint) session types. The classical projection
rules are conservative; this paper widens what is projectable
without losing safety. Reference for designing protocols with
three or more parties (currently rare in pane — most are
bilateral pane↔server). Background for the C1 heterogeneous
session loop principle in
`analysis/session_types/principles`.

---

### `[Rit77]`

Dennis M. Ritchie. 1977. *The UNIX Time-sharing System — A
Retrospective.* Bell Laboratories, Murray Hill, New Jersey.
Presented at the Tenth Hawaii International Conference on the
System Sciences, Honolulu, January 1977. *NEEDS BACKFILL:
exact proceedings citation and DOI, if any.*

**Aliases:** `[Ritchie]`, `[Rit]`

Ritchie's retrospective on the design choices that shaped
early Unix. Covers the file system, the shell, processes,
the philosophy of small composable tools, and the trade-offs
made deliberately vs accidentally. Background for
understanding what Plan 9 was reacting to (and what Be was
reacting to in turn). Low retrieval frequency; cited by the
plan9-systems-engineer when explaining the historical
motivation for Plan 9's design choices. Note that there are
at least two distinct Ritchie retrospectives — the 1974 CACM
*The UNIX Time-Sharing System* and the 1984 BSTJ *The
Evolution of the UNIX Time-sharing System*; the vendored
source for pane is the 1977 Hawaii conference version, which
is yet another piece.

---

### `[Spi14]`

Arnaud Spiwack. 2014. *A Dissection of L.* *NEEDS BACKFILL:
venue and exact publication date.* Build metadata in the
source README dates the paper to April 2014; venue unconfirmed.

**Aliases:** `[Spi]`

Dissects System L (a presentation of classical sequent
calculus with explicit terms, coterms, and commands) into its
constituent parts. Each connective is introduced separately,
each rule justified, each polarity choice motivated. Useful
as a structural reference when the typing rule for a
particular sequent-calculus connective is needed. Pane's
three-sort structure (terms / coterms / commands) and λμμ̃
grounding trace back here.

---

### `[Sun26]`

Lizheng Sun. 2026. *MemX: A Local-First Long-Term Memory
System for AI Assistants.* Preprint, March 2026. No arXiv
identifier or external URL found in the source document.

**Aliases:** `[MemX]`

Local-first hybrid retrieval system for AI assistant memory,
implemented in Rust on libSQL. Key mechanisms: vector recall
(DiskANN) and keyword recall (FTS5) merged via Reciprocal
Rank Fusion with k=60; four-factor re-ranking (semantic 0.45,
recency 0.25 with 30-day half-life, frequency 0.05,
importance 0.10); z-score plus sigmoid normalisation;
low-confidence rejection (R1) when both keyword and vector
signals are weak; two-layer deduplication. Key empirical
findings: semantic density per record drives retrieval
quality (fact-level chunking doubles Hit@5 vs session-level
on LongMemEval); deduplication is data-dependent; R1
rejection is the only candidate with zero false negatives.
The principles ported to pane's serena memory live in
`policy/memory_discipline` — fact-level granularity,
frontmatter as manual reranker, query-organised index,
type-aware namespaces, write-once status, merge test for
duplicates, low-confidence rejection, access vs retrieval
separation.

---

### `[TCP11]`

Bernardo Toninho, Luís Caires, and Frank Pfenning. 2011.
*Dependent Session Types via Intuitionistic Linear Type
Theory.* In Proceedings of the 13th International ACM SIGPLAN
Symposium on Principles and Practices of Declarative
Programming (PPDP '11). Association for Computing Machinery,
New York, NY, USA, 161–172. ISBN 9781450307765.
<https://doi.org/10.1145/2003476.2003499>

**Aliases:** *(none)*

Extends session types with type-level dependence on prior
message contents — the protocol type can use values exchanged
earlier in the conversation. Built on intuitionistic linear
logic, preserving the linear discipline. §3 introduces the
dependent arrow notation `↪` for "the type of the next
message depends on the value just received." Pane's
`policy/ghost_state_discipline` was synthesized partly from
this paper — dependent session types formalize what pane
does well in places (`ReplyPort`, `TimerToken`) and reveal
where it does not yet (`CompletionReplyPort` token
correlation). The `↪` notation is borrowed for pane's
protocol documentation.

---

### `[UD]`

Toby Ueno and Ankush Das. *Practical Refinement Session Type
Inference* (Extended Version). *NEEDS BACKFILL: venue, year,
DOI.* Source filename `esop25.tex` suggests ESOP 2025 but
venue metadata not confirmed from the document header.

**Aliases:** `[RST]`

Combines refinement types (predicates on values) with session
types (protocols on channels). Inference algorithm makes the
combination usable in practice. A lighter-weight alternative
to full dependent session types for value-constrained
protocols. Background reference for proposals to add
value-level constraints to pane's typed protocols (not yet
adopted; candidate for Phase 2 `session_id` refinement).

---

## Maintenance

**Adding an entry:**

1. Cross-check the primary source for the full author list
   (in the order the source gives), the exact title, the
   venue, the year, and a DOI or stable URL if one exists.
2. Pick a canonical key (author initials + year for
   multi-author papers; short author key for single-author;
   descriptive acronym for entries without resolved metadata,
   flagged `NEEDS BACKFILL` until an author can be confirmed).
3. Add a row to the bibliography index and a full entry below
   in ACM citation style.
4. Add the key to the corresponding serena
   `reference/papers/<name>` memory's `citation_key:`
   frontmatter field so agents can resolve from the key back
   to the serena memory via frontmatter grep.
5. The entry's annotation must describe what the reference
   contributes to pane specifically. If you cannot write such
   a paragraph, the entry does not belong in the bibliography.

**Resolving a `NEEDS BACKFILL` marker:** read the primary
source (the paper PDF, the preprint, the published journal
edition), extract the full bibliographic data exactly as the
source gives it, and replace the marker. Do not guess author
identities, years, or DOIs from the short title or from memory.
Principle 10 (epistemic strength matches the source) applies to
bibliography entries as much as to any other citation — better
to leave the marker than to hallucinate.

**Removing an entry:**

1. Run `just cite-lint --by-key <Key>` to list every file
   citing the key. If any hits exist, do not remove — either
   rename the citations or pick a different entry to retire.
2. Remove the bibliography-index row and the full entry.
3. Remove the `citation_key:` field from the corresponding
   serena memory (or retire the memory altogether if the paper
   is no longer referenced anywhere).

**Running the mechanical audit:**

```
just cite-lint
```

`cite-lint` resolves every `[Key]` token in `//!` / `///` doc
comments across `crates/*/src/` against this file. A green
`cite-lint` is necessary but not sufficient — see
`STYLEGUIDE.md` §"Mechanical vs semantic audit". The
formal-verifier owns the semantic side of the audit.
