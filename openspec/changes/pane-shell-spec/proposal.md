## Why

pane-shell is build sequence phase 4 — the first usable pane client. It bridges a PTY to the pane protocol: shell output (VT escape sequences) → cell grid writes, pane input events → PTY input. Without it, pane is a compositor that can render hardcoded rectangles. With it, pane is a terminal you can use.

The architecture spec has high-level constraints (xterm-256color, screen buffer model, dirty regions) but no behavioral contracts specifying what the VT parser handles, how the PTY bridge works, how the tag line integrates with shell state, or how the pane protocol maps to terminal semantics.

## What Changes

- Define behavioral contracts for the pane-shell crate
- Specify the VT parser scope (what escape sequences are supported)
- Specify the screen buffer model (primary + alternate, scrollback)
- Specify the PTY bridge (how keyboard/mouse events map to PTY input)
- Specify tag line integration (working directory, shell status, dirty state)
- Specify the CellRegion dirty tracking model

## Specs Affected

### New
- `pane-shell`: VT parser, PTY bridge, screen buffer, tag line integration, dirty tracking

### Modified
- None (architecture constraints already exist, this adds detailed contracts)

## Impact

- New spec at openspec/specs/pane-shell/spec.md
- No code changes — this is spec-only
- Informs the implementation when we build pane-shell
