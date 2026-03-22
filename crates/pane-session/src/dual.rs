use crate::types::{Send, Recv, End};

/// The dual of a session type — obtained by swapping Send/Recv.
///
/// If one endpoint follows session type S, the other must follow Dual<S>.
/// Duality is an involution: Dual<Dual<S>> = S.
///
/// In linear logic: negation. Send (tensor) becomes Recv (par) and vice versa.
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

// End ↔ End
impl HasDual for End {
    type Dual = End;
}
