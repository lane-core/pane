## ADDED Requirements

### Requirement: PTY bridge lifecycle
pane-shell SHALL create a CellGrid pane via the pane protocol, fork a shell process (default: user's login shell from `$SHELL`) connected via a PTY, and bridge I/O between the PTY and the pane protocol until the shell exits. When the shell exits, pane-shell SHALL send a Close request for the pane.

**Polarity**: Boundary (bridges Value cell writes and Compute event handling)
**Crate**: `pane-shell`

#### Scenario: Shell launch
- **WHEN** pane-shell starts
- **THEN** it SHALL create a CellGrid pane, fork a shell, and begin bridging I/O

#### Scenario: Shell exit
- **WHEN** the shell process exits (EOF on PTY)
- **THEN** pane-shell SHALL close its pane and exit

#### Scenario: Pane close requested
- **WHEN** the compositor sends CloseRequested
- **THEN** pane-shell SHALL send SIGHUP to the shell process, wait briefly, then exit

### Requirement: VT parser via vte
pane-shell SHALL use the `vte` crate to parse VT escape sequences from PTY output. The parser SHALL handle all sequences required by `xterm-256color` terminfo: cursor movement (CUU/CUD/CUF/CUB/CUP), erase (ED/EL), scroll regions (DECSTBM), character attributes (SGR), 256-color and RGB color (SGR 38/48), alternate screen (DECSET/DECRST 1049), bracketed paste mode (DECSET/DECRST 2004), and mouse reporting (DECSET 1000/1002/1003/1006).

**Polarity**: Compute (consumes byte stream, produces buffer mutations)
**Crate**: `pane-shell`

#### Scenario: Colored output
- **WHEN** the shell outputs `\e[31mhello\e[0m`
- **THEN** the screen buffer SHALL contain "hello" with fg=Red, and subsequent text SHALL have default attributes

#### Scenario: Cursor movement
- **WHEN** the shell outputs `\e[10;5H` (move cursor to row 10, col 5)
- **THEN** subsequent characters SHALL be written starting at row 10, col 5

#### Scenario: Alternate screen
- **WHEN** the shell outputs `\e[?1049h`
- **THEN** pane-shell SHALL switch to the alternate buffer and clear it
- **WHEN** the shell outputs `\e[?1049l`
- **THEN** pane-shell SHALL restore the primary buffer

### Requirement: Screen buffer model
pane-shell SHALL maintain two screen buffers (primary and alternate), each a grid of `Cell` values with dimensions matching the pane geometry (cols × rows). The primary buffer SHALL have an associated scrollback ring buffer. The alternate buffer SHALL NOT have scrollback.

**Polarity**: Value (the buffer is structured data)
**Crate**: `pane-shell`

#### Scenario: Buffer dimensions
- **WHEN** the compositor sends a Resize event (cols=80, rows=24)
- **THEN** both buffers SHALL be resized to 80×24, preserving content where possible

#### Scenario: Scrollback
- **WHEN** text scrolls off the top of the primary buffer
- **THEN** the scrolled-off rows SHALL be preserved in the scrollback ring buffer (up to a configurable limit)

#### Scenario: Scroll view
- **WHEN** the user scrolls back (Scroll event with negative delta)
- **THEN** pane-shell SHALL display scrollback content and mark the pane as dirty (unread output below viewport)

### Requirement: Dirty tracking
pane-shell SHALL track which rows have been modified since the last frame. On each frame tick, pane-shell SHALL collect all dirty rows into one or more CellRegion writes and send them to the compositor. After sending, dirty flags SHALL be cleared.

**Polarity**: Boundary
**Crate**: `pane-shell`

#### Scenario: Single character output
- **WHEN** the shell outputs a single character
- **THEN** the row containing the cursor SHALL be marked dirty

#### Scenario: Frame tick
- **WHEN** a frame tick occurs and rows 3, 7, 8 are dirty
- **THEN** pane-shell SHALL send CellRegion writes covering those rows and clear the dirty flags

#### Scenario: No changes
- **WHEN** a frame tick occurs and no rows are dirty
- **THEN** pane-shell SHALL NOT send any CellRegion writes

### Requirement: Input bridge
pane-shell SHALL translate pane KeyEvent messages into byte sequences and write them to the PTY. The translation SHALL follow xterm key encoding conventions (e.g., Enter → `\r`, Up → `\e[A`, Ctrl+C → `\x03`). When mouse reporting is enabled, MouseEvent messages SHALL be translated to the appropriate xterm mouse encoding.

**Polarity**: Compute (consumes events, produces PTY bytes)
**Crate**: `pane-shell`

#### Scenario: Regular key
- **WHEN** a KeyEvent for 'a' (no modifiers) is received
- **THEN** pane-shell SHALL write `a` to the PTY

#### Scenario: Control key
- **WHEN** a KeyEvent for 'c' with Ctrl modifier is received
- **THEN** pane-shell SHALL write `\x03` (ETX) to the PTY

#### Scenario: Arrow key
- **WHEN** a KeyEvent for Up is received
- **THEN** pane-shell SHALL write `\e[A` to the PTY (or `\eOA` in application cursor mode)

#### Scenario: Mouse with reporting
- **WHEN** a MouseEvent is received and mouse reporting is enabled (DECSET 1000)
- **THEN** pane-shell SHALL encode the event per the xterm mouse protocol and write it to the PTY

### Requirement: B2/B3 mouse semantics
When mouse reporting is NOT enabled by the shell, B2 (middle-click) and B3 (right-click) SHALL be handled by pane-shell for text execution and routing respectively. B1 (left-click) SHALL perform text selection. When mouse reporting IS enabled, all mouse buttons SHALL be forwarded to the PTY and B2/B3 pane semantics SHALL be suspended.

**Hazard**: Applications that enable mouse reporting (vim, htop) take over B2/B3. The user loses execute/route until the app exits or disables mouse reporting. This is the correct behavior — the app asked for mouse input.

#### Scenario: B2 without mouse reporting
- **WHEN** the user B2-clicks text "cargo build" and mouse reporting is off
- **THEN** pane-shell SHALL send "cargo build" as a TagExecute action

#### Scenario: B2 with mouse reporting
- **WHEN** the user B2-clicks and mouse reporting is on
- **THEN** pane-shell SHALL forward the click to the PTY as a mouse event

### Requirement: Tag line integration
pane-shell SHALL maintain its tag line with: the current working directory as `name`, standard built-in actions (Del, Snarf, Get, Put), and a text region for user commands. The working directory SHALL be updated when the shell sends OSC 7 (`\e]7;file://host/path\e\\`) or periodically by reading `/proc/$PID/cwd`.

**Polarity**: Value
**Crate**: `pane-shell`

#### Scenario: Directory change
- **WHEN** the shell changes directory and sends OSC 7
- **THEN** the tag line name SHALL update to reflect the new working directory

#### Scenario: Tag execute
- **WHEN** the user B2-clicks "cargo test" in the tag line
- **THEN** pane-shell SHALL write "cargo test\n" to the PTY

### Requirement: TERM environment
pane-shell SHALL set `TERM=xterm-256color` in the shell process environment. The PTY dimensions SHALL be set via `TIOCSWINSZ` ioctl when the pane is created and on every Resize event.

#### Scenario: Initial terminal size
- **WHEN** pane-shell creates the pane and receives the initial geometry
- **THEN** the PTY SHALL be configured with the matching columns and rows via TIOCSWINSZ

#### Scenario: Resize
- **WHEN** the compositor sends a Resize event
- **THEN** pane-shell SHALL update the PTY size via TIOCSWINSZ, sending SIGWINCH to the shell
