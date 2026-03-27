# Pane Architecture Specification

This document describes what pane IS, technically — the engineering decisions that implement the principles in the foundations spec. It is written from the perspective of how the Be engineers would proceed if they were building BeOS's successor on Linux in 2026, knowing what we know now about what worked, what didn't, and what the theoretical work of the last two decades makes possible.

The foundations spec carries the philosophy. This document carries the engineering.

---

## 1. Vision

Pane is a desktop environment, Wayland compositor, and Linux distribution. One thing, not three. The degree of integration is NeXTSTEP-level: the user does not perceive a compositor running on a distro. They perceive pane.

The architecture recovers what made BeOS work — message-passing discipline, per-component threading, infrastructure-first design, the kit as programming model — on a platform where we don't control the kernel. Linux provides the primitives (processes, scheduling, filesystems, devices, networking) and stays out of the way. Pane provides the personality: the protocol, the compositor, the kits, the aesthetic, the extension model.

What session types add to the Be model: compile-time enforcement of the protocol discipline that BeOS achieved by engineering convention. The BMessage `what` code becomes a branch in a session type enum. The convention "send B_REPLY after B_REQUEST_COMPLETED" becomes a type: after `Recv<Request>`, the type is `Send<Reply>`. The guarantee is the same — protocol adherence, no stuck states, local reasoning per component — but the enforcement moves from developer skill to compiler verification.

What Linux adds to the Be model: the entire hardware ecosystem, a battle-tested network stack, compositing for free via Wayland, and 120,000+ packages via Nix. Haiku spent 25 years rebuilding what Be had, plus what Be never got to (package management, networking, POSIX compliance, layout management). Pane sidesteps all of that and focuses entirely on the desktop experience layer.

**Dependency philosophy.** Pane is a radically opinionated distribution that determines its own dependencies with complete freedom. Convention and legacy do not constrain our choices. We target the latest kernel interfaces, the newest viable subsystems, and the most forward-looking infrastructure when there are significant payoffs for our design model — provided we have reasonable confidence in their future support, or at minimum can maintain them ourselves if needed. FUSE-over-io_uring (Linux 6.14+), bcachefs when it matures, PipeWire over PulseAudio, s6 over systemd — these are not risky bets on bleeding edge. They are the choices of a distribution that is building for the next decade, not accommodating the last one. We are futureproofing, not backward-compatible.

---

## 2. The Pane Primitive

A pane is the universal object of the system. Every interface — to the user, to the system, to other components, to scripts — is a pane.

### What a pane is

A pane is one object with multiple views:

- **Visual**: tag line + body + chrome, rendered on screen
- **Protocol**: a session-typed endpoint for communication with the compositor and other components
- **Filesystem**: a node under `/srv/pane/` exposing state for scripts and tools
- **Semantic**: roles, values, and actions for accessibility infrastructure

These views are projections of the same internal state. When state changes through one view, others reflect it. The relationship between internal state and each view is governed by optics — composable, bidirectional access paths that satisfy GetPut and PutGet laws up to semantic equality. Violations under failure (a crashed component, a lagging view) are temporary; recovery semantics restore the invariant.

### The three parts of every pane

**Tag line.** Editable text that serves as title, command bar, and menu simultaneously. Inspired by acme's tag line — text IS the interface. The tag line is always compositor-rendered (the compositor owns the chrome). Tag line content travels through the pane protocol: the client sends tag content, the compositor renders it. Text in the tag line is actionable: execute (run as command) and route (kit evaluates routing rules for pattern-matched dispatch).

**Body.** The content area. For pane-native clients: text, widgets, or a hybrid. For legacy Wayland clients: an opaque wl_surface. The body is always client-rendered — the Wayland model — with visual consistency coming from the shared kits, not from the compositor rendering on behalf of clients.

**Chrome.** Tag lines, borders, focus indicators, split handles. Always compositor-rendered via smithay's GLES renderer (the compositor's own rendering engine — distinct from Vello, which is the client-side widget rendering engine). The chrome is pane's visual identity — one opinionated look, consistent across all panes regardless of whether their content is native or legacy.

### Pane composition

Panes compose spatially. Two panes viewed together form a compound structure whose concrete presentation depends on context: on screen, spatial arrangement in the layout tree; on the filesystem, sibling entries under `/srv/pane/`. The layout tree is the compositor's model of this composition — a tree of containers where branches define splits and leaves hold panes.

Panes compose temporally through the protocol. A conversation between a client and the compositor is a session. Multiple panes per connection are sub-sessions. The session type tracks where each conversation is in its lifecycle.

**Compositional equivalence.** The layout tree, pane-fs, and the pane protocol must encode composition relationships consistently. A split in the layout tree has a corresponding directory in pane-fs and emits structural events through the protocol. No composition primitive exists in one view without a representation in the others. Concretely: introducing a new composition mode (tabbed stacking, linked groups, transient overlays) requires filesystem and protocol representations before it ships. The test is automation-complete: for any composition relationship, a script must be able to discover that it exists, query its properties, and dissolve it through the standard protocol without special-case APIs.

### The pane as filesystem node

Each pane is exposed under `/srv/pane/<id>/` via pane-fs (FUSE). The filesystem representation presents the abstraction level relevant to the consumer:

- `tag` — the tag line content (read/write)
- `body` — the body content at the semantic level (for a shell: command output; for an editor: file content)
- `attrs/` — typed attributes (pane type, title, dirty state, working directory)
- `ctl` — control interface (write commands to manipulate the pane)

The specific tree structure evolves with implementation, but the principle is fixed: pane state is files, and any tool that can read files can inspect pane state.

**Composition in the filesystem.** When panes are composed, pane-fs reflects the composition structure as directory nesting. A split containing panes A and B appears as a directory under `/srv/pane/` with its own `attrs/` (encoding orientation, ratio, and split type) and child entries `A/` and `B/`. Independent panes are top-level entries; composed panes are nested under their container. The filesystem tree mirrors the layout tree's nesting — not as a consequence of the compositional equivalence invariant, but as the filesystem's native expression of it. Reparenting a pane (moving it into or out of a split) changes its position in the filesystem hierarchy. Tools that walk `/srv/pane/` see composition structure directly; they do not need to reconstruct it from per-pane geometry attributes.

---

## 3. Server Decomposition

Each server is a separate process. Servers communicate via session-typed protocols over unix sockets. Each server runs its own threaded looper — a thread with a message queue, processing messages sequentially. This is the BLooper model: each looper is an actor with a mailbox, processing one message at a time. Concurrency arises from many loopers running simultaneously, not from concurrency within a looper.

The compositor is the exception: its Wayland core uses calloop for fd polling because smithay requires it. calloop is scoped to the compositor — it does not define the system-wide concurrency model. Other servers use std::thread + channels.

### Why no router server

The original architecture included a central router server (pane-route). This has been eliminated. Here is why.

In BeOS, BMessenger carried messages directly between applications via kernel ports. There was no central message broker. Application A obtained a BMessenger for application B (via BRoster or direct handoff) and sent messages directly. This worked because the messaging infrastructure was in the kernel (ports) and the client-side library (libbe.so). No intermediary could fail.

A central router is a single point of failure for all communication. If it dies, messages stop flowing. The complexity of making it resilient — circuit breakers, priority queues, pre-allocated memory, heartbeat protocols, external watchdog — is real and necessary complexity, but it exists only because we created the single point of failure in the first place. Gassée warned against this: "people developing the system now have to contend with two programming models."

Routing is now a kit-level concern. The pane-app kit loads routing rules from the filesystem, evaluates them locally, queries pane-roster's service registry for multi-match scenarios, and dispatches directly to the handler. Sender to receiver, no intermediary. The kit is a library linked into every pane-native process — it cannot "crash" independently of the process that uses it. One communication model, not two.

What the router was supposed to provide:

| Router responsibility | Where it lives now |
|---|---|
| Rule matching and dispatch | pane-app kit (local evaluation) |
| Content transformation | pane-app kit (local transformation) |
| Multi-match resolution | pane-app kit queries pane-roster's service registry |
| Handler health monitoring | Each component monitors its own connections via session liveness |
| Dead letter handling | Kit logs unroutable messages; agents can monitor the log |
| System health monitoring | pane-watchdog (minimal external process) |
| Escalation procedures | pane-watchdog + init system |

