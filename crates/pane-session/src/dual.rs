use crate::types::{Send, Recv, Select, Branch, End};

/// The dual of a session type — obtained by swapping Send/Recv and Select/Branch.
///
/// If one endpoint follows session type S, the other must follow `Dual<S>`.
/// Duality is an involution: `Dual<Dual<S>>` = S.
///
/// In linear logic: negation. Send (⊗) ↔ Recv (⅋), Select (⊕) ↔ Branch (&).
pub trait HasDual {
    type Dual;
}

/// Convenience alias: `Dual<S>` is `<S as HasDual>::Dual`.
pub type Dual<S> = <S as HasDual>::Dual;

// Send<A, S> ↔ Recv<A, Dual<S>>
impl<A, S: HasDual> HasDual for Send<A, S> {
    type Dual = Recv<A, Dual<S>>;
}

// Recv<A, S> ↔ Send<A, Dual<S>>
impl<A, S: HasDual> HasDual for Recv<A, S> {
    type Dual = Send<A, Dual<S>>;
}

// Select<L, R> ↔ Branch<Dual<L>, Dual<R>>
impl<L: HasDual, R: HasDual> HasDual for Select<L, R> {
    type Dual = Branch<Dual<L>, Dual<R>>;
}

// Branch<L, R> ↔ Select<Dual<L>, Dual<R>>
impl<L: HasDual, R: HasDual> HasDual for Branch<L, R> {
    type Dual = Select<Dual<L>, Dual<R>>;
}

// End ↔ End
impl HasDual for End {
    type Dual = End;
}
