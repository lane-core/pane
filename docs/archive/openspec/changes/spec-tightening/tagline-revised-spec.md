# Tag Line: Revised Specification

Replacement language for architecture spec section 2 ("The Pane Primitive") and
pane-app spec (tag-related types, hello-pane example, protocol messages). This
document is ready for direct extraction into those specs.

Reviewed against BeOS UX principles by the Be systems engineer consultant.
Self-vet in section 6.

---

## 1. Architecture Spec: Revised Section 2 Tag Line Language

Replace the current paragraph beginning "**Tag line.** Editable text that
serves as title, command bar, and menu simultaneously..." with the following.

---

**Tag line.** A modal command surface that serves as identity, command bar, and
discovery mechanism for a pane. The tag line is always compositor-rendered (the
compositor owns the chrome). Tag line content travels through the pane protocol:
the client declares a title and a command vocabulary; the compositor renders the
chrome and manages the activation lifecycle.

The tag line has two states:

**At rest.** The tag line shows the pane's identity -- its title. The visual
presentation depends on the pane's context:

- **Floating panes** display a BeOS-style tab: a compact, colored, asymmetric
  shape sitting on the pane border. The tab contains the title, a close
  widget, and a zoom widget. This is the direct descendant of BeOS's
  `B_TITLED_WINDOW_LOOK` -- identity at a glance, a grab handle for movement,
  minimal cognitive load. The tab also carries a small command indicator glyph
  (`:` or `>_`) that signals the command surface exists.

- **Tiled panes** display a thin name strip at the top edge -- enough for
  identity, not a command bar. The strip shows the title and the command
  indicator. This is the tiled equivalent of the floating tab: structure is
  always visible, as the aesthetic spec requires. Without this strip, tiled
  panes lose identity and warmth. The strip is the border between "clean" and
  "anonymous."

**Activated.** The user triggers the command surface (activation key, click on
the command indicator, or compositor binding). A text input cursor appears in
the tab (floating) or the name strip expands into a command input field (tiled).
The user types commands. A completion dropdown appears below the input,
populated by the pane's command vocabulary. Enter executes the selected command.
Escape dismisses the command surface instantly, unconditionally, with no
confirmation. This is the transient promise: the command surface has the
temporal profile of a BPopUpMenu -- invoke, choose, dismiss -- not a modal
dialog.

**Empty-query mode.** Activating the command surface with no input (just the
activation gesture followed by nothing) shows a browsable, categorized list
of all available commands. This is the which-key discovery pattern and the
menu-bar safety net. New users browse; experienced users type to narrow. The
completion entries show keyboard shortcuts alongside command names, teaching
the fast path while providing the slow path -- exactly as BeOS menu items
displayed accelerator keys beside labels.

**The tag line is opt-in.** A pane without a tag line is a component -- a
building block meant to be interacted with through its parent's tag or its own
content area. The developer decides which panes have tags. The API makes this
decision natural: creating a pane with a title and vocabulary gives it a tag;
creating a pane with neither gives it a bare content area. A weather widget
has a tag (city autocomplete, refresh commands). A notification pane has a tag
(response templates). An editor subcomponent inside an IDE pane probably does
not -- the IDE pane's tag covers it.

**Scope.** When a pane is a container (its body is other panes), its tag line
governs container-level operations: layout manipulation, bulk actions, context
propagation. When the command surface activates, the compositor visually
indicates scope -- the governed region highlights, and the command input shows
a scope label. Leaf-pane commands target only that pane. Container commands
target the container and its children. The default activation gesture targets
the most specific pane (the leaf). Reaching the container level requires an
explicit scope-up gesture. This prevents the "which level am I commanding?"
confusion that plagues i3's `focus parent` / `focus child`.

**Scope is a developer responsibility.** The system provides the infrastructure
-- visual scope indicators, activation routing, hierarchical command dispatch.
The developer provides the vocabulary and decides which panes expose commands.
The API should make it natural to follow good design guidelines: a well-designed
container offers layout commands; a well-designed leaf offers content commands.
The system does not enforce this boundary, but completion categories and naming
conventions make the right choice obvious.

---

## 2. Revised pane-app Tag Types

These replace `TagLine`, `TagAction`, `TagCommand`, and `BuiltInAction` in
`pane-proto::tag` and the corresponding usage in the pane-app kit.

### Design rationale

