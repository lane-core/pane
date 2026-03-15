## 1. Workspace and crate scaffold

- [x] 1.1 Create root `Cargo.toml` with workspace definition pointing to `crates/*`
- [x] 1.2 Create `crates/pane-proto/Cargo.toml` with dependencies: serde (derive), postcard, proptest (dev)
- [x] 1.3 Create `crates/pane-proto/src/lib.rs` with module declarations
- [x] 1.4 Verify `cargo build` and `cargo test` pass on the empty crate

## 2. Core identity and primitive types

- [x] 2.1 Define `PaneId` newtype (NonZeroU32) with serde + Arbitrary derives
- [x] 2.2 Define `PaneKind` enum (CellGrid, Surface) with serde + Arbitrary derives
- [x] 2.3 Define `Color` enum (Default, Named, Indexed, Rgb) with serde + Arbitrary derives
- [x] 2.4 Define `CellAttrs` bitflags (bold, dim, italic, underline, blink, reverse, hidden, strikethrough) with serde + Arbitrary derives
- [x] 2.5 Define `Cell` struct (char, fg Color, bg Color, attrs CellAttrs) with serde + Arbitrary derives

## 3. Cell grid and input types

- [x] 3.1 Define `CellRegion` struct (col, row, width, cells Vec) with serde + Arbitrary derives
- [x] 3.2 Define `KeyEvent` struct (key, modifiers, press/release) with serde + Arbitrary derives
- [x] 3.3 Define `MouseEvent` struct (col, row, button, modifiers, event kind) with serde + Arbitrary derives

## 4. Protocol message enums

- [x] 4.1 Define `PaneRequest` enum with variants: Create, Close, WriteCells, Scroll, SetTag, SetDirty, RequestGeometry
- [x] 4.2 Define `PaneEvent` enum with variants: Key, Mouse, Resize, Focus, CloseRequested, TagExecute, TagPlumb, Plumb
- [x] 4.3 Define `PlumbMessage` struct (src, dst, wdir, content_type, attrs, data)
- [x] 4.4 Implement serde + Arbitrary derives for all message types

## 5. Serialization helpers

- [x] 5.1 Create `wire` module with `serialize<T: Serialize>` and `deserialize<T: DeserializeOwned>` functions wrapping postcard
- [x] 5.2 Add length-prefixed framing helpers (write u32 length prefix, then payload) for stream-oriented transports

## 6. Protocol state machine

- [x] 6.1 Define `ProtocolState` enum (Disconnected, Connected, Active { pane_id })
- [x] 6.2 Implement `ProtocolState::apply(request) -> Result<ProtocolState, ProtocolError>` with valid transition logic
- [x] 6.3 Define `ProtocolError` enum with descriptive variants for each invalid transition

## 7. Property-based tests

- [x] 7.1 Round-trip serialization test for all types: serialize then deserialize, assert equality
- [x] 7.2 State machine fuzz test: apply arbitrary sequence of PaneRequests, assert no panics
- [x] 7.3 State machine invariant test: after Create, state is Active; after Close, state is Connected
- [x] 7.4 Verify `cargo test` passes with all property tests
