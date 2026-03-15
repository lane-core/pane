## 1. Crate scaffold

- [ ] 1.1 Create `crates/pane-comp/Cargo.toml` with dependencies: pane-proto (path), smithay (winit + renderer_glow features), calloop, cosmic-text, tracing/tracing-subscriber
- [ ] 1.2 Create `crates/pane-comp/src/main.rs` with minimal main function
- [ ] 1.3 Verify `cargo build -p pane-comp` compiles

## 2. Smithay winit backend setup

- [ ] 2.1 Initialize smithay winit backend (display, event loop, OpenGL context)
- [ ] 2.2 Set up calloop event loop with smithay's wayland source
- [ ] 2.3 Implement the render loop: clear screen, present frame, handle window events (close, resize)
- [ ] 2.4 Verify the compositor opens a window and displays a solid background color

## 3. Font loading and glyph atlas

- [ ] 3.1 Load a default monospace font via cosmic-text, derive cell metrics (width, height, baseline)
- [ ] 3.2 Create glyph atlas module: rasterize ASCII glyphs into a texture atlas on the GPU
- [ ] 3.3 Implement on-demand glyph rasterization for cache misses
- [ ] 3.4 Verify glyph atlas populates correctly (log atlas dimensions, glyph count)

## 4. Cell grid rendering

- [ ] 4.1 Implement cell-to-quad mapping: for each Cell, emit a background quad and a textured foreground quad from the atlas
- [ ] 4.2 Handle Cell colors: map pane-proto Color variants to RGBA values
- [ ] 4.3 Handle Cell attributes: bold (font weight or synthetic), italic, underline (drawn as a line below the cell)
- [ ] 4.4 Render a hardcoded CellRegion (welcome message with mixed colors/attributes) and verify it displays correctly

## 5. Pane chrome

- [ ] 5.1 Render tag line: draw a colored background bar above the body, render tag text in monospace
- [ ] 5.2 Render beveled borders around the pane (light top/left edge, dark bottom/right edge)
- [ ] 5.3 Compose full pane frame: tag line + borders + cell grid body
- [ ] 5.4 Verify the complete pane renders with visible chrome, tag, and body content

## 6. Integration

- [ ] 6.1 Wire up window resize to recalculate grid dimensions (cols x rows from window size and cell metrics)
- [ ] 6.2 Add tracing/logging for compositor lifecycle events (init, resize, shutdown)
- [ ] 6.3 Verify `cargo run -p pane-comp` shows a complete pane and exits cleanly on window close
