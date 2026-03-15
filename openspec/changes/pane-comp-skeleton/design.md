## Context

pane-proto exists with wire types, cell grid types, and a protocol state machine. The next step is a compositor that can render those types visually. This skeleton is deliberately minimal — one hardcoded pane, no client connections, no tiling — to prove the rendering pipeline before adding complexity.

smithay 0.7.0 is the compositor library. It provides Wayland protocol handling, backend abstraction (winit for dev, DRM/KMS for production), and integrates with calloop. Reference compositors: Anvil (smithay's example), cosmic-comp (System76), niri.

## Goals / Non-Goals

**Goals:**
- Boot a smithay compositor with the winit backend (opens a window on the host desktop)
- Render a cell grid from pane-proto Cell types using GPU-accelerated text
- Draw a tag line above the cell body
- Draw pane chrome (beveled borders, background colors)
- Establish the calloop event loop structure that all future compositor work builds on

**Non-Goals:**
- Pane protocol server (no client connections — pane content is hardcoded)
- Tiling layout or multiple panes
- Input dispatch to panes (keyboard/mouse events)
- DRM/KMS backend (winit only for now)
- XWayland or legacy client support
- The Looper/Handler actor abstraction (that's pane-app)

## Decisions

### 1. Winit backend for development

smithay's winit backend runs the compositor as a window on an existing desktop. This means we can develop and test without switching TTYs or risking display lockups. DRM/KMS is added later once the rendering pipeline is proven.

**Alternative considered:** DRM/KMS from the start. Rejected — slower development iteration, risk of display lockouts during debugging.

### 2. OpenGL (glow) for rendering

smithay's `renderer_glow` feature provides an OpenGL ES renderer. We'll use this for the cell grid: build a glyph atlas texture, map cells to textured quads. This is the same approach alacritty uses — proven to be fast for terminal-style rendering.

**Alternative considered:** Vulkan via smithay. Rejected — more complex setup, OpenGL is sufficient for 2D text rendering and better supported across hardware. Can migrate later if needed.

**Alternative considered:** Pixman (software rendering). Rejected — too slow for large cell grids at 60fps.

### 3. cosmic-text for font shaping and rasterization

cosmic-text provides font loading, shaping (via rustybuzz/swash), and rasterization in pure Rust. It handles Unicode, ligatures, fallback fonts, and subpixel positioning. Used by cosmic-comp and other Rust GUI projects.

**Alternatives considered:**
- fontdue: Simpler but no shaping (no ligatures, no complex scripts). Too limited.
- ab_glyph: No shaping. Same limitation.
- Direct freetype/harfbuzz bindings: C dependencies, more setup. cosmic-text wraps these concerns in pure Rust.

### 4. Glyph atlas architecture

On startup (and when font/size changes): rasterize the ASCII range + commonly-used glyphs into a texture atlas. Cache glyph metrics (advance, bearing). On each frame: for each cell in the grid, look up the glyph in the atlas, emit a textured quad with the cell's fg/bg colors.

Glyphs not in the atlas are rasterized on demand and inserted. The atlas grows as needed (or uses multiple textures if it fills up).

### 5. Hardcoded pane content for skeleton

The skeleton creates a single pane with:
- Tag: `~/src/pane  Del Snarf Get Put | Look`
- Body: a grid of cells showing a welcome message and some colored text to demonstrate the rendering pipeline

This proves the full path: Cell types → glyph atlas lookup → textured quads → composited frame. Client connections replace the hardcoded content in a future change.

### 6. Chrome rendering

Pane chrome is drawn by the compositor, not by clients (like BeOS's app_server):
- Tag line: monospace text on a colored background bar, visually distinct from the body
- Borders: 1-2px beveled lines around the pane (raised/sunken effect via light/dark edge colors)
- Focus indicator: border color changes when focused (only one pane in skeleton, always focused)

## Risks / Trade-offs

**[smithay API stability]** → smithay 0.7.0 is the latest stable release but the API has changed between major versions. Mitigation: pin to 0.7.x, follow smithay's patterns from Anvil. cosmic-comp and niri provide real-world reference for the same API version.

**[glyph atlas complexity]** → Proper text rendering (Unicode, fallback fonts, ligatures) is a deep problem. Mitigation: cosmic-text handles shaping/rasterization. We only need to manage the atlas texture and quad emission. Start with monospace ASCII, extend to full Unicode incrementally.

**[winit limitations]** → The winit backend doesn't support all Wayland protocols (layer-shell, etc.). Mitigation: this is a development tool, not the production backend. DRM/KMS comes in a later change.
