# Development Workflow

How we work on pane. Process and infrastructure.

**Style conventions** are in [`STYLEGUIDE.md`](../STYLEGUIDE.md) at the project root.
**Agent workflow** (four-agent consultation, pane-architect, formal-verifier) is in serena `pane/agent_workflow`.

## Documentation

```
STYLEGUIDE.md                     — code, formatting, writing conventions (universal)
CLAUDE.md                         — agent-specific project instructions
docs/
├── foundations.md                — design philosophy (Tier 1, rarely changes)
├── manifesto.md                  — historical positioning
├── architecture.md               — system design (Tier 2, evolves with design)
├── kit-documentation-style.md    — API doc style guide for kit crates
├── naming-conventions.md         — identifier naming policy (Be conventions + Rust idiom)
├── aesthetic.md                  — visual design language
├── optics-design-brief.md        — optics role, layers, boundary rules
├── language-deliberation.md      �� Rust vs OCaml/Haskell assessment
├── introduction.md               — user-facing intro
├── agents.md                     — agent integration
├── licensing.md                  — license structure
├── workflow.md                   — this file
reference/
├── README.md                     — what's here and why
├── haiku-book/                   — Haiku Book API reference (MIT, from haiku/haiku)
└── plan9/                        — Plan 9 man pages and papers (MIT, Plan 9 Foundation)
```

**Specs are living documents.** Edit them directly when the design evolves. No formal change proposals — the git history is the change record.

**Authority is graded:**
- **Tier 1 (foundations.md):** Principles. Implementation-independent. Changes rarely and only through deliberate decision.
- **Tier 2 (architecture, aesthetic):** Design intent. Evolves but changes are deliberate.
- **Tier 3 (pane-fs, compositor, etc.):** Component specs for unimplemented subsystems. Updated as implementation reveals the right shape.
- **Code:** The source of truth for implemented subsystems. API signatures, method contracts, type definitions, heritage annotations, and kit-level documentation live in Rust doc comments (`///` and `//!`).

## Process Rules

**Before implementing a new subsystem or major feature:**
Follow the agent workflow in serena `pane/agent_workflow`. The short version:
1. All four design agents in parallel (Plan 9, Be, optics, session types)
2. Synthesize → Lane refines
3. pane-architect implements
4. formal-verifier validates
5. Memory + doc freshness

**After any substantial refactor** (mass rename, API restructure):
1. Code review — correctness, idiom, consistency
2. Stale documentation review (parallel) — all comments, specs, docs, memories
3. If review fixes are themselves substantial → another stale doc review
4. Repeat until clean

**If a block requires deviating from the plan:**
1. Stop immediately
2. Present what happened and why it's a block
3. Present options with consequences
4. Wait for direction

## Memory

Serena (.serena/memories/) is the sole working memory system. Project context, naming policies, process rules, and decision records live there. Key divergence trackers:
- `pane/beapi_divergences` — every naming/structural deviation from BeOS with rationale
- `pane/plan9_divergences` — every adaptation of Plan 9 concepts with rationale

## Building

```
cargo check          # all macOS-buildable crates (default-members)
cargo test           # workspace tests
cargo fmt            # format per rustfmt.toml
cargo clippy         # lint
just build-comp      # cross-build compositor for Linux
just dev-comp        # build + push to VM + restart
just vm-ssh          # SSH into test VM
```
