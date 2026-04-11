---
type: policy
status: current
supersedes: [pane/agent_workflow, auto-memory/feedback_agent_workflow]
created: 2026-04-05
last_updated: 2026-04-10
importance: high
keywords: [agent_workflow, four_design_agents, plan9, be, optics, session-type, pane-architect, formal-verifier]
agents: [all]
---

# Agent Workflow for Pane Development

Standard process for new subsystems and significant changes. For
small fixes, the task completion checklist is sufficient.

## Step 1: Four design agents in parallel

Launch ALL FOUR for every new design question:

- **plan9-systems-engineer** — Plan 9 namespace model, 9P protocol,
  /proc and rio precedent, distributed state
- **be-systems-engineer** — BeOS/Haiku API design, BLooper /
  BHandler / BMessage, hey scripting, app_server
- **optics-theorist** — profunctor optics theory, lens / prism /
  traversal design, law verification, Clarke et al.
- **session-type-consultant** — session type safety, linear / affine
  types, protocol soundness, invariant analysis

**Why all four:** Each brings a different lens. Plan 9 provides the
namespace / protocol model. Be provides the empirical API design
experience. Optics provides the state projection formalism. Session
types provide the safety analysis. Skipping one leaves a blind spot.

## Step 2: Synthesize and present to Lane

Distill agent findings into a unified summary with open questions.
Lane refines, resolves conflicts, makes decisions.

### Follow-up rounds (as needed)

If open questions remain after Lane's refinement, launch another
consultation round. Follow-up rounds may be **targeted** — not every
question needs all four agents:

- Structural / type design questions → all four
- Domain-specific questions → the 1–2 agents with relevant expertise

Iterate until the design converges. Three rounds was typical for
PeerAuth (initial → accessor design → canonicalization). One round
is fine for simpler features.

### Design agents should note Rust-specific implications

When a design recommendation includes Rust attributes
(`#[non_exhaustive]`, `#[must_use]`, etc.) or patterns with
ergonomic consequences, agents should state the practical API
impact. The pane-architect will catch these regardless, but
surfacing them during design avoids surprises.

## Step 3: pane-architect implements

Writes Rust code faithful to project foundations. **One task per
dispatch, review between — not bulk.** Runs the task completion
checklist.

**Why not general-purpose:** pane-architect reads the foundations
docs, checks naming conventions, and writes code faithful to the
project's theoretical grounding. A general-purpose agent doesn't
have this context.

## Step 4: formal-verifier validates and writes tests

Audits implementation against architecture spec invariants
(I1–I13, S1–S6). **Writes tests for every gap found** — the
verifier is the subject matter expert on what invariants need
testing.

If a test cannot be written because the protocol or design is
incomplete, **escalate the design question to Lane** before
proceeding. Do not skip the test, write a workaround, or defer
it to a report. The inability to test an invariant is a design
signal, not a test infrastructure problem.

**Must include a doc drift report:** grep the old type / API
syntax across `docs/` and serena memories, report every hit with
file:line. This turns manual scavenger hunts into checklists. The
formal-verifier flagging "docs are stale" without enumerating
locations is insufficient — Step 5 needs a concrete list to work
from.

## Step 5: Memory and doc freshness

After validation passes:

- Fix every doc drift location from the formal-verifier's report.
- Update `status` if crate structure, test count, or phase status
  changed.
- Update `PLAN.md` — mark completed items, add discovered work.
- Commit results per CLAUDE.md commit format.

## Rules

- Don't skip any of the four design agents in the initial round
- Don't substitute general-purpose agents for any named agent
- Follow-up rounds can use a targeted subset of agents
- Design agents run in parallel; steps 3–5 run sequentially
- Step 5 is not optional — stale memory is a bug
- pane-architect: one task per dispatch, review between dispatches
- formal-verifier: writes tests, escalates design gaps — does not
  defer them

## Tier-2 audit for theoretical anchors

**Mandatory** before any new `analysis/<concept>.md` (or
`analysis/<cluster>/<spoke>.md`) theoretical concept anchor that
cites external papers, framework sections, primary-source code, or
vendored references is treated as authoritative for cross-agent
retrieval.

The four-design-agent consultation (Step 1) checks that you read
the right material *before* writing. The tier-2 audit checks that
the memo's *paraphrases of cited material* are faithful to the
source *after* writing. The two are independent; you need both.

**Why mandatory.** Psh ran this procedure on its 2026-04-11 tier-1
anchor batch (22 memos). A manual spot-check by the writing agent
caught 3 hallucinations; a follow-up dispatch of 5 domain agents
caught 1 MAJOR + 6 MINOR more — a 1:2 ratio of self-caught to
agent-caught, even with the writer trying to be careful. The
discipline of catching errors at write time is unreliable; the
discipline of catching them at audit time, by a different agent
reading the sources fresh, is the practical floor on accuracy.

**Procedure.**

1. Write the anchors. Cite refs and §pointers as you go. Each
   anchor tags its primary source in
   `verified_against: [<source>@<date>]` and its cited papers in
   `related: [reference/papers/<paper>]`.
2. Identify the domain agent(s) whose scope covers each anchor:

   | Anchor topic | Auditor |
   |---|---|
   | duploids, VDCs, composition laws, polarity | optics-theorist or formal-verifier |
   | profunctor optics, MonadicLens, accessors, traversal laws | optics-theorist |
   | session types, multiparty, coprocess, wire format, sub-protocol typestate | session-type-consultant |
   | Plan 9 heritage, 9P, namespace, rio / plumber precedent | plan9-systems-engineer |
   | BeOS heritage, Haiku, BLooper, BMessage, scripting protocol | be-systems-engineer |
   | Rust implementation claims (types, lifetimes, ownership) | pane-architect |
   | anchor-audit runs, spec fidelity, invariant coverage | formal-verifier |

