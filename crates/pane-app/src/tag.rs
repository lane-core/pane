use pane_proto::tag::{
    PaneTitle, CommandVocabulary, CommandGroup, Command,
};
use pane_proto::protocol::CreatePaneTag;

/// Builder for a pane's tag line configuration.
///
/// The tag defines a pane's identity: its title and command
/// vocabulary. You construct a Tag, pass it to
/// [`App::create_pane`](crate::App::create_pane), and the compositor
/// renders the title in the tab strip and makes the commands
/// available through the command surface.
///
/// After creation, update dynamically via
/// [`Messenger::set_title`](crate::Messenger::set_title) and
/// [`Messenger::set_vocabulary`](crate::Messenger::set_vocabulary).
///
/// # Examples
///
/// Minimal (title only):
/// ```
/// use pane_app::{Tag};
/// Tag::new("Status");
/// ```
///
/// With commands:
/// ```
/// use pane_app::{Tag, cmd};
/// Tag::new("Editor")
///     .command(cmd("save", "Save file").shortcut("Ctrl+S"))
///     .command(cmd("close", "Close pane").shortcut("Alt+W"));
/// ```
///
/// # BeOS
///
/// No direct ancestor. Combines the title (from `BWindow`'s
/// constructor) with the command vocabulary (a pane concept
/// replacing menu bars).
#[derive(Debug, Clone)]
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

    /// Add a single command. Can be chained:
    /// ```
    /// use pane_app::{Tag, cmd};
    /// Tag::new("Editor")
    ///     .command(cmd("save", "Save").shortcut("Ctrl+S"))
    ///     .command(cmd("close", "Close").shortcut("Alt+W"));
    /// ```
    pub fn command(mut self, command: impl Into<Command>) -> Self {
        let command = command.into();
        if self.vocabulary.groups.is_empty() {
            self.vocabulary.groups.push(CommandGroup {
                label: "Commands".into(),
                commands: vec![command],
            });
        } else {
            self.vocabulary.groups[0].commands.push(command);
        }
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
///
/// The command name is the action identifier — when the user executes
/// the command, the handler receives `Message::CommandExecuted { command: name, args }`.
#[derive(Debug, Clone)]
pub struct CommandBuilder {
    name: String,
    description: String,
    shortcut: Option<String>,
    enabled: bool,
}

/// Create a command with the given name and description.
///
/// The name is both what the user types and what the handler receives.
///
/// ```
/// use pane_app::cmd;
/// cmd("save", "Save the current file").shortcut("Ctrl+S");
/// ```
pub fn cmd(name: impl Into<String>, description: impl Into<String>) -> CommandBuilder {
    CommandBuilder {
        name: name.into(),
        description: description.into(),
        shortcut: None,
        enabled: true,
    }
}

impl CommandBuilder {
    /// Set the keyboard shortcut displayed in completions.
    pub fn shortcut(mut self, s: impl Into<String>) -> Self {
        self.shortcut = Some(s.into());
        self
    }

    /// Set whether this command is enabled (default: true).
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Build the Command.
    pub fn build(self) -> Command {
        Command {
            name: self.name,
            description: self.description,
            shortcut: self.shortcut,
            enabled: self.enabled,
        }
    }
}

// Allow using CommandBuilder directly where Command is expected.
// cmd("save", "Save").shortcut("Ctrl+S") produces a CommandBuilder;
// Tag::command() and Tag::commands() accept Command. This impl
// bridges the gap so .build() is optional in builder chains.
impl From<CommandBuilder> for Command {
    fn from(cb: CommandBuilder) -> Command {
        cb.build()
    }
}
