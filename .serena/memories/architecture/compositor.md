---
type: architecture
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [pane-compositor, rio, app_server, compositor, window-management, Dev, multiplexer, decoration, CSD, tab, launcher, workspaces, nesting]
related: [architecture/kernel, architecture/router, architecture/proto, architecture/app, architecture/fs, decision/host_as_contingent_server, decision/kernel_naming]
agents: [plan9-systems-engineer, be-systems-engineer, session-type-consultant, optics-theorist]
---

# Architecture: pane-compositor

## Summary

pane-compositor is a Dev multiplexer — a normal pane (no
architectural privilege) that consumes real display and input
devices from pane-kernel and provides per-pane virtual
devices. It is rio in its bones (single event loop,
file-protocol authority, recursive composition) with
app_server's window management vocabulary (workspaces,
stacking, focus policy) exposed as compositor state
observable through pane-fs.

Panes render to their own buffers. The compositor composites
finished buffers. It does not render for panes. Decoration
is client-side (CSD), provided by a future pane-toolkit.

## Design philosophy

**rio's architecture + app_server's vocabulary + CSD +
text-based command surface.**

The compositor is the device multiplexer. It holds real
devices, provides virtual ones, manages window semantics.
It does not impose visual policy. Chrome is minimal (rio's
bar + a tab). The command surface is text-based and
composable with unix tools.

## Architecture

```
                pane applications
                     │
           ┌─────────┴─────────┐
           │                   │
    Typed Protocol         File protocol
  (WindowManagement,     (wctl, cons, mouse
   InputProtocol)         via pane-fs)
           │                   │
           └─────────┬─────────┘
                     │
             pane-compositor
           (Dev multiplexer)
                     │
           ┌─────────┼─────────┐
           │         │         │
      DisplayBackend InputSource AudioDevice
       (real Dev)    (real Dev)  (real Dev)
                     │
                pane-kernel
              platform backend
```

## Drawing model: Buffer-only, CSD

Panes render their own pixels — including decoration. The
compositor composites finished buffers. No command protocol,
no server-side rendering.

- Panes render to buffers (software, OpenGL, Vulkan)
- Compositor composites buffers + manages window semantics
- Decoration is CSD, provided by a future pane-toolkit
- Compositor draws only a minimal focus indicator (thin
  border, rio style) — not title bars, not buttons

This is consistent with Wayland's model (clients render
everything, compositor composites) and with
`host_as_contingent_server` (compositor doesn't impose
visual policy).

## Chrome: Rio's bar + Be's tab

Minimal chrome inspired by rio (thin colored bar indicating
focus) plus a small tab — heritage from Be's window tabs.

### The tab

The tab serves two functions:

1. **Identity.** Shows the pane's name. Sits on the bar.
2. **Command launcher.** Click or hotkey opens a command
   palette populated by the pane's tag hierarchy.

### The command launcher

Heritage: Be scripting protocol (every BHandler had a
queryable command vocabulary via `hey`) married to Plan 9's
text-is-the-interface philosophy (wctl commands were strings).

- Opens via `:` or equivalent keystroke (neovim's `:`, Emacs
  M-x model)
- Searches the pane's tag hierarchy with fzf-style fuzzy
  matching
- The tag hierarchy is the pane's declared command vocabulary
  — what the pane can do, organized as a navigable tree
- Text-based: the command surface composes with unix tools
- Readable from pane-fs: `cat /pane/3/tags | fzf` works from
  a terminal

The tab is the GUI entry point into a text-based command
surface that works equally well from a script.

## Per-pane virtual devices

The compositor provides each pane with virtual Dev devices,
exactly as rio multiplexes /dev/draw, /dev/cons, /dev/mouse:

| Virtual device | Provides | rio equivalent |
|---|---|---|
| display | Buffer submission surface | /dev/draw |
| keyboard | Key events for this pane | /dev/cons |
| pointer | Pointer events for this pane | /dev/mouse |
| wctl | Window management commands | /dev/wctl |
| snarf | Clipboard access | /dev/snarf |

