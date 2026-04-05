# Development Workflow

How we work on pane. Lightweight process, specs as living documents.

## Documentation

```
docs/
├── foundations.md                — design philosophy (Tier 1, rarely changes)
├── manifesto.md                  — historical positioning
├── architecture.md               — system design (Tier 2, evolves with design)
├── kit-documentation-style.md    — API doc style guide for kit crates
├── naming-conventions.md         — identifier naming policy (Be conventions + Rust idiom)
├── aesthetic.md                  — visual design language
├── pane-fs.md                    — filesystem interface
├── pane-compositor.md            — compositor design
├── archive/optics-design-brief.md — optic layer design (archived: stale vs architecture.md)
├── archive/scripting-optics-design.md — optics × routing exploration (archived: stale)
├── introduction.md               — user-facing intro
├── development-methodology.md    — development approach
├── archive/agent-perspective.md   — agent integration perspective (archived)
├── agents.md                     — agent integration
├── licensing.md                  — license structure
├── workflow.md                   — this file
├── archive/                      — historical research and past changes
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
- **Code:** The source of truth for implemented subsystems. API signatures, method contracts, type definitions, heritage annotations, and kit-level documentation live in Rust doc comments (`///` and `//!`). See `docs/kit-documentation-style.md` for the style guide.

## Process Rules

**Before implementing a new subsystem or major feature:**
Consult the be-systems-engineer agent with reference to the Haiku Book (`reference/haiku-book/`), the Be Newsletter archive, and Haiku source (`/Users/lane/src/haiku/`). The consultation should produce:

1. **A reading list** — specific `.dox` files from the Haiku Book that the implementor should read before writing code. Not a summary; a pointer to the primary source. The implementor reads the contract directly.
2. **Design rationale** — newsletter articles and Haiku source that explain *why* the API has its shape.
3. **A verification checklist** — concerns to check the implementation against:
   - Did we account for every documented hook/virtual method?
   - Did we address the threading/locking considerations they call out?
   - Did we handle the pitfalls they warn about (or consciously diverge)?
   - Are there methods we chose not to implement, and do we know why?
4. **Adaptation notes** — how to translate the approach given pane's architecture (Wayland, session types, filesystem scripting).

The reading list and checklist live with the implementation work (in the plan or as task notes), not in memory. They are work artifacts, not durable reference.

When dispatching agents to implement features with Be lineage, include "read `reference/haiku-book/<path>` first" in the agent prompt so it works from the primary source.

This grounds implementation plans in actual engineering experience rather than assumptions about how Be worked. The research archive in `docs/archive/openspec/changes/spec-tightening/` contains prior consultations that may be relevant.

**When writing or reviewing API documentation:**
Follow the kit documentation style guide (`docs/kit-documentation-style.md`). Key requirements:
1. Every public type and method has a doc comment
2. Core types get full treatment: overview, threading, heritage
3. Heritage annotations credit both lineages where they apply:
   - `# BeOS` sections for Be/Haiku heritage (type mapping, method naming, behavioral divergences)
   - `# Plan 9` sections for Plan 9/Inferno heritage (distributed model, namespace design, protocol patterns)
   - Both sections may appear on the same type when it draws from both traditions

