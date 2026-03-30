# Development Workflow

How we work on pane. Lightweight process, specs as living documents.

## Documentation

```
docs/
├── foundations.md          — design philosophy (Tier 1, rarely changes)
├── manifesto.md            — historical positioning
├── architecture.md         — system design (Tier 2, evolves with design)
├── pane-app.md             — application kit
├── aesthetic.md             — visual design language
├── pane-fs.md              — filesystem interface
├── pane-compositor.md      — compositor design
├── introduction.md         — user-facing intro
├── agents.md               — agent integration
├── licensing.md            — license structure
├── workflow.md             — this file
└── archive/                — historical research and past changes
```

**Specs are living documents.** Edit them directly when the design evolves. No formal change proposals — the git history is the change record.

**Authority is graded:**
- **Tier 1 (foundations.md):** Principles. Implementation-independent. Changes rarely and only through deliberate decision.
- **Tier 2 (architecture, pane-app, aesthetic):** Design intent. Evolves but changes are deliberate.
- **Tier 3 (pane-fs, compositor, etc.):** Component specs. Updated as implementation reveals the right shape.
- **Code:** The spec for implementation details. API signatures, method contracts, type definitions live in doc comments.

## Process Rules

**Before implementing a new subsystem or major feature:**
Consult the be-systems-engineer agent with reference to the Be Newsletter archive and Haiku source (`/Users/lane/src/haiku/`). The consultation should cover:
1. How did Be/Haiku implement the equivalent subsystem?
2. What newsletter articles discuss its design rationale?
3. What worked, what didn't, and what would they do differently?
4. How should we adapt their approach given pane's architecture (Wayland, session types, filesystem scripting)?

This grounds implementation plans in actual engineering experience rather than assumptions about how Be worked. The research archive in `docs/archive/openspec/changes/spec-tightening/` contains prior consultations that may be relevant.

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

**API naming:** Default to BeOS identifier conventions (snake_case per Rust). Deviations require explicit justification and are tracked in serena memory (pane/beapi_divergences).

## Memory

Serena (.serena/memories/) is the sole working memory system. Project context, naming policies, process rules, and decision records live there.

Be-engineer agent memory (.claude/agent-memory/) contains research notes from the architecture specification work. It's read-only reference material.

## Building

```
cargo check          # all macOS-buildable crates (default-members)
cargo test           # 130+ tests
just build-comp      # cross-build compositor for Linux
just dev-comp        # build + push to VM + restart
just vm-ssh          # SSH into test VM
```
