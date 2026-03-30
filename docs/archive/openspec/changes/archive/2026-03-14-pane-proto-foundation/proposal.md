## Why

pane-proto is the foundation crate that every other component in the pane desktop environment depends on. No server can accept connections, no client can send messages, no test can verify protocol correctness until the wire types, serialization format, and protocol state machine exist. This is build sequence step 1 — everything else blocks on it.

## What Changes

- Create the `pane-proto` Rust crate as the root of the pane workspace
- Define algebraic message types for client↔compositor communication (PaneRequest, PaneEvent)
- Define cell grid types (Cell, CellRegion, attributes, colors)
- Define plumb message types (PlumbMessage, PlumbRule)
- Define control types (PaneId, PaneKind, tag/body/event/ctl channel messages)
- Implement postcard-based serialization for all types
- Implement a protocol state machine with typed state transitions
- Set up property-based testing (proptest) for round-trip serialization and state machine invariants
- Establish the Cargo workspace structure for the broader pane project

## Capabilities

### New Capabilities
- `pane-protocol`: Wire types, message enums, serialization, and protocol state machine for pane client↔server communication
- `cell-grid-types`: Cell, color, attribute, and region types that define the native rendering data model

### Modified Capabilities
- `architecture`: Build sequence initiated — workspace structure established

## Impact

- Creates the `Cargo.toml` workspace at project root
- Creates `crates/pane-proto/` with its own `Cargo.toml`
- Dependencies introduced: `serde`, `postcard`, `proptest` (dev)
- All future crates (pane-comp, pane-app, pane-shell, etc.) will depend on pane-proto
- The message types defined here become the public API contract for the entire system
