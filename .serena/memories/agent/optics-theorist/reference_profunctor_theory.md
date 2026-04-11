---
name: Profunctor Optics Theory References
description: Key theoretical results from Clarke et al. and DontFear tutorial used in pane optics design
type: reference
---

**Primary references at:**
- `~/gist/profunctor-optics/arxivmain.tex` — Clarke, Elkins, Gibbons, Loregian, Milewski, Pillmore, Roman. "Profunctor optics, a categorical update." Compositionality 2023.
- `~/gist/DontFearTheProfunctorOptics/` — Tutorial with diagrams. ProfunctorOptics.md is the key file.

**Key results used in pane design:**

1. **Profunctor optic definition:** `type Optic s t a b = forall p. C p => p a b -> p s t` where C is the profunctor constraint (Strong for Lens, Choice for Prism, Strong+Choice for Affine, Wander for Traversal).

2. **Composition = function composition:** Composing optics with (.) merges constraints. Lens.Prism = Affine because Strong ∪ Choice = Strong+Choice. (DontFear, "Optic Composition is Function Composition" section)

3. **Representation theorem (Clarke et al. Thm 4.4):** Profunctor optics (forall p. Tambara p => p a b -> p s t) are isomorphic to concrete optics (existential pairs). Rust can't express the universal quantification (no rank-2 types), hence fp-library uses the Optic trait + monomorphization as approximation.

4. **Tambara modules (Clarke et al. §5):** The algebraic structure accompanying profunctor constraints. Strong ~ Tambara module for product action, Choice ~ Tambara module for coproduct action. Not directly exposed in fp-library but underlies the implementation.

5. **Lens laws (standard):** GetPut, PutGet, PutPut. For AffineTraversal: weakened to conditional on preview succeeding.

6. **Prism laws:** MatchBuild (match(build(b)) = Left b), BuildMatch (if match(s) = Left a then build(a) = s).

**The optics-design-brief.md (archived) explicitly decided NOT to use the profunctor encoding for pane's main optic types, favoring concrete encoding. But the current property.rs DOES use fp-library's profunctor encoding. This tension needs resolution.**
