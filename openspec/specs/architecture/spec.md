# Pane — Architecture Specification

## Vision

Pane is a Wayland compositor and desktop environment for Linux. It is the foundation for a complete desktop distribution.

Pane combines BeOS's integrated feel with Plan 9's text-as-interface philosophy and modern tiling window manager ideas. The design philosophy extends unix/plan9: powerfully expressive abstractions that are modular and sequential, composing to achieve an integrated user experience. No single component implements "the desktop" — the experience emerges from composition of small, focused servers.

## The Pane Primitive

The **pane** is the universal UI object. Everything — shells, editors, file managers, status widgets, legacy applications — lives in a pane. Every pane shares:

- A **tag line**: editable text that serves as title, command bar, and menu simultaneously (inspired by acme). No toolbars, no menus, no button widgets. Text is the interface.
- A **body**: the content area. May be a cell grid (native panes), a Wayland surface (legacy clients), or a hybrid.
- A **protocol connection**: communication with the compositor over typed messages.

## Target Platform

Pane targets Linux exclusively, tracking the latest stable kernel release. The system leverages Linux-specific capabilities: mount namespaces, user namespaces, fanotify, inotify, xattrs, memfd, pidfd, and seccomp.

**Init system:** Infrastructure servers are managed by a supervision-tree init system. The design is init-system-agnostic by default — s6, runit, and systemd all qualify as supervisors. pane-roster acts as the service directory (infrastructure servers register with it) and as the process supervisor for desktop applications. If a specific feature requires supervision semantics that only one init system provides, the design picks that init system rather than contorting the feature to be agnostic.

**Filesystem:** The target filesystem must support the `user.*` xattr namespace. ext4, btrfs, XFS, and bcachefs all qualify. Advanced filesystem features (snapshots, subvolumes, CoW) are available through an abstraction layer when the filesystem provides them, and degrade gracefully on filesystems that lack them.

## Design Pillars

### 1. Text as Action

Any visible text is potentially executable. Middle-click (B2) runs it as a command. Right-click (B3) routes it — sends it to the router for pattern-matched routing to the appropriate handler. Click `Makefile:42` anywhere in the system and it opens in the editor at line 42. This collapses toolbars, menus, hyperlinks, and file associations into one mechanism: clickable text and pattern matching.

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

All communication uses algebraic message types (Rust enums) wrapped in `PaneMessage<T>` — a typed core plus an open-ended attributes bag. The core provides compile-time exhaustiveness checking. The attrs bag provides BMessage-style extensibility. The protocol has a formal state machine. Property-based testing verifies round-trip serialization, state machine invariants, and exhaustive message handling.

### 5. Compositional Interfaces

Kits use Rust's native monadic idioms (`Result`/`?`, `Option` combinators, iterator chains) as the primary composition mechanism. Domain types with success/failure shape provide derived combinator APIs (`map`, `and_then`, `unwrap_or`) rather than requiring manual matching. Protocol operation sequences compose via a combinator builder in pane-app. Observable state composes via reactive signals in state-oriented kits.

