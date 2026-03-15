## ADDED Requirements

### Requirement: Result-like combinators on domain types
Custom enums in pane kits that have a success/failure or some/none shape SHALL provide standard combinator APIs (`map`, `and_then`, `unwrap_or`, `ok_or`) via derive macros or manual implementation, rather than requiring callers to match and chain manually.

#### Scenario: Domain result composition
- **WHEN** a kit function returns a domain-specific result type (e.g., plumber match, store query)
- **THEN** the caller SHALL be able to chain `.and_then()` and `.map()` on it identically to `Option` or `Result`

#### Scenario: Standard Result/Option preferred where applicable
- **WHEN** a function's outcome is naturally a success/error or some/none
- **THEN** it SHALL use standard `Result` or `Option`, not a custom type with derived combinators

### Requirement: Protocol combinator API in pane-app
The pane-app kit SHALL provide a combinator type for composing protocol operation sequences as values. The combinator SHALL support `and_then` (monadic bind) and `map` (functor map). Protocol sequences built with combinators SHALL be executable against a real connection and independently testable against a mock state.

#### Scenario: Composing a protocol sequence
- **WHEN** a client composes protocol operations via `and_then` chaining
- **THEN** the result SHALL be a single composable value that can be run against a `ProtocolState`

#### Scenario: Testing a protocol sequence without I/O
- **WHEN** a composed protocol sequence is run against an in-memory `ProtocolState`
- **THEN** it SHALL produce a result without requiring a socket connection

### Requirement: Reactive signals for state observation
Kit crates that manage observable state SHALL provide reactive signals supporting `map`, `combine`, and `contramap` for declarative dataflow composition.

#### Scenario: Live query as signal composition
- **WHEN** a client creates a store query and subscribes to change notifications
- **THEN** the live query SHALL be expressible as a signal composed from the query result and the notification stream

#### Scenario: Derived signal updates automatically
- **WHEN** an upstream signal's value changes
- **THEN** any signal derived from it (via `map` or `combine`) SHALL automatically reflect the new value

### Requirement: Compositional boundaries
Monadic composition SHALL NOT be applied to imperative operations where sequential statements are clearer. Cell grid writes, calloop event dispatch, and direct buffer mutation SHALL remain imperative.

#### Scenario: Cell grid writes remain imperative
- **WHEN** a pane client writes cells to a grid region
- **THEN** the API SHALL accept direct mutation, not a monadic wrapper

#### Scenario: Ergonomic test
- **WHEN** a kit API is designed using combinator chaining
- **THEN** it SHALL read more clearly than the equivalent sequential statements, or the combinator approach SHALL NOT be used
