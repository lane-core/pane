---
type: policy
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [citation, citation_key, bibliography, code_documentation, module_doc, function_doc, cite_lint, semantic_audit, mechanical_audit, principle_10, tier_2_audit]
related: [policy/memory_discipline, policy/agent_workflow, policy/heritage_annotations, policy/refactor_review_policy, reference/papers/_hub]
agents: [all]
---

# Code citation standard

## Rule

Every `[Key]` token in pane's `//!` and `///` doc comments must
resolve to an entry in `docs/citations.md` (the authoritative
bibliography). Citations are implementation attributes: they
live with the code that draws on the reference, they move with
refactors, they are deleted when the code is deleted. The full
standard is in `STYLEGUIDE.md` §"Code Documentation and
Citation Standard".

## Why

Code that rests on theoretical foundations needs to name the
foundations inline. The reader of a function should be able to
answer "why is this designed this way?" without leaving the
code. Credit the authors, enable traceability, support
verifiability.

Without the standard, three failure modes appear:

1. **Implicit ownership of ideas.** Code that realizes a paper
   concept without citing it reads as though pane invented the
   idea. That's dishonest and makes future reviewers
   reinterpret the design from scratch.
2. **Dead-weight citations.** Code that cites references it
   does not actually draw from (for prestige or background)
   pollutes the audit trail. Every `[Key]` must be a testable
   claim.
3. **Stale citations after refactors.** Code moves; citations
   don't automatically follow. Without a discipline, refactors
   leave citations pointing at constructs the function no
   longer realizes.

## How to apply

**Authors and implementers (writing docs):**

1. **Before citing**, check `docs/citations.md` for the
   relevant entry. If the reference you want to cite isn't
   there, add an entry first (with full author / year / title /
   annotation), then cite from the code.
2. **At the module level** (`//!`), document the module's role
   first, then list references in a `# References` section at
   the end. Module-level citations are for references that
   shaped the module's architecture.
3. **At the function level** (`///`), cite only references that
   informed *this function's* implementation specifically. If
   the reference informed the module but not the function,
   don't repeat the citation; the module doc has it.
4. **Inline form:** `[Key]` followed by section or theorem
   number where applicable. Example:
   `/// Realizes E-React ([FH] §3.2).`
5. **Preserve epistemic strength** (Principle 10 in
   `policy/memory_discipline`). If the source hedges, the
   citation hedges. "Structurally analogous to [X]" ≠
   "realizes [X]". Don't strengthen the source.

**Reviewers (refactoring or auditing):**

1. **Run `just cite-lint`** before commit on any branch that
   touches doc comments. Mechanical check: every `[Key]`
   resolves; no unused entries. **Necessary but not sufficient.**
2. **For any function with a citation that you touched during
   the refactor**, re-read the cited reference and verify the
   citation still describes the code accurately. Leave a
   reviewer comment confirming the check — "citations
   re-verified against `[JHK24]` §1 and `[FH]` §3.2." Green
   cite-lint cannot substitute for this.
3. **If a citation no longer applies**, remove it. If the
   function now draws on a different reference, update it. If
   you are unsure, flag with `// TODO: citation needs review`
   and escalate to formal-verifier.

**Formal-verifier (audit runs, per `policy/agent_workflow`
§"Tier-2 audit for theoretical anchors"):**

1. `just cite-lint` as a pre-filter — catches trivial breakage
   upfront so audit attention goes to the semantic questions.
2. Re-read each cited reference against the implementation
   claim. Per-citation verdict: CLEAN / MINOR / MAJOR.
3. Check epistemic strength — Principle 10 applies to code
   doc citations as much as to memory paraphrases.
4. Report findings as a drift report per Step 5 of the agent
   workflow.

## Key format

**Canonical:** `[AuthorInitialsYY]` — e.g. `[JHK24]` for
Jacobs/Hinrichsen/Krebbers 2024. Single-author papers use a
short author key: `[Hou]`, `[Rit]`, `[Spi]`. Anchors without
resolved author metadata use a descriptive acronym, flagged
`NEEDS BACKFILL` in the bibliography entry until an author can
be confirmed.

**Aliases:** An entry may list short-name aliases for places
where the short name reads more clearly in context. `[JHK24]`,
`[LinearActris]`, and `[dlfactris]` all resolve to the same
bibliography entry. `cite-lint` normalizes aliases to the
canonical form before checking. Prefer author+year in code;
use short-name aliases sparingly where context makes them
clearer than initials.

## Mechanical vs semantic audit

`just cite-lint` catches: typos, renamed keys, removed papers,
unused entries, alias conflicts.

`just cite-lint` does NOT catch: stale citations on refactored
functions, wrong paper attribution, epistemic strength
violations, semantic drift.

**Green cite-lint is necessary but not sufficient.** The
semantic audit is the only path to confidence; it is the
formal-verifier's job and is enforced by
`policy/refactor_review_policy` as a two-step discipline
(mechanical pre-filter + human re-read).

Treat `cite-lint` as `clippy` for citations. It keeps the
syntactic layer clean so reviewers can concentrate on the
semantic layer. A passing lint is not evidence of correct
citations — it is evidence that the substantive audit is not
being blocked by trivial noise.

## Relation to existing policies

- **`policy/memory_discipline`** §10 (Principle 10 — Epistemic
  strength matches the source) — same principle applied to
  memory paraphrases rather than code doc citations. Both are
  enforced.
- **`policy/heritage_annotations`** — specialization of this
  standard for Be/Haiku and Plan 9 source citations
  (`src/kits/app/Looper.cpp:1162` form). Those are source-code
  citations, not paper citations; keep the specialized format
  for source-code lineage and use `[Key]` form only for
  theoretical / paper references.
- **`policy/agent_workflow`** §"Tier-2 audit for theoretical
  anchors" — the formal-verifier's audit procedure now covers
  both memory-paraphrase audits (existing) and code-citation
  audits (added 2026-04-11).
- **`policy/refactor_review_policy`** — the two-step discipline
  on refactors is documented there; this policy is the rule,
  that policy is the enforcement mechanism.

## Authoritative sources

- **Bibliography:** `docs/citations.md` — canonical entries,
  full author list, annotations.
- **Serena cross-links:** each `reference/papers/<name>` memory
  carries a `citation_key:` frontmatter field matching the
  bibliography key. Three-way redundancy: frontmatter grep,
  bibliography table, `reference/papers/_hub` entry prefix.
- **STYLEGUIDE.md** — full discipline for code authors,
  including templates for module-level and function-level docs.