**Value/Compute polarity.** Protocol types carry polarity, grounded in sequent calculus (Curien & Herbelin's λμμ̃) and Call-by-Push-Value (Levy). Value types are constructed by the sender and inspected by the receiver — requests, cell data, route messages, attr values. Compute types are defined by their behavior when observed — event handlers, protocol continuations. The `Proto<A>` combinator enforces valid composition: Value-after-Value, Value-after-Compute, and Compute-after-Compute compose freely. Compute-after-Value requires explicit synchronization. Event handling has dual representations: the `PaneEvent` enum (Value — for exhaustive matching) and a `PaneHandler` builder (Compute — for declarative dispatch).

The fundamental operation is the **cut**: ⟨request | handler⟩. Protocol dispatch is a cut. Making cuts explicit enables pure-function testing of dispatch without I/O.

Monadic patterns are not forced onto imperative operations — cell grid writes, calloop event dispatch, and buffer mutation remain direct. The test: if combinator chaining reads more clearly than sequential statements, use it; if it doesn't, don't.

### 6. Semantic Interfaces

Every interface a pane exposes — filesystem, tag line, protocol messages — SHALL present the abstraction level semantically relevant to its consumer. The same object may be viewed at different levels by different consumers:

- A **human user** sees the semantic level: commands, files, directories, operations.
- A **pane application** sees a system-service level: state, exit codes, environment, capabilities.
- The **compositor** sees the rendering level: cells, regions, surfaces — because rendering IS its semantics.
- A **debugger or admin tool** sees the implementation level: byte streams, buffer state, protocol traces — because introspection IS its purpose.

The abstraction level isn't fixed — it's determined by who's looking and what they need. This operates over a permission gradient from system to user. Implementation details aren't hidden — they're available at the appropriate interface depth for consumers who need them. The principle is: match the interface to the consumer's purpose.

### 7. Filesystem as Interface

State and configuration are filesystem primitives — file content for values, xattrs for metadata, directories for structure. Plugin discovery is via well-known directories watched by pane-notify. The FUSE interface at `/srv/pane/` exposes server state for scripting and debugging. The filesystem is the database, the registry, and the configuration format.

**Caching invariant:** Servers cache filesystem state in memory at startup and update only in response to pane-notify events. The render loop and event dispatch never perform filesystem I/O.

### 8. Composable Extension

The system is extended through the same interfaces it uses internally. Routing rules, translators, input methods, and pane modes are all plugins — files in well-known directories, watched by pane-notify, composing through typed interfaces (pane protocol, attrs bag, filesystem).

A pane mode wraps a pane client library (e.g., pane-shell-lib) with domain-specific semantics: a custom tag line, custom filesystem endpoints, custom routing patterns. The terminal emulation is reused; the semantic layer is new. This produces an ecosystem where a "git pane," "mail pane," or "database pane" are thin layers over shared infrastructure — like emacs modes, but with static types, OS-level composition, and no language runtime.

Plugins compose safely because they operate on the public interface surface, not on internal state. The extension surface is the same surface the system itself uses. Adding a plugin is dropping a file in a directory. Removing it is deleting the file.

## Servers

Each server is a separate process that does exactly one thing. Servers communicate via the inter-server protocol (`PaneMessage<ServerVerb>`) over unix sockets. Infrastructure servers are managed by the init system (s6/runit) and register with pane-roster as a service directory on startup. Servers are built on calloop event loops (Looper pattern).

### pane-comp — Compositor

Manages rectangles, renders cells and surfaces. Smithay-based Wayland compositor.

Responsibilities:
- Wayland protocol handling (xdg-shell, layer-shell, xwayland)
- Layout tree: tree-based tiling (recursive splits) with tag-based visibility (dwm-style bitmask)
- Cell grid rendering: GPU-accelerated text rasterization, glyph atlas
- Surface compositing: DMA-BUF/shared memory for legacy Wayland clients
- Pane protocol server: accepts pane-native client connections (multiple panes per connection)
- Tag line rendering: draws tag lines for all panes
- Input handling: libinput integration, xkbcommon keyboard layout, key binding resolution, pointer acceleration (in-process, not a separate server — latency-critical)
- Input dispatch: routes keyboard/mouse events to the focused pane
- Chrome rendering: borders, split lines, focus indicators

Does NOT contain: routing logic, app launch logic, file type recognition. For native panes, B3-click sends a `TagRoute` event to the pane client; the client (via pane-app kit) sends the route message to pane-route. For legacy Wayland panes, pane-comp connects to pane-route directly as a fallback.

### pane-route — Router

Matches patterns on messages, routes to ports. Inspired by Plan 9's plumber.

Responsibilities:
- Maintains named ports (edit, web, image, etc.)
- Receives route messages (text fragment + source + working directory + attributes)
- Matches messages against an ordered rule set
- Routes matched messages to the appropriate port
- Applications listen on ports to receive routed messages
- Queries pane-roster's service registry for additional matching operations
- When multiple handlers match: spawns a transient floating pane (scratchpad) listing options as B2-clickable text. Single match auto-dispatches. Routing rules take priority over registered services.

Routing rules are files in well-known directories (`/etc/pane/route/rules/`, `~/.config/pane/route/rules/`), one file per rule. pane-notify watches these directories for live addition/removal.

Does NOT contain: type recognition logic (that's upstream — pane-store identifies types, attaches as attributes, routing rules match on those attributes).

### pane-roster — Roster

Service directory for infrastructure servers, process supervisor for desktop applications, and service registry for discoverable operations.

**Service directory** (for infrastructure servers):
- Infrastructure servers (pane-comp, pane-route, pane-store, pane-fs) register on startup
- Roster records identity and capabilities without assuming supervision responsibility
- Answers queries: "where is the router?", "is the store running?"
- If an infrastructure server crashes, the init system restarts it; the server re-registers with roster

**Process supervisor** (for desktop applications):
- Launches desktop apps (shells, editors, user programs) directly
- Monitors running apps, restarts on crash (distinguishes crash from clean exit via pane protocol)
- Session save/restore: serializes running app state, restores on login

**Service registry** (for discoverable operations):
- Apps register `(content_type_pattern, operation_name, description)` tuples
- Router queries the registry for multi-match scenarios
- Answers: "what operations are available for this content type?"

Does NOT contain: supervision of infrastructure servers (that's the init system).

### pane-store — Attribute Store

Indexes file attributes, emits change notifications.

Responsibilities:
- Reads and writes extended attributes on files (`user.pane.*` xattr namespace on Linux)
- Maintains an in-memory index over attribute values for fast queries (rebuilt from xattr scan on startup, like BFS)
- Uses pane-notify for filesystem change detection (fanotify for mount-wide xattr changes, inotify for targeted watches)
- Emits change notifications when watched files/attributes change
- Provides a query interface over the index

Does NOT contain: live query maintenance (that's a client-side composition of index reads + change notification subscriptions), file type recognition as a built-in (type recognition is a client of pane-store that sets type attributes based on sniffing rules).

### pane-fs — Filesystem Interface

Exposes compositor, router, and configuration state as a FUSE filesystem at `/srv/pane/`.

Responsibilities:
- Mounts FUSE filesystem at `/srv/pane/`
- Speaks pane socket protocol to other servers (it's just another client)
- Format per endpoint: plain text for text data (tag, body), JSON for structured data (cells, attrs, index), line commands for control files (ctl), JSONL for event streams (event, route ports)
- Exposes configuration at `/srv/pane/config/` mirroring `/etc/pane/`

```
/srv/pane/
  index              # JSON: list of panes
  new                # write to create a pane
  1/
    ctl              # line commands write, state read
    tag              # plain text
    body             # plain text read
    cells            # JSON: full cell grid with colors/attrs
    attrs            # JSON: attrs bag
    event            # JSONL stream
  route/
    send             # JSON write (or plain text shorthand)
    edit             # JSONL stream read
    web              # JSONL stream read
  config/
    comp/
      font           # read/write config values
      font-size
      ...
```

Does NOT contain: any server logic — pane-fs is a translation layer between FUSE operations and the socket protocol.

## Shared Infrastructure

### pane-notify — Filesystem Notification

An internal crate (not a standalone server) that abstracts over Linux filesystem notification interfaces.

- **fanotify** with `FAN_MARK_FILESYSTEM` for mount-wide watches (pane-store bulk xattr tracking)
- **inotify** for targeted watches (specific directories, config files, plugin directories)
- Consumers request watches by scope; pane-notify picks the right kernel interface
- Unified event stream integrating into calloop as an event source

### Filesystem-Based Configuration

Server configuration is stored as files in well-known directories under `/etc/pane/<server>/`. Each config key is a separate file. File content is the value. xattrs carry metadata: `user.pane.type` (string, int, float, bool), `user.pane.description`, optionally `user.pane.range` and `user.pane.options`.

Servers watch their config directories via pane-notify. Config changes take effect without server restart, without SIGHUP, without manual reload commands. All available config keys are discoverable by listing the directory.

### Filesystem-Based Plugin Discovery

Servers that support extensibility discover plugins by scanning well-known directories:
- `~/.config/pane/translators/` — content translators (type sniffing, format conversion)
- `~/.config/pane/input/` — input method add-ons (IME, connected via Wayland IME protocols)
- `~/.config/pane/route/rules/` — routing rules (one file per rule)

System-wide equivalents exist under `/etc/pane/` with user directories taking precedence. pane-notify watches these directories for live addition/removal. Plugin metadata is carried in xattrs: `user.pane.plugin.type`, `user.pane.plugin.handles`, `user.pane.plugin.description`.

## Composition Examples

Integrated behavior emerges from sequential composition of servers:

**"Open this file":**
1. Type recognizer (a client of pane-store) identifies file type, sets type attribute
2. pane-route matches type attribute against rules
3. pane-roster checks if the handler app is running
4. If not, handler is started (pane-roster launches it)
5. Handler receives the route message, opens the file

**"Right-click selected text" (native pane):**
1. pane-comp sends a `TagRoute` event to the pane client
2. The pane-app kit sends the selected text as a route message to pane-route
3. pane-route matches text against rules and queries roster for service matches
4. If single match: auto-dispatches to the handler
5. If multiple matches: spawns a transient scratchpad pane listing options
6. User B2-clicks an option; it dispatches

**"Live query (all .rs files modified today)":**
1. Client reads pane-store index with query predicate
2. Client subscribes to pane-store change notifications for matching paths
3. Client maintains the result set, updating as notifications arrive
4. This is a client-side composition — no "live query" feature in pane-store

**"Change the compositor font":**
1. User runs `echo "JetBrains Mono" > /etc/pane/comp/font`
2. pane-notify (inotify watch on `/etc/pane/comp/`) fires
3. pane-comp re-reads the font config file into memory
4. pane-comp re-rasterizes the glyph atlas and re-renders on the next frame

**"Persist and restore session":**
1. pane-comp serializes its layout tree (pane-proto types → postcard → file)
2. On restart, pane-comp deserializes and reconstructs the tree
3. pane-roster re-launches apps that were running (from serialized roster state)
4. Apps restore their own state from their own serialized settings

## Kits

Kits are Rust crate libraries that provide ergonomic access to server protocols. They are thin wrappers — the protocol is the real API. You can always bypass the kit and speak protocol directly.

### pane-proto (foundation)
Wire types (message enums), PaneMessage wrapper with attrs bag, protocol state machine, Value/Compute polarity markers, inter-server protocol types (ServerVerb + typed views), serde derivations, validation. Every other crate depends on this. No runtime dependencies — pure types and serialization.

### pane-app (application lifecycle)
Looper/Handler actor model on calloop. Pane lifecycle management. `Proto<A>` combinator for composable protocol sequences with Value/Compute polarity. `PaneHandler` builder for codata-style event dispatch. Convenience for connecting to servers and dispatching events. The `Handle<M>` type for typed actor references. Automatic connection to pane-route for B3-click handling.

### pane-ui (interface)
Cell grid writing helpers. Tag line management. Styling primitives (colors, attributes). Coordinate systems and scrolling.

### pane-text (text manipulation)
Text buffer data structures. Structural regular expressions (sam-style x/pattern/command). Editing operations (insert, delete, transform). Address expressions.

### pane-store-client (store access)
Client library for pane-store. Attribute read/write. Query building. Change notification subscription. Reactive signal composition for live queries.

### pane-notify (filesystem notification)
Abstraction over fanotify and inotify. Calloop event source. Used by pane-store, pane-comp (config), and any server that watches filesystem state.

## Compositional Layers

Kit APIs compose through three layers, each mapped to a crate boundary:

**Layer 1 — Result-like domain types (all kits, when applicable).** Custom enums with success/failure or some/none shape provide derived combinator APIs (`map`, `and_then`, `unwrap_or`, `ok_or`). Standard `Result` and `Option` remain the default — derived combinators are for domain types that parallel their shape but carry different semantics. Candidate implementation: `result-like` crate. Decision deferred to when consuming types exist.

**Layer 2 — Protocol combinators (pane-app).** A builder API for composing protocol operation sequences as testable values. Operations chain via `and_then` (bind) and `map`. The combinator type wraps `ProtocolState → Result<(A, ProtocolState), ProtocolError>`. The executor runs sequences against a real connection; tests run them against in-memory state. Polarity-aware: Value operations (produce a result) and Compute operations (fire behavior) compose according to the duploid's three-fourths associativity rule.

**Layer 3 — Reactive signals (pane-app, pane-store-client).** Signals for observable state with `map`, `combine`, `contramap`. Change notifications from pane-store become signals. Live queries are compositions of query results and notification streams. UI state (focus, dirty, tag content) can be signals that views react to. Candidate implementation: `agility` crate. Decision deferred to when consuming code is built.

## Pane Protocol

### Client ↔ Compositor

Communication between pane-native clients and pane-comp uses four logical channels over a single unix socket connection. A single connection can own multiple panes.

| Channel | Client → Compositor | Compositor → Client |
|---------|---------------------|---------------------|
| **body** | write cells (CellGrid) or widget tree (Widget) | — |
| **tag** | set tag line (structured TagLine) | tag action executed (B2/left-click), tag text routed (B3) |
| **event** | — | key, mouse, resize, focus, route message, widget events |
| **ctl** | set name, set dirty/clean, request geometry | close requested, hide, show |

All messages are `PaneMessage<PaneRequest>` or `PaneMessage<PaneEvent>` — typed core wrapped with an open attrs bag. Serialized with postcard.

The protocol state machine:

```
Disconnected → Active { panes: HashMap<PaneId, PaneKind>, pending_creates: u32 }
```

Create increments `pending_creates`. `activate(id, kind)` decrements it and inserts into the pane map. Close removes from the pane map. Operations are validated against the pane map and pane kind (WriteCells rejected for Surface panes). `ProtocolState` is local tracking — not serialized, not sent on the wire.

### Inter-Server

Servers communicate via `PaneMessage<ServerVerb>`:

```rust
enum ServerVerb { Query, Notify, Command }
```

The verb is the typed core. The attrs bag carries the payload. Type safety is recovered at the kit layer via typed view/builder patterns: each server defines structs that parse/build attrs with compile-time field validation (e.g., `RouteCommand::build().data("parse.c:42").wdir("/src").into_message()`).

This is the BMessage model: one envelope, universal routing. Any server can forward, log, or inspect any message without understanding its semantics. The wire format is universal; the access layer is typed.

## Client Classes

### Pane-native clients
Speak the pane protocol. Get full integration: tag line, cell grid body, routing, event streams, compositor-rendered chrome. Examples: shell (PTY bridge), editor, file manager, status widgets.

### Legacy Wayland clients
Speak standard xdg-shell (or xwayland for X11 apps). Get a pane wrapper: the compositor provides a tag line and borders, but the body is an opaque surface rendered by the client. Full desktop functionality (Firefox, Inkscape, etc.) works — just without routing or cell grid integration.

## pane-shell Architectural Constraints

pane-shell (the PTY bridge client) is the most important pane client — it makes the system a daily driver.

- **Terminal emulation level:** xterm-256color. Covers cursor movement, scroll regions, alternate screen buffer, 256-color and RGB color, mouse reporting, bracketed paste. `$TERM=xterm-256color`.
- **Screen buffer model:** pane-shell maintains a full screen buffer internally. VT sequences update this buffer. On each change, pane-shell sends dirty regions as CellRegion writes — not the entire screen.
- **Alternate screen:** Enter (`\e[?1049h`) swaps to a second buffer. Exit (`\e[?1049l`) restores the original. Both are internal to pane-shell. The compositor just receives cell writes.
- The compositor does not know or care that it's rendering a terminal. pane-shell is just another client writing cells.

## Layout

Tree-based tiling with tag-based visibility:

- The layout is a tree of containers. Leaf nodes hold panes. Branch nodes define splits (horizontal or vertical).
- Each pane has a tag bitmask. The compositor displays panes matching the currently selected tags. A pane can appear in multiple tag sets. Multiple tags can be viewed simultaneously (bitwise OR).
- Tiling splits are explicit visible lines on screen. The structure is always visible.
- Floating panes (scratchpad) are supported as a separate layer. Transient floating panes are used for the router's multi-match chooser.

## Aesthetic

Frutiger Aero — the polished evolution of 90s desktop design. The design philosophy: what if Be Inc. survived into the 2000s and refined their visual design alongside the early Aqua era? BeOS's information density and integration, Mac OS X Aqua 1.0's rendering refinement and warmth, combined into a power-user desktop that is both beautiful and dense.

Reference points: BeOS R5 / Haiku (density, integration, matte bevels), Mac OS X 10.0–10.2 Aqua 1.0 (rendering quality, subtle translucency, warm palette), Frutiger Aero (the intersection: depth and warmth serving comprehension).

- **Depth through lighting**: subtle vertical gradients on controls (light top, darker bottom), 1px highlight/shadow edges. Matte and solid — not glossy Aqua gel, not flat Metro. Depth communicates hierarchy.
- **Beveled borders and visible chrome**: panes have real borders. Controls look like controls. Structure is always visible. Rounded corners (3-4px radius) — approachable without losing density.
- **Selective translucency**: floating elements (scratchpads, popups) are translucent to show context. Translucency where it's beautiful and aids comprehension, not universally.
- **Warm saturated palette**: warm grey base, saturated accent colors for focus/dirty/active states. The workspace feels well-lit — not a dark cave, not a white void.
- **Typography split**: proportional sans-serif for widget chrome (labels, buttons). Monospace for cell grid content and tag line text regions. Tag line stays monospace (it's executable text where column alignment matters).
- **Color as information**: dirty state, focus, errors. Not decoration.
- **Dense but refined**: closer to BeOS than Aqua in spacing. Smaller controls, tighter layout. Enough padding to be comfortable, not enough to waste space.
- **One opinionated look**: no theme engine, no theme selector. The aesthetic IS pane's identity. Individual properties configurable via filesystem-as-config (accent color, font size) but not wholesale theme replacement.

## Accessibility

The widget content model improves accessibility over cell-grid-only: widget panes have semantic structure (buttons, labels, lists) that screen readers can interpret. Cell grid panes remain a challenge. Addressing cell grid accessibility is a research problem for later phases.

## Technology

- **Language:** Rust
- **Compositor library:** smithay
- **Event loop:** calloop (unified event loop for Wayland events, protocol messages, timers, IPC)
- **Wire format:** postcard (serde-based, varint-encoded, compact)
- **Filesystem notification:** pane-notify (fanotify + inotify abstraction)
- **FUSE:** pane-fs at `/srv/pane/`
- **Init system:** supervision-tree init (s6, runit, or systemd — agnostic by default, opinionated when forced)
- **Testing:** property-based (proptest) for protocol correctness, integration tests for server composition
- **Actor model:** Looper/Handler on calloop — each server/connection is a Looper with a typed message queue, Handle<M> for typed actor references
- **Polarity:** Value/Compute marker traits (from sequent calculus / CBPV)
- **Optics:** `optics` crate for composed access paths into nested attrs (when complexity justifies it)
- **Widget layout:** taffy (flexbox/grid layout engine, pure computation)
- **Widget rendering:** femtovg (2D vector graphics on OpenGL via glow — rounded rects, gradients, text)
- **Compositional interfaces (candidates, not commitments):**
  - Layer 1 (result-like domain types): `result-like` crate
  - Layer 3 (reactive signals): `agility` crate — widget state bindings, store notifications
  - Specific crate choices deferred to when consuming code is built

## Build Sequence

Each phase produces a testable, usable artifact:

1. **pane-proto** — message types, PaneMessage wrapper, state machine, polarity markers, inter-server protocol, property tests ✓ (built, fixes in progress)
2. **pane-notify** — fanotify/inotify abstraction, calloop integration
3. **pane-comp skeleton** — smithay compositor, single hardcoded pane, tag line + cell grid rendering
4. **pane-shell** — PTY bridge client, first usable terminal
5. **Layout tree** — tiling with splits, multiple panes, tag-based visibility
6. **pane-route** — router daemon, pattern matching, port routing, service-aware multi-match
7. **pane-roster** — service directory, app supervision, service registry, session management
8. **pane-store** — attribute indexing, change notifications, queries, in-memory index
9. **Widget rendering** — femtovg integration, taffy layout, Frutiger Aero controls
10. **pane-fs** — FUSE at `/srv/pane/`, format-per-endpoint
11. **Legacy Wayland/XWayland** — xdg-shell and xwayland support
