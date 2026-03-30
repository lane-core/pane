## ADDED Requirements

### Requirement: Client-to-compositor request messages
The pane-proto crate SHALL define a `PaneRequest` enum covering all operations a pane-native client can send to the compositor. Variants SHALL include pane lifecycle (create, close), body content (write cells, scroll), tag management (set tag text), and control operations (set name, set dirty/clean, request geometry).

#### Scenario: Exhaustive request handling
- **WHEN** a new variant is added to `PaneRequest`
- **THEN** the Rust compiler SHALL produce an error in any match expression that does not handle the new variant

#### Scenario: Request serialization round-trip
- **WHEN** any valid `PaneRequest` value is serialized with postcard and deserialized
- **THEN** the deserialized value SHALL equal the original

### Requirement: Compositor-to-client event messages
The pane-proto crate SHALL define a `PaneEvent` enum covering all events the compositor can send to a pane-native client. Variants SHALL include input events (key, mouse), lifecycle events (resize, focus, close requested), tag interaction events (tag text executed, tag text plumbed), and plumb message delivery.

#### Scenario: Exhaustive event handling
- **WHEN** a new variant is added to `PaneEvent`
- **THEN** the Rust compiler SHALL produce an error in any match expression that does not handle the new variant

#### Scenario: Event serialization round-trip
- **WHEN** any valid `PaneEvent` value is serialized with postcard and deserialized
- **THEN** the deserialized value SHALL equal the original

### Requirement: Protocol state machine
The pane-proto crate SHALL define a `ProtocolState` type that tracks the current state of a client↔compositor connection. States SHALL include Disconnected, Connected, and Active (pane created). The state machine SHALL reject invalid transitions with descriptive error messages.

#### Scenario: Valid lifecycle sequence
- **WHEN** a client sends Create while Connected
- **THEN** the state machine SHALL transition to Active and return Ok

#### Scenario: Invalid sequence rejected
- **WHEN** a client sends WriteCells while in Connected state (no pane created)
- **THEN** the state machine SHALL return an error indicating a pane must be created first

#### Scenario: State machine fuzz safety
- **WHEN** an arbitrary sequence of `PaneRequest` messages is applied to a fresh ProtocolState
- **THEN** the state machine SHALL never panic, only return Ok or descriptive Err

### Requirement: Pane identification
The pane-proto crate SHALL define a `PaneId` newtype wrapping a non-zero u32. PaneIds SHALL be opaque to clients — assigned by the compositor and returned in create responses.

#### Scenario: PaneId type safety
- **WHEN** code attempts to use a raw u32 where a PaneId is expected
- **THEN** the Rust compiler SHALL reject the code

### Requirement: Plumb message types
The pane-proto crate SHALL define a `PlumbMessage` struct with fields for source application, destination port, working directory, content type, attributes (key-value pairs), and data (the text fragment being plumbed).

#### Scenario: Plumb message round-trip
- **WHEN** any valid `PlumbMessage` is serialized with postcard and deserialized
- **THEN** the deserialized value SHALL equal the original

### Requirement: Postcard wire format
All protocol types SHALL derive `serde::Serialize` and `serde::Deserialize`. The canonical wire format SHALL be postcard. The pane-proto crate SHALL provide `serialize` and `deserialize` functions that use postcard.

#### Scenario: Compact encoding
- **WHEN** a simple PaneRequest (e.g., Close with a PaneId) is serialized
- **THEN** the output SHALL be no larger than 16 bytes

### Requirement: Property-based test coverage
The pane-proto crate SHALL derive `proptest::Arbitrary` for all message types and provide property tests for serialization round-trips and state machine invariant preservation.

#### Scenario: Arbitrary message generation
- **WHEN** proptest generates 1000 random `PaneRequest` values
- **THEN** all 1000 SHALL successfully serialize and deserialize without error
