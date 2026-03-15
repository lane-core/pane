use serde::{Deserialize, Serialize};

use crate::color::Color;

bitflags::bitflags! {
    /// Text attributes for a cell.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct CellAttrs: u8 {
        const BOLD          = 0b0000_0001;
        const DIM           = 0b0000_0010;
        const ITALIC        = 0b0000_0100;
        const UNDERLINE     = 0b0000_1000;
        const BLINK         = 0b0001_0000;
        const REVERSE       = 0b0010_0000;
        const HIDDEN        = 0b0100_0000;
        const STRIKETHROUGH = 0b1000_0000;
    }
}

/// A single character cell in the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttrs,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::Default,
            bg: Color::Default,
            attrs: CellAttrs::empty(),
        }
    }
}

/// A positioned rectangle of cells within a pane body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellRegion {
    /// Starting column.
    pub col: u16,
    /// Starting row.
    pub row: u16,
    /// Width in columns.
    pub width: u16,
    /// Cells in row-major order.
    pub cells: Vec<Cell>,
}
