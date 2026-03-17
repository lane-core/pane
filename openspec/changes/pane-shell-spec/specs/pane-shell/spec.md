## ADDED Requirements

### Requirement: Shell pane semantics
A shell pane SHALL present itself as an interactive command environment. The user interacts with commands, output, and a working directory — not with cells, buffers, or escape sequences. The pane's identity is the working directory. The pane's actions are shell commands. The pane's content is command output.

**Polarity**: Boundary
**Crate**: `pane-shell`

#### Scenario: User experience
- **WHEN** a user opens a shell pane
- **THEN** they SHALL see a shell prompt, be able to type commands, and see output — indistinguishable from any terminal, but integrated with pane's tag line, routing, and execution semantics

### Requirement: Tag line as shell interface
The tag line SHALL present the shell's semantic state:
- **name**: the current working directory (updated as the shell navigates)
- **built-in actions**: Del (close), Snarf (copy selection), Get (clear/reset), Put (not applicable for shells — may be repurposed)
- **user actions**: shell commands the user has added to the tag. B2-clicking a user action executes it as a shell command.

The tag line is the command bar for this shell session. Users add their frequently-used commands there. It persists across the session.

**Polarity**: Value
**Crate**: `pane-shell`

#### Scenario: Directory reflects navigation
- **WHEN** the user runs `cd ~/src/pane` in the shell
- **THEN** the tag line name SHALL update to `~/src/pane`

#### Scenario: Tag command execution
- **WHEN** the user B2-clicks "cargo test" in the tag line
- **THEN** `cargo test` SHALL execute in the shell as if the user typed it at the prompt

#### Scenario: Persistent user commands
- **WHEN** the user adds "make" to the tag line and later closes and reopens the shell pane
- **THEN** "make" SHALL still be in the tag line (persisted with session state)

### Requirement: Text execution and routing
Any visible text in the shell pane is actionable:
- **B1** (left-click): select text
- **B2** (middle-click): execute the selected text as a shell command
- **B3** (right-click): route the selected text (send to pane-route for pattern matching)

This is the core pane interaction model applied to terminal output. Compiler errors become clickable. File paths become navigable. URLs become openable. The user doesn't need to copy-paste — they click.

**Crate**: `pane-shell`

#### Scenario: Execute from output
- **WHEN** the user sees `cargo build --release` in the shell output and B2-clicks it
- **THEN** `cargo build --release` SHALL execute in the shell

#### Scenario: Route a file path
- **WHEN** the user sees `src/main.rs:42` in a compiler error and B3-clicks it
- **THEN** pane-route SHALL receive the text, match it against rules, and open the file in the editor at line 42

#### Scenario: Route a URL
- **WHEN** the user sees `https://docs.rs/smithay` in output and B3-clicks it
- **THEN** pane-route SHALL match the URL pattern and open it in the browser

### Requirement: Filesystem interface
When exposed via `/srv/pane/`, a shell pane SHALL present a semantic interface:

```
/srv/pane/<id>/
  cwd         # read: current working directory
  command     # write: execute a command in this shell
  output      # read: scrollback output (plain text)
  status      # read: idle/running, last exit code
  env         # read: environment variables (key=value lines)
  tag         # read/write: tag line (plain text, as always)
  ctl         # line commands: close, clear, reset
```

The interface speaks in terms of what the shell *does*, not how the terminal renders it.

**Crate**: `pane-shell` (via pane-fs translation)

#### Scenario: Script executes a command
- **WHEN** a script writes `ls -la` to `/srv/pane/1/command`
- **THEN** the shell SHALL execute `ls -la` and the output SHALL appear in the pane

#### Scenario: Script reads working directory
- **WHEN** a script reads `/srv/pane/1/cwd`
- **THEN** it SHALL receive the shell's current working directory path

#### Scenario: Script checks status
- **WHEN** a script reads `/srv/pane/1/status`
- **THEN** it SHALL receive whether a command is running and the last exit code

### Requirement: Mouse reporting coexistence
Some terminal applications (vim, htop, less) request mouse input. When the shell has enabled mouse reporting, mouse events SHALL be forwarded to the application. B2/B3 pane semantics (execute/route) SHALL be suspended while mouse reporting is active.

When mouse reporting is disabled (normal shell prompt usage), B2/B3 pane semantics resume.

**Hazard**: Users lose execute/route while a mouse-aware app is running. This is correct — the app asked for mouse input. Users can always use the tag line for execution regardless of mouse reporting state.

**Crate**: `pane-shell`

#### Scenario: vim has mouse enabled
- **WHEN** the user runs vim (which enables mouse reporting) and B2-clicks
- **THEN** the click SHALL be forwarded to vim, not interpreted as text execution

#### Scenario: Return to shell prompt
- **WHEN** the user exits vim (mouse reporting disabled)
- **THEN** B2/B3 clicks SHALL resume pane execute/route behavior

### Requirement: Working directory tracking
pane-shell SHALL track the shell's working directory via:
1. OSC 7 (`\e]7;file://host/path\e\\`) — sent by modern shells when configured
2. Fallback: reading `/proc/<pid>/cwd` periodically

The working directory determines the tag line name and the `wdir` field in route messages (so `src/main.rs:42` resolves relative to the correct directory).

**Crate**: `pane-shell`

#### Scenario: OSC 7 update
- **WHEN** the shell sends OSC 7 after `cd /tmp`
- **THEN** the tag line name and route wdir SHALL update to `/tmp`

---

## Internal Implementation Contracts

*These govern the internal mechanics between pane-shell and pane-comp. They are not user-facing.*

### Requirement: VT parser
pane-shell SHALL use the `vte` crate to parse VT escape sequences from PTY output. Supported sequences: cursor movement (CUU/CUD/CUF/CUB/CUP), erase (ED/EL), scroll regions (DECSTBM), character attributes (SGR), 256-color and RGB color (SGR 38/48), alternate screen (DECSET/DECRST 1049), bracketed paste (DECSET/DECRST 2004), mouse reporting (DECSET 1000/1002/1003/1006).

**Crate**: `pane-shell`

#### Scenario: Colored output round-trip
- **WHEN** the shell outputs `\e[31mhello\e[0m`
- **THEN** the compositor SHALL render "hello" in red

### Requirement: Screen buffer
pane-shell SHALL maintain two internal screen buffers (primary with scrollback, alternate without). The active buffer's content is translated to CellRegion writes for the compositor. Buffer dimensions match the pane geometry.

**Crate**: `pane-shell`

#### Scenario: Alternate screen
- **WHEN** vim enters alternate screen (`\e[?1049h`) and later exits (`\e[?1049l`)
- **THEN** the original shell output SHALL be restored in the pane

### Requirement: Dirty region tracking
pane-shell SHALL track modified rows and send only changed content to the compositor as CellRegion writes. The compositor receives positioned cell data — it does not know or care about VT sequences, buffers, or dirty tracking.

**Crate**: `pane-shell`

#### Scenario: Efficient updates
- **WHEN** the shell outputs a single line of text
- **THEN** pane-shell SHALL send only the affected row(s) to the compositor, not the entire screen

### Requirement: Input encoding
pane-shell SHALL translate pane KeyEvent/MouseEvent messages into byte sequences written to the PTY, following xterm encoding conventions. `TERM=xterm-256color`. PTY dimensions set via TIOCSWINSZ on create and resize.

**Crate**: `pane-shell`

#### Scenario: Resize propagation
- **WHEN** the compositor sends a Resize event
- **THEN** pane-shell SHALL update the PTY size and the shell SHALL receive SIGWINCH
