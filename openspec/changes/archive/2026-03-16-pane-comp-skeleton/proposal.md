## Why

pane-proto defines the types. Now we need something that puts pixels on screen. The compositor is the central server — it owns the display, renders cell grids, draws tag lines and chrome, accepts pane client connections, and dispatches input. Without it, everything else is theoretical. This is build sequence step 2.

The goal for this skeleton is narrow: get a smithay compositor running with a single hardcoded pane showing a tag line and cell grid body. No client protocol yet, no tiling, no input dispatch to panes. Just proof that the rendering pipeline works — compositor boots, claims a display, renders cells from pane-proto types.

## What Changes

- Create the `pane-comp` crate in the workspace
- Implement a minimal smithay compositor using the winit backend (for development — DRM/KMS comes later)
- Implement a GPU-accelerated cell grid renderer: glyph atlas, cell-to-quad mapping, font loading
- Render a single hardcoded pane with a tag line and body content using pane-proto's Cell/CellRegion types
- Draw pane chrome: beveled borders, tag line background, focus indicator
- Set up the calloop event loop integrating smithay's Wayland state

## Capabilities

### New Capabilities
- `pane-compositor`: Core compositor server — display management, cell grid rendering, pane chrome, calloop event loop
- `cell-grid-renderer`: GPU-accelerated text rendering from pane-proto Cell types — glyph atlas, font loading, quad rendering

### Modified Capabilities
- `architecture`: Build sequence phase 2 initiated

## Impact

- New crate `crates/pane-comp/` added to workspace
- Dependencies introduced: smithay (with winit, renderer_glow features), calloop, font loading (cosmic-text or fontdue or similar)
- First visual output — running `cargo run -p pane-comp` opens a window with a rendered pane
- Establishes the rendering architecture that all future visual work builds on
