# Duploid Deep Analysis: Writer Monad, Namespace Commutativity, Mixed Optics (2026-04-05)

Second-round theoretical exploration following the initial duploid analysis. Four agents (session-type, plan9, optics, be) confirmed and refined the framework.

## Writer monad Ψ(A) = (A, Vec<Effect>)

MonadicLens's set path is the writer monad Ψ composed with an outer error monad. Parse fails before set is called — error never coexists with effects. The &mut S encoding collapses the returned state into the mutable reference (isomorphic to Kleisli arrow).

Effect reordering: Vec<Effect> is a free monoid. Two effect lists commute iff one is empty. For non-trivial effects, reordering requires an effect-level interference relation (Mazurkiewicz traces). Lens laws (PutPut) govern state, not effect commutativity. Conservative rule: reorder only thunkable (pure) operations (MM14b Proposition 6).

BeOS equivalent: LinkSender's fBuffer = (status_t, Buffer<Message>), a writer monad called "reducing port round-trips." CancelMessage() = discarding uncommitted effect sequence (free in writer monad).

## ArcSwap = identity comonad

ArcSwap adds indirection, not semantic structure. Reads are pure extractions from frozen snapshots (B5). Comonad stays identity. Would become non-trivial only with read-triggered refresh (store comonad). Don't do this.

## MonadicLens = mixed optic

Clarke et al. Proposition 4.7: MndLens_Ψ ≅ Optic_(×, ⋊). View side in W (cartesian, pure). Set side in Kl(Ψ) (writer monad Kleisli). The duploid polarity maps directly: positive leg = Kl(Ψ), negative leg = W with identity comonad. This explains why close/reload don't fit MonadicLens — they'd need both legs in Kl(Ψ).

Profunctor: p a (Ψ a) → p s (Ψ s) where p is Tambara module for (×, ⋊). This is in the Lens family (cartesian decomposition, no partiality).

## Namespace commutativity

pane-fs filter views are set intersections — commutative and idempotent. No union mounts, no Lambek calculus needed. Exchange rule holds. Service map precedence is a one-time left-biased merge at startup, not ongoing composition.

Non-commutativity exists only in temporal effect ordering (writer monad), not in namespace composition.

## The shift ω_X

Handshake (dialogue duploid, double-negation monad) → active phase (plain duploid, writer monad) is NOT a monad morphism. It's a forgetful transition that destroys dialogue structure. Correct: no renegotiation in active phase.

What ω_X preserves: thunkability. Pure operations remain pure through the transition.

ActivePhase<T> should carry negotiated state (max_message_size, PeerAuth, known_services) — the "positive residue" of the dialogue.

## BeOS scripting = duploid polarity

B_GET_PROPERTY = negative (co-Kleisli extraction). B_SET_PROPERTY = oblique (positive value → negative state). ResolveSpecifier recursive walk = optic composition. BPropertyInfo table = session type expressed as capability table.

BLooper sequential dispatch = canonical bracketing for non-commutative writer monad effects.

## Key theoretical citations for code docs

- MMM25 = Mangel/Melliès/Munch-Maccagnoni 2025 "Classical Notions of Computation and the FH Theorem"
- MM14b = Munch-Maccagnoni 2014b (foundational duploid definitions)
- Clarke et al. Definition 4.6 = MonadicLens definition
- Clarke et al. Proposition 4.7 = mixed optic characterization
- MM14b Proposition 6 = thunkable ⟹ central (holds in any duploid)
- DLfActRiS (Hinrichsen/Krebbers/Birkedal, POPL 2024) = deadlock freedom for actors
