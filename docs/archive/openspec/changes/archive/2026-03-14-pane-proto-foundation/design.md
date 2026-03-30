## Context

Pane is a new Wayland compositor/DE built in Rust with smithay. The architecture spec establishes a server/kit decomposition where all components communicate via a typed message protocol over unix sockets. `pane-proto` is the foundation crate — pure types and serialization with no runtime dependencies. Every other crate in the system will depend on it.

There is no existing codebase. This is the first code written for the project. The Cargo workspace structure established here sets the pattern for all future crates.

## Goals / Non-Goals

**Goals:**
- Define the complete message vocabulary for pane client↔compositor communication
- Make the protocol self-documenting through Rust's type system (enums, not magic numbers)
- Ensure serialization round-trips are correct via property-based testing
- Establish a protocol state machine that prevents invalid message sequences
- Set up the Cargo workspace so future crates slot in cleanly

**Non-Goals:**
- Networking / transport layer (that's pane-comp's responsibility — pane-proto is wire types only)
- Async runtime integration (pane-proto is pure types, no I/O)
- The Looper/Handler actor model (that's pane-app, built on calloop)
- Plumber rule matching logic (pane-plumb — pane-proto only defines the PlumbMessage type)
- Rendering or compositor logic
- VT/ANSI escape sequence parsing (that's pane-shell)

## Decisions

### 1. Flat crate under `crates/` workspace directory

All pane crates live under `crates/`. The workspace `Cargo.toml` is at the project root. This is the standard Rust workspace pattern (used by smithay, wgpu, bevy, etc.) and avoids polluting the root with per-crate dirs.

**Alternative considered:** Monorepo with `pane-proto` at root. Rejected — doesn't scale to the 5+ crates we'll have.

### 2. postcard as wire format

postcard is serde-based, varint-encoded, no-std compatible, and designed for embedded/IPC use. It produces compact output without schema overhead.

**Alternatives considered:**
- bincode: Larger output, fixed-width integers. No advantage for IPC.
- MessagePack (rmp-serde): Self-describing but heavier. We don't need schema evolution in the wire format — the Rust types are the schema.
- capnp/flatbuffers: Zero-copy but complex build tooling and generated code. Overkill for a single-machine IPC protocol.

### 3. Enums as the message taxonomy

Two top-level enums — `PaneRequest` (client → compositor) and `PaneEvent` (compositor → client) — with one variant per operation. This gives exhaustive match checking, serde derives, and proptest `Arbitrary` derives.

**Alternative considered:** Trait objects / dynamic dispatch. Rejected — loses exhaustiveness checking, which is the primary correctness mechanism.

### 4. Protocol state machine as a separate type, not typestate

A runtime-checked `ProtocolState` struct that validates transitions, rather than compile-time typestate encoding. Typestate is elegant but makes the protocol type non-object-safe and hard to store in collections. The state machine is small enough that runtime checking with good error messages is sufficient, and it's directly testable with proptest.

**Alternative considered:** Full typestate (Connected/Creating/Active as separate types). Rejected for ergonomics — calloop handlers need to store the connection in a single field, which requires a uniform type.

### 5. Cell types model terminal semantics

A `Cell` is a character + foreground + background + attributes (bold, italic, underline, etc.). A `CellRegion` is a positioned rectangle of cells. This mirrors terminal semantics intentionally — pane-shell will translate VT sequences directly into CellRegion writes. Colors use a 256-color + RGB model (matching xterm-256color).

### 6. PaneId as a newtype over u32

Opaque, non-zero, compositor-assigned. Newtypes prevent mixing pane IDs with other integers. The compositor assigns IDs; clients receive them.

## Risks / Trade-offs

**[Protocol evolution]** → Adding new enum variants is a breaking change for postcard deserialization. Mitigation: pane-proto versions are coupled to compositor versions. This is a desktop system, not a distributed service — client and server are always co-deployed. If we later need backwards compatibility, we can add a version handshake.

**[Type completeness]** → We can't know every message type until we build pane-comp and pane-shell. Mitigation: Define the core set now (lifecycle, cells, tags, events, ctl), expect to extend as later crates reveal needs. The enum is addable, not locked.

**[Cell grid vs. surface boundary]** → The protocol needs to handle panes that are part cell-grid and part surface (hybrid). Mitigation: Defer hybrid pane support. For pane-proto v0, a pane is either cell-grid or surface, not both. The type system can accommodate a `PaneKind::Hybrid` variant later.
