## MODIFIED Requirements

### Requirement: TagLine as structured data
The tag line SHALL be represented as structured data, not a raw string. The `TagLine` struct SHALL contain: a name (pane identity), a list of built-in actions, a list of user-defined actions, and an editable text region.

Cell grid panes SHALL render the tag line as monospace text (joining labels with spaces, `|` between built-in and user sections). Widget panes SHALL render the tag line as a graphical tab with proportional-font buttons alongside an editable text region. Both presentations derive from the same `TagLine` data.

```rust
struct TagLine {
    name: String,
    actions: Vec<TagAction>,
    user_actions: Vec<TagAction>,
}
struct TagAction {
    label: String,
    command: TagCommand,
}
enum TagCommand {
    BuiltIn(BuiltInAction),
    Shell(String),
    Route(String),
}
```

**Polarity**: Value
**Crate**: `pane-proto::message`

#### Scenario: Cell grid tag rendering
- **WHEN** a cell grid pane has TagLine { name: "~/src", actions: [Del, Get, Put], user_actions: [cargo build] }
- **THEN** the compositor SHALL render: `~/src  Del Get Put | cargo build` in monospace

#### Scenario: Widget tag rendering
- **WHEN** a widget pane has the same TagLine data
- **THEN** the compositor SHALL render: a tab labeled "~/src", beveled buttons for Del/Get/Put, and a text region for user commands

### Requirement: Tag line interactivity
In cell grid panes, tag actions are B2-clickable text (existing behavior). In widget panes, tag action buttons respond to left-click (natural widget expectation) AND B2-click (power-user consistency). B3-click on any tag text routes it, regardless of presentation.

#### Scenario: Left-click on widget tag button
- **WHEN** a user left-clicks a "Del" button in a widget pane's tag
- **THEN** the pane SHALL close (same as B2-clicking "Del" in a cell grid tag)

#### Scenario: B3 on tag button label
- **WHEN** a user B3-clicks the text "cargo build" in a widget pane's tag button
- **THEN** the text "cargo build" SHALL be routed

### Requirement: PaneKind includes Widget
`PaneKind` SHALL include a `Widget` variant alongside `CellGrid` and `Surface`.

#### Scenario: Widget pane in protocol state
- **WHEN** a client creates a Widget pane and the compositor activates it
- **THEN** `ProtocolState::Active` SHALL track the pane with `kind: PaneKind::Widget`
