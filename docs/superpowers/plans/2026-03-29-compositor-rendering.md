# Compositor Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Compositor draws pane chrome with real client titles and renders client cell content — replacing all hardcoded demo content with live protocol-driven rendering.

**Architecture:** The `Decorator` trait (faithful to Be's Decorator) owns chrome geometry — footprint, content rect, hit regions. CPU-side text rendering composites glyphs into RGBA buffers, uploaded to GPU via `import_memory()`. The renderer iterates real `PaneState` from the protocol server, using the Decorator for layout and TextBuffer for text. Two font families: Inter (UI sans, tag titles) and Monoid (monospace, body cells).

**Tech Stack:** smithay 0.7 (GlesRenderer, GlesFrame, GlesTexture), cosmic-text 0.18, calloop 0.14, serde/bitflags for cell types.

---

## File Structure

- **Create:** `crates/pane-comp/src/decorator.rs` — Decorator trait + DefaultDecorator
- **Create:** `crates/pane-comp/src/text_buffer.rs` — CPU text rendering to RGBA buffers
- **Modify:** `crates/pane-comp/src/glyph_atlas.rs` — font family parameter, cell region rendering
- **Modify:** `crates/pane-comp/src/pane_renderer.rs` — rewrite to use Decorator + real PaneState + textures
- **Modify:** `crates/pane-comp/src/state.rs` — texture management, connect renderer to PaneState
- **Modify:** `crates/pane-comp/src/main.rs` — module registration, font config
- **Modify:** `crates/pane-server/src/lib.rs` — extend PaneState with content, vocabulary, dirty tracking

---

## Task 1: Decorator Trait + DefaultDecorator

Pure geometry — no rendering, no GPU. Testable on macOS.

**Files:**
- Create: `crates/pane-comp/src/decorator.rs`
- Modify: `crates/pane-comp/src/main.rs` (add `mod decorator;`)

- [ ] **Step 1: Create decorator.rs with trait + types**

```rust
// crates/pane-comp/src/decorator.rs

use smithay::utils::{Physical, Point, Rectangle, Size};

/// Which part of the chrome a point falls on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitRegion {
    /// The tag bar (title area).
    Tag,
    /// The close button within the tag.
    CloseButton,
    /// A border edge — carries the edge for resize cursors.
    Border(Edge),
    /// The body content area.
    Body,
    /// Outside the pane entirely.
    None,
}

/// Border edges for resize hit testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Visual state passed to the decorator for rendering decisions.
#[derive(Debug, Clone)]
pub struct DecorState {
    pub title: String,
    pub focused: bool,
}

/// Geometry of the decorator's chrome and content area.
/// All rectangles are in output-physical coordinates.
#[derive(Debug, Clone)]
pub struct DecorGeometry {
    /// The full frame including chrome.
    pub frame: Rectangle<i32, Physical>,
    /// The tag bar rectangle.
    pub tag: Rectangle<i32, Physical>,
    /// The body content area (frame minus chrome footprint).
    pub content: Rectangle<i32, Physical>,
    /// The border width in pixels.
    pub border_px: i32,
}

/// Chrome geometry and hit testing. Faithful to Be's Decorator concept:
/// the decorator owns the chrome, reports its footprint, and the content
/// area is what remains.
pub trait Decorator {
    /// Compute chrome geometry for a pane at the given frame rectangle.
    /// The frame is the total allocated area including chrome.
    fn geometry(&self, frame: Rectangle<i32, Physical>, cell_h: i32) -> DecorGeometry;

    /// Hit-test a point against the chrome regions.
    fn hit_test(&self, geom: &DecorGeometry, point: Point<i32, Physical>) -> HitRegion;
}

/// The default pane decorator — BeOS-inspired yellow tab with beveled borders.
pub struct DefaultDecorator;

impl Decorator for DefaultDecorator {
    fn geometry(&self, frame: Rectangle<i32, Physical>, cell_h: i32) -> DecorGeometry {
        let border_px = 4;
        let tag_h = cell_h;

        // Tag sits at the top of the frame
        let tag = Rectangle::new(
            frame.loc,
            Size::from((frame.size.w, tag_h)),
        );

        // Content is frame minus tag (top) and borders (left, right, bottom)
        let content = Rectangle::new(
            Point::from((
                frame.loc.x + border_px,
                frame.loc.y + tag_h + border_px,
            )),
            Size::from((
                frame.size.w - border_px * 2,
                frame.size.h - tag_h - border_px * 2,
            )),
        );

        DecorGeometry { frame, tag, content, border_px }
    }

    fn hit_test(&self, geom: &DecorGeometry, point: Point<i32, Physical>) -> HitRegion {
        let p = point;

        // Outside frame entirely
        if !contains(geom.frame, p) {
            return HitRegion::None;
        }

        // In the body content area
        if contains(geom.content, p) {
            return HitRegion::Body;
        }

        // In the tag bar
        if contains(geom.tag, p) {
            // Close button: rightmost cell_h x cell_h square of the tag
            let close_x = geom.tag.loc.x + geom.tag.size.w - geom.tag.size.h;
            if p.x >= close_x {
                return HitRegion::CloseButton;
            }
            return HitRegion::Tag;
        }

        // Must be on a border — determine which edge
        let b = geom.border_px;
        let f = geom.frame;
        let on_left = p.x < f.loc.x + b;
        let on_right = p.x >= f.loc.x + f.size.w - b;
        let on_top = p.y < geom.tag.loc.y + geom.tag.size.h + b;
        let on_bottom = p.y >= f.loc.y + f.size.h - b;

        let edge = match (on_left, on_right, on_top, on_bottom) {
            (true, _, true, _) => Edge::TopLeft,
            (true, _, _, true) => Edge::BottomLeft,
            (_, true, true, _) => Edge::TopRight,
            (_, true, _, true) => Edge::BottomRight,
            (true, _, _, _) => Edge::Left,
            (_, true, _, _) => Edge::Right,
            (_, _, true, _) => Edge::Top,
            (_, _, _, true) => Edge::Bottom,
            _ => Edge::Left, // fallback, shouldn't happen
        };

        HitRegion::Border(edge)
    }
}

fn contains(rect: Rectangle<i32, Physical>, point: Point<i32, Physical>) -> bool {
    point.x >= rect.loc.x
        && point.x < rect.loc.x + rect.size.w
        && point.y >= rect.loc.y
        && point.y < rect.loc.y + rect.size.h
}
```

- [ ] **Step 2: Add module to main.rs**

Add `mod decorator;` to the module declarations in `crates/pane-comp/src/main.rs`.

- [ ] **Step 3: Write geometry tests**

Add at the bottom of `decorator.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn frame_400x300() -> Rectangle<i32, Physical> {
        Rectangle::new(Point::from((100, 50)), Size::from((400, 300)))
    }

    #[test]
    fn content_rect_excludes_chrome() {
        let dec = DefaultDecorator;
        let geom = dec.geometry(frame_400x300(), 20);

        // Content starts after tag (20px) + border (4px) on top,
        // and border (4px) on left
        assert_eq!(geom.content.loc.x, 104);
        assert_eq!(geom.content.loc.y, 74); // 50 + 20 + 4
        // Content width = frame width - 2 * border
        assert_eq!(geom.content.size.w, 392); // 400 - 8
        // Content height = frame height - tag - 2 * border
        assert_eq!(geom.content.size.h, 272); // 300 - 20 - 8
    }

    #[test]
    fn tag_spans_full_width() {
        let dec = DefaultDecorator;
        let geom = dec.geometry(frame_400x300(), 20);

        assert_eq!(geom.tag.loc, frame_400x300().loc);
        assert_eq!(geom.tag.size.w, 400);
        assert_eq!(geom.tag.size.h, 20);
    }

    #[test]
    fn hit_test_body() {
        let dec = DefaultDecorator;
        let geom = dec.geometry(frame_400x300(), 20);
        // Center of content area
        let mid = Point::from((300, 200));
        assert_eq!(dec.hit_test(&geom, mid), HitRegion::Body);
    }

    #[test]
    fn hit_test_tag() {
        let dec = DefaultDecorator;
        let geom = dec.geometry(frame_400x300(), 20);
        // In the tag, but not close button
        let p = Point::from((200, 55));
        assert_eq!(dec.hit_test(&geom, p), HitRegion::Tag);
    }

    #[test]
    fn hit_test_close_button() {
        let dec = DefaultDecorator;
        let geom = dec.geometry(frame_400x300(), 20);
        // Rightmost 20px of tag (tag height = 20)
        let p = Point::from((495, 55));
        assert_eq!(dec.hit_test(&geom, p), HitRegion::CloseButton);
    }

    #[test]
    fn hit_test_border() {
        let dec = DefaultDecorator;
        let geom = dec.geometry(frame_400x300(), 20);
        // Left border area (x = 101, between frame left and content left)
        let p = Point::from((101, 200));
        assert_eq!(dec.hit_test(&geom, p), HitRegion::Border(Edge::Left));
    }

    #[test]
    fn hit_test_outside() {
        let dec = DefaultDecorator;
        let geom = dec.geometry(frame_400x300(), 20);
        let p = Point::from((0, 0));
        assert_eq!(dec.hit_test(&geom, p), HitRegion::None);
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p pane-comp decorator`
Expected: All 6 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/pane-comp/src/decorator.rs crates/pane-comp/src/main.rs
git commit -m "Decorator trait + DefaultDecorator: chrome geometry and hit testing"
```

---

## Task 2: Extend PaneState + Wire SetContent

The protocol server needs to store content, vocabulary, and dirty state so the renderer can access it.

**Files:**
- Modify: `crates/pane-server/src/lib.rs`

- [ ] **Step 1: Extend PaneState struct**

In `crates/pane-server/src/lib.rs`, replace the `PaneState` struct:

```rust
use pane_proto::tag::CommandVocabulary;

/// Per-pane state tracked by the compositor.
pub struct PaneState {
    /// Which client owns this pane.
    pub client_id: usize,
    /// Current title (from SetTitle or CreatePane).
    pub title: String,
    /// Current command vocabulary.
    pub vocabulary: CommandVocabulary,
    /// Latest body content (serialized CellRegion bytes from SetContent).
    pub content: Option<Vec<u8>>,
    /// Whether this pane has focus.
    pub focused: bool,
    /// Whether the title has changed since last render.
    pub title_dirty: bool,
    /// Whether the body content has changed since last render.
    pub content_dirty: bool,
}
```

- [ ] **Step 2: Update CreatePane handler to store vocabulary**

In `handle_message`, update the `CreatePane` arm:

```rust
ClientToComp::CreatePane { tag } => {
    let pane_id = self.alloc_pane_id();
    let title = tag.as_ref()
        .map(|t| t.title.text.clone())
        .unwrap_or_default();
    let vocabulary = tag.as_ref()
        .map(|t| t.vocabulary.clone())
        .unwrap_or_default();

    info!("client {} creating pane {:?} '{}'", client_id, pane_id, title);

    self.panes.insert(pane_id, PaneState {
        client_id,
        title,
        vocabulary,
        content: None,
        focused: false,
        title_dirty: true,
        content_dirty: false,
    });

    if let Some(client) = self.clients.get_mut(&client_id) {
        client.panes.push(pane_id);
        let response = CompToClient::PaneCreated { pane: pane_id, geometry };
        Self::send_to_client(client, &response);
    }
}
```

- [ ] **Step 3: Update SetTitle to mark dirty**

```rust
ClientToComp::SetTitle { pane, title } => {
    if let Some(state) = self.panes.get_mut(&pane) {
        state.title = title.text.clone();
        state.title_dirty = true;
        info!("pane {:?} title: '{}'", pane, state.title);
    }
}
```

- [ ] **Step 4: Wire SetContent to store bytes**

```rust
ClientToComp::SetContent { pane, content } => {
    if let Some(state) = self.panes.get_mut(&pane) {
        state.content = Some(content);
        state.content_dirty = true;
    }
}
```

- [ ] **Step 5: Wire SetVocabulary to store vocabulary**

```rust
ClientToComp::SetVocabulary { pane, vocabulary } => {
    if let Some(state) = self.panes.get_mut(&pane) {
        state.vocabulary = vocabulary;
        info!("pane {:?} vocabulary updated", pane);
    }
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p pane-server`
Expected: All existing tests pass. (No new tests needed — these are field additions to an existing struct.)

Run: `cargo check -p pane-comp`
Expected: Compiles. (The renderer still uses hardcoded data; it doesn't read PaneState yet.)

- [ ] **Step 7: Commit**

```bash
git add crates/pane-server/src/lib.rs
git commit -m "PaneState: store content, vocabulary, dirty tracking for renderer"
```

---

## Task 3: System Font Configuration

Configure cosmic-text to use Inter (UI), Monoid (mono), Gelasio (serif) as system defaults.

**Files:**
- Modify: `crates/pane-comp/src/glyph_atlas.rs`
- Modify: `crates/pane-comp/src/main.rs`

- [ ] **Step 1: Parameterize GlyphAtlas with font family**

In `glyph_atlas.rs`, change `GlyphAtlas::new` to accept a font family name:

```rust
impl GlyphAtlas {
    pub fn new(font_size: f32, font_family: &str) -> anyhow::Result<Self> {
        let mut font_system = FontSystem::new();

        let family = if font_family.eq_ignore_ascii_case("monospace") {
            Family::Monospace
        } else {
            Family::Name(font_family)
        };

        // Derive cell metrics from a reference buffer
        let metrics = Metrics::new(font_size, font_size * 1.2);
        let mut buffer = Buffer::new(&mut font_system, metrics);
        buffer.set_text(
            &mut font_system,
            "M",
            &Attrs::new().family(family),
            Shaping::Advanced,
            None,
        );
```

Also update `rasterize_glyph` to store and use the family:

Add a `font_family: String` field to the struct, and use it in `rasterize_glyph`:

```rust
pub struct GlyphAtlas {
    font_system: FontSystem,
    swash_cache: SwashCache,
    font_size: f32,
    font_family: String,
    // ... rest unchanged
}
```

In `rasterize_glyph`, change the `Attrs` line:

```rust
let family = if self.font_family.eq_ignore_ascii_case("monospace") {
    Family::Monospace
} else {
    Family::Name(&self.font_family)
};
buffer.set_text(
    &mut self.font_system,
    &s,
    &Attrs::new().family(family),
    Shaping::Advanced,
    None,
);
```

- [ ] **Step 2: Update main.rs to pass font family**

In `run()`, change the atlas creation:

```rust
// System default monospace font: Monoid (falls back to system monospace)
let mut atlas = GlyphAtlas::new(opts.font_size, "Monoid")?;
```

- [ ] **Step 3: Run tests**

Run: `cargo check -p pane-comp`
Expected: Compiles. (Font fallback is handled by cosmic-text — if Monoid isn't installed, it falls back to system monospace.)

- [ ] **Step 4: Commit**

```bash
git add crates/pane-comp/src/glyph_atlas.rs crates/pane-comp/src/main.rs
git commit -m "System fonts: Monoid for monospace atlas, parameterized font family"
```

---

## Task 4: CPU Text Renderer (TextBuffer)

Renders text into CPU-side RGBA buffers using the glyph atlas. Two modes: proportional layout (for tag titles, using cosmic-text layout engine) and cell grid (for body content, using the atlas directly).

**Files:**
- Create: `crates/pane-comp/src/text_buffer.rs`
- Modify: `crates/pane-comp/src/glyph_atlas.rs` (add pixel-coordinate accessors)
- Modify: `crates/pane-comp/src/main.rs` (add `mod text_buffer;`)

- [ ] **Step 1: Add pixel-coordinate glyph access to GlyphAtlas**

In `glyph_atlas.rs`, add methods to read glyph pixels from the atlas:

```rust
impl GlyphAtlas {
    /// Get the pixel-coordinate bounds of a glyph in the atlas.
    /// Returns (x, y, width, height) in atlas pixels, plus bearing offsets.
    pub fn glyph_atlas_rect(&mut self, ch: char) -> Option<GlyphRect> {
        let info = self.get_glyph(ch)?;
        if info.width == 0 || info.height == 0 {
            return None;
        }
        Some(GlyphRect {
            atlas_x: (info.u0 * self.atlas_width as f32) as u32,
            atlas_y: (info.v0 * self.atlas_height as f32) as u32,
            width: info.width,
            height: info.height,
            bearing_x: info.bearing_x,
            bearing_y: info.bearing_y,
        })
    }

    /// Read a single pixel's alpha from the atlas at (x, y).
    pub fn atlas_alpha(&self, x: u32, y: u32) -> u8 {
        let idx = (y as usize * self.atlas_width as usize + x as usize) * 4 + 3;
        self.atlas_data.get(idx).copied().unwrap_or(0)
    }
}

/// Pixel-coordinate glyph location in the atlas.
#[derive(Debug, Clone, Copy)]
pub struct GlyphRect {
    pub atlas_x: u32,
    pub atlas_y: u32,
    pub width: u32,
    pub height: u32,
    pub bearing_x: f32,
    pub bearing_y: f32,
}
```

- [ ] **Step 2: Create text_buffer.rs — cell grid rendering**

```rust
// crates/pane-comp/src/text_buffer.rs

use crate::cell::{Cell, CellRegion};
use crate::glyph_atlas::GlyphAtlas;
use crate::pane_renderer::color_to_rgba;
use pane_proto::color::Color;

/// An RGBA pixel buffer for CPU-side text rendering.
pub struct TextBuffer {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl TextBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0u8; (width * height * 4) as usize],
        }
    }

    /// Fill the entire buffer with a solid color.
    pub fn clear(&mut self, r: u8, g: u8, b: u8, a: u8) {
        for pixel in self.data.chunks_exact_mut(4) {
            pixel[0] = r;
            pixel[1] = g;
            pixel[2] = b;
            pixel[3] = a;
        }
    }

    /// Set a single pixel (bounds-checked, no-op if out of range).
    fn set_pixel(&mut self, x: i32, y: i32, r: u8, g: u8, b: u8, a: u8) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let idx = (y as usize * self.width as usize + x as usize) * 4;
        if a == 255 {
            self.data[idx] = r;
            self.data[idx + 1] = g;
            self.data[idx + 2] = b;
            self.data[idx + 3] = 255;
        } else if a > 0 {
            // Alpha blend: src over dst
            let sa = a as u16;
            let da = self.data[idx + 3] as u16;
            let inv_sa = 255 - sa;
            self.data[idx] = ((r as u16 * sa + self.data[idx] as u16 * inv_sa) / 255) as u8;
            self.data[idx + 1] = ((g as u16 * sa + self.data[idx + 1] as u16 * inv_sa) / 255) as u8;
            self.data[idx + 2] = ((b as u16 * sa + self.data[idx + 2] as u16 * inv_sa) / 255) as u8;
            self.data[idx + 3] = (sa + da * inv_sa / 255) as u8;
        }
    }

    /// Fill a rectangle with a solid color.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, r: u8, g: u8, b: u8, a: u8) {
        for py in y..y + h {
            for px in x..x + w {
                self.set_pixel(px, py, r, g, b, a);
            }
        }
    }

    /// Render a single glyph from the atlas at (x, y) with the given color.
    /// The position is the top-left of the cell, not the glyph baseline.
    pub fn draw_glyph(
        &mut self,
        atlas: &mut GlyphAtlas,
        ch: char,
        cell_x: i32,
        cell_y: i32,
        fg: [u8; 4],
    ) {
        let rect = match atlas.glyph_atlas_rect(ch) {
            Some(r) => r,
            None => return, // space or unknown glyph
        };

        let baseline = atlas.baseline();
        let gx = cell_x + rect.bearing_x as i32;
        let gy = cell_y + (baseline - rect.bearing_y) as i32;

        for py in 0..rect.height {
            for px in 0..rect.width {
                let alpha = atlas.atlas_alpha(rect.atlas_x + px, rect.atlas_y + py);
                if alpha > 0 {
                    self.set_pixel(
                        gx + px as i32,
                        gy + py as i32,
                        fg[0],
                        fg[1],
                        fg[2],
                        alpha,
                    );
                }
            }
        }
    }

    /// Render a CellRegion into this buffer using the glyph atlas.
    /// The region is placed at pixel (0, 0) in the buffer.
    pub fn render_cell_region(
        &mut self,
        atlas: &mut GlyphAtlas,
        region: &CellRegion,
        cell_w: i32,
        cell_h: i32,
    ) {
        for row in 0..region.height {
            for col in 0..region.width {
                let idx = row as usize * region.width as usize + col as usize;
                let cell = &region.cells[idx];

                let cx = col as i32 * cell_w;
                let cy = row as i32 * cell_h;

                // Draw cell background if non-default
                if cell.bg != Color::Default {
                    let [r, g, b, _] = color_to_rgba(&cell.bg);
                    self.fill_rect(
                        cx, cy, cell_w, cell_h,
                        (r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255,
                    );
                }

                // Draw glyph
                if cell.ch != ' ' {
                    let [r, g, b, _] = color_to_rgba(&cell.fg);
                    let fg = [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255];
                    self.draw_glyph(atlas, cell.ch, cx, cy, fg);
                }
            }
        }
    }

    /// Render a plain text string for the tag title.
    /// Uses the atlas glyphs in a fixed-pitch layout.
    pub fn render_tag_text(
        &mut self,
        atlas: &mut GlyphAtlas,
        text: &str,
        x: i32,
        y: i32,
        fg: [u8; 4],
        cell_w: i32,
    ) {
        for (i, ch) in text.chars().enumerate() {
            if ch != ' ' {
                self.draw_glyph(atlas, ch, x + i as i32 * cell_w, y, fg);
            }
        }
    }
}
```

- [ ] **Step 3: Add module to main.rs**

Add `mod text_buffer;` to `crates/pane-comp/src/main.rs`.

- [ ] **Step 4: Write tests for TextBuffer**

Add at the bottom of `text_buffer.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_fills_buffer() {
        let mut buf = TextBuffer::new(2, 2);
        buf.clear(255, 0, 0, 255);
        // First pixel: red, full alpha
        assert_eq!(&buf.data[0..4], &[255, 0, 0, 255]);
        // Last pixel: same
        assert_eq!(&buf.data[12..16], &[255, 0, 0, 255]);
    }

    #[test]
    fn fill_rect_bounds_checked() {
        let mut buf = TextBuffer::new(4, 4);
        buf.clear(0, 0, 0, 255);
        buf.fill_rect(1, 1, 2, 2, 255, 255, 255, 255);

        // (0,0) = black
        assert_eq!(buf.data[0], 0);
        // (1,1) = white
        let idx = (1 * 4 + 1) * 4;
        assert_eq!(buf.data[idx], 255);
        // (3,3) = black
        let idx = (3 * 4 + 3) * 4;
        assert_eq!(buf.data[idx], 0);
    }

    #[test]
    fn out_of_bounds_pixel_ignored() {
        let mut buf = TextBuffer::new(2, 2);
        buf.clear(0, 0, 0, 255);
        buf.set_pixel(-1, 0, 255, 0, 0, 255); // should not panic
        buf.set_pixel(0, 100, 255, 0, 0, 255); // should not panic
        assert_eq!(buf.data[0], 0); // unchanged
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p pane-comp text_buffer`
Expected: All 3 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/pane-comp/src/text_buffer.rs crates/pane-comp/src/glyph_atlas.rs crates/pane-comp/src/main.rs
git commit -m "TextBuffer: CPU text rendering to RGBA for tag titles and cell grids"
```

---

## Task 5: GPU Texture Management

Upload RGBA buffers to GlesTexture via `import_memory()`. Manage per-pane textures with dirty tracking.

**Files:**
- Modify: `crates/pane-comp/src/state.rs`
- Modify: `crates/pane-comp/src/pane_renderer.rs`

- [ ] **Step 1: Add texture cache to PaneRenderer**

In `pane_renderer.rs`, replace the struct with a version that manages per-pane GPU textures:

```rust
use std::collections::HashMap;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::gles::{GlesFrame, GlesRenderer, GlesTexture};
use smithay::utils::{Physical, Rectangle, Size, Transform};
use smithay::backend::renderer::Color32F;

use pane_proto::color::Color;
use pane_proto::message::PaneId;

use crate::cell::CellRegion;
use crate::decorator::{DecorGeometry, Decorator, DefaultDecorator};
use crate::glyph_atlas::GlyphAtlas;
use crate::text_buffer::TextBuffer;

/// Tag line and chrome colors
const TAG_BG: Color32F = Color32F::new(0.95, 0.85, 0.35, 1.0);
const TAG_BG_UNFOCUSED: Color32F = Color32F::new(0.85, 0.82, 0.72, 1.0);
const BORDER_LIGHT: Color32F = Color32F::new(0.7, 0.68, 0.62, 1.0);
const BORDER_DARK: Color32F = Color32F::new(0.25, 0.23, 0.20, 1.0);
const BODY_BG: Color32F = Color32F::new(1.0, 1.0, 1.0, 1.0);
const BODY_FG: [f32; 4] = [0.1, 0.1, 0.1, 1.0];

/// Per-pane cached GPU textures.
struct PaneTextures {
    /// Tag title text (rendered on transparent background).
    title_tex: Option<GlesTexture>,
    title_size: Size<i32, Physical>,
    /// Body content (cell grid with backgrounds and glyphs).
    body_tex: Option<GlesTexture>,
    body_size: Size<i32, Physical>,
}

/// Renders pane chrome + content using Decorator geometry and real PaneState.
pub struct PaneRenderer {
    decorator: DefaultDecorator,
    textures: HashMap<PaneId, PaneTextures>,
}
```

- [ ] **Step 2: Implement texture upload helper**

Add to `pane_renderer.rs`:

```rust
impl PaneRenderer {
    pub fn new() -> Self {
        Self {
            decorator: DefaultDecorator,
            textures: HashMap::new(),
        }
    }

    /// Upload an RGBA TextBuffer to GPU, returning a GlesTexture.
    fn upload_texture(
        renderer: &mut GlesRenderer,
        buf: &TextBuffer,
    ) -> anyhow::Result<GlesTexture> {
        renderer.import_memory(
            &buf.data,
            Fourcc::Abgr8888,
            Size::from((buf.width as i32, buf.height as i32)),
            false,
        ).map_err(|e| anyhow::anyhow!("import_memory: {e}"))
    }
}
```

Note: The `Fourcc` variant must match the byte order. The TextBuffer stores [R, G, B, A] per pixel. On little-endian, `Abgr8888` reads bytes as R, G, B, A when interpreted as a 32-bit word in ABGR order. **Verify this during implementation** — if colors appear wrong, try `Rgba8888` or `Argb8888` instead.

- [ ] **Step 3: Implement title texture rendering**

```rust
impl PaneRenderer {
    /// Render or re-render the tag title texture for a pane.
    pub fn update_title_texture(
        &mut self,
        renderer: &mut GlesRenderer,
        atlas: &mut GlyphAtlas,
        pane_id: PaneId,
        title: &str,
        cell_w: i32,
        cell_h: i32,
        tag_width: i32,
    ) -> anyhow::Result<()> {
        let buf_w = tag_width.max(1) as u32;
        let buf_h = cell_h.max(1) as u32;
        let mut buf = TextBuffer::new(buf_w, buf_h);
        // Transparent background — chrome color drawn as solid rect underneath
        buf.clear(0, 0, 0, 0);

        // Dark text for title
        let fg = [25u8, 25, 25, 255];
        buf.render_tag_text(atlas, title, 4, 0, fg, cell_w);

        let tex = Self::upload_texture(renderer, &buf)?;
        let entry = self.textures.entry(pane_id).or_insert_with(|| PaneTextures {
            title_tex: None,
            title_size: Size::default(),
            body_tex: None,
            body_size: Size::default(),
        });
        entry.title_tex = Some(tex);
        entry.title_size = Size::from((buf_w as i32, buf_h as i32));
        Ok(())
    }
}
```

- [ ] **Step 4: Implement body texture rendering**

```rust
impl PaneRenderer {
    /// Render or re-render the body content texture for a pane.
    pub fn update_body_texture(
        &mut self,
        renderer: &mut GlesRenderer,
        atlas: &mut GlyphAtlas,
        pane_id: PaneId,
        region: &CellRegion,
        cell_w: i32,
        cell_h: i32,
    ) -> anyhow::Result<()> {
        let buf_w = (region.width as i32 * cell_w).max(1) as u32;
        let buf_h = (region.height as i32 * cell_h).max(1) as u32;
        let mut buf = TextBuffer::new(buf_w, buf_h);
        buf.clear(255, 255, 255, 255); // white body background

        buf.render_cell_region(atlas, region, cell_w, cell_h);

        let tex = Self::upload_texture(renderer, &buf)?;
        let entry = self.textures.entry(pane_id).or_insert_with(|| PaneTextures {
            title_tex: None,
            title_size: Size::default(),
            body_tex: None,
            body_size: Size::default(),
        });
        entry.body_tex = Some(tex);
        entry.body_size = Size::from((buf_w as i32, buf_h as i32));
        Ok(())
    }

    /// Remove cached textures for a pane that no longer exists.
    pub fn remove_pane(&mut self, pane_id: &PaneId) {
        self.textures.remove(pane_id);
    }
}
```

- [ ] **Step 5: Run check**

Run: `cargo check -p pane-comp`
Expected: Compiles. (The render method hasn't been rewritten yet — that's Task 6.)

- [ ] **Step 6: Commit**

```bash
git add crates/pane-comp/src/pane_renderer.rs
git commit -m "PaneRenderer: GPU texture upload for tag titles and cell grid bodies"
```

---

## Task 6: Rewrite PaneRenderer — Integrate Everything

Replace hardcoded rendering with Decorator geometry, real PaneState, and GPU textures. This is the integration task.

**Files:**
- Modify: `crates/pane-comp/src/pane_renderer.rs`
- Modify: `crates/pane-comp/src/state.rs`
- Modify: `crates/pane-comp/src/main.rs`

- [ ] **Step 1: Add render method using Decorator + textures**

In `pane_renderer.rs`, add the main render method:

```rust
use smithay::backend::renderer::gles::element::PixelShaderElement;

impl PaneRenderer {
    /// Render a single pane's chrome and content at the given frame rect.
    pub fn render_pane(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        pane_id: &PaneId,
        pane_frame: Rectangle<i32, Physical>,
        focused: bool,
        cell_h: i32,
        damage: &Rectangle<i32, Physical>,
    ) -> anyhow::Result<()> {
        let geom = self.decorator.geometry(pane_frame, cell_h);

        // --- Tag background ---
        let tag_bg = if focused { TAG_BG } else { TAG_BG_UNFOCUSED };
        solid(frame, damage, geom.tag, tag_bg)?;

        // --- Tag title text (textured quad on top of tag bg) ---
        if let Some(textures) = self.textures.get(pane_id) {
            if let Some(ref title_tex) = textures.title_tex {
                frame.render_texture_from_to(
                    title_tex,
                    Rectangle::from_loc_and_size(
                        (0.0, 0.0),
                        (textures.title_size.w as f64, textures.title_size.h as f64),
                    ),
                    geom.tag,
                    &[*damage],
                    &[],
                    Transform::Normal,
                    1.0,
                    None,
                    &[],
                ).map_err(|e| anyhow::anyhow!("render title: {e}"))?;
            }
        }

        // --- Beveled borders ---
        let b = geom.border_px;
        let below_tag_y = geom.tag.loc.y + geom.tag.size.h;
        let border_h = pane_frame.size.h - geom.tag.size.h;

        // Light edge: top + left
        solid(frame, damage, Rectangle::new(
            (pane_frame.loc.x, below_tag_y).into(),
            (pane_frame.size.w, b).into(),
        ), BORDER_LIGHT)?;
        solid(frame, damage, Rectangle::new(
            (pane_frame.loc.x, below_tag_y).into(),
            (b, border_h).into(),
        ), BORDER_LIGHT)?;

        // Dark edge: bottom + right
        solid(frame, damage, Rectangle::new(
            (pane_frame.loc.x, pane_frame.loc.y + pane_frame.size.h - b).into(),
            (pane_frame.size.w, b).into(),
        ), BORDER_DARK)?;
        solid(frame, damage, Rectangle::new(
            (pane_frame.loc.x + pane_frame.size.w - b, below_tag_y).into(),
            (b, border_h).into(),
        ), BORDER_DARK)?;

        // --- Body background ---
        solid(frame, damage, geom.content, BODY_BG)?;

        // --- Body content (textured quad) ---
        if let Some(textures) = self.textures.get(pane_id) {
            if let Some(ref body_tex) = textures.body_tex {
                frame.render_texture_from_to(
                    body_tex,
                    Rectangle::from_loc_and_size(
                        (0.0, 0.0),
                        (textures.body_size.w as f64, textures.body_size.h as f64),
                    ),
                    geom.content,
                    &[*damage],
                    &[],
                    Transform::Normal,
                    1.0,
                    None,
                    &[],
                ).map_err(|e| anyhow::anyhow!("render body: {e}"))?;
            }
        }

        Ok(())
    }
}
```

Keep the `solid()` helper and `color_to_rgba()` + color conversion functions from the current file — they're still needed.

- [ ] **Step 2: Update CompState to drive texture updates + rendering**

In `state.rs`, rewrite `render_frame` to iterate real panes:

```rust
impl CompState {
    /// Update dirty textures, then render all panes.
    pub fn render_frame(&mut self) {
        let output_size = self.size;
        let output_rect = Rectangle::from_size(output_size);
        let cell_w = self.cell_width as i32;
        let cell_h = self.cell_height as i32;

        // --- Phase 1: Update dirty textures (needs &mut GlesRenderer, before bind) ---
        {
            let renderer = self.backend.renderer();
            let atlas = &mut self.atlas;
            let pane_renderer = &mut self.pane_renderer;

            for (&pane_id, pane_state) in &mut self.server.panes {
                if pane_state.title_dirty {
                    if let Err(e) = pane_renderer.update_title_texture(
                        renderer, atlas, pane_id, &pane_state.title,
                        cell_w, cell_h, output_size.w,
                    ) {
                        tracing::warn!("title texture for {:?}: {e}", pane_id);
                    }
                    pane_state.title_dirty = false;
                }

                if pane_state.content_dirty {
                    if let Some(ref content_bytes) = pane_state.content {
                        match pane_proto::deserialize::<crate::cell::CellRegion>(content_bytes) {
                            Ok(region) => {
                                if let Err(e) = pane_renderer.update_body_texture(
                                    renderer, atlas, pane_id, &region, cell_w, cell_h,
                                ) {
                                    tracing::warn!("body texture for {:?}: {e}", pane_id);
                                }
                            }
                            Err(e) => {
                                tracing::warn!("deserialize content for {:?}: {e}", pane_id);
                            }
                        }
                    }
                    pane_state.content_dirty = false;
                }
            }
        }

        // --- Phase 2: Render frame ---
        let render_result = (|| -> anyhow::Result<()> {
            let (renderer, mut target) = self.backend.bind()
                .map_err(|e| anyhow::anyhow!("bind: {e}"))?;
            let mut frame = renderer.render(&mut target, output_size, Transform::Normal)
                .map_err(|e| anyhow::anyhow!("render: {e}"))?;
            frame.clear(BG_COLOR, &[output_rect])
                .map_err(|e| anyhow::anyhow!("clear: {e}"))?;

            let damage = output_rect;

            // Render each pane
            // For now: single pane, full window minus margins
            // TODO: layout engine assigns frame rects per pane
            for (&pane_id, pane_state) in &self.server.panes {
                let pane_w = output_size.w - 60;
                let pane_h = output_size.h - 60;
                let pane_frame = Rectangle::new(
                    (30, 30).into(),
                    (pane_w.max(100), pane_h.max(100)).into(),
                );

                if let Err(e) = self.pane_renderer.render_pane(
                    &mut frame,
                    &pane_id,
                    pane_frame,
                    pane_state.focused,
                    cell_h,
                    &damage,
                ) {
                    tracing::warn!("pane render error: {e}");
                }
            }

            frame.finish()
                .map_err(|e| anyhow::anyhow!("finish: {e}"))?;
            Ok(())
        })();

        if let Err(e) = render_result {
            tracing::warn!("frame render error: {e}");
        }

        if let Err(e) = self.backend.submit(Some(&[output_rect])) {
            tracing::warn!("submit error: {e}");
        }
    }
}
```

**Important borrow checker note:** The Phase 1 block borrows `self.server.panes`, `self.backend`, `self.atlas`, and `self.pane_renderer` simultaneously. If the borrow checker rejects this, extract the pane data into a temporary `Vec` first:

```rust
let dirty_panes: Vec<_> = self.server.panes.iter()
    .filter(|(_, s)| s.title_dirty || s.content_dirty)
    .map(|(&id, s)| (id, s.title.clone(), s.content.clone(), s.title_dirty, s.content_dirty))
    .collect();
```

Then iterate the temporary vec. After updating textures, clear the dirty flags on the originals.

- [ ] **Step 3: Update PaneRenderer::new() in main.rs**

In `main.rs`, change:

```rust
let pane_renderer = PaneRenderer::new(&atlas);
```

to:

```rust
let pane_renderer = PaneRenderer::new();
```

- [ ] **Step 4: Clean up remove_client to drop textures**

In `state.rs`, update `process_client_messages` or add a hook: when a client disconnects and its panes are removed, also remove their cached textures:

```rust
for id in disconnected {
    // Get pane IDs before removing the client
    if let Some(client) = self.server.clients.get(&id) {
        for pane_id in &client.panes {
            self.pane_renderer.remove_pane(pane_id);
        }
    }
    self.server.remove_client(id);
}
```

- [ ] **Step 5: Build and fix compilation errors**

Run: `cargo check -p pane-comp 2>&1 | tee /tmp/pane-check.log`

This is the integration point — expect borrow checker issues, import mismatches, or API mismatches with smithay. Fix iteratively until it compiles. Key things to watch for:

- `import_memory` may require `use smithay::backend::renderer::ImportMem;` trait import
- `render_texture_from_to` may require `use smithay::backend::renderer::Frame;` or a trait import
- `Fourcc` enum variant name — verify the correct byte-order variant
- The `&mut self.server.panes` borrow in Phase 1 may conflict with `&mut self.backend` — resolve with the temporary vec approach if needed

- [ ] **Step 6: Test in VM**

Run: `just build-comp && just vm-push`

Then in the VM:
```bash
pane-comp --log debug
```

In a second terminal in the VM, run pane-hello (if it creates a pane with a title). Verify:
1. The tag bar shows the real title from pane-hello (not hardcoded text)
2. If pane-hello sends SetContent, the body shows cell content
3. If no clients are connected, the window shows just the background (no pane chrome)

- [ ] **Step 7: Commit**

```bash
git add crates/pane-comp/src/pane_renderer.rs crates/pane-comp/src/state.rs crates/pane-comp/src/main.rs
git commit -m "Render real panes: Decorator geometry, protocol-driven titles and content"
```

---

## Task 7: Fallback Rendering When No Clients Connected

When no clients are connected, the compositor should show a minimal welcome or empty desktop — not a blank grey window.

**Files:**
- Modify: `crates/pane-comp/src/state.rs`

- [ ] **Step 1: Add fallback rendering for empty desktop**

In the render loop, after checking `self.server.panes`, if empty, render a centered status text. This uses the same TextBuffer approach:

```rust
if self.server.panes.is_empty() {
    // Render a simple "pane" watermark centered on screen
    // This is temporary — will be replaced by a proper desktop
    // background when the layout engine exists.
}
```

The simplest approach: do nothing extra. The background clear already produces a clean warm-grey desktop. The pane chrome only appears when a client connects. This is the correct behavior.

- [ ] **Step 2: Verify empty desktop looks clean**

In VM: run `pane-comp` with no clients. Verify the window shows the warm grey background only.

- [ ] **Step 3: Commit (combined with any polish)**

```bash
git add -u
git commit -m "Polish: clean empty desktop, remove dead hardcoded rendering code"
```

---

## Task 8: Update PLAN.md + Serena State

**Files:**
- Modify: `PLAN.md`

- [ ] **Step 1: Mark rendering task complete in PLAN.md**

Change:
```
- [ ] **Rendering** — compositor draws pane chrome ...
```
to:
```
- [x] **Rendering** — compositor draws pane chrome (title bar from Tag), body area receives client content. Decorator trait, CPU text rendering, GPU texture pipeline.
```

- [ ] **Step 2: Update serena current_state**

Update `pane/current_state` memory to reflect:
- Decorator trait exists (DefaultDecorator: yellow tab, beveled borders)
- Text rendering pipeline: atlas → CPU TextBuffer → GPU GlesTexture
- PaneState stores content, vocabulary, dirty tracking
- System fonts: Inter (UI), Monoid (mono), Gelasio (serif)
- What's next: input routing (Phase 4.2)

- [ ] **Step 3: Commit**

```bash
git add PLAN.md
git commit -m "PLAN.md: mark compositor rendering complete"
```

---

## Notes

**Testing strategy:** Tasks 1 and 4 have unit tests for pure logic (geometry, pixel compositing). Tasks 5-6 require GPU context and must be visually verified in the VM via `just build-comp && just vm-push`. The `test-renderer` feature flag exists for future headless integration tests but is not used in this plan.

**Font availability:** If Inter, Monoid, or Gelasio are not installed, cosmic-text falls back to the system's default for that family. The fonts should be installed in the VM's Nix configuration. This plan does not bundle fonts into the binary.

**Performance:** CPU text rendering is adequate for the current scale. A 200x60 cell grid produces a ~1600x1020 RGBA buffer (~6.5MB), uploadable in well under 1ms. Texture re-upload only happens when content changes (dirty tracking). Future optimization: custom GlesTexProgram shader for GPU-side glyph tinting, eliminating the CPU compositing step.

**Fourcc byte order:** The correct `Fourcc` variant for [R,G,B,A] byte-order data depends on the platform's endianness and smithay's interpretation. `Abgr8888` is the standard choice for little-endian RGBA. If colors render incorrectly, try `Rgba8888`. This must be verified in the VM.

**Borrow checker:** Task 6 Step 2 borrows multiple fields of `CompState` simultaneously. If rustc rejects the split borrows through `self`, refactor to pass individual fields as parameters or use temporary data extraction.

**Tag title font (v1 simplification):** This plan renders tag titles using the monospace glyph atlas (Monoid), not Inter. Proportional Inter rendering requires cosmic-text's layout engine for variable-width glyph positioning — a separate follow-up. The monospace tag title is functional and correct; switching to Inter is a polish task.

**CellRegion location:** `CellRegion` is defined in `pane-comp/src/cell.rs`, not `pane-proto`. The compositor deserializes it from opaque `SetContent` bytes. For clients to send CellRegion, the type should eventually move to `pane-proto`. For now, pane-hello doesn't send SetContent, so this doesn't block the plan. Body content rendering activates when a future client does send it.
