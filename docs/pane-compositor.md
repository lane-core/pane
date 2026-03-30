# pane-comp — The Compositor

Composites client surfaces, manages layout, renders chrome. This is the app_server equivalent — the rendering service that every client talks to.

Clients render their own content (the Wayland model). The compositor composites those client buffers with its own chrome. Visual consistency comes from the kits, not from centralized rendering — the same division of labor as BeOS's app_server/Interface Kit split.

---

## 1. Rendering Model

### Client-side rendering

Each pane renders its own body content into buffers. For pane-native clients: the Interface Kit (pane-ui) renders text, widgets, and graphics into shared-memory (memfd) or GPU-memory (DMA-BUF) buffers. For legacy Wayland clients: the client renders into its own wl_surface via whatever toolkit it uses. The compositor never renders body content on behalf of clients.

The kit manages the double-buffer lifecycle: allocate, render into back buffer, submit via `wl_surface.attach` + damage + commit, wait for `wl_buffer.release` before reusing. This is hidden from the developer — they draw; the kit handles the rest.

### Compositor-side rendering

The compositor renders:

- **Chrome**: tag lines (with editable text, cursor, selection), beveled borders, split handles, focus indicators. Chrome is pane's visual identity — one opinionated look, consistent across all panes regardless of content.
- **Compositing**: composites all client buffers into the output framebuffer via smithay's GLES renderer.
- **Layout**: positions pane buffers according to the layout tree.
- **Presentation**: submits the final framebuffer to DRM/KMS via page flip.

Tag line content travels through the pane protocol: the client sends tag content, the compositor renders it. The compositor owns the chrome; the client owns the body.

---

## 2. Wayland Protocol Support

### Core protocols

wl_shm, wl_seat, xdg-shell, linux-dmabuf, viewporter, fractional-scale (wp_fractional_scale_v1), presentation-time.

### ext- protocols (cross-compositor, preferred over wlr-)

- **ext-session-lock** — screen locking
- **ext-idle-notify** — idle detection
- **ext-image-copy-capture** — screenshot and screen recording
- **ext-data-control** — clipboard management
- **ext-color-management** — HDR support

### Compositor-specific protocols

- **layer-shell** — panels, overlays, backgrounds
- **xdg-decoration** — server-side decoration negotiation
- **wlr-output-management** — per-output configuration (resolution, position, scale, orientation)
- **Input method protocols** — text input, input method popups

### Pane protocol

Accepts pane-native client connections over a unix socket, multiple panes per connection (sub-sessions). The session type tracks where each conversation is in its lifecycle.

---

## 3. Threading Model — Three Tiers

BeOS's app_server had three levels: Desktop (shared state coordinator), ServerApp (one thread per application), ServerWindow (one thread per window). pane-comp preserves this structure:

| BeOS | Pane | Role |
|---|---|---|
| Desktop | Compositor main thread (calloop) | Shared state, compositing, Wayland protocol, input dispatch |
| ServerApp | Dispatcher thread (1 per connection) | Demuxes incoming socket messages to per-pane threads |
| ServerWindow | Pane thread (1 per pane) | Handles session protocol for a single pane |

### Compositor main thread

calloop (epoll-based event loop) drives the main thread because smithay requires it for Wayland fd polling. calloop is scoped to this thread — it is an implementation detail of the compositor, not a system-wide concurrency model.

The main thread handles:
- Wayland protocol dispatch (all smithay interactions — smithay is `!Send` by design, correctly confining Wayland protocol handling to this thread)
- Surface compositing and frame presentation
- Input event reception from libinput
- Input dispatch to the focused pane
- Key binding resolution (in-process — latency-critical)
- Frame timing and callback coordination

### Dispatcher threads (1 per connection)

When a pane-native client connects, a dispatcher thread is spawned. The dispatcher reads messages from the unix socket and demuxes them to the appropriate per-pane thread by pane ID. This is the ServerApp equivalent.

### Per-pane threads (1 per pane, server-side)

Each pane gets its own server-side thread. A slow pane cannot block its siblings. This is the BeOS guarantee: one ServerWindow thread per window, verified in the Haiku source (`ServerWindow` inherits `MessageLooper`, spawns a dedicated thread on `Run()`).

Per-pane threads process protocol messages and communicate with the main thread via channels. They never touch smithay objects directly.

### The two-thread pattern

Each pane has two threads — one client-side (the looper in pane-app) and one server-side (the pane thread in pane-comp). Messages flow from client to server asynchronously and are batched and flushed. Synchronous calls force a flush and round-trip. The default is async; sync only when a response is needed.

