# Agent Implementation Guide

**Read CLAUDE.md first.** It is the authoritative project instruction
file. This file adds two things: a phase-to-spec mapping and a hazard
list. For everything else, use the openspec tooling directly.

## Getting started

```sh
openspec show <change>                          # read proposal + architecture
openspec instructions tasks --change <change>   # enriched context with dependencies
openspec show --type spec <spec-name>           # read a spec's contracts
openspec validate --all                         # structural validation
```

Read `openspec/specs/workflow/spec.md` in full before your first commit.

Use the `pane` schema for new changes:
```sh
openspec new change --schema pane "<name>"
```


## Specs by build phase

### Phase 1 (pane-proto) — COMPLETE ✓
- `pane-protocol`: message types, PaneMessage wrapper, state machine, polarity markers
- `cell-grid-types`: Cell, Color, CellAttrs, CellRegion, input events

### Phase 2 (pane-notify)
- `pane-notify`: fanotify/inotify abstraction, calloop integration

### Phase 3 (pane-comp skeleton) — IN PROGRESS
- `pane-compositor` (in pane-comp-skeleton change): winit backend, calloop event loop, chrome
- `cell-grid-renderer` (in pane-comp-skeleton change): glyph atlas, cell rendering, font loading
- Requires Linux build environment (smithay/wayland deps)

### Phase 4 (pane-shell)
- Architecture spec §pane-shell constraints: xterm-256color, screen buffer, dirty regions

### Phase 5+ (servers)
- Architecture spec §Servers: pane-route, pane-roster, pane-store, pane-fs
- `filesystem-config`: config-as-files, xattr metadata, reactive updates
- `plugin-discovery`: well-known directories, live add/remove


## Hazards

Things that will trip you up — not documented elsewhere in the specs:

1. **macOS can't build pane-comp.** smithay requires wayland-sys (Linux-only).
   Workspace default members exclude pane-comp. Use `nix develop` for
   consistent toolchain, Linux box for pane-comp.
2. **CellRegion requires validated construction** via `CellRegion::new()`.
   Direct struct construction bypasses the `width * height` invariant.
3. **FKey requires TryFrom<u8>** — values outside 1-24 are rejected.
4. **ProtocolState is NOT serializable** — no Serialize/Deserialize.
5. **PaneEvent::Created includes `kind`** — multi-pane redesign added this.
6. **PlumbMessage is gone** — renamed to RouteMessage. TagPlumb → TagRoute.
7. **frame() returns Result** — errors on payloads > u32::MAX.
8. **pane-input is not a server** — input handling is in pane-comp.
9. **Filesystem caching invariant** — servers never do fs I/O in the
   render loop. Cache on startup, update on pane-notify events only.
