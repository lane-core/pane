## ADDED Requirements

### Requirement: TypedView trait
pane-proto SHALL define a `TypedView` trait for parsing a `PaneMessage<ServerVerb>` into a validated typed struct. The trait SHALL provide a `parse` method that validates the verb, checks for required attrs, verifies attr types, and returns either the typed view or a `ViewError`.

```rust
trait TypedView: Sized {
    fn parse(msg: &PaneMessage<ServerVerb>) -> Result<Self, ViewError>;
}
```

**Polarity**: Boundary (transforms a Value message into a typed accessor)
**Crate**: `pane-proto::server::views`

#### Scenario: Parse valid message
- **WHEN** a `PaneMessage<ServerVerb>` with correct verb and all required attrs is passed to `TypedView::parse`
- **THEN** it SHALL return Ok with the typed view providing accessor methods for each field

#### Scenario: Parse message with wrong verb
- **WHEN** a `PaneMessage<ServerVerb::Query>` is passed to a view expecting `ServerVerb::Command`
- **THEN** it SHALL return `ViewError::WrongVerb`

#### Scenario: Parse message with missing required attr
- **WHEN** a message missing the `"data"` attr is parsed as a `RouteCommand`
- **THEN** it SHALL return `ViewError::MissingField("data")`

### Requirement: TypedBuilder pattern
pane-proto SHALL provide a builder pattern for constructing `PaneMessage<ServerVerb>` with compile-time enforcement of required fields. The builder SHALL use typestate to make missing required fields a compile error, not a runtime error.

**Polarity**: Value (constructs a message)
**Crate**: `pane-proto::server::views`

#### Scenario: Builder enforces required fields
- **WHEN** code calls `RouteCommand::build().wdir("/src").into_message()` without calling `.data()`
- **THEN** the compiler SHALL reject the code (missing required field)

#### Scenario: Builder produces valid message
- **WHEN** `RouteCommand::build().data("parse.c:42").wdir("/src").src("pane-comp").into_message()` is called
- **THEN** the result SHALL be a `PaneMessage<ServerVerb>` with `core: ServerVerb::Command` and attrs containing data, wdir, src fields

### Requirement: ViewError type
pane-proto SHALL define a `ViewError` enum with variants for each failure mode: wrong verb, missing required field, wrong field type, invalid field value.

**Polarity**: Value
**Crate**: `pane-proto::server::views`

#### Scenario: Error messages are descriptive
- **WHEN** a `ViewError::MissingField("data")` is displayed
- **THEN** the message SHALL identify which field is missing and which view expected it

### Requirement: RouteCommand view
pane-proto SHALL define a `RouteCommand` typed view for the "route a text fragment" inter-server message. Required fields: `data` (string — the text to route), `wdir` (string — working directory). Optional fields: `src` (string — source application), `content_type` (string — content type hint).

**Polarity**: Value
**Crate**: `pane-proto::server::route`

#### Scenario: RouteCommand round-trip
- **WHEN** a `RouteCommand` is built, serialized to `PaneMessage<ServerVerb>`, serialized to bytes, deserialized, and parsed back into `RouteCommand`
- **THEN** all fields SHALL match the original values

### Requirement: RouteQuery view
pane-proto SHALL define a `RouteQuery` typed view for querying available handlers for content. Required fields: `data` (string — the text to match). Optional fields: `content_type` (string — type hint).

**Polarity**: Value
**Crate**: `pane-proto::server::route`

#### Scenario: RouteQuery produces Query verb
- **WHEN** `RouteQuery::build().data("parse.c:42").into_message()` is called
- **THEN** the message's core SHALL be `ServerVerb::Query`

### Requirement: RosterRegister view
pane-proto SHALL define a `RosterRegister` typed view for infrastructure server registration. Required fields: `signature` (string — app signature), `kind` (string — "infrastructure" or "application"). Optional fields: `socket` (string — socket path for connecting to this server).

**Polarity**: Value
**Crate**: `pane-proto::server::roster`

#### Scenario: Infrastructure server registration
- **WHEN** pane-route starts and registers with pane-roster
- **THEN** it SHALL send a `PaneMessage<ServerVerb::Notify>` parseable as `RosterRegister` with `kind: "infrastructure"`

### Requirement: RosterServiceRegister view
pane-proto SHALL define a `RosterServiceRegister` typed view for registering a discoverable operation. Required fields: `operation` (string — operation name), `content_type` (string — content type pattern the operation handles), `description` (string — human-readable description).

**Polarity**: Value
**Crate**: `pane-proto::server::roster`

#### Scenario: Service registration
- **WHEN** an editor registers a "format-json" service
- **THEN** it SHALL send a `PaneMessage<ServerVerb::Notify>` parseable as `RosterServiceRegister` with `operation: "format-json"` and `content_type: "application/json"`