3. Dispatch the auditor agents in parallel. Each gets a brief
   listing the anchors it owns and the verification rules:

   - Every §pointer or line number is real and points to material
     on the claimed topic.
   - Every substantive claim traces to a specific passage in the
     cited reference.
   - Epistemic strength matches the source (per
     `policy/memory_discipline` §10).
   - No invented narrative, symptoms, examples, or details.
   - Output: per-anchor verdict (CLEAN / MINOR / MAJOR) with
     quotes from source vs anchor when divergent.

4. Fold corrections. Update each corrected anchor's
   `verified_against:` frontmatter to record the audit date and
   the agent that verified it. Bump `last_updated:` to merge
   time.
5. Only after the audit pass and folded corrections may the
   anchors be marked authoritative for cross-agent retrieval.

**Relation to `analysis/verification/`.** The verification cluster
audits implementation against invariant enumeration (I1–I13,
S1–S6) — code and spec fidelity. The tier-2 audit audits memo
paraphrase against primary-source epistemic strength — text
fidelity. Different kinds of audit, same discipline root. The
formal-verifier owns both because both end in a verdict table
plus correction list; the other domain agents audit within their
scope of expertise.

**Out of scope for the audit.** Upstream issues found in the
primary source (a typo in `docs/architecture.md`, a mis-citation
in a paper anchor, a stale claim in `architecture/<crate>`) are
flagged in the audit report and routed to Lane for separate
resolution. The tier-2 audit fixes the anchors; source documents
get their own review pass.

**Skipping the audit.** Permitted only for tier-3 anchors, short
corrections that don't introduce new citations, or anchors that
cite only pane's own materials (`docs/architecture.md`,
`architecture/<crate>`, `decision/<topic>`). Any new external
paper citation triggers the audit on the next pass.

## Tier-2 code citation audit

**Parallel to the anchor audit above** but triggered by code
changes rather than memo writes. Applies to `[Key]`-form
citations in `//!` and `///` doc comments inside
`crates/*/src/`. Standard: `policy/code_citation_standard`.
Bibliography: `docs/citations.md`.

**Triggered by:**

- Any PR that touches a module or function with an existing
  `[Key]` citation (refactor review).
- Any PR that adds a new citation.
- Periodic audit pass run by formal-verifier (quarterly or
  when `status` notes significant architectural drift).

**Two-step procedure (mandatory for both).**

1. **Mechanical pre-filter.** Run `just cite-lint`. Catches
   typos, renamed keys, removed papers, unused bibliography
   entries, alias conflicts. Failure → fix trivial breakage
   before proceeding. **Green cite-lint is necessary but not
   sufficient.**
2. **Semantic audit.** Re-read every cited reference against
   the implementation claim. For each `[Key]` in the touched
   code, the auditor (reviewer for PR-level, formal-verifier
   for periodic) verifies:
   - The cited reference is actually a reference the function
     draws from (not a background citation).
   - The inline phrasing respects epistemic strength
     (`policy/memory_discipline` §10 — Principle 10). "Realizes"
     is stronger than "structurally analogous to"; the citation
     must not strengthen what the source says.
   - If the code was refactored, the function still implements
     the cited construct. A stale citation on a refactored
     function is worse than a missing one.
   - The citation's cited section / theorem number (e.g.,
     `[FH] §3.2`) is a real section in the source and covers
     the claim.

**Reviewer comment template.** On PRs with code citations, the
reviewer leaves an explicit comment confirming the semantic
check:

> Citations re-verified against `[JHK24]` §1 (LinearActris
> affine-plus-closure-capability encoding matches
> `ReplyPort::drop`) and `[FH]` §3.2 (E-Suspend / E-React match
> `Dispatch<H>::insert` / `fire_reply`). No drift.

**Do not substitute `cite-lint` green for the semantic check.**
CI green proves the keys resolve; it does not prove the
citations are correct. The reviewer comment is the evidence
that the semantic audit happened.

**Auditor mapping.** Same table as the anchor audit above:

| Citation topic | Auditor |
|---|---|
| duploids, VDCs, composition laws, polarity | optics-theorist or formal-verifier |
| profunctor optics, MonadicLens, accessors | optics-theorist |
| session types, multiparty, coprocess, wire format | session-type-consultant |
| Plan 9 heritage, 9P, namespace | plan9-systems-engineer (heritage annotations use `path:line` form per `policy/heritage_annotations`) |
| BeOS heritage, Haiku, BLooper, BMessage | be-systems-engineer (same) |
| Rust implementation claims | pane-architect |
| anchor-audit runs, spec fidelity, invariant coverage, periodic citation audit | formal-verifier |

**Relation to the anchor audit.** Both audits check paraphrase
fidelity against primary sources; they differ in which artifact
is being audited (memo vs code doc comment). A single agent
dispatch can cover both if the touched PR includes memory
changes and code changes — the auditor applies the same
discipline to both. In practice, refactor-review-policy
triggers the code citation audit; tier-2 anchor audit triggers
on memo writes. `formal-verifier` owns the periodic sweep that
catches drift neither trigger caught.

## Provenance

Workflow established 2026-04-05 after agents were bypassed in
earlier sessions. Refined over sessions 2 and 3 with Step 5 (memory
freshness) added explicitly. Tier-2 audit procedure ported
2026-04-11 from psh's agent-workflow after psh's 2026-04-11 tier-1
anchor batch established the 1:2 self-caught-to-agent-caught
hallucination ratio. Tier-2 code citation audit added 2026-04-11
alongside the code citation standard and bibliography
(`docs/citations.md`, `STYLEGUIDE.md` §"Code Documentation and
Citation Standard", `policy/code_citation_standard`).
