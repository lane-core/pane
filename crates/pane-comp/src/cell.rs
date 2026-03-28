use serde::{Deserialize, Serialize};

use pane_proto::color::Color;

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

/// Error when constructing a CellRegion with inconsistent dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellRegionError {
    pub width: u16,
    pub height: u16,
    pub cells_len: usize,
}

impl std::fmt::Display for CellRegionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "CellRegion: width={} * height={} = {}, but cells.len() = {}",
            self.width,
            self.height,
            self.width as usize * self.height as usize,
            self.cells_len
        )
    }
}

impl std::error::Error for CellRegionError {}

/// A positioned rectangle of cells within a pane body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellRegion {
    /// Starting column.
    pub col: u16,
    /// Starting row.
    pub row: u16,
    /// Width in columns.
    pub width: u16,
    /// Height in rows.
    pub height: u16,
    /// Cells in row-major order. Must have exactly width * height elements.
    pub cells: Vec<Cell>,
}

impl CellRegion {
    /// Construct a CellRegion, validating that cells.len() == width * height.
    pub fn new(
        col: u16,
        row: u16,
        width: u16,
        height: u16,
        cells: Vec<Cell>,
    ) -> Result<Self, CellRegionError> {
        let expected = width as usize * height as usize;
        if cells.len() != expected {
            return Err(CellRegionError {
                width,
                height,
                cells_len: cells.len(),
            });
        }
        Ok(Self {
            col,
            row,
            width,
            height,
            cells,
        })
    }
}
