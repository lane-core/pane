## MODIFIED Requirements

### Requirement: Build sequence phase 1 — pane-proto
The pane project SHALL maintain a Cargo workspace at the project root. The first crate in the workspace SHALL be `pane-proto` under `crates/pane-proto/`. The pane-proto crate SHALL have no runtime dependencies beyond serde and postcard, and SHALL be usable by all other pane crates as a dependency.

#### Scenario: Workspace builds
- **WHEN** `cargo build` is run at the workspace root
- **THEN** pane-proto SHALL compile without errors

#### Scenario: No runtime coupling
- **WHEN** pane-proto's dependency tree is examined
- **THEN** it SHALL NOT depend on smithay, calloop, wayland-server, or any other compositor/runtime crate
