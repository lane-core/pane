---
type: analysis
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [pane-router, pane-translator, optics, prism, getter, affine_traversal, monadic_lens, writer_monad, message_filter, routing]
related: [architecture/proto, architecture/fs, analysis/optics/scope_boundaries, analysis/optics/panefs_taxonomy]
sources: [CBG24 Prop 4.7, CBG24 Def 4.13, CBG24 Def 4.20, DontFear Part 3]
verified_against: [crates/pane-proto/src/monadic_lens.rs, crates/pane-proto/src/filter.rs]
agents: [optics-theorist]
---

# Router + Translator Optics Analysis (2026-04-12)

Six-question analysis for pane-router and pane-translator design.

## Key findings

1. **Router rules are NOT optics.** Transform case uses optics internally
   (Setter/over), but routing decisions are control flow + side effects.
   Keep MessageFilter<M> as the core. ~20% non-optic boundary holds.

2. **Predicate composition is Boolean algebra over Getters.** NOT
   Traversals or Affines. Getters extract fields (MIME type, size);
   predicates are plain `Fn(&M) -> bool`; composition is `&&`/`||`.

3. **Translator IS a Prism** (lossless case). Identify = match,
   Translate = from/to. Laws hold. Lossy translators are plain
   functions with quality metadata, not optics. Translator registry
   is selection over Prisms, not optic composition.

4. **State projection through router = Lens ∘ Prism = AffineTraversal.**
   Pre-filter model (attribute exists or doesn't for observer).
   PutGet edge case: if set causes its own revocation, law fails.
   Recommendation: pre-filter (Option A) for attribute-level ACL,
   permission guard (Option C) for coarse pane-level ACL.

5. **Host devices: Getter for read-only, request-based for read-write.**
   MonadicLens requires deterministic effects (gate 2). System-mediated
   device control violates PutGet (system may constrain the value).
   Not thunkable in writer monad.

6. **Router effects separate from LensEffect.** Writer monad scope
   (CBG24 Prop 4.7) is too narrow for filesystem operations.
   Audit logging is thunkable; file copy/symlink/delete are not.
   Two options: separate RouterEffect type, or imperative middleware.

## Decision points for Lane

- Translator: Prism for lossless vs plain-function for lossy (clear)
- Router ACL model: pre-filter (AffineTraversal) vs permission guard
- Router effects: declarative RouterEffect enum vs imperative middleware
- Device control: request-based API shape (not MonadicLens)
