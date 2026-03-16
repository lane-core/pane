## MODIFIED Requirements

### Requirement: Build sequence phase 2 — pane-comp skeleton
The pane project workspace SHALL include a `pane-comp` binary crate under `crates/pane-comp/`. The pane-comp crate SHALL depend on pane-proto for cell and protocol types, smithay for Wayland compositor functionality, and calloop for event loop management.

#### Scenario: Compositor builds
- **WHEN** `cargo build -p pane-comp` is run
- **THEN** pane-comp SHALL compile without errors

#### Scenario: Compositor runs
- **WHEN** `cargo run -p pane-comp` is executed on a system with a running Wayland or X11 session
- **THEN** a window SHALL appear displaying a rendered pane with tag line and cell grid
