---
name: Concrete Optics Implementation Guidance
description: Three profunctor insights translated to concrete implementation guidance — linear lens handles, capability enum, PutPut coalescing
type: project
---

Completed 2026-04-05. Lane requested translation of three profunctor optics insights into actionable implementation guidance for pane's concrete optics encoding.

**Three insights and their implementation form:**

1. **Obligation handles as linear lenses**: Clarke et al. Def 4.12. No new trait or type — documented pattern with four-point checklist. Tests verify: success-consumes, drop-compensates, at-most-once (compile_fail), coverage. Lives as doc comments on each handle type in pane-app.

2. **Optic subtyping for capability negotiation**: Two-level AttrCapability enum (ReadOnly, ReadWrite) on AttrReader in pane-fs/src/attrs.rs. satisfies() method encodes the subtyping lattice. Used for FUSE permissions (Phase 1) and wire advertisement (Phase 2). Recommendation: Option A (two-level) over extended lattice — extend only when a concrete attribute needs it.

3. **PutPut as coalescing predicate**: PutPut is definitional for lawful Lens. Every Attribute already tests PutPut. Recommendation: Option C — all Attributes are coalescable by definition; carry bool flag at type-erasure boundary. Coalescing runs in looper batch pass (pane-app), per-attribute within batch, never across batch boundaries.

**Key constraint**: None of these require profunctor encoding, new optics crate, or fp-library changes. Theory validates design through representation theorem (Clarke et al. Thm 4.4); implementation stays concrete.

**Why:** This is the deliverable from the profunctor optics deliberation. Translates theoretical results into code-level guidance for implementing agents.

**How to apply:** Reference this when reviewing optics-related PRs or when agents ask about obligation handles, capability negotiation, or write coalescing.