The old types modeled an acme-style persistent tag line: a name string plus
a list of clickable action labels. The new types model a modal command surface:
an at-rest title, a command vocabulary that populates the completion dropdown
on activation, and a completion provider for dynamic completions.

The separation follows BeOS's own separation of concerns. BWindow had a title
(`SetTitle`/`Title()`), a menu bar (`SetKeyMenuBar`/`KeyMenuBar()`), and
keyboard shortcuts (`AddShortcut`). These were three independent aspects of
the window's command surface, declared independently. The new pane types follow
the same pattern: title is one thing, command vocabulary is another, completion
behavior is a third.

### Wire types (pane-proto::tag)

```rust
use serde::{Deserialize, Serialize};

/// The pane's at-rest identity. Displayed in the floating tab or tiled
/// name strip. This is the SetTitle/Title() equivalent.
///
/// A pane with `title: None` has no tag line -- it is a component pane,
/// visible only as content within its parent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneTitle {
    /// The display name shown in the tab/strip. Required for tagged panes.
    pub text: String,
    /// Short form for narrow contexts (e.g., tiled strip when space is
    /// constrained). If None, the compositor truncates `text` with an
    /// ellipsis. If Some, this is used verbatim.
    pub short: Option<String>,
}

/// The set of commands a pane offers through its command surface.
///
/// This is the menu-bar equivalent. BeOS's BMenuBar declared what commands
/// existed and how they were organized; CommandVocabulary does the same for
/// the completion-driven command surface.
///
/// The vocabulary is static in the sense that it is declared upfront and
/// updated explicitly (via set_vocabulary). Dynamic per-keystroke behavior
/// is handled by CompletionProvider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandVocabulary {
    /// Grouped commands. Each group is a category (layout, content, etc.)
    /// shown as a section header in the empty-query browsable list.
    pub groups: Vec<CommandGroup>,
}

/// A named group of commands, displayed as a category in the completion
/// dropdown during empty-query browsing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandGroup {
    /// Category label ("Layout", "Content", "Navigation", etc.).
    pub label: String,
    /// Commands in this group, displayed in order.
    pub commands: Vec<Command>,
}

/// A single command in the vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    /// The command name as the user would type it. Must be unique
    /// within the pane's vocabulary.
    pub name: String,
    /// Human-readable description shown in the completion dropdown.
    pub description: String,
    /// Keyboard shortcut, if any. Displayed alongside the command name
    /// in completions, teaching the fast path. Format: "Alt+W", "Ctrl+S".
    pub shortcut: Option<String>,
    /// What happens when this command is executed.
    pub action: CommandAction,
}

/// What a command does when executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandAction {
    /// A built-in compositor action.
    BuiltIn(BuiltIn),
    /// Deliver the command to the client as a CommandExecuted event.
    /// The client handles execution. The String is opaque data the
    /// client associated with this command.
    Client(String),
    /// Evaluate as a routing expression. The kit applies routing rules
    /// and dispatches to the matched target.
    Route(String),
    /// Run as a shell command.
    Shell(String),
}

/// Built-in compositor actions. These are handled by the compositor
/// without client involvement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuiltIn {
    /// Close this pane.
    Close,
    /// Copy selection to clipboard.
    Copy,
    /// Paste from clipboard.
    Paste,
    /// Undo last action (if the compositor tracks undo).
    Undo,
    /// Redo last undone action.
    Redo,
}

/// A completion result returned by the client in response to a
/// CompletionRequest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Completion {
    /// The text to insert if this completion is selected.
    pub value: String,
    /// Display label (may differ from value -- e.g., showing
    /// "San Francisco, CA" while inserting "san-francisco").
    pub label: String,
    /// Optional detail text shown in a secondary line.
    pub detail: Option<String>,
    /// Optional icon hint (the compositor resolves to actual rendering).
    pub icon: Option<String>,
}

/// Events related to the command surface lifecycle.
/// These are variants within PaneEvent / CompToClient.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandSurfaceEvent {
    /// The user activated the command surface. The compositor is now
    /// accepting input in the tag area. The client may want to update
    /// its vocabulary or prepare its completion provider.
    Activated,
    /// The command surface was dismissed (Escape, focus loss, or
    /// command execution). The pane returns to at-rest state.
    Dismissed,
    /// The user executed a client-handled command.
    /// Contains the Command::name and any arguments the user typed
    /// after the command name.
    Executed {
        command: String,
        args: String,
    },
    /// The compositor is requesting completions for the current input.
    /// The client should respond with SetCompletions.
    CompletionRequest {
        /// The current text in the command input.
        input: String,
        /// A token to correlate the response with this request.
        /// Stale responses (wrong token) are discarded.
        token: u64,
    },
}
```

