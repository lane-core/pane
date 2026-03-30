//! Keyboard shortcut filter — transforms key combos into commands.
//!
//! BWindow::AddShortcut equivalent, implemented as a composable Filter.
//! When a key event matches a registered shortcut, it's transformed
//! into a CommandExecuted message. This bridges the keyboard and
//! the command surface.

use pane_proto::event::{Key, KeyEvent, KeyState, Modifiers};

use crate::event::Message;
use crate::filter::{MessageFilter, FilterAction};

/// A key combination: key + modifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyCombo {
    pub key: Key,
    pub modifiers: Modifiers,
}

impl KeyCombo {
    pub fn new(key: Key, modifiers: Modifiers) -> Self {
        KeyCombo { key, modifiers }
    }

    fn matches(&self, event: &KeyEvent) -> bool {
        event.key == self.key
            && event.modifiers == self.modifiers
            && event.state == KeyState::Press
    }
}

/// A registered shortcut: key combo → command name + args.
#[derive(Debug)]
struct Shortcut {
    combo: KeyCombo,
    command: String,
    args: String,
}

/// Filter that intercepts key events matching registered shortcuts
/// and transforms them into CommandExecuted messages.
///
/// ```ignore
/// use pane_app::shortcuts::{ShortcutFilter, KeyCombo};
/// use pane_proto::event::{Key, Modifiers};
///
/// let mut shortcuts = ShortcutFilter::new();
/// shortcuts.add(
///     KeyCombo::new(Key::Char('s'), Modifiers::CTRL),
///     "save", "",
/// );
/// shortcuts.add(
///     KeyCombo::new(Key::Char('w'), Modifiers::ALT),
///     "close", "",
/// );
/// pane.add_filter(shortcuts);
/// ```
#[derive(Debug)]
pub struct ShortcutFilter {
    shortcuts: Vec<Shortcut>,
}

impl ShortcutFilter {
    pub fn new() -> Self {
        ShortcutFilter { shortcuts: Vec::new() }
    }

    /// Whether any shortcuts are registered.
    pub fn is_empty(&self) -> bool {
        self.shortcuts.is_empty()
    }

    /// Register a keyboard shortcut. When the key combo is pressed,
    /// the filter transforms it into `Message::CommandExecuted { command, args }`.
    pub fn add(&mut self, combo: KeyCombo, command: impl Into<String>, args: impl Into<String>) {
        self.shortcuts.push(Shortcut {
            combo,
            command: command.into(),
            args: args.into(),
        });
    }
}

impl Default for ShortcutFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageFilter for ShortcutFilter {
    fn matches(&self, event: &Message) -> bool {
        matches!(event, Message::Key(_))
    }

    fn filter(&mut self, event: Message) -> FilterAction {
        if let Message::Key(ref key_event) = event {
            for shortcut in &self.shortcuts {
                if shortcut.combo.matches(key_event) {
                    return FilterAction::Pass(Message::CommandExecuted {
                        command: shortcut.command.clone(),
                        args: shortcut.args.clone(),
                    });
                }
            }
        }
        FilterAction::Pass(event)
    }
}