Per-pane DeviceRegistry replaces Plan 9's per-process
namespace. Each pane sees only its own virtual devices.

## Window management: Two-tier pattern

Same pattern as pane-kernel — file protocol + typed API,
both accessing the same compositor state:

**File protocol (scripts/automation):**
```
echo resize 0 0 800 600 > /pane/comp/windows/3/ctl
cat /pane/comp/windows/3/title
ls /pane/comp/windows/
echo workspace 2 > /pane/comp/windows/3/ctl
```

**Typed protocol (application code):**
```rust
#[non_exhaustive]
pub enum WindowManagement {
    Resize { rect: Rect },
    Move { pos: Point },
    Minimize,
    Maximize,
    Close,
    SetTitle(String),
    Focus,
    SetWorkspace(u32),
}
```

Window state (list, stacking order, workspace assignment,
focus, geometry) observable through pane-fs via MonadicLens
projections.

## Input routing: Compositor routes, router filters

Clean separation:

1. **Compositor** determines the routing target
   - Keyboard → focused pane (click-to-focus)
   - Pointer → pane under cursor (geometry-based)
2. **pane-router** applies security policy and filter chain
3. Filtered events delivered as Protocol messages

The input filter chain (BInputServerFilter heir) runs at
the compositor level before per-pane routing.

## Workspaces

Heritage: app_server Desktop/Workspace model. rio had none.

Workspaces are compositor state: which panes are on which
workspace, which workspace is active. Observable via pane-fs
(`/pane/comp/workspaces/`). Switchable via wctl or typed
protocol.

## Recursive composition (nesting)

Works by construction. The Dev trait's uniformity means a
virtual device satisfies the same bounds as a real device.
An inner pane-compositor consumes an outer compositor's
virtual devices — same code path. One buffer copy per
nesting level.

This is rio's most elegant property, preserved. A
pane-compositor running inside another pane-compositor
is transparent — no special case, no kernel support.

## The compositor as a pane

**A pane with special capabilities but no special privilege.**
Consistent with `host_as_contingent_server`. The compositor
is just a pane that happens to hold the real display and
input devices and provides virtual ones to other panes.
Another compositor instance on a remote machine is
architecturally identical — it holds different real devices
but provides the same virtual device interface.

## What's IN vs OUT

| IN (pane-compositor) | OUT (other crate) |
|---|---|
| Buffer compositing | Buffer rendering (pane application) |
| Virtual device multiplexing | Real device abstraction (pane-kernel) |
| Window semantics (focus, stacking, geometry) | Window decoration (future pane-toolkit, CSD) |
| Minimal chrome (bar + tab) | Rich UI widgets (pane-toolkit) |
| Command launcher (tag hierarchy) | Tag definition (pane application) |
| Input spatial routing | Input policy filtering (pane-router) |
| Workspace management | — |
| Focus indicator (thin border) | Title bars, buttons (CSD via toolkit) |

## Provenance

Design established 2026-04-12 via two-agent targeted round
(plan9-systems-engineer on rio architecture,
be-systems-engineer on app_server architecture). Lane
corrected the initial SSD recommendation to CSD (consistent
with buffer-only model and Wayland ecosystem). Tab/launcher
spec from Lane: Be's window tab doubles as a command palette
populated by the pane's tag hierarchy, composable with fzf
and unix tools.

## See also

- `architecture/kernel` — pane-kernel Dev trait, DeviceRegistry
- `architecture/router` — input filter chain, security ACLs
- `architecture/fs` — pane-fs namespace, compositor state projection
- `architecture/proto` — Protocol, Handles, Message
- `decision/host_as_contingent_server` — compositor has no privilege
- `decision/kernel_naming` — exokernel framing
- `reference/smithay` — smithay viability for Wayland backend