### Kit types (pane-app)

```rust
/// Builder for a pane's tag line configuration.
///
/// This is the developer-facing API for declaring what a pane's command
/// surface looks like. It follows the builder pattern so that the common
/// case (title + a few commands) is concise, while the full case
/// (dynamic completions, custom groups) is accessible without ceremony.
///
/// # Examples
///
/// Minimal (title only, no commands):
/// ```
/// Tag::new("Status")
/// ```
///
/// With commands:
/// ```
/// Tag::new("Editor").commands(vec![
///     cmd("save", "Save the current file")
///         .shortcut("Ctrl+S")
///         .client("save"),
///     cmd("close", "Close this pane")
///         .shortcut("Alt+W")
///         .built_in(BuiltIn::Close),
/// ])
/// ```
///
/// With dynamic completions:
/// ```
/// Tag::new("Weather").commands(vec![
///     cmd("city", "Change city").client("set-city"),
/// ]).on_completion(|input, respond| {
///     let matches = lookup_cities(input);
///     respond(matches);
/// })
/// ```
pub struct Tag {
    title: PaneTitle,
    vocabulary: CommandVocabulary,
    completion_provider: Option<CompletionFn>,
}

/// The completion callback type. Receives the current input text and
/// a responder that delivers completions back to the compositor.
///
/// This runs on the pane's looper thread. If the completion requires
/// slow work (network lookup, database query), spawn a thread and
/// call the responder from there -- the responder is Send.
pub type CompletionFn = Box<dyn FnMut(&str, CompletionResponder) + Send>;

/// Delivers completion results to the compositor. Send + Clone so it
/// can be moved into spawned threads for async completion work.
#[derive(Clone)]
pub struct CompletionResponder { /* internal: token + writer */ }

impl CompletionResponder {
    /// Send completions to the compositor.
    pub fn send(&self, completions: Vec<Completion>) -> Result<()> { .. }
}

impl Tag {
    /// Create a tag with the given title. No commands, no completions.
    /// The pane will have a tab/strip showing the title, but the command
    /// surface will be empty (only built-in compositor commands appear).
    pub fn new(title: impl Into<String>) -> Self { .. }

    /// Set the short title for constrained contexts.
    pub fn short(mut self, short: impl Into<String>) -> Self { .. }

    /// Set the command vocabulary. Replaces any previously set commands.
    pub fn commands(mut self, commands: Vec<Command>) -> Self { .. }

    /// Set grouped commands (explicit categories for empty-query browsing).
    pub fn groups(mut self, groups: Vec<CommandGroup>) -> Self { .. }

    /// Register a dynamic completion provider. Called when the user types
    /// in the command surface and the static vocabulary doesn't cover
    /// the input (or when a command accepts arguments that need completion).
    pub fn on_completion<F>(mut self, f: F) -> Self
    where
        F: FnMut(&str, CompletionResponder) + Send + 'static,
    { .. }
}

/// Convenience constructor for a Command. Designed for readable
/// inline vocabulary declarations.
pub fn cmd(name: impl Into<String>, description: impl Into<String>) -> CommandBuilder {
    CommandBuilder { name: name.into(), description: description.into(), ..Default::default() }
}

/// Builder for individual commands. Terminates with an action setter.
pub struct CommandBuilder {
    name: String,
    description: String,
    shortcut: Option<String>,
}

impl CommandBuilder {
    /// Set the keyboard shortcut display string.
    pub fn shortcut(mut self, s: impl Into<String>) -> Self { .. }

    /// This command is handled by the client. The string is opaque data
    /// delivered in the Executed event.
    pub fn client(self, data: impl Into<String>) -> Command { .. }

    /// This command is a built-in compositor action.
    pub fn built_in(self, action: BuiltIn) -> Command { .. }

    /// This command triggers routing evaluation.
    pub fn route(self, expr: impl Into<String>) -> Command { .. }

    /// This command runs a shell command.
    pub fn shell(self, cmd: impl Into<String>) -> Command { .. }
}
```

### The "no tag" case

A pane created without a `Tag` has no tag line -- no tab, no strip, no
command surface. It is a component pane. The API makes this the path of
least resistance for sub-panes:

```rust
// Component pane: no tag, just content.
let child = app.create_pane(None)?;