The resilience research (circuit breakers, priority queues, Erlang heart patterns) remains valuable — it informs pane-watchdog's design and the kit's error handling. But the resilience is distributed into the right places rather than concentrated in a single server.

### pane-comp — The Compositor

Composites client surfaces, manages layout, renders chrome. This is the app_server equivalent — the rendering service that every client talks to.

**Responsibilities:**
- Wayland protocol handling via smithay — core: wl_shm, wl_seat, xdg-shell, linux-dmabuf, viewporter, fractional-scale, presentation-time; ext- protocols (cross-compositor, preferred over wlr-): ext-session-lock, ext-idle-notify, ext-image-copy-capture, ext-data-control, ext-color-management (HDR); compositor-specific: layer-shell, xdg-decoration, input method protocols
- Layout tree: recursive tiling with tag-based visibility (dwm-style bitmask)
- Surface compositing: composites all client buffers into the output framebuffer
- Pane protocol server: accepts pane-native client connections over a unix socket, multiple panes per connection
- Chrome rendering: tag lines, beveled borders, split handles, focus indicators
- Input handling: libinput integration, xkbcommon keyboard layout, key binding resolution (in-process — latency-critical)
- Input dispatch: routes input events to the focused pane via the appropriate protocol (pane protocol for native clients, wl_seat events for legacy clients)
- Frame timing: coordinates frame callbacks across all clients, submits composited output to DRM/KMS

**Does not contain:** routing logic, application launch logic, file type recognition, attribute indexing, or any server functionality beyond compositing and input. For native panes, a route action sends a TagRoute event to the pane client; the pane-app kit evaluates routing rules and dispatches. For legacy panes, the compositor handles route dispatch through its own kit integration.

**Threading model — three tiers, faithful to BeOS:** BeOS's app_server had three levels: Desktop (shared state coordinator), ServerApp (one thread per application), ServerWindow (one thread per window). Pane preserves this:

| BeOS | Pane | Role |
|---|---|---|
| Desktop | Compositor main thread (calloop) | Shared state, compositing, Wayland protocol, input dispatch |
| ServerApp | Dispatcher thread (1 per connection) | Demuxes incoming socket messages to per-pane threads |
| ServerWindow | Pane thread (1 per pane) | Handles session protocol for a single pane |

Each pane gets its own server-side thread — not one thread per connection. A slow pane cannot block its siblings. This is the BeOS guarantee: one ServerWindow thread per window, verified in the Haiku source (`ServerWindow` inherits `MessageLooper`, spawns a dedicated thread on `Run()`).

**Shared state and locking:** The compositor's layout tree is shared state accessed by per-pane threads (to read their position/size) and the main thread (to composite). This mirrors Haiku's `Desktop::fWindowLock` — a reader-writer lock where drawing commands take the read lock (concurrent across panes) and structural changes (move, resize, create/destroy) take the write lock. Per-pane threads process protocol messages and communicate with the main thread via channels; they never touch smithay objects directly (smithay is `!Send` by design, which correctly confines Wayland protocol handling to the main thread).

**Cost:** At 50 panes: 50 pane threads + dispatchers = ~1MB physical memory, ~1.5μs context switch overhead, negligible on 2026 hardware. Pierre Raynaud-Richard's measurements from Be Newsletter #4-46 (~20KB per thread) hold — modern Linux threads are comparable.

**The two-thread pattern:** Each pane has two threads — one client-side (the looper in pane-app) and one server-side (the pane thread in pane-comp). Messages flow from client to server asynchronously and are batched and flushed. Synchronous calls force a flush and round-trip. The default is async; sync only when a response is needed.

### pane-watchdog — System Health Monitor

A minimal external process, inspired by Erlang's `heart`. Deliberately simple — the less it does, the harder it is to kill.

**Responsibilities:**
- Heartbeat monitoring of critical infrastructure (compositor, roster) via direct pipes — not through the pane protocol
- Detecting unresponsive components via missed heartbeats (3 missed beats at 2-second intervals = 6-second worst-case detection)
- Triggering escalation on failure: flush journal to disk (pre-opened fd, direct write(2) — no buffered I/O, no path resolution, no allocation), broadcast persist-state alert
- Notifying the init system to restart failed components

**The external watchdog principle:** The thing being monitored cannot reliably monitor itself. Erlang solves this with heart — a separate C program with its own process, communicating through a pipe with a trivial protocol (length + opcode, 5 messages total). Pane-watchdog follows this pattern. It runs outside the pane server ecosystem. If every pane server dies, pane-watchdog is still running and can trigger recovery.

**What pane-watchdog does NOT do:** routing, message dispatch, application-level functionality, circuit breaker management. It checks pulses and pulls the emergency brake. Nothing else.

### pane-roster — The Application Directory

The component that makes the application ecology work. BeOS's BRoster + registrar, unified.

**Service directory** (for infrastructure servers):
- Infrastructure servers register on startup: identity, capabilities, communication endpoint
- Answers queries: "where is the store?", "is the compositor running?"
- Does NOT restart servers — the init system handles that. When a server crashes and restarts, it re-registers.

