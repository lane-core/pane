use pane_proto::cell::{Cell, CellAttrs, CellRegion};
use pane_proto::color::{Color, NamedColor};
use smithay::backend::renderer::gles::GlesFrame;
use smithay::utils::{Physical, Rectangle, Size};
use smithay::backend::renderer::Color32F;

use crate::glyph_atlas::GlyphAtlas;

/// Tag line and chrome colors — 90s-inspired, BeOS-ish
const TAG_BG: Color32F = Color32F::new(0.95, 0.85, 0.35, 1.0); // BeOS-inspired warm yellow
const BORDER_LIGHT: Color32F = Color32F::new(0.7, 0.68, 0.62, 1.0);
const BORDER_DARK: Color32F = Color32F::new(0.3, 0.28, 0.25, 1.0);
const BODY_BG: Color32F = Color32F::new(0.12, 0.12, 0.14, 1.0);
const BODY_FG: [f32; 4] = [0.85, 0.85, 0.80, 1.0];

/// Border width in pixels
const BORDER_PX: i32 = 4;

/// Renders a single hardcoded pane with tag line, borders, and cell grid body.
pub struct PaneRenderer {
    tag_text: String,
    body_cells: CellRegion,
    cell_w: i32,
    cell_h: i32,
}

/// Draw a solid rectangle, using itself as damage.
fn solid(
    frame: &mut GlesFrame<'_, '_>,
    rect: Rectangle<i32, Physical>,
    color: Color32F,
) -> anyhow::Result<()> {
    frame.draw_solid(rect, &[rect], color)
        .map_err(|e| anyhow::anyhow!("draw_solid: {e}"))?;
    Ok(())
}

impl PaneRenderer {
    pub fn new(atlas: &GlyphAtlas) -> Self {
        let cell_w = atlas.cell_width() as i32;
        let cell_h = atlas.cell_height() as i32;

        let tag_text = "~/src/pane  Del Snarf Get Put | Look".to_string();

        let width = 120u16;
        let height = 30u16;
        let mut cells = Vec::with_capacity(width as usize * height as usize);

        // Row 0: empty
        for _ in 0..width { cells.push(Cell::default()); }

        // Row 1: " pane — desktop environment"
        let line1 = " pane \u{2014} desktop environment";
        for (i, ch) in (0..width).zip(line1.chars().chain(std::iter::repeat(' '))) {
            cells.push(Cell {
                ch,
                fg: if i < 5 { Color::Named(NamedColor::BrightCyan) } else { Color::Default },
                bg: Color::Default,
                attrs: if i < 5 { CellAttrs::BOLD } else { CellAttrs::empty() },
            });
        }

        // Row 2: empty
        for _ in 0..width { cells.push(Cell::default()); }

        // Row 3: " text is the interface"
        let line3 = " text is the interface";
        for (_, ch) in (0..width).zip(line3.chars().chain(std::iter::repeat(' '))) {
            cells.push(Cell {
                ch,
                fg: Color::Named(NamedColor::Green),
                bg: Color::Default,
                attrs: CellAttrs::ITALIC,
            });
        }

        // Row 4: empty
        for _ in 0..width { cells.push(Cell::default()); }

        // Row 5: " B2=execute  B3=route"
        let line5 = " B2=execute  B3=route";
        for (_, ch) in (0..width).zip(line5.chars().chain(std::iter::repeat(' '))) {
            cells.push(Cell {
                ch,
                fg: Color::Named(NamedColor::Yellow),
                bg: Color::Default,
                attrs: CellAttrs::empty(),
            });
        }

        let body_cells = CellRegion::new(0, 0, width, height, cells)
            .expect("hardcoded region dimensions are valid");

        Self { tag_text, body_cells, cell_w, cell_h }
    }