// Tagged pane: has a tab/strip and command surface.
let main = app.create_pane(Tag::new("My App").commands(vec![...]))?;
```

`create_pane` accepts `impl Into<Option<Tag>>`, so both `Tag::new("Title")`
and `None` work without ceremony. This makes the developer's decision
visible and natural: you either give the pane a name and commands, or you
don't. There is no "empty tag line" -- there is a tag, or there is no tag.

### Updated create_pane signature

```rust
impl App {
    /// Create a new pane.
    ///
    /// Pass a `Tag` to give the pane a tag line (tab/strip + command
    /// surface). Pass `None` for a component pane with no chrome.
    pub fn create_pane(
        &self,
        tag: impl Into<Option<Tag>>,
    ) -> Result<Pane, PaneError> { .. }

    /// Create a pane with explicit geometry hints.
    pub fn create_pane_with(
        &self,
        tag: impl Into<Option<Tag>>,
        hints: PaneHints,
    ) -> Result<Pane, PaneError> { .. }
}
```

### Updated Pane operations

```rust
impl Pane {
    /// Update the pane's title. The compositor re-renders the tab/strip.
    /// No-op if this is a component pane (no tag).
    pub fn set_title(&self, title: PaneTitle) -> Result<()> { .. }

    /// Replace the command vocabulary. The compositor updates its
    /// completion source. No-op if this is a component pane.
    pub fn set_vocabulary(&self, vocab: CommandVocabulary) -> Result<()> { .. }

    /// Send completions in response to a CompletionRequest.
    /// Typically called from the completion provider, not directly.
    pub fn set_completions(
        &self,
        token: u64,
        completions: Vec<Completion>,
    ) -> Result<()> { .. }
}
```

### Updated PaneEvent variants

Replace the `TagAction` and `TagRoute` variants with:

```rust
pub enum PaneEvent {
    // ... (Resize, Focus, Blur, Key, Mouse unchanged) ...

    // -- Command surface --

    /// The command surface was activated (user invoked it).
    CommandActivated,
    /// The command surface was dismissed.
    CommandDismissed,
    /// A client-handled command was executed.
    CommandExecuted { command: String, args: String },
    /// The compositor needs completions for the current input.
    CompletionRequest { input: String, token: u64 },

    // ... (Close, Disconnected, Reconnected, ScriptQuery unchanged) ...
}
```

And the corresponding Handler trait methods:

```rust
pub trait Handler {
    // ... (ready, resized, focused, blurred, key, mouse unchanged) ...

    /// The command surface was activated.
    fn command_activated(&mut self, _pane: &Pane) -> Result<bool> {
        Ok(true)
    }

    /// The command surface was dismissed.
    fn command_dismissed(&mut self, _pane: &Pane) -> Result<bool> {
        Ok(true)
    }

    /// A client-handled command was executed by the user.
    fn command_executed(
        &mut self,
        _pane: &Pane,
        _command: &str,
        _args: &str,
    ) -> Result<bool> {
        Ok(true)
    }

    /// The compositor is requesting completions.
    /// Default: no dynamic completions (static vocabulary only).
    fn completion_request(
        &mut self,
        _pane: &Pane,
        _input: &str,
        _token: u64,
    ) -> Result<bool> {
        Ok(true)
    }

    // ... (close_requested, disconnected, reconnected, script_query unchanged) ...
}
```

### Updated wire protocol messages

```rust
pub enum ClientToComp {
    /// Set the pane's title (at-rest display).
    SetTitle { pane: PaneId, title: PaneTitle },
    /// Set the pane's command vocabulary.
    SetVocabulary { pane: PaneId, vocab: CommandVocabulary },
    /// Deliver completions in response to a CompletionRequest.
    SetCompletions { pane: PaneId, token: u64, completions: Vec<Completion> },
    // ... (SetContent, RequestClose, CreatePane, ScriptReply, HeartbeatAck) ...
    // Note: CreatePane now carries Option<Tag> (serialized as title + vocab)
    //       instead of TagLine.
}

pub enum CompToClient {
    // ... (PaneCreated, Resize, Focus, Blur, Key, Mouse unchanged) ...
    /// Command surface lifecycle and execution events.
    Command { pane: PaneId, event: CommandSurfaceEvent },
    // ... (Close, CloseAck, ScriptQuery, Heartbeat unchanged) ...
    // Note: TagAction and TagRoute are removed.
}
```

---

## 3. Revised Hello-Pane Example

```rust
use pane_app::{App, Tag, cmd, BuiltIn};

