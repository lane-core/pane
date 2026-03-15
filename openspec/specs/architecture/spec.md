# Pane — Architecture Specification

## Vision

Pane is a Wayland compositor and desktop environment for Linux. It is the foundation for a complete desktop distribution.

Pane combines BeOS's integrated feel with Plan 9's text-as-interface philosophy and modern tiling window manager ideas. The design philosophy extends unix/plan9: powerfully expressive abstractions that are modular and sequential, composing to achieve an integrated user experience. No single component implements "the desktop" — the experience emerges from composition of small, focused servers.

## The Pane Primitive

The **pane** is the universal UI object. Everything — shells, editors, file managers, status widgets, legacy applications — lives in a pane. Every pane shares:

- A **tag line**: editable text that serves as title, command bar, and menu simultaneously (inspired by acme). No toolbars, no menus, no button widgets. Text is the interface.
- A **body**: the content area. May be a cell grid (native panes), a Wayland surface (legacy clients), or a hybrid.
- A **protocol connection**: communication with the compositor over typed messages.

## Design Pillars

### 1. Text as Action

Any visible text is potentially executable. Middle-click (B2) runs it as a command. Right-click (B3) plumbs it — sends it to the plumber for pattern-matched routing to the appropriate handler. Click `Makefile:42` anywhere in the system and it opens in the editor at line 42. This collapses toolbars, menus, hyperlinks, and file associations into one mechanism: clickable text and pattern matching.

### 2. Cell Grid as Native Rendering

The compositor owns a GPU-accelerated text renderer. Pane-native clients send cell content (character, foreground, background, attributes) and the compositor rasterizes. This produces:

- Consistent fonts and styling across all panes
- Terminal-derived widgets as first-class citizens, not trapped inside a terminal emulator
- The same rendering model for shells, editors, file managers, and status widgets

Surface compositing is available for inline images and legacy Wayland clients, but the cell grid is the default content model.

### 3. Modular Composition

The system decomposes into small servers (separate processes) and thin client kits (Rust crate libraries). Each server does exactly one thing. Integrated behavior emerges from sequential composition of servers, not from any single server knowing about everything.

This is the unix/plan9 principle applied at the desktop level: servers are filters/services with clean interfaces, and the user experience is an emergent property of their composition.

### 4. Typed Protocol with Correctness Guarantees

All server/client communication uses algebraic message types (Rust enums) serialized with postcard. The protocol has a formal state machine. Property-based testing verifies round-trip serialization, state machine invariants, and exhaustive message handling. The compiler enforces that every message variant is handled.

### 5. Declarative State

Configuration is data, not code. Settings are pane-proto message types serialized to files — the same types used for IPC are used for persistence. No separate config format, no config file parsers, no scripting languages for configuration. Typed values you can read, write, and watch.

### 6. Compositional Interfaces

Kits use Rust's native monadic idioms (`Result`/`?`, `Option` combinators, iterator chains) as the primary composition mechanism. Where domain types have a success/failure or some/none shape, they provide derived combinator APIs (`map`, `and_then`, `unwrap_or`) rather than requiring manual matching. Protocol operation sequences compose via a combinator builder in pane-app. Observable state composes via reactive signals in state-oriented kits. Monadic patterns are not forced onto imperative operations — cell grid writes, calloop event dispatch, and buffer mutation remain direct. The test: if combinator chaining reads more clearly than sequential statements, use it; if it doesn't, don't.

## Servers

Each server is a separate process that does exactly one thing. Servers communicate via the pane message protocol over unix sockets. Servers are built on calloop event loops (Looper pattern).

### pane-comp — Compositor

Manages rectangles, renders cells and surfaces. Smithay-based Wayland compositor.

Responsibilities:
- Wayland protocol handling (xdg-shell, layer-shell, xwayland)
- Layout tree: tree-based tiling (recursive splits) with tag-based visibility (dwm-style bitmask)
- Cell grid rendering: GPU-accelerated text rasterization, glyph atlas
- Surface compositing: DMA-BUF/shared memory for legacy Wayland clients
- Pane protocol server: accepts pane-native client connections
- Tag line rendering: draws tag lines for all panes
- Input dispatch: routes keyboard/mouse events to the focused pane
- Chrome rendering: borders, split lines, focus indicators

Does NOT contain: layout algorithms (these are properties of the tree structure), plumbing logic, app launch logic, file type recognition.

### pane-plumb — Plumber

Matches patterns on messages, routes to ports. Inspired by Plan 9's plumber.

