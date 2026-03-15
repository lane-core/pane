/// A Value type — constructed by the sender, inspected by the receiver.
/// Requests, cell data, route messages, and attr values are Values.
/// Grounded in sequent calculus (positive/data types) and CBPV.
pub trait Value {}

/// A Compute type — defined by its behavior when observed.
/// Event handlers and protocol continuations are Computations.
/// Grounded in sequent calculus (negative/codata types) and CBPV.
pub trait Compute {}
