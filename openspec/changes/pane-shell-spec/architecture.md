## Context

pane-shell is a pane-native client. It creates a CellGrid pane, forks a shell process connected via a PTY, translates VT escape sequences into CellRegion writes, and translates pane input events (KeyEvent, MouseEvent) into PTY input bytes. The compositor renders the cells — it doesn't know it's rendering a terminal.

Reference implementations: alacritty (GPU terminal, Rust, uses vte crate for parsing), foot (Wayland-native terminal, C, custom VT parser), wezterm (Rust, multiplexing terminal). All maintain a screen buffer and send damage to the renderer.

The key difference from these: pane-shell doesn't render. It sends cell data to the compositor via the pane protocol. This means the screen buffer is the authoritative state, and dirty tracking determines what gets sent each frame.

## Goals / Non-Goals

**Goals:**
- Define what VT sequences pane-shell supports
- Define the screen buffer data model
- Define the PTY ↔ pane protocol bridge
- Define tag line integration with shell state
- Define dirty tracking for efficient CellRegion updates

**Non-Goals:**
- Implementation (that's a future change)
- Sixel/kitty graphics protocol (future extension)
- Terminal multiplexing (that's the compositor's job — tiling layout)
- Shell integration (OSC sequences for prompt marking, etc. — future extension)

## Decisions

### 1. Use the `vte` crate for VT parsing

vte is the de facto Rust VT parser (used by alacritty, wezterm). It handles the state machine for ANSI/VT escape sequences and calls trait methods for each parsed element (print character, execute control, CSI dispatch, etc.). Writing our own parser would be significant effort for no benefit.

### 2. Screen buffer: two grids + scrollback

- **Primary buffer**: rows × cols grid of Cell values. This is what the shell normally writes to.
- **Alternate buffer**: same dimensions. Used by full-screen apps (vim, htop, less). Entered via `\e[?1049h`, exited via `\e[?1049l`.
- **Scrollback**: a ring buffer of rows that scrolled off the top of the primary buffer. Not sent to the compositor unless the user scrolls back.

The active buffer (primary or alternate) is what gets sent to the compositor as CellRegion writes.

### 3. Dirty tracking: per-row bitset

Each row has a dirty bit. When a VT sequence modifies a cell, the row is marked dirty. On each frame tick (driven by the compositor's frame callback or a timer), all dirty rows are collected into CellRegion writes and sent to the compositor. The dirty bits are then cleared.

This is coarser than per-cell tracking but much simpler and sufficient for terminal workloads where whole lines change at once (shell output, scrolling).

### 4. Tag line reflects shell state

- **name**: the current working directory (updated via OSC 7 `\e]7;file://host/path\e\\` or by reading `/proc/pid/cwd`)
- **actions**: standard set (Del, Snarf, Get, Put) plus user-configurable
- **dirty state**: the pane is "dirty" when there's unread output below the viewport (new output arrived while scrolled back)

### 5. $TERM = xterm-256color

pane-shell advertises `TERM=xterm-256color` to the shell process. This provides broad compatibility with existing terminal applications. A custom terminfo entry (`pane-256color`) may be added later for pane-specific capabilities.

## Risks / Trade-offs

**[VT compatibility]** → No terminal emulator is 100% xterm-compatible. Edge cases in terminal applications will surface. Mitigation: use vte (battle-tested by alacritty), test with vttest, and add application-specific workarounds as needed.

**[Dirty tracking granularity]** → Per-row tracking sends full rows even if only one cell changed. For a 200-column terminal with a blinking cursor, this means sending 200 cells per blink. Mitigation: acceptable for initial implementation. Per-cell tracking can be added later if profiling shows it matters.

## Open Questions

- Should pane-shell handle OSC 52 (clipboard)? This would let terminal apps set the clipboard. The compositor would need to mediate.
- Should pane-shell handle OSC 8 (hyperlinks)? Terminal hyperlinks could map to B3-click routing.
- How does mouse reporting interact with B2/B3 click semantics? If the shell has mouse reporting enabled, do B2/B3 clicks go to the shell or to the compositor for execute/route?
