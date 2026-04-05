---
name: Profunctor Encoding Feasibility for Pane
description: Analysis of whether profunctor optics can satisfy pane's constraints (Send+Sync, dyn-compatible, by-reference, !Clone handler)
type: project
---

Analyzed 2026-04-03. Conclusion: full profunctor encoding is infeasible for pane's runtime layer; hybrid compile-time/concrete architecture is the viable path.

**Blocking constraints:**
1. **dyn-compatibility vs rank-2 types**: `forall p. C p => p a b -> p s t` cannot be stored behind a vtable. Fundamentally incompatible. Must instantiate at specific profunctors (Forget for view, Function for set) before crossing type-erasure boundary.
2. **By-reference access**: Standard profunctor dimap requires `s -> a` (by-value). By-reference needs `&s -> &a`, which changes the underlying category from Set to something like Borrow. Partially solvable for reads, but `&mut S` doesn't compose through cartesian (can't duplicate `&mut`).
3. **Send+Sync**: NOT solvable by just switching to ArcBrand. The !Send is baked into the optic struct definitions — they use `CloneableFn::Of` (trait objects without Send+Sync bounds), not `SendCloneableFn::SendOf`. ArcBrand does implement SendCloneableFn, but Lens/Shop/Forget don't use it. Would require 400-600 lines of parallel send-aware optic types.

**Representation theorem justifies the middle ground:** Clarke et al. Theorem th:profrep guarantees concrete ↔ profunctor isomorphism. So: profunctor-style composition at compile time → concrete at type-erasure boundary → dyn-compatible AttrReader/AttrWriter at runtime. The boundary crossing preserves laws.

**lens-rs approach analyzed:** Inverts the encoding — optic is a zero-sized type, source type implements traits (LensRef/LensMut/Lens). Not true profunctor optics; closer to van Laarhoven transposed. Solves by-reference natively. Composition via type nesting. Main cost: heavy derive machinery, trait coherence complexity.

**Three options presented to Lane:** (A) lens-rs-style inverted traits, (B) concrete-only (already approved), (C) hybrid with LensLike trait. Recommended C has best cost/benefit but B is already validated.

**Why:** This analysis was prompted by Lane asking whether profunctor optics could work within pane's specific constraints before committing to the concrete encoding.

**How to apply:** The concrete encoding (Option B/C) is the right choice for pane. The profunctor theory validates the design through the representation theorem — concrete optics are not "lesser" than profunctor optics, they are isomorphic. Cite this analysis if the question comes up again.
