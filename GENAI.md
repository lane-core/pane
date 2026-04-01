# GenAI Use in Pane

This document discloses how generative AI is used in the development
of pane. Its structure follows standard practice for GenAI transparency
in publicly funded open source, modeled on
[NLnet's GenAI policy](https://nlnet.nl/policies/generativeAI/).

## Model

**Claude Opus 4.6** (Anthropic), accessed via **Claude Code** (Anthropic's
CLI tool). Model ID: `claude-opus-4-6`. Specialist subagents
(be-systems-engineer, plan9-systems-engineer, session-type-consultant)
also use Claude Opus 4.6.

Anthropic's terms of use grant users ownership of outputs generated
through their API/tools. Claude Code does not train on user inputs
or outputs. Outputs are not reconstructed from copyrighted sources.

## How AI is used

Pane is developed by a human architect (Lane) with substantial AI
assistance. The human provides the intellectual contribution required
for copyright eligibility under EU law: architectural synthesis,
problem formulation, design direction, review, and acceptance of all
output. The AI is a tool that executes within constraints set by the
human.

**Human responsibilities (intellectual creation):**
- Architectural synthesis — recognizing cross-lineage unifications
  (e.g., BeOS live queries as Plan 9 synthetic filesystems), setting
  design direction, evaluating trade-offs between competing approaches
- Problem formulation — defining what to build, why, and what "done"
  looks like. The architecture is the prompt.
- Review and acceptance — all AI output is reviewed before commit.
  The human reads the code, verifies it against the design intent,
  runs tests, and accepts or rejects.
- Design decisions — choice of session types, calloop migration
  strategy, cancel-on-drop semantics, ownership verification model,
  etc. These are human decisions informed by AI research, not AI
  decisions accepted by a human.

**AI responsibilities (execution):**
- Code generation — implementation of designs specified by the human
- Cross-referencing — consulting Haiku source, Plan 9 man pages,
  session type theory, and other reference material during
  implementation
- Code review — specialist agents audit code for correctness, style
  conformity, heritage annotation completeness
- Research synthesis — reading primary sources and producing structured
  findings for human evaluation

The AI does not make unsupervised architectural decisions. When agents
disagree (e.g., timer cancellation strategy), all positions are
presented to the human, who decides.

## What is AI-generated

Substantially all code in the repository was generated or co-generated
with AI assistance. This includes:

- **Implementation code** — crate source files in `crates/`
- **Tests** — unit and integration tests
- **Documentation** — doc comments, `# BeOS` and `# Plan 9` heritage
  annotations, design documents in `docs/`
- **Reference material indexes** — `benewsletter-index.md`,
  `benewsletter-wisdom.md` (AI-generated summaries of Be Newsletter
  articles; the newsletters themselves are not in this repo)
- **Memory artifacts** — `.serena/memories/` and `.claude/agent-memory/`
  contain AI-generated research notes and decision records

Documents authored primarily by the human: `README.md` (methodology
section), `docs/foundations.md`, `docs/manifesto.md`.

## What is NOT AI-generated

- **Architectural decisions** — the choice of BeOS + Plan 9 lineage,
  the session-type foundation, the calloop migration strategy, the
  kit API shape — these are human decisions informed by AI research
- **Vendored reference material** — `reference/haiku-book/` (MIT,
  Haiku project) and `reference/plan9/` (MIT, Plan 9 Foundation) are
  primary sources, not AI output

## Per-commit provenance

Commits that contain AI-generated code include a `Generated-with:`
trailer naming the model. Example:

    Generated-with: Claude opus-4-6 (Anthropic) via Claude Code

For commits prior to the adoption of this convention, the development
pattern serves as disclosure: the repository has a single human
contributor, and substantially all commits are produced in Claude Code
sessions.

When a commit is the result of a specific prompt or task, the commit
message describes the intent (what the human asked for) and the
outcome (what the AI produced). Detailed prompt/response logs for
individual sessions are archived in the repository:

- `.claude/` — plans, agent definitions, agent memory
- `.serena/memories/` — decision records, divergence trackers,
  session summaries (e.g., `pane/session_2026_03_31_summary`)

These are in-repo, require no third-party login, and persist with
the git history.

## Licensing compliance

All AI-generated code is original to this project — it implements
designs specified by the human architect against the project's own
type system and protocol definitions. No code is copied from
copyrighted sources.

Per EU law (European Parliament report on Generative AI and
Copyright, p. 93): purely AI-generated outputs without substantial
human intellectual contribution are not eligible for copyright
protection. The human contribution to pane meets the "intellectual
creation" standard: the architecture, type system constraints,
session-type protocols, and design decisions that shape every
implementation are human intellectual work. The AI produces code
*within* those constraints; the constraints themselves are the
creative contribution.

Reference material included in the repository:
- **Haiku Book** (`reference/haiku-book/`): MIT license, Haiku
  project. Attribution in `reference/haiku-book/LICENSE`.
- **Plan 9 documentation** (`reference/plan9/`): MIT license, Plan 9
  Foundation. Attribution in `reference/plan9/LICENSE`.
- **Be Newsletter** references: short attributed excerpts and topical
  summaries used as design guidance. The newsletters were published
  by Be, Inc. as developer education material (1995-2000). They are
  not reproduced in this repository; the Haiku project hosts the
  archive at `haiku-os.org/legacy-docs/benewsletter/`.

## Contributor policy

Contributors may use GenAI tools. Requirements:

1. **Disclose use** — note the model and how it was used in the
   commit message or PR description.
2. **Include a `Generated-with:` trailer** on commits containing
   AI-generated code.
3. **Verify outputs** — ensure generated code compiles, passes tests,
   and does not reproduce copyrighted material.
4. **Follow project standards** — `docs/kit-documentation-style.md`
   and `docs/naming-conventions.md` apply regardless of how the code
   was produced.
5. **Human accountability** — the contributor, not the AI, is
   responsible for correctness and design decisions.

The project's `.claude/` directory and `CLAUDE.md` define the
conventions and constraints that AI agents operate under.
