## ADDED Requirements

### Requirement: Widget pane content model
Pane SHALL support a `PaneKind::Widget` content model alongside `CellGrid` and `Surface`. Widget pane clients send a `WidgetNode` tree description to the compositor. The compositor renders the widget tree using its own rendering pipeline. Clients never draw pixels.

**Polarity**: Value (WidgetNode tree is constructed data sent to compositor)
**Crate**: `pane-proto::widget`

#### Scenario: Widget pane creation
- **WHEN** a client sends Create with `kind: PaneKind::Widget`
- **THEN** the compositor SHALL accept the pane and render widget content from subsequent WidgetNode tree updates

### Requirement: WidgetNode tree type
pane-proto SHALL define a `WidgetNode` enum describing composable widget elements. The compositor renders the tree; the client describes it.

Core widget types:
- `Button { label, id }` — clickable control
- `Label { text }` — static text
- `TextInput { value, placeholder, id }` — editable text field
- `Slider { min, max, value, id }` — range control
- `Checkbox { label, checked, id }` — boolean toggle
- `List { items, selected, id }` — selectable list
- `HBox { children, spacing }` — horizontal layout
- `VBox { children, spacing }` — vertical layout
- `Scroll { child }` — scrollable container
- `Separator` — visual divider
- `CellGrid { region }` — embedded cell grid region (hybrid panes)

**Polarity**: Value
**Crate**: `pane-proto::widget`

#### Scenario: Preferences panel
- **WHEN** a preferences app sends a VBox containing Labels, Sliders, and Checkboxes
- **THEN** the compositor SHALL render them as a vertical form with appropriate controls

#### Scenario: Hybrid pane
- **WHEN** a widget tree contains a `CellGrid` node alongside Labels and Buttons
- **THEN** the compositor SHALL render the cell grid inline within the widget layout

### Requirement: Widget events
When users interact with widget controls, the compositor SHALL send `WidgetEvent` messages back to the client containing the widget `id` and the interaction.

**Polarity**: Compute (events are consumed by handlers)
**Crate**: `pane-proto::widget`

#### Scenario: Button click
- **WHEN** a user clicks a Button with id=5
- **THEN** the compositor SHALL send `WidgetEvent::Clicked { id: 5 }` to the client

#### Scenario: Slider change
- **WHEN** a user drags a Slider with id=3 to value 0.75
- **THEN** the compositor SHALL send `WidgetEvent::ValueChanged { id: 3, value: 0.75 }` to the client

### Requirement: Layout via taffy
The compositor SHALL use the taffy layout engine to compute widget positions and sizes from the WidgetNode tree. Clients describe layout intent (flex direction, spacing, sizing constraints). The compositor computes geometry.

#### Scenario: Flexible layout
- **WHEN** a widget tree specifies an HBox with two children, one flex-grow and one fixed-width
- **THEN** taffy SHALL compute the flexible child's width to fill remaining space

### Requirement: Widget rendering via femtovg
The compositor SHALL render widget controls using femtovg (or equivalent 2D vector renderer) within the existing GL context. Widget rendering shares the same frame as cell grid rendering and surface compositing.

#### Scenario: Rounded button rendering
- **WHEN** a Button is rendered
- **THEN** it SHALL be drawn as a rounded rectangle with gradient fill, highlight/shadow edges, and proportional text label

### Requirement: Reactive widget state
pane-ui kit SHALL support reactive widget state via agility signals. Widget values (text input content, slider position, checkbox state) SHALL be expressible as signals. Signal changes trigger widget tree updates sent to the compositor.

**Crate**: `pane-ui`

#### Scenario: Slider bound to signal
- **WHEN** a client creates a Slider bound to a `Signal<f32>`
- **THEN** updating the signal SHALL cause the widget tree to re-send the updated slider value to the compositor