fn main() -> pane_app::Result<()> {
    let app = App::connect("com.example.hello")?;

    let pane = app.create_pane(
        Tag::new("Hello").commands(vec![
            cmd("close", "Close this pane")
                .shortcut("Alt+W")
                .built_in(BuiltIn::Close),
        ]),
    )?;

    pane.run(|event| match event {
        pane_app::PaneEvent::Key(key) if key.is_escape() => Ok(false),
        pane_app::PaneEvent::Close => Ok(false),
        _ => Ok(true),
    })
}
```

Fourteen lines. The tag is declared as a title ("Hello") with one command
(close). At rest, the pane shows a tab labeled "Hello" with a close widget
and a `:` indicator. Hitting the activation key opens the command surface;
typing "close" or browsing the empty-query list shows the close command
with its Alt+W shortcut. The command vocabulary is the menu bar; the
activation gesture is the menu click.

---

## 4. Second Example: Weather Widget

A pane with dynamic completions showing the developer experience of
providing a command vocabulary and a completion provider.

```rust
use pane_app::{App, Tag, cmd, Handler, Pane, PaneEvent};
use pane_proto::tag::{Completion, PaneTitle};

struct Weather {
    city: String,
    data: Option<WeatherData>,
}

impl Handler for Weather {
    fn ready(&mut self, pane: &Pane) -> pane_app::Result<()> {
        self.refresh(pane)
    }

    fn command_executed(
        &mut self,
        pane: &Pane,
        command: &str,
        args: &str,
    ) -> pane_app::Result<bool> {
        match command {
            "city" => {
                self.city = args.to_string();
                pane.set_title(PaneTitle {
                    text: format!("Weather: {}", self.city),
                    short: Some(self.city.clone()),
                })?;
                self.refresh(pane)?;
            }
            "refresh" => { self.refresh(pane)?; }
            _ => {}
        }
        Ok(true)
    }

    fn completion_request(
        &mut self,
        pane: &Pane,
        input: &str,
        token: u64,
    ) -> pane_app::Result<bool> {
        // Only complete the "city" command's argument.
        if let Some(query) = input.strip_prefix("city ") {
            let matches = lookup_cities(query);
            pane.set_completions(token, matches)?;
        }
        Ok(true)
    }

    fn close_requested(&mut self, _pane: &Pane) -> pane_app::Result<bool> {
        Ok(false) // accept close
    }
}

impl Weather {
    fn refresh(&mut self, pane: &Pane) -> pane_app::Result<()> {
        // Spawn a thread for the network call. The looper stays responsive.
        let city = self.city.clone();
        let handle = pane.handle();
        std::thread::spawn(move || {
            if let Ok(data) = fetch_weather(&city) {
                let _ = handle.post_content(data.render().as_bytes());
            }
        });
        Ok(())
    }
}

fn main() -> pane_app::Result<()> {
    let app = App::connect("com.pane.weather")?;

    let pane = app.create_pane(
        Tag::new("Weather: San Francisco")
            .short("SF")
            .commands(vec![
                cmd("city", "Change city").client("set-city"),
                cmd("refresh", "Refresh weather data")
                    .shortcut("Ctrl+R")
                    .client("refresh"),
                cmd("close", "Close widget")
                    .shortcut("Alt+W")
                    .built_in(pane_app::BuiltIn::Close),
            ]),
    )?;

    pane.run_with(Weather {
        city: "San Francisco".into(),
        data: None,
    })
}
```

What this demonstrates:

1. **Title as identity.** The at-rest tab shows "Weather: San Francisco"
   (or "SF" when space is tight). The title updates dynamically when the
   user changes cities.

2. **Static vocabulary for structure.** Three commands are declared upfront.
   Empty-query browsing shows all three, categorized. The "city" command
   accepts an argument; "refresh" and "close" do not.

3. **Dynamic completions for arguments.** When the user types `city `, the
   completion provider fires. It does a prefix search over a city database
   and returns matches. The developer owns the completion logic; the
   compositor owns the rendering.

4. **Looper discipline.** The network call for weather data is spawned on a
   separate thread. The looper thread stays responsive. The result is
   delivered back via `handle.post_content`. This is Hoffman's rule
   in practice: "Keeping a window locked or its thread occupied for long
   periods of time is Not Good."

5. **The developer provides the vocabulary.** The system provides the
   infrastructure (command surface rendering, completion dropdown, scope
   indicators). The developer provides the commands, descriptions,
   shortcuts, and completion logic. The same division of labor as BeOS
   menus: the Interface Kit rendered the menu; the developer populated it.

---

## 5. Filesystem Representation

The tag line's filesystem representation under `/pane/<id>/` updates to
match:

```
/pane/<id>/
    title         # read: current title text; write: update title
    commands/     # directory listing shows command names
        close     # read: command metadata (description, shortcut, action)
        city      # read: command metadata
        refresh   # read: command metadata
    ctl           # write "activate" to open command surface
                  # write "dismiss" to close it
                  # write "exec <command> <args>" to execute a command
