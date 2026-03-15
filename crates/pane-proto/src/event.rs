use std::fmt;

use serde::{Deserialize, Serialize};

bitflags::bitflags! {
    /// Keyboard modifier state.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Modifiers: u8 {
        const SHIFT = 0b0001;
        const CTRL  = 0b0010;
        const ALT   = 0b0100;
        const SUPER = 0b1000;
    }
}

/// A key identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Key {
    /// A Unicode character.
    Char(char),
    /// A named key.
    Named(NamedKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamedKey {
    Enter,
    Tab,
    Backspace,
    Escape,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    F(FKey),
    Insert,
}

/// Function key number (1-24). Validated at construction time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FKey(u8);

impl FKey {
    pub fn get(self) -> u8 {
        self.0
    }
}

/// Error when constructing an FKey with an out-of-range value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FKeyError(pub u8);

impl fmt::Display for FKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "function key {} out of range (valid: 1-24)", self.0)
    }
}

impl std::error::Error for FKeyError {}

impl TryFrom<u8> for FKey {
    type Error = FKeyError;

    fn try_from(n: u8) -> Result<Self, Self::Error> {
        if (1..=24).contains(&n) {
            Ok(FKey(n))
        } else {
            Err(FKeyError(n))
        }
    }
}

/// Whether a key was pressed or released.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyState {
    Press,
    Release,
}

/// A keyboard event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEvent {
    pub key: Key,
    pub modifiers: Modifiers,
    pub state: KeyState,
}

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    Back,
    Forward,
}

/// Kind of mouse event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseEventKind {
    Press(MouseButton),
    Release(MouseButton),
    Move,
    ScrollUp,
    ScrollDown,
}

/// A mouse event in cell coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MouseEvent {
    pub col: u16,
    pub row: u16,
    pub kind: MouseEventKind,
    pub modifiers: Modifiers,
}
