## 1. Architecture spec full rewrite

- [x] 1.1 Rewrite architecture spec as single coherent document incorporating all decisions from v1, compositional-interfaces, and v2
- [x] 1.2 Rename plumb/plumber to route/router throughout (pane-plumb → pane-route, PlumbMessage → RouteMessage, etc.)
- [x] 1.3 Merge pillar 5 (Declarative State) into pillar 6 (Filesystem as Interface), add caching invariant
- [x] 1.4 Add "Target Platform" section: Linux-only, latest kernel, s6/runit, filesystem requirements
- [x] 1.5 Add "Filesystem as Interface" as design pillar: config-as-files, plugin discovery, FUSE at /srv/pane/, xattr-as-metadata, caching invariant
- [x] 1.6 Add Value/Compute polarity framework to Compositional Interfaces pillar (sequent calculus grounding, dual representations, cut as dispatch)
- [x] 1.7 Add pane-notify to servers section (fanotify/inotify abstraction)
- [x] 1.8 Add pane-fs to servers section (FUSE at /srv/pane/, format-per-endpoint)
- [x] 1.9 Remove pane-input from servers (input handling is a module within pane-comp, IME add-ons are separate processes)
- [x] 1.10 Update pane-roster with hybrid model (init supervises infrastructure, roster supervises apps, service registry)
- [x] 1.11 Update pane-route with service-aware multi-match (scratchpad chooser pane)
- [x] 1.12 Add filesystem-based plugin discovery section
- [x] 1.13 Add filesystem-as-configuration section
- [x] 1.14 Update protocol section: PaneMessage wrapper with attrs, multi-pane per connection, state machine with pending creates + pane map
- [x] 1.15 Add pane-shell architectural notes (xterm-256color, screen buffer model, alternate screen, incremental dirty regions)
- [x] 1.16 Note accessibility as a known gap
- [x] 1.17 Add inter-server protocol: ServerVerb + attrs with typed views/builders (BMessage-inspired)
- [x] 1.18 Clarify compositor↔router relationship (native panes route via client kit, legacy panes route directly)
- [x] 1.19 Update technology section with pane-notify, FUSE, s6/runit, optics, Value/Compute traits
- [x] 1.20 Update build sequence to include pane-notify, pane-fs, and reorder phases

## 2. pane-proto: PaneMessage wrapper

- [x] 2.1 Define `AttrValue` enum (String, Int, Float, Bool, Bytes, Attrs) with serde derives
- [x] 2.2 Define `PaneMessage<T>` struct wrapping core T + attrs Vec
- [x] 2.3 Implement optics-style accessor methods: `attr()`, `attrs_all()`, `set_attr()`, `insert_attr()`
- [x] 2.4 Add `optics` crate as dependency
- [x] 2.5 Update `PaneRequest`/`PaneEvent` usage to go through `PaneMessage` wrapper
- [x] 2.6 Update serialization helpers and tests for `PaneMessage`

## 3. pane-proto: Polarity marker traits

- [x] 3.1 Define `Value` marker trait for constructed/data types (PaneRequest, RouteMessage, CellRegion, AttrValue)
- [x] 3.2 Define `Compute` marker trait for observed/codata types (event handler patterns)
- [x] 3.3 Implement Value for PaneRequest, RouteMessage, CellRegion, Cell, AttrValue
- [x] 3.4 Implement Compute for PaneEvent

## 4. pane-proto: Rename plumb to route

- [x] 4.1 Rename `PlumbMessage` → `RouteMessage` with fields: src, dst, wdir, content_type, attrs, data
- [x] 4.2 Rename `PaneEvent::Plumb` → `PaneEvent::Route`
- [x] 4.3 Rename `PaneEvent::TagPlumb` → `PaneEvent::TagRoute`
- [x] 4.4 Update all tests for renamed types

## 5. pane-proto: Inter-server protocol types

- [x] 5.1 Define `ServerVerb` enum (Query, Notify, Command)
- [x] 5.2 Define inter-server message as `PaneMessage<ServerVerb>`
- [x] 5.3 Add typed view/builder pattern: example `RouteCommand` struct with `parse()` and `build()` methods over attrs bag

## 6. pane-proto: State machine redesign (multi-pane)

- [x] 6.1 Redesign ProtocolState: Disconnected → Active { panes: HashMap<PaneId, PaneKind>, pending_creates: u32 }
- [x] 6.2 connect() transitions Disconnected → Active with empty pane map, errors on non-Disconnected
- [x] 6.3 apply(Create) increments pending_creates, apply(Close) removes from pane map
- [x] 6.4 activate(id, kind) decrements pending_creates, inserts into pane map
- [x] 6.5 Validate PaneId and PaneKind on all pane-scoped operations
- [x] 6.6 Remove Serialize/Deserialize derives from ProtocolState

## 7. pane-proto: Type and wire fixes

- [x] 7.1 Add `height: u16` to CellRegion, add constructor that validates cells.len() == width * height
- [x] 7.2 Replace `NamedKey::F(u8)` with `NamedKey::F(FKey)` where FKey is a newtype with TryFrom<u8> validating 1-24
- [x] 7.3 Fix `frame()` to use `try_into::<u32>()` with error instead of `as u32`
- [x] 7.4 Re-export `frame` and `frame_length` from crate root
- [x] 7.5 Add doc comment to `Scroll::delta` specifying sign convention and unit

## 8. Update tests

- [x] 8.1 Update roundtrip tests for PaneMessage wrapper and AttrValue
- [x] 8.2 Update roundtrip tests for renamed RouteMessage
- [x] 8.3 Update state machine tests for multi-pane model (create multiple panes, close individually)
- [x] 8.4 Update CellRegion tests for height field and validation
- [x] 8.5 Update FKey generation in proptest strategies
- [x] 8.6 Add test: connect() on non-Disconnected returns error
- [x] 8.7 Add test: WriteCells on Surface pane returns error
- [x] 8.8 Add test: ServerVerb message round-trip
- [x] 8.9 Add test: typed view parse/build round-trip for RouteCommand
- [x] 8.10 Verify all tests pass