**Application lifecycle** (for desktop applications):
- Facilitates launching applications (launch semantics: single-launch apps deliver the launch message to the existing instance, matching BRoster's B_SINGLE_LAUNCH / B_EXCLUSIVE_LAUNCH / B_MULTIPLE_LAUNCH)
- Monitors running applications, distinguishes crash from clean exit
- Session save/restore: serializes running app state, restores on login

**Service registry** (for discoverable operations):
- Applications register `(content_type_pattern, operation_name, description, quality_rating)` tuples
- The pane-app kit queries the registry during routing for multi-match scenarios
- Quality-based selection when multiple handlers match: the Translation Kit pattern. Self-declared quality ratings enable automatic selection without central authority.

**Implementation:** pane-roster is a BServer-pattern looper: its own thread, its own message queue, session-typed conversations with every registered component. Process tracking uses pidfd for race-free liveness detection.

### pane-store — Attribute Store

BFS's attribute indexing and query engine, reimplemented in userspace over Linux xattrs.

**Responsibilities:**
- Reads and writes extended attributes on files (`user.pane.*` xattr namespace)
- Maintains an in-memory index over attribute values (rebuilt from xattr scan on startup)
- Uses fanotify with `FAN_MARK_FILESYSTEM` for mount-wide xattr change detection — one mark covers the entire filesystem, no recursive directory walking
- Emits change notifications when watched attributes change
- Provides a query interface over the index (predicate language modeled after BQuery)
- Supports live queries: a client that subscribes to change notifications and maintains a query result set gets automatic updates when files enter or leave the result set. This is client-side composition, not a server feature — same as BeOS.

**The BFS gap:** Linux xattrs are opaque byte blobs. BFS attributes were typed and the filesystem understood the types. pane-store bridges this gap by encoding type information in attribute naming conventions (`user.pane.type` declares the type of the primary value attribute) and by providing userspace indexing that BFS provided at the filesystem level. Queries are slower than BFS (no kernel-level B+ tree) but more flexible (pane-store can index any attribute dynamically).

**Free attributes:** Certain attributes are always available and always indexed: pane type, creation time, modification time, MIME type. This mirrors BFS's three always-indexed attributes (name, size, last_modified) that ensured basic queries always worked without explicit index creation.

**Target filesystem:** btrfs. No alternatives, no hedging. btrfs supports ~16KB per xattr value with no per-inode total limit — sufficient for pane's metadata. Beyond xattrs, btrfs provides snapshots (system-level rollback beyond Nix generations), CoW (atomic file modifications), transparent compression (zstd), and send/receive (efficient system image transfer) — all of which align with pane's distribution philosophy. ext4's xattr limit (~4KB total) is insufficient. btrfs is already the default on Fedora and openSUSE.

### pane-fs — Filesystem Interface

Plan 9's gift: if state is a file, any tool can access it. pane-fs is a FUSE filesystem at `/srv/pane/` that exposes pane state for scripts, remote access, and tools in any language.

pane-fs is a translation layer — it converts FUSE operations into pane protocol messages. It is just another client of the pane servers. It has no special privilege and no server logic.

The filesystem provides universality that typed protocols cannot (any language, any tool). The typed protocol provides safety that the filesystem cannot (compile-time verification, session guarantees). Both are needed.

**Three-tier access model.** The system offers three access tiers, each appropriate for different consumers and latencies:

| Tier | Mechanism | Latency | Use case |
|---|---|---|---|
| **Filesystem** | FUSE at `/srv/pane/` | ~15-30μs per op | Shell scripts, inspection, configuration, event monitoring. Human-speed operations where 30μs is invisible. |
| **Protocol** | Session-typed unix sockets | ~1.5-3μs per op | Kit-to-server communication, rendering, input dispatch, bulk state queries. Machine-speed operations. |
| **In-process** | Kit API (direct function calls) | Sub-microsecond | Application logic within a pane-native client. No IPC, no serialization. |

The principle: if you'd be comfortable with 30μs latency and per-file granularity, use the filesystem. If you need machine-speed access with typed guarantees, use the protocol. If you're inside a pane-native client, the kit handles everything — the developer doesn't choose a tier, the kit chooses for them.

pane-fs targets FUSE-over-io_uring (Linux 6.14+), which halves the overhead and eliminates concurrency bottlenecks via per-CPU queues. As a distribution that controls its kernel version, pane requires io_uring-backed FUSE — this is not an optional optimization but a baseline expectation.

### Notifications

Notifications are panes — not a separate subsystem. A notification is created as a floating pane with attributes (source, timestamp, content, priority). It participates in routing, has a filesystem projection, and can be queried via pane-store. Retention policies, routing to logs, and dismissal are all standard pane operations — no dedicated notification server is needed.

System events that produce notifications (D-Bus signals via pane-dbus, agent mail, build results) create notification panes through the pane protocol. The compositor manages their display (floating, anchored, transient). The pane-store indexes their attributes. The user's routing rules determine what happens to them. This is infrastructure-first design: the notification "feature" emerges from composing existing infrastructure.

---

## 4. Kit Decomposition

Kits are the programming model. Not wrappers over a protocol — they ARE the developer experience. When a developer uses the Interface Kit, they are using a complete UI programming model that happens to communicate with the compositor internally, the same way BeOS's libbe.so presented a coherent world of BWindows and BViews while communicating with app_server through kernel ports.

Developers loved the BeAPI because it was thoughtful — small, composable primitives designed by people who wrote real applications. Schillings: "common things are easy to implement and the programming model is CLEAR." That is the standard. The API is the user interface for developers.

The kit hierarchy is layered, not flat:

```
pane-ai (agent infrastructure)
pane-media (PipeWire abstraction)
pane-input (generalized keybinding grammar)
    |
pane-text (text buffers, structural regexps)
pane-ui (text, widgets, styling)
    |
pane-app (application lifecycle, looper, routing)
pane-store-client (attribute access, queries)
    |
pane-proto (wire types, session definitions)
pane-notify (fanotify/inotify abstraction)
```

Each kit builds on the ones below it. pane-proto is the foundation — pure types and serialization, analogous to the Support Kit. pane-app is the messaging and lifecycle layer — analogous to the Application Kit. pane-ui is the rendering layer — analogous to the Interface Kit. The hierarchy ensures clean dependency ordering and prevents circular dependencies.

### pane-proto — Foundation

Wire types (message enums, session type definitions), inter-server protocol types, serde derivations, validation. Every other crate depends on this. No runtime dependencies — pure types and serialization.

The session types defined here are the single source of truth for every protocol in the system. When someone changes a protocol (adds a request variant, restructures the handshake), every client that doesn't update fails to compile. Protocol evolution is a refactoring operation, not a debugging expedition.

### pane-app — Application Lifecycle

The developer's primary interface for building pane-native applications. Analogous to BeOS's Application Kit.

**Looper.** A thread with a message queue, processing messages sequentially. This is BLooper in Rust: `std::thread::spawn` a thread that reads from a channel, dispatches to handlers, runs until stopped. The looper is the concurrency primitive — each pane-native application has at least one looper (the app looper), and each window-equivalent has its own looper. Heavy work goes in spawned threads; the looper thread stays responsive.

**Handler.** Processes messages within a looper's context. Handlers chain — if a handler doesn't recognize a message, it passes to the next handler. This is the chain-of-responsibility pattern from BHandler, which gives each handler self-contained logic for the messages it understands.

**Routing.** Built into the kit, not a separate server. The kit:
- Loads routing rules from the filesystem (`/etc/pane/route/rules/`, `~/.config/pane/route/rules/`), one file per rule
- Watches rule directories via pane-notify for live updates — drop a file, gain a behavior
- On route action: evaluates rules locally, transforms content, resolves the target
- Queries pane-roster's service registry for multi-match scenarios
- Quality-based selection when multiple handlers match (Translation Kit pattern)
- Dispatches directly to the handler — sender to receiver, no intermediary

**Connection management.** Session-typed connections to the compositor and other servers. The kit handles reconnection transparently — if a server restarts, the kit detects the session death and reconnects. Messages queue in the kit's send buffer during the reconnection window.

**Application lifecycle.** Registration with pane-roster, launch semantics (single/exclusive/multiple), graceful shutdown.

### pane-ui — Interface

Text rendering, styling primitives, layout, widget rendering. The rendering infrastructure that all native pane clients share. Tag line content is set through the pane-app kit (which provides the API for declaring tag text and actions) and rendered by the compositor (which owns the chrome). pane-ui does not render tag lines — it renders body content.

**Text rendering.** GPU-accelerated text rendering with glyph atlas and instanced drawing. Text-oriented panes (shells, editors, logs) are first-class — not trapped inside a terminal emulator but rendered directly by the kit with the same quality as any other content. Client-side rendering means each process manages its own glyph rasterization — this is the standard Wayland cost that every GTK/Qt application already pays. The kit mitigates it through memoization: rasterized glyphs are cached and shared across pane-native processes via shared memory, so the per-process cost is a lookup rather than re-rasterization.

**Widget rendering.** Vello (GPU-compute 2D rendering via wgpu) for vector graphics, taffy for flexbox/grid layout. Widgets have semantic structure (buttons, labels, lists, text inputs) with roles, values, and actions — the accessibility tree is a byproduct of the widget model.

**The pane visual language.** Beveled borders, subtle gradients, warm saturated palette — the Frutiger Aero aesthetic, built into the kit. A developer using the Interface Kit produces output that looks like a pane application without effort, because the kit encodes the visual language. This is how BeOS achieved its integrated feel and how NeXTSTEP achieved its — not by centralizing rendering, but by providing a kit good enough that everyone used it.

**Layout management from day one.** Haiku's biggest GUI mistake was deferring layout management. Every application written before the Layout API existed had to be manually migrated. Pane's Interface Kit has layout management (taffy — flexbox/grid) from the beginning. Applications specify relationships ("this goes next to that, this fills remaining space"), not coordinates.

**Frame pacing and buffer management.** The kit manages the double-buffer lifecycle: allocate buffers from shared memory (memfd) or GPU memory (DMA-BUF), render into the back buffer, submit via wl_surface.attach + damage + commit, wait for wl_buffer.release before reusing. All of this is hidden from the developer — they draw; the kit handles the rest.

### pane-text — Text Manipulation

Text buffer data structures and structural regular expressions (sam-style `x/pattern/command`). This kit provides the editing primitives that pane-shell and editor panes compose with.

Sam's structural regular expressions are the key conceptual contribution here. Pike: "the use of regular expressions to describe the structure of a piece of text rather than its contents." The `x` command extracts all matches of a pattern within a selection; `y` operates on the intervals between matches. These compose: `x/\n/ { ... }` iterates over lines; `x/[^ ]+/` iterates over words. The structure is whatever the pattern says it is — no built-in line bias.

### pane-input — Generalized Keybinding

The Input Kit: a composable interaction grammar that works uniformly across all pane types. Vim's compositional structure generalized beyond text editing.

**The grammar engine.** N operators times M objects = N*M interactions. New operators compose with existing objects; new objects compose with existing operators. The grammar has four components:

1. **Operators** (verbs): delete, yank, change, open, route — actions meaningful in context
2. **Objects** (nouns): word, line, file, widget, pane — addressable units in content
3. **Motions**: next word, previous file, parent directory — navigation across objects
4. **Counts**: multipliers on motions

The dot command repeats the last operator + object combination. This is the specific property that makes the grammar qualitatively different from CUA: learning compounds multiplicatively, not additively.

**The keymap hierarchy.** Layered resolution:
1. System-wide (compositor scope — Super modifier prefix, never conflicts with pane bindings)
2. Kit-level (common to all panes: navigation, standard operators)
3. Content-type (text objects for text panes, file objects for file managers)
4. Pane-local (specific to a pane instance)

This mirrors Emacs's global -> major-mode -> minor-mode -> local chain, translated to pane's content-type system.

**Modes are first-class.** Named modes at every level — compositor modes (resize, layout), pane modes (Normal, Insert), content-type modes. Ephemeral modes (Hydra-style) for rapid command sequences. Mode transitions are visible in the tag line.

**Discoverability.** which-key-style display of available bindings after a prefix or mode switch. The tag line participates: it shows available commands as clickable text. New users click; experienced users type. Progressive disclosure: CUA floor -> which-key discovery -> modal efficiency -> custom grammar extensions.

**Default mode.** Insert-as-default for new panes (matches user expectations from every other application), with explicit Normal mode entry via a configurable key. The grammar is available and discoverable, not hidden.

### pane-store-client — Store Access

Client library for pane-store. Attribute read/write, query building, change notification subscription. Reactive signal composition for live queries — the client subscribes to change notifications and maintains a query result set locally. This is how BeOS's live queries worked: the infrastructure is general-purpose, the composition is client-side.

### pane-media — Media Abstraction

PipeWire already implements the Media Kit's graph-based model at the system level. Pane's media kit is a thin Rust wrapper — not a reimplementation.

The kit wraps PipeWire's client API with pane's session-typed conventions. It exposes the media node graph as pane-visible state (nodes and connections inspectable via pane-fs, parameters as filesystem-based configuration). It delegates all policy to WirePlumber (PipeWire's session manager).

What pane provides that PipeWire alone doesn't: visibility and control through pane's interaction model. Media nodes as panes with tag lines. Routing as visible graph connections. Parameters as files that pane-notify watches. The media graph becomes a first-class part of the desktop experience, not a hidden subsystem.

### pane-ai — Agent Infrastructure

Agents are system users, not applications. They participate through the same protocols, filesystem interfaces, and routing infrastructure as human users, in sandboxed environments with permissions governed by declarative specification.

**Agents as Unix users.** Each agent runs as an actual system user — its own account, its own home directory, its own filesystem view (scoped via Linux user namespaces), its own Nix profile. `who` shows which agents are active. `finger agent.reviewer` shows its specification and current task. Everything an agent does is visible through the same tools you inspect anything else with.

**The `.plan` file.** An agent's behavior — what tools it can use, what panes it can observe, what files it can access — is declared in `.plan`: a human-readable, editable, version-controllable artifact in its home directory. The `.plan` IS the agent's identity. The governance question (who authors and audits it) is resolved by the same mechanisms as any other configuration: filesystem permissions, version control, audit trails.

**Communication through Unix primitives.** The multi-user Unix communication tools — `write`, `talk`, `mail`, `mesg`, `wall` — are paradigmatic examples of the patterns we recover:

- `write`: agent sends a one-liner to your pane (brief, one-directional)
- `talk`: focused interactive session (split-screen, bidirectional)
- `mail`: asynchronous, persistent, queryable. Files with typed attributes — agent communication becomes queryable by pane-store, filterable by routing rules
- `mesg y/n`: one-bit availability protocol. Agent respects `mesg n` by queuing as mail
- `wall`: broadcast to all inhabitants

These were designed for multi-inhabitant systems. The inhabitants have arrived.

**Local models are first class.** A user running entirely on local models gets the same agent infrastructure, the same `.plan` governance, the same communication patterns, the same scripting protocol integration as a user with API access to frontier models. The system is designed local-first; remote APIs are an enhancement, not a requirement.

Models are managed as a kit concern — discovery, loading, inference, resource scheduling. Models are files on the filesystem, managed through the same infrastructure as everything else. Model format support (GGUF, safetensors, etc.) is extensible via the Translation Kit pattern: drop a model translator, the system gains a format. Resource-aware scheduling ensures inference doesn't starve the compositor or latency-sensitive operations.

**Routing rules as data governance.** The same routing infrastructure that dispatches content to handlers dispatches inference requests to models. Routing rules determine what data is processed locally vs sent to remote APIs — the routing rule IS the privacy policy, expressed declaratively, enforceable by the system, inspectable by the user.

A rule might say: queries touching files under `~/work/` go to the local model. General knowledge questions go to the remote API. Anything containing credentials never leaves the machine. The rules are files in directories — the same extension surface as everything else. The user sees and controls exactly what goes where. When a `.plan` specifies `model: local-only`, Landlock enforcement guarantees no data leaves the system.

**Local/remote as a routing decision, not an application decision.** The agent kit provides a uniform interface regardless of where inference happens. Switching between models is a routing configuration change, not an application change. Users can optimize across model strengths — fast local model for low-latency work, strong remote model for complex reasoning, private local model for sensitive data — with the routing rules governing dispatch and the session protocol governing the conversation. The experience is one continuous interaction with the system.

---

## 5. The Scripting Protocol

Session types + optics = the recovery of BeOS's most important feature.

### What BeOS had

Every BHandler implemented `ResolveSpecifier()` and `GetSupportedSuites()`. Any running application's state was queryable and modifiable at runtime through a structured protocol. The `hey` command-line tool could script any application:

```
hey Tracker get Frame of Window 0
hey StyledEdit set Value of View "textview" of Window 0 to "hello"
```

This was compositional and dynamic. Each handler peeled off one specifier and forwarded to the next. "Get Frame of Window 1 of Application Tracker" was resolved by Tracker peeling off "Application Tracker" (that's me), forwarding "Window 1" to the window, which peeled it off and forwarded "Frame" to the frame handler.

This was one of BeOS's most important features. Every application was automatable through the same messaging system it used internally.

### How pane recovers it

Session types are the horizontal structure — conversation over time. Optics are the vertical structure — state access at each moment. Together they provide the scripting protocol.

A scripting interaction is a session: send a query (an optic-addressed access into the handler's state), receive a result, optionally loop. "Get property X of object Y" is a lens access. "Set property X to Z" is a lens set. "What properties do you expose?" returns the available optics — discoverability as part of the protocol.

The session type governs conversation safety (you can't send a set before receiving the capabilities). The optics govern state access safety (GetPut, PutGet). Monadic error handling covers the case where a query doesn't resolve.

### The hard problem: dynamic specifier chains

BeOS's scripting protocol resolved specifier chains at runtime. The chain "get Frame of Window 1 of Application Tracker" was resolved by each handler peeling off one specifier and forwarding. This was compositional and dynamic — the structure was only known at runtime.

Optics are typically static. This is the hardest design problem in translating BeOS's scripting to pane's typed world.

The approach: dynamic optic composition at the protocol level. Each handler advertises its available optics (via GetSupportedSuites equivalent). A specifier chain is a sequence of optic accesses. The client constructs the chain; each handler resolves one step and forwards. The session type for each step is known statically (it's always "send specifier, receive result or forward"), but the chain length and specific optics are dynamic.

This is a controlled runtime dynamism within a statically-typed protocol. The session type ensures each step is well-formed. The optic laws ensure each access is consistent. The dynamic composition ensures the full chain works. It's the same pattern as BeOS's ResolveSpecifier — peel, resolve, forward — but with each step type-checked.

The filesystem interface provides the fallback. If the typed scripting protocol is too rigid for a particular use case, the filesystem at `/srv/pane/` provides the same access in a weakly-typed but universally accessible form. Shell scripts use the filesystem; compiled programs use the typed protocol. Both access the same underlying state.

---

## 6. Threading and Concurrency

### The model

Per-component threads with message queues. This is BeOS's BLooper model, realized in Rust.

Each component — each application, each window-equivalent, each server — runs its own thread with its own message queue. Messages are processed sequentially within each component. Concurrency arises from many components running simultaneously, not from concurrency within a component.

This model produced stability in BeOS not despite the complexity of pervasive multithreading but because of it. Message passing eliminated shared mutable state. Per-handler operational semantics eliminated global state entanglement. The protocol replaced the global coordinator. The scheduler had enough thread granularity to maintain responsiveness.

### How it maps to Rust

Rust's ownership system provides compile-time guarantees that BeOS enforced by convention. Send + Sync traits guarantee that data crossing thread boundaries is safe. The `#[must_use]` attribute on session endpoints catches forgotten responses. The borrow checker prevents shared mutable state — the thing BeOS's "Commandment #1" tried to prevent, now enforced by the compiler.

**The looper in Rust:**

```
// Conceptual — the actual API is the pane-app kit
let (tx, rx) = std::sync::mpsc::channel();
std::thread::spawn(move || {
    while let Ok(msg) = rx.recv() {
        // Sequential message processing — one at a time
        handler.message_received(msg);
    }
});
```

No async runtime. No system-wide executor. Just threads and channels. This is simpler than async/await, more predictable, and matches the actor model that BeOS proved works for desktop systems. Async/await would be appropriate for a high-throughput network server; it's wrong for a desktop environment where the concurrency grain is one-thread-per-window and the message rate is modest.

The one exception is the compositor. calloop (an epoll-based event loop) drives the compositor's main thread because smithay requires it for Wayland fd polling. session type channel operations are integrated with calloop via fd-based event sources. This is an implementation detail of the compositor, not a system-wide pattern.

### The benaphore lesson

Schillings' benaphore (atomic variable + semaphore, fast-path the uncontested case) teaches the right principle: optimize for the common case. In pane's threading model, most lock acquisitions are uncontested (a component accessing its own data from its own thread). Rust's standard library already provides this optimization — `Mutex` uses futex on Linux, which is essentially a benaphore: atomic check first, kernel involvement only on contention.

### What runs on what thread

| Component | Thread model |
|---|---|
| Compositor main loop | calloop (single thread, epoll-driven) — smithay, Wayland protocol, compositing, input |
| Dispatcher (1 per connection) | Dedicated thread — demuxes socket I/O to per-pane threads |
| Pane thread (1 per pane, server-side) | Dedicated thread — session protocol for one pane |
| Client looper (1 per pane, client-side) | Dedicated thread — the BLooper equivalent in pane-app |
| pane-roster | Dedicated looper thread |
| pane-store | Dedicated looper thread + worker threads for initial scan |
| pane-watchdog | Single thread (deliberately minimal) |
| pane-fs (FUSE) | Thread pool (FUSE operations may block) |

The threading granularity is per-pane, not per-widget. 50 panes = ~100 threads (client + server side) = ~2MB memory. The cost is the tax for concurrency — spend it at the right granularity.

---

## 7. Protocol Design

### Session types

Every interaction between components is a session — a typed conversation. The session type describes the entire protocol: what each party sends and receives, in what order, with what branches. The compiler enforces that both parties follow complementary protocols (duality). Deadlock freedom is guaranteed by the tree topology constraint.

Pane uses a custom session type implementation — a typestate `Chan<S, Transport>` designed for pane's exact needs. The theoretical basis is the Caires-Pfenning/Wadler correspondence between linear logic and concurrent processes. Key properties:

- **Duality is automatic.** `Dual<Recv<A, Recv<B>>>` = `Send<A, Send<B>>`. The compositor's protocol view is derived mechanically from the client's.
- **Branching uses standard Rust enums.** Enum variants contain session continuations. Pattern matching is exhaustive.
- **Transport-aware from the ground up.** Unlike par (which uses in-memory oneshot channels that can't cross process boundaries), pane's session types are parameterized over transport — unix sockets with postcard serialization for production, in-memory channels for testing.
- **Crash-safe.** `recv()` returns `Err(SessionError::Disconnected)`, not a panic. A crashed client produces a typed event, not a compositor crash. This is the property par cannot provide (it panics on drop).
- **calloop-compatible.** The compositor side registers socket fds with calloop as event sources — callback-driven, no async executor needed. Client side uses plain threads.

The formal session type primitives are verified in Lean/Agda. Par and dialectic are design references, not dependencies.

### The transport bridge *(Phase 2 — complete)*

The custom `Chan<S, T>` implementation bridges session types to unix sockets directly. Each `send()` serializes with postcard and writes to the socket; each `recv()` reads, deserializes, and advances the typestate. The session type enforces both message shapes (via Rust enums) and conversation ordering (via typestate advancement) on the wire — not just in tests.

The transport is parameterized: `UnixTransport` for production (length-prefixed postcard over unix domain sockets), `MemoryTransport` for testing (mpsc channels). Both implement the same `Transport` trait. Crash safety is proven: a dropped peer produces `Err(SessionError::Disconnected)`, not a panic.

The compositor side integrates with calloop via `SessionSource` — a calloop `EventSource` with a non-blocking accumulation buffer. The socket fd is registered for readiness notification; messages are read without blocking mode toggling. `MAX_MESSAGE_SIZE` (16MB) guards against malicious length prefixes.

**Event actor validation.** Fowler and Hu's Maty (OOPSLA) proves that event-driven actors participating in multiple sessions through a single event loop are deadlock-free across sessions, provided handlers terminate. Pane's compositor — a calloop event loop with per-client `SessionSource` registrations — is this model. Each connected client is a session. The calloop callback is Maty's handler. The compositor never blocks on a single client; it processes whichever session has a pending message. This is the same architecture BeOS's app_server proved empirically (one Desktop thread multiplexing N ServerWindows), now proven formally.

**Why binary session types suffice despite multiparty interactions.** The compositor mediates N client sessions, but N is dynamic and there is no meaningful global type spanning all client interactions. Maty handles the same pattern — the paper's ID server and chat server both service N clients — not through N-party type constructors but through repeated binary session registration, with the actor's internal state mediating cross-session coordination. Pane follows this exactly: each compositor-client relationship is a binary session; the compositor's shared state (layout tree, focus tracking) coordinates between them.

**Active phase decomposition.** The pane protocol has a nondeterminism problem: during the active phase, either side can send at any time. The resolution follows Maty's chat server pattern: session types for structured phases (handshake, negotiation, teardown), typed message enums for the bidirectional repeating phase. Session types govern where ordering matters; typed enums with exhaustive matching govern where bidirectional freedom matters.

### Protocol phasing

The pane protocol decomposes into three phases, each with a typing strategy matched to its structure.

**Handshake (session-typed).** Client sends `ClientHello`; compositor responds with `ServerHello`; client sends capabilities; compositor selects accept or reject via `Select`/`Branch`. The session type captures the sequence and branching exhaustively. After the handshake reaches `End`, the caller recovers the transport via `finish()` for the next phase.

**Active (typed enums, event-driven).** Both sides communicate via typed message enums on the same socket. Each direction has its own enum (`ClientToComp`, `CompToClient`). Both sides send when ready. The compositor dispatches via calloop handler; the client dispatches via its looper thread. Rust's exhaustive `match` guarantees every variant is handled. No session-type ordering constraint — the guarantee is type safety of each individual message, not sequencing.

**Teardown (session-typed or crash boundary).** Graceful: active phase enums include `RequestClose` / `CloseAck`. Crash: socket drops, `SessionEvent::Disconnected` fires. Cleanup proceeds identically minus the acknowledgment. This is Maty's affine session model: a session can be abandoned at any point, and the surviving party handles cancellation through its error path.

### Async by default

The default interaction is asynchronous. A fire-and-forget operation (send content, continue without waiting) is a `Send` followed by continuation. A request-response is a `Send` followed by a `Recv`. The distinction is in the type.

Fire-and-forget operations can be batched. The kit accumulates async messages and flushes in chunks — the same optimization BeOS's Interface Kit used for drawing commands. George Hoffman (Be Newsletter #2-36): "The Interface Kit caches asynchronous calls and sends them in large chunks at a time. A synchronous call requires that this cache be flushed." Synchronous calls are much slower because they force a flush and a round-trip. The guideline is the same now as it was then: async by default, sync only when you need the response.

### Crash handling

Rust has affine types (values can be dropped), not linear types (values must be used). A crashed client drops its session endpoints. The counterpart's next send/recv panics.

This is not acceptable for a compositor. The strategy:

- Each client session is wrapped with a crash boundary (catch_unwind or equivalent)
- A dropped session endpoint produces a "session terminated" event, not a panic
- The compositor cleans up the dead client's panes and continues serving others
- This is analogous to how BeOS's app_server handled unresponsive windows: discard messages, continue

The session type doesn't model crash because crash is a failure of the protocol's preconditions, not a protocol event. The crash boundary operates outside the session type system — it catches the failure and translates it into a typed cleanup event.

### Error composition

The foundations spec (§6) commits to monadic error composition — failures as values that compose through the same typed channels as the happy path. The architecture realizes this at three levels:

**Application-level errors** are branches in the session type. A request that can fail returns `Result<Success, Error>` as a choice — the sender handles both branches. This is typed, exhaustive, and composable: a pipeline of operations that each return Result composes via and_then/map, with errors propagating through the pipeline until handled.

**Component-level crashes** are caught at session boundaries (the crash handling above). The crash becomes a typed event in the supervisor's protocol — not an exception, not a panic, but a value that the roster and watchdog process through their own typed error paths.

**Recovery strategies** — retry with backoff, fallback to alternative handler, graceful degradation — are themselves composable operations. The pane-app kit provides combinators for common patterns: retry a routing dispatch N times, fall back to a secondary handler, degrade to filesystem-only access if pane-store is unavailable. These compose the same way application-level Results compose — via monadic chaining, not ad-hoc try/catch.

The principle: every error path in the system is typed and composable. An operation either succeeds (producing a value and a continuation) or fails (producing a typed error that propagates through the composition until something handles it). This is the foundations spec's "failures are values, not exceptions" realized at every level of the architecture.

### Message content

Pane messages are typed Rust enums serialized with postcard (serde-based, varint-encoded, compact). The specific message types are defined in pane-proto and evolve with the implementation.

The spirit of BMessage: rich, composable, introspectable data that can flow through the system without tight coupling. BMessage carried typed fields (B_STRING_TYPE, B_INT32_TYPE, etc.) addressable by name. Pane's messages carry typed fields addressable by Rust struct fields — stronger typing (compile-time field access) with the same loose coupling (a handler processes messages it understands and ignores others).

### Heartbeat

Infrastructure servers heartbeat each other on their direct session-typed channels. The heartbeat is a typed message in the session protocol:

- Compositor heartbeats pane-watchdog (interval: 2s, threshold: 3 misses = 6s detection)
- pane-roster heartbeats pane-watchdog
- Heartbeat is in-band but distinguishable — a `Heartbeat(sequence_n)` / `HeartbeatAck(sequence_n)` pair
- pane-watchdog monitors critical infrastructure; ordinary clients are monitored by session liveness (socket errors)

---

## 8. The Composition Model

The design bet: if the infrastructure is right, integrated experiences emerge without being designed top-down.

### The canonical proof: BeOS email

No component in BeOS implemented email. Five general-purpose systems composed:

1. **mail_daemon**: POP/SMTP transport. Wrote message files with typed BFS attributes (MAIL:from, MAIL:subject, MAIL:status, etc.)
2. **BFS**: stored the attributes, indexed them, evaluated queries against them
3. **Tracker**: displayed files with attribute columns (it knew nothing about email — it just displayed files)
4. **BQuery + live queries**: "MAIL:status == New" as a live inbox that updated in real time
5. **BeMail**: viewed and composed messages (just opened files)

Each was general-purpose. mail_daemon didn't know about Tracker. Tracker didn't know about email. The email UX emerged from the infrastructure. The same infrastructure immediately supported IM, contacts, music libraries — different attributes, same mechanism.

### How pane composes

**Routing composes content with handlers.** Text is activated. The pane-app kit evaluates routing rules locally. Content is transformed (extract filename, line number, URL). The target is resolved via pane-roster's service registry. Dispatch is direct — sender to receiver. Whether the content came from a user action, a D-Bus signal (via pane-dbus bridge), or a filesystem event, the routing is the same.

**Attribute indexing composes metadata with queries.** pane-store indexes file attributes and emits change notifications. A client that subscribes to change notifications and maintains a query result set has a live query — without pane-store implementing "live queries" as a feature. The composition is client-side, exactly as it was in BeOS.

**Filesystem exposure composes system state with tools.** Anything exposed at `/srv/pane/` is scriptable. A shell script that reads `/srv/pane/index` lists all panes. Writing to a pane's control file manipulates it. The filesystem is the universal FFI.

**Session persistence composes lifecycle with state.** The compositor serializes layout. The roster serializes the running app list. Each app serializes its own state. On restart, each component restores its part. No single component owns "the session" — it's emergent from each component following its protocol.

**The translation pattern composes format knowledge with applications.** Following the Translation Kit: translators handle format conversion as a system service. Applications work with interchange formats; translators handle the rest. The number of translators is linear (one per format), not quadratic (one per format pair). Drop a translator binary into `~/.config/pane/translators/`, the whole system gains a format.

### What makes composition work

Five properties, all present in BeOS, all recoverable in pane:

1. **Typed attributes on files, indexed by the filesystem.** Without this, there's no queryable data. Pane: xattrs + pane-store indexing.
2. **Live queries delivered as messages.** Without this, views are static snapshots. Pane: pane-store change notifications + client-side composition.
3. **A file manager that displays attributes as columns.** Without this, metadata is invisible. Pane: the file manager pane reads attributes via pane-store-client and displays them.
4. **A type system connecting files to handlers.** Without this, double-click doesn't know what to open. Pane: routing rules + pane-roster service registry.
5. **Data producers writing attributes, not managing databases.** Without this, data is locked in proprietary stores. Pane: agents and services write files with attributes; the infrastructure composes the rest.

---

## 9. The Distribution Layer

Pane is a distribution, not a DE on a distro. The integration is NeXTSTEP-level: one thing.

### Nix as build substrate

Nix builds the entire system — kernel through desktop — as a single, transitively-closed derivation. The system closure is one artifact: a Nix derivation whose output contains kernel, initrd, s6 boot scripts, s6-rc compiled service database, `/etc/pane/` defaults, pane server binaries, kit libraries, and a system profile linking to all installed packages.

Nix is not the identity. It is the backstage infrastructure. The user does not interact with Nix to use pane — they interact with pane's interfaces. Nix builds the system and manages its evolution. This is the NeXTSTEP/Mach relationship: Mach provided the right primitives and was otherwise invisible. The identity was in the personality layer.

**The overlay approach.** Pane does not fork nixpkgs. It depends on nixpkgs as a flake input. Pane provides:
1. An overlay replacing `systemd.lib` with libudev-zero (breaks the transitive systemd dependency chain)
2. A custom system builder (pane's equivalent of `nixos/`) using s6
3. Service definitions for pane's infrastructure servers
4. The pane packages themselves (servers, kits)

The ~120,000 application packages in nixpkgs are used as-is. Zero merge conflicts with upstream. Package updates come for free by bumping the nixpkgs input.

### s6 as init

s6 is the init system. s6-linux-init provides PID 1. s6-svscan supervises service supervisors. s6-rc manages service dependencies via a compiled database.

**The boot sequence:**
1. Kernel execs `/sbin/init` (s6-linux-init-maker output)
2. s6-linux-init mounts tmpfs at `/run`, sets up the environment, execs s6-svscan on `/run/service`. s6-svscan becomes PID 1 for the lifetime of the machine.
3. Early services start (catch-all logger, s6-svscan-log)
4. rc.init runs as stage 2: mounts filesystems, starts networking, brings up s6-rc
5. s6-rc activates the service set from the compiled dependency database

**Pre-registered endpoints.** Following Haiku's launch_daemon pattern and systemd's socket activation: communication endpoints (unix sockets) for pane servers are created before the servers start. Messages queue until the server is ready. This eliminates startup ordering as a concern.

With s6, this is achieved via s6-fdholder: a program that holds open file descriptors across process restarts. s6-fdholder creates the sockets at boot; each server retrieves its socket on startup. If a server crashes and restarts, it retrieves the same socket — zero-downtime from the clients' perspective, because their connection endpoint never went away.

**Readiness notification.** Each pane server signals readiness by writing a newline to a designated fd (specified in `notification-fd` in the service directory). s6-supervise catches this, updates status, and broadcasts to subscribers. Dependent services wait for readiness — not just process start. This is correct dependency ordering: "pane-store is ready to serve" not "pane-store's process exists."

**Service definitions.** Each pane server is a longrun with a `run` script, a `notification-fd` file, and s6-rc dependency declarations:

```
# /etc/s6-rc/source/pane-comp/run
#!/bin/execlineb -P
fdmove -c 2 1
s6-fdholder-retrieve /run/s6-fdholder/s <wayland socket fd>
exec pane-comp --socket-fd 3
```

```
# /etc/s6-rc/source/pane-comp/notification-fd
3
```

```
# /etc/s6-rc/source/pane-comp/dependencies.d/
pane-roster
elogind
```

The s6-rc source directories are Nix derivation outputs. `s6-rc-compile` processes them into a binary database. The entire service graph is a Nix expression.

### Mutable configuration on an immutable base

Pane wants two things: an immutable, reproducible system base (Nix's strength) and writable, live configuration (`/etc/pane/` — filesystem-as-interface commitment).

**The reconciliation:**

1. At build time, Nix produces `/nix/store/<hash>-pane-config/` with all default configs
2. On first boot (or after `pane-rebuild switch`), an activation script diffs new defaults against `/etc/pane/`:
   - New keys: added with default values
   - Changed defaults: updated if user hasn't modified; preserved if user has
   - Removed keys: flagged for cleanup
3. `/etc/pane/` is a regular writable directory on a persistent volume
4. User modifications are tracked via xattr (`user.pane.modified = true`) or a manifest file

Nix owns the defaults. The user owns the overrides. The activation script mediates. This is the Haiku packagefs shine-through pattern: the package layer provides defaults, the writable layer provides overrides.

### Atomic upgrades and rollback

Each `pane-rebuild switch` creates a new Nix generation. The previous generation is preserved. Rollback is one command or one boot menu selection away. Generations are cheap — Nix's content-addressed store deduplicates shared packages.

The s6-rc service transition is: compile new database from new service definitions, run `s6-rc-update` to live-switch. Services restart against the new definitions. No reboot required for most changes.

### Per-user profiles

Each system user (human or agent) has an independent Nix profile. Users install, remove, and rollback packages independently. Packages shared between users are deduplicated in the store.

An agent's environment is declaratively specified: `.plan` describes behavior, Nix profile describes tools. Both are versionable, shareable, reproducible. `nix profile diff-closures` shows exactly what changed — full auditability.

---

## 10. The Aesthetic and Rendering Model

### Client-side rendering, kit-mediated consistency

Pane embraces the Wayland rendering model: each pane renders its own content into buffers. The compositor composites those buffers with its own chrome. Visual consistency comes from the kits, not from centralized rendering.

This is how BeOS worked. App_server composited, but visual consistency came from every application using the Interface Kit. The kit encoded the visual language — fonts, colors, control styles, layout conventions. When every application uses the same kit, they produce the same look without a central authority forcing it.

Pane achieves this identically: the Interface Kit (pane-ui) provides shared rendering infrastructure. Glyph atlas, color palette, control styles, layout primitives. A developer using pane-ui produces output that looks like pane because the kit makes it the path of least resistance. The compositor composites the result and adds chrome — the same division of labor as BeOS's app_server/Interface Kit split.

### The compositor's rendering responsibilities

- Compositing all client buffers into the output framebuffer (via smithay's GLES renderer)
- Rendering chrome: tag lines (with editable text, cursor, selection), beveled borders, split handles, focus indicators
- Layout: positioning pane buffers according to the layout tree
- Presenting the final framebuffer to DRM/KMS via page flip

The compositor gets compositing for free via Wayland/smithay. This is the single largest advantage over Haiku, which has wanted compositing for 15 years and still doesn't have it. Pane starts composited.

### The aesthetic

Frutiger Aero — what if Be survived into the 2000s and refined alongside early Aqua.

- **Depth through lighting.** Subtle vertical gradients on controls. 1px highlight/shadow edges. Matte and solid — not glossy Aqua gel, not flat Metro.
- **Beveled borders and visible chrome.** Panes have real borders. Controls look like controls. Structure is always visible. Rounded corners (3-4px radius).
- **Selective translucency.** Floating elements (scratchpads, popups) are translucent to show context. Translucency where it aids comprehension, not universally.
- **Warm saturated palette.** Warm grey base, saturated accent colors for focus/dirty/active states.
- **Typography split.** Proportional sans-serif for widget chrome. Monospace for text content and tag lines. Tag line stays monospace — it's executable text where column alignment matters.
- **One opinionated look.** No theme engine. The aesthetic IS pane's identity. Individual properties configurable (accent color, font size) but not wholesale theme replacement.

### HiDPI and multi-monitor

Pane inherits proper HiDPI from Wayland. Fractional-scale (wp_fractional_scale_v1) provides per-output scale as a fraction with denominator 120. The Interface Kit renders at the scaled resolution and uses viewporter to set the surface destination to the unscaled size. No blurriness, no upscaling artifacts.

Multi-monitor is handled by smithay's DRM backend. Per-output configuration (resolution, position, scale, orientation) via wlr-output-management protocol.

Both of these are things Haiku still struggles with. Pane gets them from the platform.

---

## 11. Technology Choices

| Concern | Choice | Rationale |
|---|---|---|
| **Language** | Rust | Ownership gives compile-time threading guarantees that BeOS enforced by convention. Send/Sync are the type-level encoding of "Commandment #1." |
| **Compositor framework** | smithay | Composable building blocks for Wayland compositors in Rust. Protocol handling, DRM, input, rendering. |
| **Compositor event loop** | calloop | Epoll-based, callback-oriented. Required by smithay. Scoped to the compositor only. |
| **Session types** | `pane-session` crate: custom typestate `Chan<S, Transport>` | Transport-aware, crash-safe (Err not panic), calloop-compatible. Primitives verified in Lean/Agda. Par and dialectic as design references. |
| **Wire format** | postcard | Serde-based, varint-encoded, compact binary. |
| **Init system** | s6 + s6-rc | Small, composable, readiness-aware. Service directories are derivations. Compiled dependency database. |
| **Build system** | Nix | Declarative, reproducible, atomic upgrades, rollback. ~120k packages via nixpkgs. |
| **Filesystem notification** | fanotify + inotify | fanotify for mount-wide (pane-store). inotify for targeted (config, plugins). |
| **FUSE** | Custom FUSE-over-io_uring module (on `io-uring` crate) | No existing Rust FUSE library supports io_uring. The kernel interface is small (two io_uring subcommands + standard FUSE opcodes); pane-fs needs only a bounded subset. Built directly on `/dev/fuse` + io_uring for maximum performance. |
| **Audio/media** | PipeWire | Graph-based media framework. Replaces PulseAudio + JACK. BeOS Media Kit model. |
| **Widget layout** | taffy | Flexbox/grid layout engine. Pure computation. |
| **Widget rendering** | Vello | GPU-compute 2D rendering via wgpu. The forward-looking choice over femtovg (OpenGL). Currently in active development by the Linebender project; will be stable by pane's widget rendering phase. |
| **Text rendering** | GPU glyph atlas | Instanced rendering. Shared across all pane-native clients via the Interface Kit. |
| **Input processing** | libinput + xkbcommon | Industry standard. Hardware abstraction and keyboard layout processing. |
| **D-Bus bridge** | zbus crate | Rust D-Bus implementation. pane-dbus translates at the boundary. |
| **Process tracking** | pidfd | Race-free process identity. Integrates into epoll for lifecycle monitoring. |
| **Sandboxing** | Landlock (primary) + seccomp (defense-in-depth) | Landlock: unprivileged, filesystem/network/signal scoping, maps 1:1 to `.plan` permissions. seccomp: syscall filtering as second layer. |
| **Testing** | proptest + pane-session MemoryTransport | Property-based tests for protocol correctness. Session types verified in-memory without sockets. |

---

## 12. Build Sequence

Each phase produces a testable, usable artifact. The ordering follows dependency: foundations first, then the things that build on them.

### Phase 1: Protocol Foundation
1. **pane-proto** — message types, session type definitions, property tests. The types that everything else depends on. *Status: built, session type migration in progress.*

### Phase 2: Transport Bridge (highest priority prototype)
2. **Session types over unix sockets** — the single most important prototype in the project. Prove that pane's custom session types (`Chan<S, UnixSocketTransport>`) can be driven over unix sockets with postcard serialization: a server-side calloop-driven endpoint talking to a client-side threaded looper, with the session type verified end-to-end. If this works, every protocol built afterward inherits the guarantee. If it doesn't, we discover the constraint before building a mountain of protocol code on a shaky foundation.
3. **calloop + session type integration proof** — drive pane's session type channel operations from within a calloop event loop via fd-based event sources. Verify that the typestate transitions, crash handling, and calloop's callback model coexist cleanly. This determines whether the compositor's session handling works as designed.

In Phase 1, session types define the protocol and verify message shapes at compile time. Phase 2 closes the gap: conversation ordering — what is sent when, by whom — is verified on the wire, not just in-memory tests. The systems work comes before the graphics work because the protocol is the foundation everything else stands on.

### Phase 3: Core Infrastructure + Minimal Agent Prototype
4. **pane-notify** — fanotify/inotify abstraction. Looper integration (calloop for compositor, channels for others).
5. **pane-app kit** — looper abstraction, handler chain, routing, connection management. The kit that application developers program against. Built on the verified transport from Phase 2.
6. **Minimal agent infrastructure** — agent user accounts, `.plan` file convention, message passing over the pane protocol, Unix communication patterns (`write`/`mail`/`mesg`). Not the full AI Kit — just enough for agents to participate as system users. From this point on, agents inhabit the system under development: running tests, monitoring builds, exercising protocols under multi-user load. Pane is developed by its own inhabitants.

### Phase 4: Minimal Compositor
6. **pane-comp skeleton** — smithay compositor, single hardcoded pane, tag line + text rendering. First pixels on screen. Now built on session-verified protocols, not hand-written state machines.
7. **pane-shell** — PTY bridge client, first usable terminal. The milestone that makes pane a daily driver.

### Phase 5: Tiling Desktop
8. **Layout tree** — tiling with splits, multiple panes, tag-based visibility. Multiple shells on screen.
9. **Input binding** — compositor-level key bindings, focus management, tag switching.

### Phase 6: Infrastructure
10. **Routing** — routing rules in the pane-app kit, filesystem-based rule loading, live rule updates.
11. **pane-roster** — service directory, app lifecycle, service registry.
12. **pane-store** — attribute indexing, change notifications, queries.
13. **pane-watchdog** — heartbeat monitoring, escalation.

### Phase 7: Richness
14. **Widget rendering** — Vello + taffy, Frutiger Aero controls.
15. **pane-fs** — FUSE at `/srv/pane/`.
16. **pane-dbus** — D-Bus bridge (notifications, PipeWire portals, NetworkManager).
17. **pane-media** — PipeWire kit wrapper.

### Phase 8: Ecosystem
18. **Legacy Wayland/XWayland** — xdg-shell, xdg-decoration, XWayland integration.
19. **pane-input kit** — generalized grammar engine, keymap hierarchy, discoverability.
20. **pane-ai** — agent infrastructure, `.plan` specification, Unix communication patterns.

### Phase 9: Distribution
21. **Nix system builder** — s6-linux-init, s6-rc service database, kernel, initrd, system closure.
22. **Binary cache** — Cachix for the libudev-zero overlay and pane packages.
23. **Installer** — from ISO to running pane system.

The critical path is phases 1-5. Phase 2 is the make-or-break: if the transport bridge works, every protocol built afterward is verified end-to-end. If it reveals constraints, we redesign before investing in compositor code. Once pane-shell works inside pane-comp with tiling (end of Phase 5), pane is a usable daily driver with session-verified protocols. Everything after that is enrichment on a sound foundation.

---

## 13. Open Questions

Things the Be engineers would flag as "we need to prototype this before committing."

### Session type transport bridge *(promoted to Phase 2 — first milestone)*
Moved from open question to the build sequence's highest-priority prototype. The transport bridge determines the guarantee level of every protocol built afterward. See §7 (The transport bridge) and §12 (Phase 2).

### calloop + session type integration *(Phase 2 — complete)*
Pane's custom session types are fd-based, not async/futures-based. The compositor registers session socket fds with calloop as event sources — callback-driven, no executor needed. `SessionSource` provides non-blocking accumulation with `SessionEvent::Message` / `SessionEvent::Disconnected` dispatch. Validated by Phase 2 prototype: 10 tests passing including crash recovery.

### Dynamic optic composition for scripting
How do optic-addressed property accesses compose across handler boundaries at runtime? BeOS solved this with ResolveSpecifier, which was compositional and dynamic. Optics are typically static. The runtime chain resolution pattern (peel, resolve, forward) needs a concrete prototype with at least three levels of nesting to prove it works ergonomically.

### The affine/linear gap
Rust's `#[must_use]` generates warnings for dropped session endpoints, not hard errors. A crashed process drops endpoints silently. The crash boundary (catch_unwind + cleanup) is the mitigation, but it operates outside the type system. Is there a principled way to handle this that doesn't require every session boundary to be wrapped in catch_unwind? Needs investigation once the first real multi-client compositor is running.

### Filesystem notification at scale
fanotify with FAN_MARK_FILESYSTEM watches the entire filesystem for xattr changes. On a system with millions of files, how much event traffic does this generate? Is the filtering (only `FAN_ATTRIB` events, only `user.pane.*` xattrs) sufficient to keep the event rate manageable? Needs measurement on a real system with realistic file counts.

### xattr size limits *(resolved)*
Pane targets btrfs exclusively. ext4's 4KB xattr limit is too restrictive; btrfs's ~16KB per value with no total limit is sufficient. The opinionated distribution philosophy means we pick one filesystem and commit.

### Widget rendering performance
Vello for widget rendering — GPU-compute via wgpu. Currently in active development (Linebender project). Needs validation: can Vello's rendering model integrate cleanly with smithay's GLES compositor? The wgpu→GLES backend exists but may have constraints. Needs prototyping once widget rendering phase begins.

### Selection-first vs. verb-first in the Input Kit
Kakoune's argument for selection-first (visual feedback before commitment) is strong. Vim's verb-first is more efficient for experts. The Input Kit could support both — verb-first in Normal mode with visual selection as alternative. Or it could commit to one model. Needs user testing with both approaches on non-text panes (file manager, process monitor) to determine which generalizes better.

### Agent governance
The `.plan` file declares an agent's permissions, but who audits the declaration? A malicious or poorly-written `.plan` could grant excessive access. The trust model needs to be concrete: who can create agents, who can modify `.plan` files, what are the defaults for new agents, how does the human discover what an agent has done?

### The two-world problem
Pane-native clients and legacy Wayland apps are two worlds. The mitigation strategy is progressive integration, not an all-or-nothing switch.

A pane application is a `.app` directory — inspired by macOS's application bundles, built on Nix flakes. The directory contains the binary (or wrapper script), integration metadata, pane-specific hooks, and any auxiliary helpers. Installation: paste a flake URL into a system tag line, an installation pane walks you through the process (automated, but rendering progress to the user so they can intervene if needed). The flake is evaluated, the app is installed with its pane integration.

Progressive integration means an application starts as a bare Wayland wrapper and can gain pane-native interfaces incrementally. Each improvement — a tag line configuration, routing rules for its content types, filesystem endpoints for its state, a `.plan`-governed agent companion — is an addition to the `.app` directory, not a rewrite of the application. The framework models user actions as an internal representation with metadata, so that existing applications can be encapsulated in pane's interaction model without requiring their source code to change.

The kits must still be so good that building a pane-native app is the easiest way to build a Linux desktop application. But the architecture accounts for gradual adoption: the ecosystem grows by wrapping first, then deepening integration over time.

### PipeWire screen capture integration
pane-comp needs to implement the xdg-desktop-portal ScreenCast D-Bus interface for screen sharing (WebRTC, OBS, etc.). This requires feeding compositor frame data into a PipeWire video source node. The integration between smithay's rendering pipeline and PipeWire's buffer model needs prototyping.

---

## Sources

### BeOS / Haiku
- Be Newsletter archive (231 issues, 1995-1999) — design rationale from the engineers who built it
- Haiku source code (~/src/haiku) — the reference implementation
- Giampaolo, "Practical File System Design with the Be File System" (1998)

### Session Types
- Honda, "Types for Dyadic Interaction" (CONCUR 1993)
- Caires, Pfenning, "Session Types as Intuitionistic Linear Propositions" (CONCUR 2010)
- Wadler, "Propositions as Sessions" (ICFP 2012)
- faiface/par crate — session types for Rust (design reference, not a dependency)
- boltlabs-inc/dialectic — transport-polymorphic session types (design reference)
- Fowler & Hu, "Speak Now: Safe Actor Programming with Multiparty Session Types" (OOPSLA) — event actor model validating calloop architecture
- boltlabs-inc/dialectic — transport-polymorphic session types (design reference)

### Systems
- skarnet.org/software/s6 — process supervision
- skarnet.org/software/s6-rc — dependency management
- docs.pipewire.org — media framework
- wayland-book.com — display protocol
- github.com/Smithay/smithay — compositor framework
- nixos.org — build infrastructure

### Design
- Pike, "Structural Regular Expressions" (1987)
- Pike, "Acme: A User Interface for Programmers" (1994)
- Fowler et al., "Exceptional Asynchronous Session Types" (POPL 2019)
- Erlang/OTP heart module — external watchdog pattern
