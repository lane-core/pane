use serde::{Deserialize, Serialize};

/// Color for cell foreground or background.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Color {
    /// The pane's configured default color.
    Default,
    /// Standard ANSI named color.
    Named(NamedColor),
    /// Indexed color (0-255, xterm-256color palette).
    Indexed(u8),
    /// 24-bit RGB color.
    Rgb(u8, u8, u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamedColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}
