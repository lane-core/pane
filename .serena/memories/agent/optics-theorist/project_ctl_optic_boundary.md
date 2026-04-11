---
name: ctl Command / Optic Boundary Analysis
description: Theoretical analysis of which ctl commands can route through optics vs require separate dispatch — monadic lens as key extension
type: project
---

Analyzed 2026-04-05. Lane asked whether ctl command dispatch can be routed entirely through the optic layer.

**Boundary:**
- Optic-expressible (~80%): simple field sets (Lens), compound mutations with deterministic side-effects (Lens with invariant-maintaining setter), partial mutations (Affine), state+notification commands like `focus` (Monadic Lens per Clarke et al. Def 4.6), queries (Fold/AffineFold).
- Not optic-expressible (~20%): lifecycle transitions (`close` → Flow::Stop, no focus/state decomposition), IO-first commands (`reload` — data flow is effect→state, backwards from optic model).

**Key theoretical result:** Monadic lenses (MndLens_Ψ) from Clarke et al. are genuine optics (compose, have profunctor representation via Kleisli category). Concrete form: pure get, effectful set returning Ψ T. For pane: `update: (&mut S, A) -> Vec<Effect>`. Handles `focus`, `hide/show`, and similar set+notify patterns.

**Open design decision:** Whether to use monadic lenses (effects tied to lens, prevents wiring divergence, more complex optic type) or pure optics + dispatch-layer effect generation (simpler optic type, reintroduces coupling risk). Presented both options to Lane for decision.

**Why:** This determines whether pane needs two dispatch mechanisms or can mostly unify ctl through optics.

**How to apply:** When implementing ctl dispatch, the split falls on the optic/non-optic boundary. Lifecycle commands stay in Flow. Most other commands route through optic set. The monadic lens decision shapes the optic crate's type hierarchy.
