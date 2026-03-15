use pane_proto::cell::{Cell, CellAttrs, CellRegion};
use pane_proto::color::{Color, NamedColor};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::utils::Size;

use crate::glyph_atlas::GlyphAtlas;

/// Tag line and chrome colors — 90s-inspired, BeOS-ish
const TAG_BG: [f32; 4] = [0.85, 0.82, 0.72, 1.0]; // warm beige
const TAG_FG: [f32; 4] = [0.1, 0.1, 0.1, 1.0]; // near-black text
const BORDER_LIGHT: [f32; 4] = [0.7, 0.68, 0.62, 1.0]; // light bevel edge
const BORDER_DARK: [f32; 4] = [0.3, 0.28, 0.25, 1.0]; // dark bevel edge
const BODY_BG: [f32; 4] = [0.12, 0.12, 0.14, 1.0]; // dark background
const BODY_FG: [f32; 4] = [0.85, 0.85, 0.80, 1.0]; // warm white text

/// Border width in pixels
const BORDER_W: u16 = 2;

/// Renders a single hardcoded pane with tag line, borders, and cell grid body.
pub struct PaneRenderer {
    tag_text: String,
    body_cells: CellRegion,
    cell_w: u16,
    cell_h: u16,
}

impl PaneRenderer {
    pub fn new(atlas: &GlyphAtlas) -> Self {
        let cell_w = atlas.cell_width();
        let cell_h = atlas.cell_height();

        // Hardcoded tag line
        let tag_text = "~/src/pane  Del Snarf Get Put | Look".to_string();

        // Hardcoded body content — welcome message with mixed colors
        let width = 40u16;
        let height = 6u16;
        let mut cells = Vec::with_capacity(width as usize * height as usize);

        // Row 0: empty
        for _ in 0..width {
            cells.push(Cell::default());
        }

        // Row 1: " pane — desktop environment"
        let line1 = " pane — desktop environment";
        for (i, ch) in (0..width).zip(line1.chars().chain(std::iter::repeat(' '))) {
            cells.push(Cell {
                ch,
                fg: if i < 5 {
                    Color::Named(NamedColor::BrightCyan)
                } else {
                    Color::Default
                },
                bg: Color::Default,
                attrs: if i < 5 {
                    CellAttrs::BOLD
                } else {
                    CellAttrs::empty()
                },
            });
        }

        // Row 2: empty
        for _ in 0..width {
            cells.push(Cell::default());
        }

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
        for _ in 0..width {
            cells.push(Cell::default());
        }

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

        Self {
            tag_text,
            body_cells,
            cell_w,
            cell_h,
        }
    }

    /// Render the complete pane frame into the current GL context.
    ///
    /// For this skeleton, we use smithay's renderer for clearing regions.
    /// Full cell-to-quad rendering with the glyph atlas texture requires
    /// custom GL calls which will be implemented incrementally.
    pub fn render(
        &self,
        _renderer: &mut GlesRenderer,
        _atlas: &mut GlyphAtlas,
        _window_size: Size<i32, smithay::utils::Physical>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Skeleton: geometry calculations only.
        // Actual GL quad rendering is the next step — for now the compositor
        // displays the cleared background color, proving the render loop works.

        let _tag_height = self.cell_h;
        let _body_x = BORDER_W;
        let _body_y = self.cell_h + BORDER_W;
        let _body_w = self.body_cells.width * self.cell_w;
        let _body_h = self.body_cells.height * self.cell_h;

        // TODO: upload atlas texture to GPU
        // TODO: draw tag background quad (TAG_BG)
        // TODO: draw tag text glyphs (TAG_FG)
        // TODO: draw border quads (BORDER_LIGHT top/left, BORDER_DARK bottom/right)
        // TODO: draw body background quad (BODY_BG)
        // TODO: for each cell: draw bg quad, draw fg glyph quad from atlas

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
            // Standard colors — map to named
            let named = match idx {
                0 => NamedColor::Black,
                1 => NamedColor::Red,
                2 => NamedColor::Green,
                3 => NamedColor::Yellow,
                4 => NamedColor::Blue,
                5 => NamedColor::Magenta,
                6 => NamedColor::Cyan,
                7 => NamedColor::White,
                8 => NamedColor::BrightBlack,
                9 => NamedColor::BrightRed,
                10 => NamedColor::BrightGreen,
                11 => NamedColor::BrightYellow,
                12 => NamedColor::BrightBlue,
                13 => NamedColor::BrightMagenta,
                14 => NamedColor::BrightCyan,
                15 => NamedColor::BrightWhite,
                _ => unreachable!(),
            };
            named_to_rgba(&named)
        }
        16..=231 => {
            // 6x6x6 color cube
            let idx = idx - 16;
            let r = (idx / 36) % 6;
            let g = (idx / 6) % 6;
            let b = idx % 6;
            let to_f = |v: u8| if v == 0 { 0.0 } else { (55.0 + 40.0 * v as f32) / 255.0 };
            [to_f(r), to_f(g), to_f(b), 1.0]
        }
        232..=255 => {
            // Greyscale ramp
            let v = (8.0 + 10.0 * (idx - 232) as f32) / 255.0;
            [v, v, v, 1.0]
        }
    }
}
