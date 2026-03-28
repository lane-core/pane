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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    /// The command name as the user types it. Unique within the vocabulary.
    pub name: String,
    /// Human-readable description shown in completions.
    pub description: String,
    /// Keyboard shortcut displayed alongside the command (e.g., "Ctrl+S").
    pub shortcut: Option<String>,
    /// What happens when executed.
    pub action: CommandAction,
    /// Whether this command is currently available. Disabled commands
    /// appear grayed out in the command surface (BMenuItem::SetEnabled).
    pub enabled: bool,
}

/// What a command does when executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandAction {
    /// Built-in compositor action.
    BuiltIn(BuiltIn),
    /// Sent to the client's handler as a command_executed event.
    Client(String),
    /// Dispatched through the routing infrastructure.
    Route(String),
}

/// Built-in compositor actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuiltIn {
    /// Close this pane.
    Close,
    /// Copy selection to clipboard.
    Copy,
    /// Paste from clipboard.
    Paste,
    /// Undo last action.
    Undo,
    /// Redo last undone action.
    Redo,
}

/// A completion entry returned by the pane's completion provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Completion {
    /// The completion text to insert.
    pub text: String,
    /// Description shown alongside.
    pub description: Option<String>,
}
