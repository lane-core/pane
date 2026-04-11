# Writer Monad and Shift Analysis (2026-04-05)

## PSI characterization
The active phase monad is genuinely PSI(A) = (A, Vec<Effect>), a writer monad over the free monoid (Vec<Effect>, ++, []). Parse failures live outside PSI -- the Result<> in AttrWriter is a prism guarding entry to the Kleisli category, not part of the monad. The `set` fn pointer is infallible once you have a typed value.

Clarke et al. Definition 4.6: MndLens_PSI((A,B),(S,T)) = W(S,A) x W(S x B, PSI(T)). Error case is outside this structure.

## Effect batching
Kleisli composition is associative (list concatenation). Effects CANNOT be reordered (free monoid on 2+ generators is non-commutative). Partial reordering possible for independent targets (Mazurkiewicz traces / partial commutativity), but the duploid framework doesn't see this.

Thunkability criterion (MM25): a set operation is batchable iff its effect list is deterministic as fn(state, value). All current fn-pointer setters satisfy this. PutPut is the lens-law manifestation. Two thunkable sets to different attributes can be batched but not reordered.

## Shift omega (handshake -> active)
omega_X : Handshake -> ActivePhase is a positive shift (MM25 section 3.2). It is NOT a monad morphism -- dialogue's double-negation monad and active's writer monad are different monads on different categories. omega forgets dialogue structure (no renegotiation after shift). This is correct by design.

omega preserves thunkability: pure handshake operations remain pure in active phase.

## ArcSwap comonad
ArcSwap is the identity comonad with a performance optimization (Arc indirection). extract = load() is pure observation. No duplicate operation. Comonad stays trivial as long as B5 holds (AttrReader closures are pure) and reads don't trigger refresh.

WARNING: Read-triggered refresh would make extract effectful, requiring store comonad Store(S,-). Don't do this.

## Namespace commutativity
pane-fs computed projections avoid Plan 9's non-commutative namespace monoid. Filter views compose as set intersection (commutative, idempotent). Service map precedence is configuration-time left-biased merge, not runtime algebraic composition. Exchange rule holds. Lambek calculus not needed unless union-mount overlays are added.

## Related psh concept anchor

- **`../psh/.serena/memories/analysis/monadic_lens`** — psh's
  keyword-shaped anchor for `MonadicLens` as
  Kleisli-generalization-of-lens, citing Clarke et al.
  Definition 4.6 and the Kl(Ψ) structure directly. psh's
  examples (shell variable disciplines) differ from pane's
  (kit API + FUSE attribute writers), but the underlying
  theory is identical. Not vendored into pane; cited as the
  external theoretical source.
