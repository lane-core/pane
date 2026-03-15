## MODIFIED Requirements

### Requirement: Message wrapper with attributes
All protocol messages SHALL be wrapped in a `PaneMessage<T>` type that carries the typed core message and an open-ended attributes bag. The attributes bag SHALL be a `Vec<(String, AttrValue)>` preserving insertion order and allowing duplicate keys. `AttrValue` SHALL support String, Int, Float, Bool, Bytes, and nested Attrs variants.

#### Scenario: Attrs round-trip
- **WHEN** a PaneMessage with attrs is serialized and deserialized
- **THEN** all attrs SHALL be preserved in order with their types intact

#### Scenario: Attrs extensibility
- **WHEN** a plumber attaches an `addr=42` attr to a message
- **THEN** the receiving client SHALL be able to read the attr via `msg.attr("addr")` without the attr being part of the core enum

#### Scenario: Nested attrs
- **WHEN** a drag-and-drop message carries nested attrs (e.g., file metadata with sub-attributes)
- **THEN** the nested structure SHALL serialize, deserialize, and be accessible via optics-style accessors

### Requirement: Protocol state machine — PendingCreate state
The protocol state machine SHALL include a PendingCreate state. `apply(Create)` on Connected SHALL transition to PendingCreate. Only `activate(pane_id, kind)` SHALL transition PendingCreate to Active. A second Create while in PendingCreate SHALL be rejected with a descriptive error.

#### Scenario: Create then activate
- **WHEN** a client sends Create while Connected
- **THEN** the state SHALL transition to PendingCreate
- **WHEN** the compositor responds with Created
- **THEN** `activate()` SHALL transition to Active

#### Scenario: Double create rejected
- **WHEN** a client sends Create while in PendingCreate
- **THEN** the state machine SHALL return an error indicating a create is already in flight

### Requirement: PaneKind tracked in Active state
`ProtocolState::Active` SHALL track the `PaneKind` of the active pane. The state machine SHALL reject `WriteCells` and `Scroll` requests when the active pane's kind is `Surface`.

#### Scenario: WriteCells rejected for Surface pane
- **WHEN** a client sends WriteCells to a Surface-kind pane
- **THEN** the state machine SHALL return an error indicating cell operations are not valid for surface panes

### Requirement: Connect idempotency
`connect()` SHALL transition Disconnected to Connected. Calling `connect()` on any other state SHALL return an error, not silently succeed.

#### Scenario: Double connect rejected
- **WHEN** `connect()` is called on an Active state
- **THEN** it SHALL return an error indicating the connection is already established

### Requirement: Frame length safety
The `frame()` function SHALL reject payloads larger than `u32::MAX` bytes with an explicit error, not silently truncate via `as u32` cast.

#### Scenario: Oversized payload
- **WHEN** `frame()` is called with a payload exceeding u32::MAX bytes
- **THEN** it SHALL return an error

### Requirement: Scroll delta convention
`PaneRequest::Scroll::delta` SHALL be documented: positive values scroll down (toward newer content), negative values scroll up. The unit SHALL be rows.

#### Scenario: Scroll down
- **WHEN** a client sends Scroll with delta=5
- **THEN** the compositor SHALL scroll the pane body down by 5 rows

### Requirement: Wire helpers exported
`frame()` and `frame_length()` SHALL be re-exported from the crate root alongside `serialize` and `deserialize`.

#### Scenario: Import from crate root
- **WHEN** a consumer writes `use pane_proto::frame`
- **THEN** it SHALL compile without needing to reference the wire module directly

### Requirement: ProtocolState is not a wire type
`ProtocolState` SHALL NOT derive `Serialize` or `Deserialize`. It is local per-connection tracking and SHALL NOT be sent on the wire or persisted.

#### Scenario: ProtocolState not serializable
- **WHEN** code attempts to serialize a ProtocolState
- **THEN** the compiler SHALL reject it

### Requirement: Value/Compute polarity markers
pane-proto SHALL define `Value` and `Compute` marker traits. Value types (PaneRequest, RouteMessage, CellRegion, Cell, AttrValue) SHALL implement `Value`. Compute types (PaneEvent handler patterns) SHALL implement `Compute`. These traits enable compile-time enforcement of valid composition polarity in downstream combinator APIs.

#### Scenario: Request implements Value
- **WHEN** code checks whether PaneRequest implements Value
- **THEN** the compiler SHALL confirm it does

#### Scenario: Polarity-aware composition
- **WHEN** a downstream combinator restricts composition to matching polarities
- **THEN** the Value and Compute marker traits SHALL provide the compile-time information needed

### Requirement: Multi-pane per connection
A single client connection SHALL support multiple active panes. `ProtocolState::Active` SHALL track a `HashMap<PaneId, PaneKind>` of active panes and a count of pending creates. Create SHALL increment the pending count. `activate()` SHALL decrement pending and insert into the pane map. Close SHALL remove from the pane map.

#### Scenario: Two panes on one connection
- **WHEN** a client sends Create twice (with activate between them)
- **THEN** both panes SHALL be tracked in the state machine's pane map

#### Scenario: Operations validated per-pane
- **WHEN** a client sends WriteCells with a PaneId not in the active pane map
- **THEN** the state machine SHALL return an error

### Requirement: Rename plumb to route
All plumb/plumber terminology SHALL be renamed to route/router. `PlumbMessage` → `RouteMessage`. `PaneEvent::Plumb` → `PaneEvent::Route`. `PaneEvent::TagPlumb` → `PaneEvent::TagRoute`.

#### Scenario: RouteMessage type exists
- **WHEN** code references `RouteMessage`
- **THEN** the compiler SHALL find the type (formerly PlumbMessage)

### Requirement: Inter-server protocol
Inter-server communication SHALL use `PaneMessage<ServerVerb>` where `ServerVerb` is an enum with variants Query, Notify, Command. The attrs bag SHALL carry the payload. Type safety SHALL be provided by typed view/builder patterns in per-server kit modules.

#### Scenario: Inter-server message round-trip
- **WHEN** a `PaneMessage<ServerVerb>` is serialized and deserialized
- **THEN** the verb and all attrs SHALL be preserved

#### Scenario: Typed view validates attrs
- **WHEN** a typed view's `parse()` method is called on a message missing required attrs
- **THEN** it SHALL return a descriptive error
