## 1. Protocol types

- [x] 1.1 Add `PaneKind::Widget` variant
- [x] 1.2 Define `TagLine` struct (name, actions, user_actions) and `TagAction`/`TagCommand` types
- [x] 1.3 Define `WidgetNode` enum with core widget types (Button, Label, TextInput, Slider, Checkbox, List, HBox, VBox, Scroll, Separator, CellGrid)
- [x] 1.4 Define `WidgetEvent` enum (Clicked, ValueChanged, TextChanged, Selected, etc.)
- [x] 1.5 Add serde derives and proptest Arbitrary for all new types
- [x] 1.6 Update `PaneRequest` to include SetWidgetTree variant
- [x] 1.7 Update `PaneEvent` to include Widget variant carrying WidgetEvent
- [x] 1.8 Verify `cargo build && cargo test`

## 2. TagLine protocol integration

- [x] 2.1 Replace raw tag String in PaneRequest::SetTag with TagLine struct
- [x] 2.2 Update PaneEvent::TagExecute and TagRoute to reference TagAction
- [x] 2.3 Update existing tests for TagLine changes
- [x] 2.4 Verify `cargo build && cargo test`

## 3. Architecture spec update

- [x] 3.1 Rewrite aesthetic section: Frutiger Aero, BeOS density + Aqua refinement
- [x] 3.2 Add dual content model (CellGrid + Widget + Surface) to architecture
- [x] 3.3 Add taffy, femtovg, agility to technology section
- [x] 3.4 Document TagLine dual presentation (text in cell grids, graphical in widget panes)
- [x] 3.5 Update build sequence to include widget rendering phase
