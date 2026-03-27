use pane_proto::tag::{
    PaneTitle, CommandVocabulary, CommandGroup, Command, CommandAction, BuiltIn,
};
use pane_proto::protocol::CreatePaneTag;

/// Builder for a pane's tag line configuration.
///
/// # Examples
///
/// Minimal (title only):
/// ```ignore
/// Tag::new("Status")
/// ```
///
/// With commands:
/// ```ignore
/// Tag::new("Editor").commands(vec![
///     cmd("save", "Save the current file")
///         .shortcut("Ctrl+S")
///         .client("save"),
///     cmd("close", "Close this pane")
///         .shortcut("Alt+W")
///         .built_in(BuiltIn::Close),
/// ])
/// ```
pub struct Tag {
    title: PaneTitle,
    vocabulary: CommandVocabulary,
}

impl Tag {
    /// Create a tag with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Tag {
            title: PaneTitle {
                text: title.into(),
                short: None,
            },
            vocabulary: CommandVocabulary::default(),
        }
    }

    /// Set the short title for constrained contexts.
    pub fn short(mut self, short: impl Into<String>) -> Self {
        self.title.short = Some(short.into());
        self
    }

    /// Set commands (flat list, wrapped in a default group).
    pub fn commands(mut self, commands: Vec<Command>) -> Self {
        self.vocabulary.groups = vec![CommandGroup {
            label: "Commands".into(),
            commands,
        }];
        self
    }

    /// Set grouped commands (explicit categories).
    pub fn groups(mut self, groups: Vec<CommandGroup>) -> Self {
        self.vocabulary.groups = groups;
        self
    }

    /// Convert to the wire representation for pane creation.
    pub fn into_wire(self) -> CreatePaneTag {
        CreatePaneTag {
            title: self.title,
            vocabulary: self.vocabulary,
        }
    }
}

/// Builder for a single command. Created via `cmd()`.
pub struct CommandBuilder {
    name: String,
    description: String,
    shortcut: Option<String>,
}

/// Create a command builder with the given name and description.
///
/// ```ignore
/// cmd("save", "Save the current file")
///     .shortcut("Ctrl+S")
///     .client("save")
/// ```
pub fn cmd(name: impl Into<String>, description: impl Into<String>) -> CommandBuilder {
    CommandBuilder {
        name: name.into(),
        description: description.into(),
        shortcut: None,
    }
}

impl CommandBuilder {
    /// Set the keyboard shortcut displayed in completions.
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }

    /// The command is handled by the client's Handler.
    pub fn client(self, data: impl Into<String>) -> Command {
        Command {
            name: self.name,
            description: self.description,
            shortcut: self.shortcut,
            action: CommandAction::Client(data.into()),
        }
    }

    /// The command is a built-in compositor action.
    pub fn built_in(self, action: BuiltIn) -> Command {
        Command {
            name: self.name,
            description: self.description,
            shortcut: self.shortcut,
            action: CommandAction::BuiltIn(action),
        }
    }

    /// The command dispatches through the routing infrastructure.
    pub fn route(self, expr: impl Into<String>) -> Command {
        Command {
            name: self.name,
            description: self.description,
            shortcut: self.shortcut,
            action: CommandAction::Route(expr.into()),
        }
    }
}