    /// Render the complete pane frame.
    pub fn render(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        _atlas: &GlyphAtlas,
        window_size: Size<i32, Physical>,
    ) -> anyhow::Result<()> {
        let pane_w = self.body_cells.width as i32 * self.cell_w;
        let pane_h = self.cell_h + BORDER_PX * 2 + self.body_cells.height as i32 * self.cell_h;

        let pane_x = (window_size.w - pane_w - BORDER_PX * 2) / 2;
        let pane_y = (window_size.h - pane_h) / 2;

        // --- Tag line background ---
        solid(frame, Rectangle::new(
            (pane_x, pane_y).into(),
            (pane_w + BORDER_PX * 2, self.cell_h).into(),
        ), TAG_BG)?;

        // --- Beveled borders ---
        let body_top = pane_y + self.cell_h;
        let body_area_w = pane_w + BORDER_PX * 2;
        let body_area_h = self.body_cells.height as i32 * self.cell_h + BORDER_PX * 2;

        // Light edge (top + left)
        solid(frame, Rectangle::new(
            (pane_x, body_top).into(),
            (body_area_w, BORDER_PX).into(),
        ), BORDER_LIGHT)?;
        solid(frame, Rectangle::new(
            (pane_x, body_top).into(),
            (BORDER_PX, body_area_h).into(),
        ), BORDER_LIGHT)?;

        // Dark edge (bottom + right)
        solid(frame, Rectangle::new(
            (pane_x, body_top + body_area_h - BORDER_PX).into(),
            (body_area_w, BORDER_PX).into(),
        ), BORDER_DARK)?;
        solid(frame, Rectangle::new(
            (pane_x + body_area_w - BORDER_PX, body_top).into(),
            (BORDER_PX, body_area_h).into(),
        ), BORDER_DARK)?;

        // --- Body background ---
        let body_x = pane_x + BORDER_PX;
        let body_y = body_top + BORDER_PX;
        solid(frame, Rectangle::new(
            (body_x, body_y).into(),
            (pane_w, self.body_cells.height as i32 * self.cell_h).into(),
        ), BODY_BG)?;

        // --- Cell backgrounds (colored cells only) ---
        for row in 0..self.body_cells.height {
            for col in 0..self.body_cells.width {
                let idx = row as usize * self.body_cells.width as usize + col as usize;
                if idx >= self.body_cells.cells.len() { break; }
                let cell = &self.body_cells.cells[idx];

                if cell.bg != Color::Default {
                    let [r, g, b, a] = color_to_rgba(&cell.bg);
                    solid(frame, Rectangle::new(
                        (body_x + col as i32 * self.cell_w, body_y + row as i32 * self.cell_h).into(),
                        (self.cell_w, self.cell_h).into(),
                    ), Color32F::new(r, g, b, a))?;
                }
            }
        }

        // Glyph rendering (text) requires uploading the atlas texture to the GPU
        // and using render_texture_from_to — deferred to the next iteration.
        // The skeleton renders: tag background, beveled borders, body background,
        // and colored cell backgrounds. Text glyphs are the next step.

        Ok(())
    }
}

/// Map a pane-proto Color to RGBA float values.
pub fn color_to_rgba(color: &Color) -> [f32; 4] {
    match color {
        Color::Default => BODY_FG,
        Color::Named(named) => named_to_rgba(named),
        Color::Indexed(idx) => indexed_to_rgba(*idx),
        Color::Rgb(r, g, b) => [*r as f32 / 255.0, *g as f32 / 255.0, *b as f32 / 255.0, 1.0],
    }
}

fn named_to_rgba(color: &NamedColor) -> [f32; 4] {
    match color {
        NamedColor::Black => [0.0, 0.0, 0.0, 1.0],
        NamedColor::Red => [0.8, 0.2, 0.2, 1.0],
        NamedColor::Green => [0.2, 0.8, 0.2, 1.0],
        NamedColor::Yellow => [0.8, 0.8, 0.2, 1.0],
        NamedColor::Blue => [0.2, 0.4, 0.8, 1.0],
        NamedColor::Magenta => [0.8, 0.2, 0.8, 1.0],
        NamedColor::Cyan => [0.2, 0.8, 0.8, 1.0],
        NamedColor::White => [0.75, 0.75, 0.75, 1.0],
        NamedColor::BrightBlack => [0.4, 0.4, 0.4, 1.0],
        NamedColor::BrightRed => [1.0, 0.4, 0.4, 1.0],
        NamedColor::BrightGreen => [0.4, 1.0, 0.4, 1.0],
        NamedColor::BrightYellow => [1.0, 1.0, 0.4, 1.0],
        NamedColor::BrightBlue => [0.4, 0.6, 1.0, 1.0],
        NamedColor::BrightMagenta => [1.0, 0.4, 1.0, 1.0],
        NamedColor::BrightCyan => [0.4, 1.0, 1.0, 1.0],
        NamedColor::BrightWhite => [1.0, 1.0, 1.0, 1.0],
    }
}

fn indexed_to_rgba(idx: u8) -> [f32; 4] {
    match idx {
        0..=15 => {
            let named = match idx {
                0 => NamedColor::Black, 1 => NamedColor::Red,
                2 => NamedColor::Green, 3 => NamedColor::Yellow,
                4 => NamedColor::Blue, 5 => NamedColor::Magenta,
                6 => NamedColor::Cyan, 7 => NamedColor::White,
                8 => NamedColor::BrightBlack, 9 => NamedColor::BrightRed,
                10 => NamedColor::BrightGreen, 11 => NamedColor::BrightYellow,
                12 => NamedColor::BrightBlue, 13 => NamedColor::BrightMagenta,
                14 => NamedColor::BrightCyan, 15 => NamedColor::BrightWhite,
                _ => unreachable!(),
            };
            named_to_rgba(&named)
        }
        16..=231 => {
            let idx = idx - 16;
            let r = (idx / 36) % 6;
            let g = (idx / 6) % 6;
            let b = idx % 6;
            let to_f = |v: u8| if v == 0 { 0.0 } else { (55.0 + 40.0 * v as f32) / 255.0 };
            [to_f(r), to_f(g), to_f(b), 1.0]
        }
        232..=255 => {
            let v = (8.0 + 10.0 * (idx - 232) as f32) / 255.0;
            [v, v, v, 1.0]
        }
    }
}