Responsibilities:
- Maintains named ports (edit, web, image, etc.)
- Receives plumb messages (text fragment + source + working directory + attributes)
- Matches messages against an ordered rule set
- Routes matched messages to the appropriate port
- Applications listen on ports to receive routed messages

Rules are declarative typed structures, not a DSL. They are serialized pane-proto types stored as files.

Does NOT contain: type recognition logic (that's upstream — pane-store identifies types, attaches as attributes, plumber rules can match on those attributes).

### pane-input — Input Server

Translates hardware events to input messages.

Responsibilities:
- Input device discovery and management (via libinput)
- Keyboard layout / keymap handling (via xkbcommon)
- Key binding resolution
- Pointer acceleration and configuration
- Input method (IM) framework integration

Does NOT contain: focus management (that's pane-comp), command execution (key bindings produce messages, receivers act on them).

### pane-roster — Roster

Tracks running pane clients and their identities.

Responsibilities:
- Maintains a list of connected pane clients with their app signatures
- Emits events when clients connect/disconnect
- Answers queries: "is app X running?", "list all running apps", "which app has focus?"
- Single-launch enforcement: if a client declares single-launch, roster knows if it's already running

Does NOT contain: app launch logic (launching is: query roster, exec binary if absent, roster observes the new connection — a sequence of operations, not a roster feature).

### pane-store — Attribute Store

Indexes file attributes, emits change notifications.

Responsibilities:
- Reads and writes extended attributes on files (xattr on Linux/macOS)
- Maintains an index over attribute values for fast queries
- Emits change notifications when watched files/attributes change (node monitoring)
- Provides a query interface over the index

Does NOT contain: live query maintenance (that's a client-side composition of index reads + change notification subscriptions), settings management (settings are just serialized pane-proto types — any client can persist its own), file type recognition as a built-in (type recognition is a client of pane-store that sets type attributes based on sniffing rules).

## Composition Examples

Integrated behavior emerges from sequential composition of servers:

**"Open this file":**
1. Type recognizer (a client of pane-store) identifies file type, sets type attribute
2. pane-plumb matches type attribute against rules
3. pane-roster checks if the handler app is running
4. If not, handler is started (exec)
5. Handler receives the plumb message, opens the file

**"Right-click selected text":**
1. pane-comp sends selected text as a plumb message to pane-plumb
2. pane-plumb matches text against rules (filename:line pattern, URL pattern, etc.)
3. Matched message is routed to the appropriate port
4. Listening application receives and acts on it

**"Live query (all .rs files modified today)":**
1. Client reads pane-store index with query predicate
2. Client subscribes to pane-store change notifications for matching paths
3. Client maintains the result set, updating as notifications arrive
4. This is a client-side composition — no "live query" feature in pane-store

**"Persist and restore session":**
1. pane-comp serializes its layout tree (pane-proto types → postcard → file)
2. On restart, pane-comp deserializes and reconstructs the tree
3. pane-roster re-launches apps that were running (from serialized roster state)
4. Apps restore their own state from their own serialized settings

## Kits

Kits are Rust crate libraries that provide ergonomic access to server protocols. They are thin wrappers — the protocol is the real API. You can always bypass the kit and speak protocol directly.

### pane-proto (foundation)
Wire types (message enums), serde derivations, protocol state machine, validation. Every other crate depends on this. No runtime dependencies — pure types and serialization.

### pane-app (application lifecycle)
Looper/Handler actor model on calloop. Pane lifecycle management. Convenience for connecting to servers and dispatching events. The `Handle<M>` type for typed actor references.

### pane-ui (interface)
Cell grid writing helpers. Tag line management. Styling primitives (colors, attributes). Coordinate systems and scrolling.

### pane-text (text manipulation)
Text buffer data structures. Structural regular expressions (sam-style x/pattern/command). Editing operations (insert, delete, transform). Address expressions.

### pane-store-client (store access)
Client library for pane-store. Attribute read/write. Query building. Change notification subscription.

## Compositional Layers

Kit APIs compose through three layers, each mapped to a crate boundary:

**Layer 1 — Result-like domain types (all kits, when applicable).** Custom enums with success/failure or some/none shape provide derived combinator APIs (`map`, `and_then`, `unwrap_or`, `ok_or`). Standard `Result` and `Option` remain the default — derived combinators are for domain types that parallel their shape but carry different semantics (e.g., plumber match results, store query outcomes). Applicable kits: pane-plumb, pane-store-client, pane-app.

**Layer 2 — Protocol combinators (pane-app).** A builder API for composing protocol operation sequences as testable values. Operations chain via `and_then` (bind) and `map`. The combinator type wraps `ProtocolState → Result<(A, ProtocolState), ProtocolError>` — the state monad with error handling. The executor runs sequences against a real connection; tests run them against in-memory state. This lives in pane-app, not pane-proto, because it requires runtime context.

**Layer 3 — Reactive signals (pane-app, pane-store-client).** Signals for observable state with `map`, `combine`, `contramap`. Change notifications from pane-store become signals. Live queries are compositions of query results and notification streams. UI state (focus, dirty, tag content) can be signals that views react to. This replaces manual callback registration with declarative dataflow.

## Pane Protocol

Communication between pane-native clients and pane-comp uses four logical channels over a single unix socket connection:

| Channel | Client → Compositor | Compositor → Client |
|---------|---------------------|---------------------|
| **body** | write cells (position, character, attributes) | — |
| **tag** | set tag text | tag text executed (B2 click), tag text plumbed (B3 click) |
| **event** | — | key, mouse, resize, focus, plumb message delivery |
| **ctl** | set name, set dirty/clean, request geometry | close requested, hide, show |

Messages are Rust enums serialized with postcard. The protocol has a state machine:

```
Disconnected → Connected → PaneCreated → Active ⇄ { Writing, Receiving }
                                                  → Closed
```

Invalid state transitions are type errors (compile-time) or protocol violations (rejected at runtime with error messages).

## Client Classes

### Pane-native clients
Speak the pane protocol. Get full integration: tag line, cell grid body, plumbing, event streams, compositor-rendered chrome. Examples: shell (PTY bridge), editor, file manager, status widgets.

### Legacy Wayland clients
Speak standard xdg-shell (or xwayland for X11 apps). Get a pane wrapper: the compositor provides a tag line and borders, but the body is an opaque surface rendered by the client. Full desktop functionality (Firefox, Inkscape, etc.) works — just without plumbing or cell grid integration.

## Layout

Tree-based tiling with tag-based visibility:

- The layout is a tree of containers. Leaf nodes hold panes. Branch nodes define splits (horizontal or vertical).
- Each pane has a tag bitmask. The compositor displays panes matching the currently selected tags. A pane can appear in multiple tag sets. Multiple tags can be viewed simultaneously (bitwise OR).
- Tiling splits are explicit visible lines on screen. The structure is always visible.
- Floating panes (scratchpad) are supported as a separate layer.

## Aesthetic

90s-inspired, visible structure:

- **Monospace as design language**: tags, bodies, status — all monospace, all the same font family. Not a limitation — a deliberate choice for consistency and information density.
- **Beveled borders and visible chrome**: panes have real borders. You can see the structure. Not borderless windows in void.
- **Color as information**: dirty state, focus, errors. Not decoration. No gradients, no shadows, no transparency.
- **Chunky, readable text**: clear mouse affordances. Text you can point at and click.

## Technology

- **Language:** Rust
- **Compositor library:** smithay
- **Event loop:** calloop (unified event loop for Wayland events, protocol messages, timers, IPC)
- **Wire format:** postcard (serde-based, varint-encoded, compact)
- **Testing:** property-based (proptest) for protocol correctness, integration tests for server composition
- **Actor model:** Looper/Handler on calloop — each server/connection is a Looper with a typed message queue, Handle<M> for typed actor references
- **Compositional interfaces (candidates, not commitments):**
  - Layer 1 (result-like domain types): `result-like` crate — derive macros for Option/Result-style combinators on custom enums
  - Layer 3 (reactive signals): `agility` crate — FRP signals with map/combine/contramap
  - Specific crate choices deferred to when consuming code is built

## Build Sequence

Each phase produces a testable, usable artifact:

1. **pane-proto** — message types, serde, state machine, property tests
2. **pane-comp skeleton** — smithay compositor, single hardcoded pane, tag line + cell grid rendering
3. **pane-shell** — PTY bridge client, first usable terminal
4. **Layout tree** — tiling with splits, multiple panes, tag-based visibility
5. **pane-plumb** — plumber daemon, pattern matching, port routing
6. **pane-input** — input server, key binding, device management
7. **pane-roster** — app tracking, signatures, single-launch
8. **pane-store** — attribute indexing, change notifications, queries
9. **Legacy Wayland/XWayland** — xdg-shell and xwayland support
