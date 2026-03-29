use serde::{Deserialize, Serialize};

/// The pane's at-rest identity. Displayed in the floating tab or
/// tiled name strip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneTitle {
    /// The display name shown in the tab/strip.
    pub text: String,
    /// Short form for narrow contexts. If None, the compositor
    /// truncates `text` with an ellipsis.
    pub short: Option<String>,
}

/// The set of commands a pane offers through its command surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CommandVocabulary {
    /// Grouped commands. Each group is a category shown as a section
    /// header in the empty-query browsable list.
    pub groups: Vec<CommandGroup>,
}

/// A named group of commands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandGroup {
    /// Category label ("Layout", "Content", etc.).
    pub label: String,
    /// Commands in this group, displayed in order.
    pub commands: Vec<Command>,
}

/// A single command in the vocabulary.
///
/// Every command goes to the handler as `Message::CommandExecuted`.
/// The handler decides what to do — close, save, route, whatever.
/// There are no "built-in" compositor actions; the handler is always
/// in control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    /// The command name as the user types it. Unique within the vocabulary.
    pub name: String,
    /// Human-readable description shown in completions.
    pub description: String,
    /// Keyboard shortcut displayed alongside the command (e.g., "Ctrl+S").
    pub shortcut: Option<String>,
    /// Whether this command is currently available. Disabled commands
    /// appear grayed out in the command surface.
    pub enabled: bool,
}

/// A completion entry returned by the pane's completion provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Completion {
    /// The completion text to insert.
    pub text: String,
    /// Description shown alongside.
    pub description: Option<String>,
}