**Technical writing standard:**
Documentation uses strict technical language. Explain what the code
does, its operational semantics, and design heritage where relevant.
Do not lead with formalism names or use them as marketing ("CLL
types", "EAct actor framework"). Reference formalisms briefly where
they clarify design rationale — e.g., "Design heritage: Fowler et
al.'s EAct" — not as the primary framing. Avoid emphatic "IS"
constructions ("pane-app IS the EAct framework"). Write balanced,
ergonomic technical prose.
4. Hook methods document trigger, default behavior, and concrete use case
5. `cargo doc` builds without warnings

**When designing new protocol features or sub-protocols:**
Consult the be-systems-engineer (for Be/Haiku heritage), the plan9-systems-engineer (for distributed architecture and location-transparent design), and the session-type-consultant (for protocol soundness). The plan9-systems-engineer should review any feature touching state exposure, namespace design, cross-instance communication, or filesystem interfaces. The session-type-consultant should review:

1. **Protocol soundness** — is the proposed session type correct? Are there stuck states, orphaned channels, or missing branches?
2. **Ownership discipline** — do correlation IDs at the API surface have typed handles? Is ghost state recognized and documented?
3. **Failure composition** — does the `Drop`-based recovery chain compose correctly through newtypes? Are there panic paths that bypass cleanup?
4. **Invariant identification** — what runtime invariants does the design require? (e.g., `panic = unwind`, no custom `Drop` on wrappers, `Err` not panic on downcast failure)

Also consult the EAct-derived session-type design principles in serena memory `pane/session_type_design_principles`. Specifically:
1. New sub-protocols (clipboard, DnD, observer, inter-pane messaging) should use typestate handles at the API surface, not session-type the active-phase transport (principle C2).
2. New channels into the looper should be designed as separate typed channels with multi-source select in mind (principle C1).
3. Failure modes should consider per-conversation callbacks, not just actor-level death notification (principle C3).

See also `pane/eact_analysis_gaps` for structural gaps to address and `pane/eact_what_not_to_adopt` for anti-patterns to avoid.

**The functoriality principle — type shapes constrain the design space:**
`Prog(Phase1 + Phase2) ≠ Prog(Phase1) + Prog(Phase2)`. The programs
buildable on the full architecture are not decomposable into programs
buildable on each phase independently. Phase 1 type signatures shape
what developers (including us) build. A Phase 1 type that omits
structure needed later produces patterns that assume that structure
doesn't exist — an ecosystem that can't cleanly accommodate the full
design. Every type in Phase 1 must be the full architecture's type,
populated minimally. ServiceRouter with one entry, not a bare sender.
ServiceId { uuid, name }, not a bare string. HashMap<(ConnectionId,
token)>, not HashMap<token>. The cost is near-zero. The alternative is
a guaranteed breaking change across every downstream consumer.

Demonstrated by: BeOS's string-based application signatures shaped an
ecosystem built on `strcmp()`. When structured identity was needed
(launch daemon, package management), everything was string-comparison
all the way down. The type simplification in Phase 1 prevented clean
evolution in Phase 2.

**After any substantial refactor** (mass rename, API restructure):
1. Code review — correctness, idiom, consistency
2. Stale documentation review (parallel) — all comments, specs, docs, memories
3. If review fixes are themselves substantial → another stale doc review
4. Repeat until clean

**After a major revision or design audit** (new design principles, research integration, architectural analysis):
1. Update all affected serena memories, docs, and code comments
2. Audit the codebase for adherence to the new principles — produce a structured report of aligned, evolving, and contradicting code
3. Update PLAN.md with any discovered debt or new tasks
4. Produce a handoff summary for the other agent: what changed, what to re-read, what it means for ongoing work
5. Commit everything together with a descriptive message

**If a block requires deviating from the plan:**
1. Stop immediately
2. Present what happened and why it's a block
3. Present options with consequences
4. Wait for direction

**API naming:** Default to BeOS identifier conventions (snake_case per Rust). Deviations require explicit justification and are tracked in serena memory (`pane/beapi_divergences`). Plan 9 design lineage and divergences are tracked in serena memory (`pane/plan9_divergences`).

## Memory

Serena (.serena/memories/) is the sole working memory system. Project context, naming policies, process rules, and decision records live there. Key divergence trackers:
- `pane/beapi_divergences` — every naming/structural deviation from BeOS with rationale
- `pane/plan9_divergences` — every adaptation of Plan 9 concepts with rationale

Agent memories (.claude/agent-memory/) contain research notes from consultations:
- `be-systems-engineer/` — Be/Haiku architecture research
- `plan9-systems-engineer/` — Plan 9/Inferno distributed systems research
- `session-type-consultant/` — protocol analysis and session-type gap analysis

These are read-only reference material.

## Building

```
cargo check          # all macOS-buildable crates (default-members)
cargo test           # 145+ tests
just build-comp      # cross-build compositor for Linux
just dev-comp        # build + push to VM + restart
just vm-ssh          # SSH into test VM
```
