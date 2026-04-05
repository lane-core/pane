---
name: Optics Layer Design Analysis
description: Design decisions and analysis for pane's optic-backed attribute system — fp-library removal, concrete encoding, dual representation
type: project
---

Comprehensive optics layer design analysis, updated 2026-04-03.

**Recommended design: Concrete encoding with dual representation (Option 2c).**

fp-library's profunctor encoding is fundamentally incompatible with pane's requirements:
1. `Lens::from_view_set` uses `CloneableFn::new` internally, not `SendCloneableFn`. Even with ArcBrand, the stored closures are `Arc<dyn Fn(A) -> B>` (no Send+Sync on trait object). There is no `SendLens` in fp-library 0.15.0.
2. By-value `view(S) -> A` requires `S: Clone` at the call site.
3. Lens type is generic over PointerBrand, making it !dyn-compatible.

**Why:** The three requirements (Send+Sync for cross-thread snapshot reads, dyn-compatibility for type erasure at pane-fs boundary, by-reference for zero-copy reads) are all violated by fp-library. This was verified by tracing through fp-library 0.15.0 source: brands.rs -> fn_brand.rs -> cloneable_fn.rs -> lens.rs.

**Recommended architecture:**
- `Lens<S, A>` — fn pointers, zero-cost, the common case (flat field access)
- `BoxLens<S, A>` — Box<dyn Fn + Send + Sync>, for composition results
- `LensLike<S, A>` trait — unifies both at type-erasure boundary, object-safe
- `Affine<S, A>` — partial access (optional fields, enum variants)
- `Getter<S, A>` — read-only computed values
- `assert_lens_laws` / `assert_affine_laws` — test harness for law enforcement
- `AttrWriter<S>` — write path with text parsing, handler controls acceptance
- fp-library removed from pane-proto dependencies

**How to apply:** This shapes the pane-optic crate (listed in PLAN.md as preserved from prototype, but not yet created). AttrReader/AttrWriter in pane-fs consume LensLike trait objects. Handler declaration uses Lens (fn pointers) directly. Composition is manual in the common case.