```

Scripting a pane's commands from the shell:

```sh
# List available commands:
ls /pane/3/commands/

# Read a command's metadata:
cat /pane/3/commands/city
# name: city
# description: Change city
# action: client(set-city)

# Execute a command:
echo 'exec city "San Francisco"' > /pane/3/ctl

# Activate the command surface programmatically:
echo 'activate' > /pane/3/ctl
```

This preserves the compositional equivalence invariant: every command
surface operation is accessible through the filesystem. Scripts can
discover commands, inspect their metadata, and execute them. No special
tool, no special protocol -- just files.

---

## 6. Self-Vet: Review Against BeOS UX Principles

### Schillings's clarity test

> "Common things are easy to implement and the programming model is CLEAR.
> You don't need to know hundreds of details to get simple things working."
> -- Benoit Schillings, Be Newsletter #1-2

**Hello-pane.** 14 lines, same as before. The `Tag::new("Hello").commands(..)`
pattern reads as natural English: "a tag named Hello with these commands."
The builder chain is linear -- no nesting, no trait objects, no generics visible
to the developer. The `cmd` convenience function keeps command declarations
compact.

**Verdict: passes.** The common case (title + a few commands) is a one-liner.
The uncommon case (dynamic completions) adds one method call. The rare case
(grouped categories, short titles) adds one more. Progressive complexity
matches progressive need.

### Warmth and approachability

The UX review flagged two warmth concerns. Status of each:

1. **Tiled panes need identity at rest.** Addressed. Tiled panes get a thin
   name strip. The spec language is explicit: "The strip is the border
   between clean and anonymous." The strip provides the same identity function
   as the floating tab, adapted to the tiled context. This parallels BeOS's
   `window_look` system -- different visual presentations for different
   contexts, same identity function.

2. **The command indicator glyph.** The floating tab and tiled strip both
   carry a small `:` or `>_` glyph that signals "there is more here." This
   is the visible affordance the UX review requested. It is the persistent
   visual cue that a command surface exists, filling the role that a menu bar
   fills in BeOS. It does not compete with the title for attention, but it is
   always present. A user who sees the glyph and clicks it discovers the
   command surface. After that, completion teaches the rest.

**Remaining gap:** The spec does not define a first-run hint (e.g., a brief
overlay on first focus of a tagged pane). This is a polish item, not a spec
item. The glyph is the structural answer; a first-run hint is a UX layer
the compositor can add without protocol changes.

### Discoverability

The UX review identified BeOS's three-layer discoverability model: menus
(browse without knowing), shortcuts (fast path for known commands), and
context menus (what can I do here?). The revised design maps to all three:

| BeOS layer | Pane equivalent |
|---|---|
| Menus (browse) | Empty-query browsable list with categories |
| Shortcuts (accelerate) | Shortcut strings displayed in completions |
| Context menus (situational) | Completion narrows by input context |

The empty-query mode is the critical piece. Without it, the command surface
has the vim problem: powerful but invisible. With it, the user can always
activate and browse -- the same safety net that BeOS menus provided.

**Verdict: passes.** The three-layer model is preserved in translation.

### Modality

The UX review concluded this is "acceptable modality" that passes the Be
test on every count. The revised spec preserves this:

- **Does not block the system.** Per-pane command surface. Other panes
  continue functioning.
- **Visible.** The command surface is visually distinct when active.
- **Transient.** Escape always dismisses, unconditionally. The spec
  repeats this guarantee explicitly.
- **User-initiated.** Only activated by deliberate gesture.

This is structurally identical to BPopUpMenu: invoke, choose, dismiss.
The difference is typed input instead of pointer clicks, which is
an improvement for keyboard-driven users and no worse for pointer users
(the glyph is clickable, completions are clickable).

**Verdict: passes.**

### The hierarchical concern

The UX review flagged that hierarchical commands (container vs. leaf) must
make scope visually unambiguous. The revised spec addresses this:

- Default activation targets the leaf (most specific pane).
- Container-level requires an explicit scope-up gesture.
- Visual scope indication (border highlighting, scope label in command input).
- The developer decides which levels have commands.

The spec does not over-specify the scope-up gesture (modifier key, double
activation, explicit command) -- that is compositor UX, not protocol design.
The protocol supports it: the compositor knows the hierarchy and can route
activation to any level.

**Verdict: passes with one note.** The scope-up gesture needs UX testing
before the compositor ships. The protocol is ready; the interaction design
is open. This is the right order -- protocol first, interaction refinement
second.

### What would Gassee think?

> "You know you've done something right when your platform is perverted --
> programmers use your product in ways you hadn't thought of."
> -- Jean-Louis Gassee, Be Newsletter #4-10

The command vocabulary is a platform primitive. Developers fill it with
their own content. The completion provider is an extension point.
Together, they create fertile ground for creative use: a music player
whose command surface searches tracks by lyric fragments, a file manager
whose completions show path previews, a chat client whose commands include
emoji search. The system provides the input surface and the rendering; the
developer provides the intelligence. This is the same shape as BeOS's
Translation Kit: the system provides the framework, developers provide
the translators.

**Verdict: passes.** The design invites creative use without prescribing it.

### What this design gives up

Honesty requires noting what is lost relative to both BeOS and the acme
model:

1. **Permanent command visibility (acme).** Acme's tag line was always
   visible with executable text. Users could see and click commands without
   any invocation gesture. The modal design sacrifices this for cleanliness.
   The mitigation (indicator glyph, empty-query browsing) is adequate but
   not equivalent. A user who never activates the command surface has a
   dumb window manager. BeOS never had this failure mode because the menu
   bar was always there.

2. **Positional muscle memory (BeOS menus).** In BeOS, a menu item was
   always in the same position. Users developed spatial memory: "Save is
   the third item in the File menu." Completion-driven discovery trades
   positional memory for nominal memory (remembering command names). This
   is a genuine tradeoff, not purely a win. The mitigation is that command
   names are generally more memorable than menu positions, and fuzzy
   matching compensates for imperfect recall.

3. **Pointer-only browsing.** BeOS menus required zero typing. Open the
   menu with a click, scan visually, click the item. The command surface
   requires at least the activation gesture and then either typing or
   pointer interaction with the completion dropdown. For users who avoid
   the keyboard, this is slightly more friction. The mitigation is that
   the indicator glyph is clickable and the completion list is navigable
   by pointer.

None of these are dealbreakers. They are honest costs of the design choice.
The benefits (cleaner at-rest appearance, keyboard efficiency, extensible
completion, unified interaction model) outweigh them for pane's target
audience. But they should be documented, not hidden.

---

## Sources

- Haiku source: `headers/os/interface/Window.h` (window_look, window_feel, window_type, SetTitle, SetKeyMenuBar, AddShortcut)
- Haiku source: `src/servers/app/decorator/DefaultDecorator.cpp` (tab rendering, gradient fill)
- Haiku source: `src/servers/app/decorator/TabDecorator.h` (fFocusTabColor, fNonFocusTabColor)
- Be Newsletter #1-2: Schillings on API clarity ("the programming model is CLEAR")
- Be Newsletter #2-36: Hoffman on window thread responsiveness, Adams on BLooper/BMessage/BHandler/BMessageFilter
- Be Newsletter #4-10: Gassee on platform perversion and expressive power
- Be Newsletter #4-22: Modality philosophy ("Not all questions deserve undivided attention")
- Be Newsletter #2-6: Modal dialog Q&A ("modeless unless the interaction HAS to be executed")
- UX review: `review-tagline-ux.md` (modal concern, discoverability analysis, warmth recommendations)
- Input Kit research: `research-input-kit.md` (vim grammar, completion models, which-key pattern)
- Be wisdom compendium: `benewsletter-wisdom.md`
- Current architecture spec: section 2 ("The Pane Primitive")
- Current pane-app spec: sections 1-15
- Current pane-proto tag types: `crates/pane-proto/src/tag.rs`