### Cost

At 50 panes: 50 pane threads + dispatchers = ~1MB physical memory, ~1.5us context switch overhead, negligible on 2026 hardware. Pierre Raynaud-Richard's measurements from Be Newsletter #4-46 (~20KB per thread) hold — modern Linux threads are comparable.

---

## 4. Shared State and Locking

The compositor's layout tree is shared state accessed by per-pane threads (to read their position/size) and the main thread (to composite and apply structural changes).

**RwLock on the layout tree.** This mirrors Haiku's `Desktop::fWindowLock` — a reader-writer lock where:
- Drawing commands take the **read lock** (concurrent across panes — multiple pane threads can read their geometry simultaneously)
- Structural changes (move, resize, create/destroy) take the **write lock** (exclusive — the main thread holds this during layout mutations)

Per-pane threads communicate with the main thread via channels for operations that require compositor action (damage, resize requests, tag updates). The channel is the boundary: pane threads enqueue; the main thread dequeues and acts within its calloop iteration.

---

## 5. Layout Tree

Recursive tiling with tag-based visibility (dwm-style bitmask). The layout tree is the compositor's model of pane composition — a tree of containers where branches define splits and leaves hold panes.

Spatial composition of panes is represented as tree structure. Two panes viewed together form a compound structure whose concrete presentation is their arrangement in the layout tree. On the filesystem, this same structure appears as sibling entries under `/pane/`.

---

## 6. Input Handling

- **libinput** integration for hardware abstraction
- **xkbcommon** for keyboard layout processing
- Key binding resolution is in-process and latency-critical — no IPC for the critical path
- Input dispatch routes events to the focused pane via the appropriate protocol:
  - Pane protocol events for native clients
  - wl_seat events for legacy Wayland clients

---

## 7. Three-Tier Access Model

The compositor participates in a system-wide three-tier access model for pane state:

| Tier | Mechanism | Latency | Use case |
|---|---|---|---|
| **Filesystem** | pane-fs (FUSE at `/pane/`) | ~15-30us per op | Shell scripts, inspection, configuration, event monitoring. Human-speed operations. |
| **Protocol** | Session-typed unix sockets | ~1.5-3us per op | Kit-to-compositor communication, rendering, input dispatch, bulk state queries. Machine-speed operations. |
| **In-process** | Kit API (direct function calls) | Sub-microsecond | Application logic within a pane-native client. No IPC, no serialization. |

The compositor is the protocol tier's primary server. pane-fs is a translation layer that converts FUSE operations into pane protocol messages — it is just another client of the compositor with no special privilege.

---

## 8. HiDPI and Multi-Monitor

Fractional-scale (wp_fractional_scale_v1) provides per-output scale as a fraction with denominator 120. pane-ui renders at the scaled resolution and uses viewporter to set the surface destination to the unscaled size. No blurriness, no upscaling artifacts.

Multi-monitor is handled by smithay's DRM backend. Per-output configuration (resolution, position, scale, orientation) via wlr-output-management protocol.

---

## 9. Boundary

**Does not contain:** routing logic, application launch logic, file type recognition, attribute indexing, or any server functionality beyond compositing and input.

For native panes, a route action sends a TagRoute event to the pane client; the pane-app kit evaluates routing rules and dispatches. For legacy panes, the compositor handles route dispatch through its own kit integration.

**Dependencies:** pane-notify for config reactivity (compositor watches `/etc/pane/comp/` for live configuration changes). pane-roster for service registration (compositor registers on startup, heartbeats pane-watchdog). pane-session for session-typed client connections. Compositional equivalence invariant (architecture §2): the layout tree must be consistent with pane-fs directory nesting and protocol session structure.

---

## Requirements

### Requirement: Compositor boots with winit backend

The pane-comp binary SHALL initialize a smithay compositor using the winit backend, creating a window on the host desktop. The compositor main thread SHALL run a calloop event loop that processes Wayland protocol events and drives frame compositing.

#### Scenario: Compositor launches
- **WHEN** `cargo run -p pane-comp` is executed
- **THEN** a window SHALL appear on the host desktop displaying the compositor output

#### Scenario: Clean shutdown
- **WHEN** the compositor window is closed
- **THEN** the process SHALL exit cleanly without panics or resource leaks

### Requirement: Client-side body rendering

