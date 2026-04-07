# GenAI Use in Pane

This document discloses how generative AI is used in the development of pane,
in compliance with the
[NLnet Generative AI Policy v1.1](https://nlnet.nl/policies/generativeAI/)
(effective 2025-12-08). All commits in this repository postdate that
effective date (first commit: 2026-03-14). No transition provision applies.

## Model

**Claude Opus 4.6** (Anthropic), accessed via **Claude Code** (Anthropic's
CLI tool). Model ID: `claude-opus-4-6`. Specialist subagents
(be-systems-engineer, plan9-systems-engineer, session-type-consultant,
optics-theorist, formal-verifier) also use Claude Opus 4.6.

Anthropic's terms of use grant users ownership of outputs generated through
their API and tools. Claude Code does not train on user inputs or outputs.

## Human and AI roles

The `README.md` methodology section (lines 192–271) describes the development
relationship in full. The summary: pane is developed by a human architect
(Lane) with substantial AI assistance. All intellectual direction — problem
formulation, architectural synthesis, design decisions, review, and
acceptance — is human. The AI executes within constraints set by the human.

**Human responsibilities:**
- Architectural synthesis: recognizing cross-lineage unifications (e.g., BeOS
  live queries and Plan 9 synthetic filesystems as dual expressions of
  "namespace as indexed state with materialized views"), setting design
  direction, evaluating trade-offs
- Problem formulation: defining what to build, why, and what "done" looks
  like. The architecture is the prompt.
- Design decisions: session type conventions, calloop migration strategy,
  cancel-on-drop semantics, ownership verification model, etc. These are human
  decisions informed by AI research, not AI decisions ratified by a human.
- Review and acceptance: all AI output is read, verified against design intent,
  tested, and explicitly accepted or rejected before commit.

**AI responsibilities:**
- Code generation: implementation of designs specified by the human
- Cross-referencing: consulting Haiku source, Plan 9 man pages, session type
  literature, and other reference material during implementation
- Code review: specialist agents audit for correctness, style, and heritage
  annotation completeness
- Research synthesis: reading primary sources and producing structured findings
  for human evaluation

When agents disagree, all positions are presented to the human, who decides.
The AI does not make unsupervised architectural decisions.

## What is AI-generated

Substantially all code in the repository was generated or co-generated with AI
assistance. This includes:

- **Implementation code** — crate source files in `crates/`
- **Tests** — unit and integration tests
- **Documentation** — doc comments, `# BeOS` and `# Plan 9` heritage
  annotations, design documents in `docs/` (except where noted below)
- **Reference material indexes** — `benewsletter-index.md`,
  `benewsletter-wisdom.md` (AI-generated summaries of Be Newsletter articles;
  the newsletters themselves are not in this repo)
- **Memory artifacts** — `.serena/memories/` and `.claude/agent-memory/`
  contain AI-generated research notes and decision records

Documents authored primarily by the human: `README.md` (methodology section),
`docs/foundations.md`, `docs/manifesto.md`.

## What is NOT AI-generated

- **Architectural decisions** — the choice of BeOS + Plan 9 lineage, the
  session-type foundation, the calloop migration strategy, the kit API shape —
  these are human decisions informed by AI research
- **Vendored reference material** — `reference/haiku-book/` (MIT, Haiku
  project) and `reference/plan9/` (MIT, Plan 9 Foundation) are primary
  sources, not AI output

## Per-commit provenance

Every commit containing AI-generated code ends with a `Generated-with:`
trailer:

    Generated-with: Claude opus-4-6 (Anthropic) via Claude Code

This trailer is required by `CLAUDE.md` and is enforced as a project
convention. For commits prior to this convention being adopted, the trailer
was added retroactively as a `git note` on each affected commit (456 commits).
These notes are visible via `git log --show-notes`.

Commit messages use a dual-provenance format: the first paragraph describes
human design direction in third person (what Lane decided, why, and what
was asked); the second paragraph begins with "Agent steps:" and describes
what the agent did, including model, subagent consultations, and verification
performed. This format is specified in `CLAUDE.md`.

## Prompt and session logging

The project maintains complete, verifiable records of AI-assisted development:

**In-repository logs** (committed, require no third-party login):

- `docs/genai-log/raw/` — 35 session files, one per Claude Code session.
  Contains every human prompt verbatim, with timestamps. These are the actual
  inputs that drove the AI-generated work.
- `docs/genai-log/digest/` — 35 per-session summaries. Each digest lists the
  session date range, model, prompt count, key design directives from the
  human, and the commits produced in that session.

**Locally preserved archive** (gitignored due to size):

- `.genai-archive/` — full JSONL session archives exported from Claude Code,
  covering all 35 sessions. Total size: ~217MB. These contain complete
  prompt/response pairs for every session. Available for inspection on request;
  not committed because the size is impractical for a source repository.

The `docs/genai-log/` directory serves as the primary audit trail for grant
compliance purposes. The raw logs establish what was asked; the digests
establish the decisions made; the commit messages and `git notes` establish
what was produced.

Note: `.serena/memories/` and `.claude/agent-memory/` are design knowledge
bases — research notes, decision records, and architectural memory — not
prompt/response logs. They are useful for understanding design rationale but
are not the source of record for AI usage.

## Licensing compliance

All AI-generated code is original to this project — it implements designs
specified by the human architect against the project's own type system and
protocol definitions. No code is copied from copyrighted sources.

Per EU law (European Parliament report on Generative AI and Copyright, p. 93):
purely AI-generated outputs without substantial human intellectual contribution
are not eligible for copyright protection. The human contribution to pane
meets the "intellectual creation" standard: the architecture, type system
constraints, session-type protocols, and design decisions that shape every
implementation are human intellectual work. The AI produces code *within*
those constraints; the constraints are the creative contribution.

Reference material included in the repository:

- **Haiku Book** (`reference/haiku-book/`): MIT license, Haiku project.
  Attribution in `reference/haiku-book/LICENSE`.
- **Plan 9 documentation** (`reference/plan9/`): MIT license, Plan 9
  Foundation. Attribution in `reference/plan9/LICENSE`.
- **Be Newsletter** references: short attributed excerpts and topical summaries
  used as design guidance. The newsletters were published by Be, Inc. as
  developer education material (1995–2000). They are not reproduced in this
  repository; the Haiku project hosts the archive at
  `haiku-os.org/legacy-docs/benewsletter/`.

## Scope note

The approved grant work covers the pane desktop environment infrastructure
(pane-session, pane-fs, pane-app, and related crates). The repository does
not currently contain an AI kit or AI-inference crate. If AI-assisted features
are added to the scope of grant work in the future, this document will be
updated.

## Contributor policy

Contributors may use GenAI tools. Requirements:

1. **Disclose use** — note the model and how it was used in the commit message
   or PR description.
2. **Include a `Generated-with:` trailer** on commits containing AI-generated
   code.
3. **Verify outputs** — ensure generated code compiles, passes tests, and does
   not reproduce copyrighted material.
4. **Follow project standards** — `docs/kit-documentation-style.md` and
   `docs/naming-conventions.md` apply regardless of how the code was produced.
5. **Human accountability** — the contributor, not the AI, is responsible for
   correctness and design decisions.

The project's `.claude/` directory and `CLAUDE.md` define the conventions and
constraints that AI agents operate under.
