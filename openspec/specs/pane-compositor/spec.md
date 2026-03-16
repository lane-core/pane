## ADDED Requirements

### Requirement: Compositor boots with winit backend
The pane-comp binary SHALL initialize a smithay compositor using the winit backend, creating a window on the host desktop. The compositor SHALL run a calloop event loop that processes display events.

#### Scenario: Compositor launches
- **WHEN** `cargo run -p pane-comp` is executed
- **THEN** a window SHALL appear on the host desktop displaying the compositor output

#### Scenario: Clean shutdown
- **WHEN** the compositor window is closed
- **THEN** the process SHALL exit cleanly without panics or resource leaks

### Requirement: Pane chrome rendering
The compositor SHALL draw pane chrome (tag line, borders) around pane content. Chrome is rendered by the compositor, not by clients. The tag line SHALL be a monospace text bar above the pane body. Borders SHALL use a beveled style (light/dark edge colors) to create visible structure.

#### Scenario: Tag line visible
- **WHEN** a pane is displayed
- **THEN** a tag line with text SHALL be rendered above the pane body on a distinct background color

#### Scenario: Borders visible
- **WHEN** a pane is displayed
- **THEN** beveled borders SHALL be visible around the pane, distinguishing it from the background

### Requirement: Calloop event loop
The compositor SHALL use calloop as its event loop, integrating smithay's Wayland event sources. The loop SHALL process display events each iteration and trigger frame rendering.

#### Scenario: Frame rendering
- **WHEN** the event loop runs
- **THEN** the compositor SHALL render a frame on each display refresh cycle