Pane body content SHALL be rendered by clients into buffers (shared-memory or DMA-BUF). The compositor SHALL composite these buffers into the output framebuffer. The compositor SHALL NOT render body content on behalf of clients.

#### Scenario: Native client renders body
- **WHEN** a pane-native client submits a buffer via the pane protocol
- **THEN** the compositor SHALL composite that buffer at the pane's position in the layout tree

#### Scenario: Legacy client renders body
- **WHEN** a legacy Wayland client commits a wl_surface
- **THEN** the compositor SHALL composite that surface with pane chrome applied

### Requirement: Compositor-rendered chrome

The compositor SHALL draw pane chrome (tag line, borders, split handles, focus indicators) around pane content. Chrome is rendered by the compositor, not by clients.

#### Scenario: Tag line visible
- **WHEN** a pane is displayed
- **THEN** a tag line with text SHALL be rendered above the pane body on a distinct background color

#### Scenario: Borders visible
- **WHEN** a pane is displayed
- **THEN** beveled borders SHALL be visible around the pane, distinguishing it from the background

### Requirement: Per-pane server-side threads

The compositor SHALL spawn a dedicated thread for each pane. A slow or blocked pane thread SHALL NOT prevent other pane threads from processing their protocol messages.

#### Scenario: Pane isolation
- **WHEN** a pane's server-side thread is blocked (e.g., waiting on a slow client)
- **THEN** other panes SHALL continue to process protocol messages and render normally

#### Scenario: Dispatcher demuxing
- **WHEN** a pane-native client connection carries messages for multiple panes
- **THEN** a dispatcher thread SHALL demux those messages to the correct per-pane threads

### Requirement: Layout tree with RwLock

The layout tree SHALL be protected by a reader-writer lock. Per-pane threads SHALL acquire the read lock to query their geometry. The main thread SHALL acquire the write lock for structural changes (create, destroy, move, resize).

#### Scenario: Concurrent geometry reads
- **WHEN** multiple pane threads query their position and size simultaneously
- **THEN** all reads SHALL proceed concurrently without blocking each other

#### Scenario: Exclusive structural mutation
- **WHEN** the main thread performs a layout change (split, close, resize)
- **THEN** it SHALL hold the write lock, and pane threads SHALL wait until the mutation completes

### Requirement: calloop scoped to main thread

calloop SHALL be used only for the compositor's main thread (Wayland fd polling, DRM, input). Per-pane threads and dispatcher threads SHALL use std::thread + channels. calloop SHALL NOT define the concurrency model for the compositor's per-pane protocol handling.

#### Scenario: Main thread uses calloop
- **WHEN** the compositor processes Wayland events, input events, or frame timing
- **THEN** calloop SHALL drive the event loop

#### Scenario: Pane threads use channels
- **WHEN** a per-pane thread receives a protocol message or sends a response
- **THEN** it SHALL use channel-based communication, not calloop event sources

### Requirement: Tag-based visibility

Each pane SHALL have a tag bitmask. The compositor SHALL display panes matching the currently selected tags. A pane can appear in multiple tag sets. Multiple tags can be viewed simultaneously (bitwise OR).

#### Scenario: Tag switching
- **WHEN** the user switches from tag 1 to tag 2
- **THEN** panes tagged with bitmask including tag 2 SHALL be displayed and tag-1-only panes SHALL be hidden

#### Scenario: Multi-tag view
- **WHEN** the user selects tags 1 and 3 simultaneously
- **THEN** all panes whose tag bitmask intersects with (1 | 3) SHALL be displayed

### Requirement: Heartbeat and crash handling

The compositor SHALL heartbeat pane-watchdog at regular intervals. The compositor SHALL wrap each client session with a crash boundary so that a crashed client produces a typed "session terminated" event, not a panic. The compositor SHALL clean up dead clients' panes and continue serving others.

#### Scenario: Client crash
- **WHEN** a pane-native client process is killed mid-session
- **THEN** the compositor SHALL detect the dropped session, remove the client's panes from the layout, and continue without interruption

#### Scenario: Watchdog heartbeat
- **WHEN** the compositor is running normally
- **THEN** it SHALL send heartbeat messages to pane-watchdog at the configured interval

### Requirement: Frame timing

The compositor SHALL coordinate frame callbacks across all clients and submit composited output to DRM/KMS via page flip. Frame timing SHALL use the presentation-time protocol to provide accurate feedback to clients.

#### Scenario: Frame pacing
- **WHEN** a client commits a new buffer
- **THEN** the compositor SHALL composite it in the next frame and send a frame callback when the client may render again
