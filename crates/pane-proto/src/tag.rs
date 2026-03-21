use serde::{Deserialize, Serialize};


/// Structured tag line data. The compositor renders this differently
/// depending on the pane kind: monospace text for cell grids,
/// graphical tab + buttons for widget panes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagLine {
    /// Pane identity (path, name, title).
    pub name: String,
    /// Built-in actions (Del, Snarf, Get, Put, etc.).
    pub actions: Vec<TagAction>,
    /// User-defined actions (right of the |).
    pub user_actions: Vec<TagAction>,
}

/// A single action in the tag line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagAction {
    /// Display label.
    pub label: String,
    /// What happens on execution (B2-click or left-click).
    pub command: TagCommand,
}

/// What a tag action does when executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TagCommand {
    /// Built-in compositor action.
    BuiltIn(BuiltInAction),
    /// Run as shell command.
    Shell(String),
    /// Send as route message.
    Route(String),
}

/// Built-in compositor actions available in the tag line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuiltInAction {
    /// Close this pane.
    Del,
    /// Copy selection to clipboard.
    Snarf,
    /// Reload/refresh pane content.
    Get,
    /// Save pane content.
    Put,
    /// Undo last action.
    Undo,
    /// Redo last undone action.
    Redo,
}

