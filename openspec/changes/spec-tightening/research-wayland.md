# Wayland Protocol Research — For Pane Compositor Design

Research for pane spec-tightening. Primary sources: The Wayland Protocol book (wayland-book.com), the Wayland documentation (wayland.freedesktop.org/docs/html/), the Wayland architecture page (wayland.freedesktop.org/architecture.html), wayland.app protocol explorer, smithay documentation (docs.rs/smithay, github.com/Smithay/smithay), Arch Wiki Wayland article, Linux kernel DRM/KMS documentation (kernel.org/doc/html/latest/gpu/drm-kms.html).

---

## 1. Wayland Protocol Fundamentals

### 1.1 What Wayland is

Wayland is a display protocol and a reference implementation (libwayland) for compositors to communicate with their clients. The compositor _is_ the display server — there is no separate server process sitting between the compositor and the client, which is the fundamental difference from X11. The Wayland documentation states the design motivation: "a lot of functionality has moved out of the X server and into client-side libraries or kernel drivers" (font rendering, direct rendering, 2D graphics), and Wayland consolidates this by making "rendering left to clients, and system wide memory management interfaces used to pass buffer handles between clients and the compositing manager."

The protocol governs a specific relationship: clients render pixels into buffers (using whatever GPU or CPU mechanism they choose), then hand those buffers to the compositor. The compositor takes all client buffers, composites them into the final image, and presents it to the display hardware. Input flows the reverse direction: the kernel delivers input events to the compositor (via libinput/evdev), and the compositor dispatches them to the appropriate client.

### 1.2 The object model

The Wayland protocol is an asynchronous object-oriented protocol. Communication happens over a Unix domain socket between each client and the compositor. Messages flow in both directions: **requests** (client to compositor) and **events** (compositor to client).

Every message targets an object identified by a 32-bit ID. Each object has an **interface** that defines what requests and events are valid for it, with each request/event assigned an opcode and a type signature. The protocol specification states: "each object has an interface which defines what requests and events are possible, and the signature of each."

Object ID allocation is split: clients allocate from [1, 0xFEFFFFFF], servers from [0xFF000000, 0xFFFFFFFF]. ID 0 is null. When either side receives a message, it looks up the object by ID, determines the interface, looks up the opcode's signature, decodes the arguments, and processes the message.

New objects are created through `new_id` arguments in requests — a request creates a new object and the client assigns it an ID from its range. Objects are destroyed through explicit `destroy` requests or through protocol-specified lifecycle events. The `wl_display.delete_id` event notifies the client that an ID has been freed for reuse.

### 1.3 Wire protocol

Messages are encoded as 32-bit words in host byte order. The header is two words: the first is the object ID, the second packs the message size in bytes (upper 16 bits) and the opcode (lower 16 bits). Arguments follow, aligned to 32-bit boundaries.

Primitive types:
- **int/uint**: 32-bit signed/unsigned
- **fixed**: 24.8 signed fixed-point (used for coordinates — sub-pixel precision without floating point)
- **object**: 32-bit object ID reference
- **new_id**: 32-bit ID that creates a new object on receipt
- **string**: 32-bit length + UTF-8 content + NUL terminator, padded to 32-bit alignment
- **array**: 32-bit length + raw data, padded to 32-bit alignment
- **fd**: file descriptor, passed via Unix socket ancillary data (out-of-band)
- **enum**: 32-bit encoded enum value

File descriptor passing over the Unix socket is critical infrastructure — it's how keymaps, pixel buffers (shared memory), DMA-BUF handles, and clipboard data move between compositor and client without copying through the socket.

### 1.4 The core interfaces

**wl_display** — the singleton bootstrap object. Every client gets it automatically on connection. It provides `sync` (create a callback that fires when all prior requests have been processed — an ordering barrier) and `get_registry` (obtain the global registry). It emits `error` events for fatal protocol violations and `delete_id` for ID recycling.

**wl_registry** — the singleton that advertises all compositor globals. On connection, the compositor sends a `global` event for each available global object, carrying the interface name, version, and a numeric name (identifier). The client calls `bind` with the name and desired version to obtain a reference. `global_remove` events notify when globals go away.

**wl_compositor** — the global that creates surfaces and regions. Two requests: `create_surface` and `create_region`. This is the entry point for all visual content.

**wl_surface** — the fundamental visual primitive. A rectangular area of pixels that may be displayed on zero or more outputs. Key requests:
- `attach`: set a pending buffer (the pixel source)
- `damage` / `damage_buffer`: mark regions that changed
- `frame`: request a callback when the compositor is ready for a new frame
- `commit`: atomically apply all pending state
- `set_opaque_region`: hint which parts are fully opaque (optimization)
- `set_input_region`: define which parts accept input
- `set_buffer_transform`: rotation/reflection
- `set_buffer_scale`: integer scale for HiDPI

Events: `enter`/`leave` (surface enters/exits an output's scanout), `preferred_buffer_scale`, `preferred_buffer_transform`.

**wl_buffer** — opaque container for pixel data. Created by factory interfaces (wl_shm_pool or linux-dmabuf). The compositor reads the buffer after `wl_surface.commit` and sends a `release` event when done, signaling the client can reuse it.

**wl_seat** — represents a user's input position (keyboard + pointer + touch). The compositor sends `capabilities` events announcing which device types are available. The client then calls `get_pointer`, `get_keyboard`, or `get_touch` to obtain input objects.

**wl_output** — represents a physical display. Advertises mode, geometry, scale, and transform. Clients use this to adapt rendering for different displays.

**wl_shm** — the shared-memory buffer factory. The simplest way to get pixels from client to compositor, and the only buffer mechanism in the core protocol. Clients create a POSIX shared memory segment, wrap it in a `wl_shm_pool`, and allocate `wl_buffer` objects from it. Guaranteed pixel formats: ARGB8888 and XRGB8888.

**wl_data_device_manager / wl_data_device / wl_data_source / wl_data_offer** — the clipboard and drag-and-drop system. Copy-paste works through MIME-type negotiation: a data source advertises available types, a data offer presents them to the receiving client, the receiver asks for a specific type, and data transfers over an fd. Selection (clipboard) is set per-seat; the keyboard-focused client receives selection change events. Drag-and-drop follows a similar pattern with additional pointer grab semantics.

**wl_region** — a set of rectangles, used for opaque and input regions on surfaces. Created via `wl_compositor.create_region`, modified with `add` and `subtract` rectangle operations.

**wl_subcompositor / wl_subsurface** — hierarchy within surfaces. A subsurface is a child surface positioned relative to a parent. Supports synchronized mode (child commits apply with parent commits) and desynchronized mode (independent commits). Used for video overlays, embedded content, and decorations.

### 1.5 Events vs requests

Requests flow client-to-compositor, events flow compositor-to-client. The asymmetry reflects the architecture: the compositor owns the display and input hardware; clients ask permission (requests) and are notified of state (events).

Critically, there is no query mechanism. The specification states: "State is broadcast on connect, events are sent out when state changes. Clients must listen for these changes and cache the state. There is no need (or mechanism) to query server state." The client builds up its understanding of the compositor's state incrementally from events and must maintain it locally. This push-based model eliminates round-trip latency for state access and prevents stale reads, but it means clients must be prepared to handle state change events at any time.

### 1.6 How Wayland differs from X11

In X11, the X server sits between the compositor and the clients. The architecture is: kernel → X server → compositor (as a client of X) → display. Input events flow: kernel → X server → client. The X server maintains the front buffer and handles modesetting, but most actual rendering is done by clients via DRI (direct rendering infrastructure) bypassing the server for pixels while still going through it for window management, input, and coordination.

The Wayland architecture page identifies the core problem: "The X server doesn't have the information to decide which window should receive the event, nor can it transform the screen coordinates to window-local coordinates." The X server also "maintains control over the front buffer and modesetting" even though it delegates composition. These are unnecessary intermediary roles.

Wayland eliminates the intermediary. The compositor talks directly to the kernel for display (DRM/KMS) and input (evdev/libinput). It _is_ the display server. The compositor "looks through its scenegraph to determine which window should receive the event" and can precisely transform coordinates because it knows all transformations. It "directly issues an ioctl to schedule a pageflip with KMS," removing the X server round-trip.

What X11 provided that Wayland eliminates:
- Server-side rendering primitives (rectangles, arcs, text) — irrelevant since clients direct-render via GPU
- A centralized font system — clients use fontconfig/freetype
- A global coordinate space visible to all clients — security liability, gone by design
- A built-in window management protocol (ICCCM/EWMH) — replaced by protocol extensions (xdg-shell)
- Network transparency at the protocol level — not a goal; remote display is a separate concern

What Wayland doesn't provide that X11 did:
- Global screen coordinates (clients don't know where they are on screen — the compositor decides)
- Ability for any client to draw on any window or read any pixel on screen
- Built-in window management (no stacking order protocol, no taskbar protocol, no desktop icons protocol)
- Global hotkey registration by arbitrary clients
- A standardized screen capture mechanism (being resolved with ext-image-copy-capture)
- Network protocol (Wayland is local Unix socket only)

New problems Wayland creates:
- Every compositor must implement its own window management policy (xdg-shell provides the vocabulary but not the policy)
- Clipboard dies when the source application exits (a fundamental consequence of fd-based transfer)
- No standardized way for accessibility tools to inspect all client content (AT-SPI2 works but is incomplete)
- Compositors diverge on protocol extension support — no single "desktop Linux" protocol surface

### 1.7 "Every frame is perfect"

This is Wayland's core rendering principle. The protocol ensures that no frame is ever presented in a partially-updated state. The mechanism is double-buffered atomic commits.

When a client changes surface state — attaching a new buffer, marking damage, changing opaque region, setting transforms — those changes accumulate as **pending state**. Nothing is visible to the compositor until the client calls `wl_surface.commit`, which atomically applies all pending state to **current state**. The compositor only reads current state.

The Wayland protocol design patterns documentation states: "Rather than applying changes immediately, most Wayland interfaces accumulate modifications in a pending state," including pixel buffers, damaged regions, opaque areas, input regions, transformations, and buffer scale. "Only when a commit request is issued does the pending state merge into the current state."

This eliminates partial-update artifacts: a client never shows half-new-buffer, half-old-buffer, or a buffer with the wrong transform. Either the entire new state is visible, or the old state persists.

The xdg-shell configure/ack_configure cycle extends this to window management state: the compositor proposes a configuration (size, state), the client acknowledges and renders to match, then commits. The xdg_surface documentation says this achieves "every frame is perfect. That means no frames are shown with a half-applied state change."

### 1.8 Double buffering and surface commits

The commit operation is the protocol's heartbeat. A surface has two state sets:

- **Pending state**: accumulated by requests (attach, damage, set_opaque_region, set_input_region, set_buffer_transform, set_buffer_scale, and protocol-extension state). Not visible to the compositor.
- **Current state**: what the compositor uses for rendering. Updated atomically by commit.

The client workflow for each frame:
1. Write pixels into a buffer (either CPU rendering into shared memory, or GPU rendering via EGL/Vulkan)
2. `wl_surface.attach` the buffer (pending)
3. `wl_surface.damage_buffer` the changed regions (pending)
4. `wl_surface.commit` — all pending state becomes current atomically

For double-buffering with shared memory, clients typically allocate two buffers from the same `wl_shm_pool`. While the compositor reads buffer A (after commit), the client writes to buffer B. When the compositor sends `wl_buffer.release` for A, the client knows A is safe to rewrite.

For GPU rendering, the workflow is similar but uses DMA-BUF handles instead of shared memory. EGL's `eglSwapBuffers` handles the buffer rotation internally.

### 1.9 Damage tracking

Damage tells the compositor which regions of a surface have changed since the last commit. Two damage mechanisms exist:

- `wl_surface.damage`: coordinates in surface-local space (affected by buffer scale and transform)
- `wl_surface.damage_buffer`: coordinates in buffer space (direct pixel coordinates)

`damage_buffer` (added in wl_surface version 4) is generally preferred because it's unambiguous — the client knows exactly which buffer pixels it changed.

The compositor uses damage to optimize rendering: if only a small region changed, it can re-composite only that region rather than the entire output. Damage accumulates between commits — multiple damage calls before a commit are unioned.

If a client forgets to report damage, the compositor won't update the displayed content for those regions, even if the buffer pixels changed. Damage is part of the pending state applied by commit.

---

## 2. Key Protocol Extensions

The core protocol (`wayland.xml`) is deliberately minimal. Real desktop functionality requires protocol extensions from the `wayland-protocols` repository (maintained by freedesktop.org) and ecosystem-specific extensions (wlr, KDE, COSMIC, etc.). Extensions go through a maturity pipeline: experimental → staging → stable.

### 2.1 xdg-shell (stable)

The standard window management extension. Replaces the deprecated `wl_shell` from the core protocol. Defines three key interfaces:

**xdg_wm_base** — the global entry point. Handles `ping`/`pong` liveness checking (the compositor pings; the client must pong or be considered unresponsive). Creates `xdg_surface` objects via `get_xdg_surface`.

**xdg_surface** — wraps a `wl_surface` with desktop semantics. Adds the configure/ack_configure cycle: the compositor sends `configure` events with a serial; the client acknowledges with `ack_configure` carrying the same serial, then commits. This ensures state negotiation is atomic — the compositor and client agree on geometry before any frame is shown.

`set_window_geometry` lets clients define their logical window boundary separately from the rendered content, important for client-side decorations where drop shadows extend beyond the window.

**xdg_toplevel** — the role for top-level application windows. Key requests:
- `set_title`, `set_app_id`: identity for taskbars/switchers
- `set_min_size`, `set_max_size`: size constraints
- `set_maximized`, `set_fullscreen`, `set_minimized`: state requests
- `move`, `resize`: interactive operations, delegating control to the compositor (the client passes an input event serial and the compositor takes over, handling the operation in its internal coordinate space)
- `show_window_menu`: compositor-managed context menu for window operations

The compositor sends `configure` events with width, height, and an array of states: `maximized`, `fullscreen`, `resizing`, `activated`, `tiled_left/right/top/bottom`. Width/height of 0 means the compositor defers to the client's preferred size.

**xdg_popup** — the role for transient surfaces (menus, tooltips, dropdowns). Positioned relative to a parent surface using a **positioner** object that specifies anchor point, gravity, constraint adjustment, and offset. Popups support grab semantics for exclusive input capture.

The surface tree: xdg-shell forms a tree with toplevels at the root and popups as children. This tree is how the compositor manages window relationships, z-ordering of popups, and cascade dismiss behavior.

The key architectural fact: xdg-shell provides the _vocabulary_ for window management (what a toplevel is, what a popup is, how state is negotiated) but imposes no _policy_. Whether the compositor tiles, stacks, or does something entirely novel is not constrained by the protocol.

### 2.2 wlr-layer-shell (wlr ecosystem)

Created by Drew DeVault for the wlroots project. Enables clients to create surfaces that function as desktop shell components — panels, docks, wallpapers, notifications, overlays — rather than application windows.

Four layers, rendered bottom to top:
1. **Background** (z=0) — wallpapers, desktop backgrounds
2. **Bottom** (z=1) — docks, panels that appear below application windows
3. **Top** (z=2) — panels, notifications above application windows
4. **Overlay** (z=3) — topmost elements, fullscreen overlays

Surfaces anchor to screen edges (top, bottom, left, right, combinations thereof). They set an **exclusive zone** — a distance that tells the compositor to reserve space so application windows don't overlap. A panel anchored to the top with exclusive zone 32 means maximized windows leave the top 32 pixels free.

Not part of wayland-protocols (it's a wlr extension), but widely adopted. Smithay implements it. For pane-comp, layer-shell is the mechanism by which external clients (if any) can provide persistent shell UI, though pane's architecture renders most shell UI natively.

### 2.3 xdg-decoration (unstable)

Negotiates whether the compositor or the client draws window decorations (title bars, borders, close/minimize/maximize buttons).

The protocol allows the compositor to announce support for server-side decorations. A client creates a decoration object for its xdg_toplevel and requests a preferred mode (client-side or server-side). The compositor responds with a `configure` event specifying the effective mode — it can override the client's preference.

"If neither party negotiates decoration mode through this protocol, clients continue to self-decorate as they see fit."

For pane-comp: all panes get compositor-rendered chrome (tag line, borders) — this is a firm architectural commitment. xdg-decoration negotiation should default to server-side, and pane-comp should provide its own decorations for legacy Wayland clients.

### 2.4 Screen capture: wlr-screencopy and ext-image-copy-capture

**wlr-screencopy** (deprecated, but still widely used): lets clients request screen content captured into client-provided buffers. Supports full output capture and regional capture, optional cursor overlay, and damage tracking. Supports both wl_shm and DMA-BUF buffers. Being superseded by ext-image-copy-capture.

**ext-image-copy-capture** (staging): the standardized replacement. Allows clients to capture image sources (displays, toplevels) into client buffers. Workflow: create a capture session → receive buffer constraint events (formats, dimensions) → attach a buffer → request capture → receive metadata and ready event. Supports damage tracking, cursor capture as a separate session, transform metadata, and presentation timestamps.

Neither protocol has a built-in access control mechanism at the protocol level. Access control is left to the compositor — implementations typically use XDG Desktop Portal (a D-Bus service) as the permission gateway, prompting the user to authorize screen sharing.

### 2.5 Input method protocols

**zwp_input_method_v2** (experimental): enables IME (input method editor) applications to intercept and transform keyboard input for complex text composition (CJK languages, etc.).

Three interfaces:
- `zwp_input_method_v2`: the central object, active when text input is focused. Sends committed text, manages pre-edit (composition) strings with cursor positioning, deletes surrounding text, receives keyboard events via a grab.
- `zwp_input_popup_surface_v2`: surfaces positioned near the text cursor for candidate displays. Compositor handles placement.
- `zwp_input_method_keyboard_grab_v2`: exclusive keyboard access for the IME to process key events before the client.

Uses double-buffered state: modification requests accumulate and apply atomically on commit. The commit sequence: replace preedit with cursor, delete surrounding text, insert commit string, calculate updated surrounding context, insert new preedit, position cursor.

For pane-comp: IME support is essential. The compositor must integrate `zwp_input_method_v2` (or its successor) so that input methods work for both native pane clients and legacy Wayland clients. For native pane clients, the cell grid rendering and tag line editing need to participate in the IME protocol.

### 2.6 DMA-BUF (linux-dmabuf, stable)

The protocol for zero-copy GPU buffer sharing between clients and the compositor. Eliminates the CPU round-trip of shared memory: "allows EGL users to transfer handles to their GPU buffers from the client to the compositor for rendering, without ever copying data to the CPU."

Buffer creation: client calls `create_params`, adds DMA-BUF planes (fd, index, offset, stride, modifier), then finalizes with `create` (async) or `create_immed` (sync). The compositor imports the handles and can texture from them directly.

**Format negotiation**: the compositor advertises supported format+modifier pairs. **Modifiers** encode hardware-specific GPU memory layout (tiling, compression). Version 5 requires all planes to use identical modifiers.

**Feedback mechanism** (version 4+): per-surface feedback tells clients which formats/modifiers are optimal for the compositor's rendering path. Feedback is organized as "preference tranches" ranked by optimization priority, including scanout hints (formats that allow direct display without compositor copy).

For pane-comp: DMA-BUF is essential for legacy Wayland clients doing GPU rendering (Firefox, games, video players). The compositor must implement linux-dmabuf to accept GPU-rendered buffers without CPU copying. Native pane clients render their own content via the Interface Kit, which handles buffer management internally — DMA-BUF negotiation is an implementation detail hidden by the kit.

### 2.7 Viewporter (stable)

Decouples buffer size from surface size, enabling cropping and scaling.

**Source rectangle**: defines which part of the buffer is used. Content outside is ignored.
**Destination size**: defines the surface's final size on screen. Content is scaled to fit.

Cropping without scaling: set source but not destination (surface size = source rectangle size). Scaling: set destination to a different size than source. The transformation order: buffer transform → buffer scale → crop and scale. Changes are double-buffered, applied on commit.

### 2.8 Fractional scale (staging)

Enables non-integer display scaling (1.25x, 1.5x, 1.75x). The compositor sends a `preferred_scale` event expressing the scale as a fraction with denominator 120 (e.g., 150 = 1.25x). Clients render at the scaled resolution and use viewporter to set the surface destination to the unscaled size.

Example: a 100x50 surface at 1.5x scale renders a 150x75 buffer, then uses `wp_viewport` to set destination 100x50. The compositor composites at the buffer's native resolution, avoiding scaling artifacts.

The `wl_surface.set_buffer_scale` remains 1 when using fractional scale — the viewporter handles the scaling math instead.

### 2.9 Presentation-time (stable)

Provides frame timing feedback for precise synchronization (critical for video playback and animation).

Clients request feedback by creating a `wp_presentation_feedback` object for a surface. After the compositor presents the frame, it reports: exact timestamp (tv_sec, tv_nsec) of when the content appeared on display, nanoseconds until next refresh, a vertical retrace counter, and quality flags (vsync, hardware clock, zero-copy, hardware completion). If the update was superseded before display, a `discarded` event fires instead.

The compositor defines a presentation clock that clients can query. Recommended precision: one millisecond or better.

### 2.10 Other notable extensions

**ext-session-lock** (staging): secure screen locking. The lock client gets exclusive rendering on all outputs; normal client rendering stops entirely. If the lock client crashes, the session stays locked. Compositor must blank outputs between lock and lock surface display.

**ext-idle-notify** (staging): notification when the user has been idle for a configurable duration. For screen savers, power management.

**keyboard-shortcuts-inhibit** (unstable): lets clients request that compositor shortcuts be suppressed for a surface/seat (for VMs, remote desktop, full-screen games). Compositor retains authority to keep critical shortcuts.

**pointer-constraints** (unstable): lock pointer to a region or confine it to a surface. Essential for games and 3D applications.

**relative-pointer** (unstable): raw, unaccelerated pointer deltas. Essential for FPS games and 3D modeling.

**ext-foreign-toplevel-list** (staging): lets privileged clients (taskbars, window switchers) enumerate all toplevel windows. Read-only — no management, just listing.

**wlr-foreign-toplevel-management** (wlr): extends foreign-toplevel-list with management (minimize, maximize, close, activate). Not standardized, but widely used.

**wlr-output-management** (wlr): lets clients configure outputs (resolution, position, scale, transform). Used by display configuration tools.

**ext-data-control** (staging): lets privileged clients (clipboard managers) access and manage the clipboard across all clients. Addresses the "clipboard dies when source app exits" problem.

**wp-content-type-hint** (staging): clients hint what kind of content they're displaying (none, photo, video, game) so the compositor can optimize output settings.

**xwayland-shell** (staging): helps identify XWayland windows and associate them with wl_surfaces.

**ext-color-management** (staging, new): standard color management for HDR, wide gamut, and color-accurate workflows.

---

## 3. Compositor Architecture

### 3.1 What a Wayland compositor does

A Wayland compositor is simultaneously:
1. A **Wayland protocol server** — accepts client connections, handles requests, sends events
2. A **window manager** — decides where windows go, handles focus, stacking, tiling
3. A **display server** — configures outputs via DRM/KMS, presents frames via page flipping
4. An **input dispatcher** — receives events from libinput, routes them to focused clients
5. A **graphics compositor** — takes all client buffers and shell UI, composites them into a final image

In X11, these roles were split across the X server, the window manager, and the compositor. Wayland unifies them. This is both the strength (no intermediary, no round-trips, no coordination overhead) and the burden (every compositor must implement everything).

### 3.2 The rendering pipeline

The rendering pipeline for a Wayland compositor on Linux:

**Display hardware → DRM/KMS → Compositor → GPU/Mesa → Client**

The data flow for each frame:

1. **Clients render** into buffers (shared memory, EGL/Vulkan → DMA-BUF)
2. **Clients commit** surfaces, transferring buffer ownership to the compositor
3. **Compositor composites**: reads all client buffers, renders shell UI, and produces a final framebuffer
4. **Compositor submits** the framebuffer to DRM/KMS for display
5. **KMS page flips** — the kernel atomically switches to the new framebuffer on the next vblank
6. **Compositor releases** old client buffers (wl_buffer.release events)
7. **Compositor sends frame callbacks** to clients, signaling they can start the next frame

Compositing can be done via:
- **GL/GLES**: compositor binds client buffers as textures, draws them to an output framebuffer using shaders
- **Vulkan**: similar approach using Vulkan render passes
- **Pixman**: software compositing for fallback/testing (no GPU required)
- **Direct scanout**: if a single client covers an entire output with a compatible buffer, the compositor can skip compositing entirely and present the client buffer directly to the display hardware — zero-copy, zero-GPU-work

### 3.3 DRM/KMS (kernel mode setting)

The kernel subsystem for display configuration and framebuffer presentation. The compositor talks to DRM/KMS via ioctls on `/dev/dri/card*` device nodes.

The display pipeline is an object hierarchy:
- **Framebuffers**: hold pixel data
- **Planes**: feed framebuffers into display hardware (primary, overlay, cursor plane types)
- **CRTCs**: blend planes and generate video signals with timing
- **Encoders**: convert CRTC output for connector types (HDMI, DP, etc.)
- **Connectors**: physical display ports

The documentation explains: "The basic object structure KMS presents to userspace is fairly simple. Framebuffers feed into planes...feed their pixel data into a CRTC for blending."

Atomic modesetting (the modern API): compositors submit complete display configurations as a single atomic commit. Either the entire configuration is applied, or it's rejected — no partial updates. This prevents visual artifacts and simplifies error handling. Compositors can test configurations without applying them (test-only commits).

Page flipping: the compositor submits a new framebuffer to a CRTC, and the kernel schedules the switch on the next vblank. The compositor receives a page flip completion event and can then start compositing the next frame. This is the vsync mechanism — frames are presented aligned to the display's refresh cycle.

### 3.4 GBM and buffer allocation

**GBM (Generic Buffer Management)** is the userspace API for allocating GPU-compatible buffers. Compositors use GBM to allocate framebuffers for compositing output and to interact with DMA-BUF client buffers. GBM is backed by the Mesa GPU driver stack.

GBM is supported by all major GPU drivers (including NVIDIA >= 495). It's the dominant buffer allocation API for Wayland compositors. EGLStreams (NVIDIA's proprietary alternative) has been deprecated — KDE dropped support after GBM became available on NVIDIA hardware.

### 3.5 Input handling

The compositor receives input from the kernel via **libinput**, which:
- Discovers input devices via udev
- Reads raw events from `/dev/input/event*` (evdev)
- Applies pointer acceleration, scroll processing, gesture recognition, touchpad configuration
- Normalizes events across diverse hardware

The Wayland architecture page notes that the compositor's security role here is important: "The Wayland compositor requires special permissions to use the evdev files, forcing Wayland clients to go through the compositor to receive input events — which, for example, prevents keylogging."

The compositor maintains a **scenegraph** of all surfaces and their positions. When an input event arrives:
1. For pointer events: hit-test the pointer position against the scenegraph to find the target surface
2. Transform coordinates from compositor-global to surface-local (accounting for all transforms, scale, position)
3. Send Wayland input events to the target client
4. Manage focus: keyboard focus follows compositor policy (click-to-focus, focus-follows-mouse, etc.)

Keyboard handling uses **xkbcommon** for keymap processing. The compositor distributes the keymap to each client as a file descriptor (memory-mapped XKB keymap). Clients interpret keycodes using this keymap. When the user switches keyboard layouts, the compositor sends an updated keymap.

**Event serials**: input events carry monotonically increasing serial numbers. Clients must provide a recent serial when making privileged requests (interactive move/resize, popup creation, set_cursor). This prevents spoofed or stale input from triggering privileged operations.

**Input frames**: related input events from a single device interaction are grouped and concluded with a `frame` event. Clients buffer events within a frame and process them as a unit.

### 3.6 Multi-monitor

The compositor manages multiple displays via:
- **wl_output**: advertises display geometry, modes, scale, and transform to clients
- **DRM/KMS**: each connector/CRTC pair represents a physical display
- **Layout**: the compositor defines the spatial arrangement of outputs (side-by-side, above-below, mirrored)

Surfaces can span outputs or move between them. The compositor sends `wl_surface.enter`/`leave` events as surfaces move across outputs, allowing clients to adapt rendering (e.g., re-render at a different scale for a different DPI display).

Output management (configuration of resolution, position, scale, transform) is handled by protocol extensions — typically `wlr-output-management` or KDE's equivalent. No standard exists in wayland-protocols stable yet.

### 3.7 How smithay structures these concerns

Smithay is a Rust library providing composable building blocks for Wayland compositors. It does not impose a compositor design — it provides modules that compositor authors select and integrate. Key abstractions:

**Backend module** — OS interaction:
- `drm`: display pipeline management, buffer submission, mode setting
- `libinput`: input device discovery and event handling via libinput bindings
- `winit`: run-as-window backend for development
- `x11`: run-as-X11-client backend for development
- `egl`: OpenGL context creation for GPU rendering
- `allocator`: buffer allocation traits with GBM implementation
- `renderer`: rendering traits with GLES2 implementation
- `session`: login session management (logind/seatd) and VT switching
- `udev`: device discovery

**Wayland module** — protocol implementations:
- `compositor`: wl_surface and wl_subsurface handling
- `shell::xdg`: xdg-shell (toplevel, popup, positioner)
- `shell::wlr_layer`: layer-shell
- `seat`: input handling, focus management
- `shm`: shared memory buffer support
- `dmabuf`: linux-dmabuf support
- `data_device`: clipboard and drag-and-drop
- `output`: wl_output management
- Plus 30+ additional protocol implementations (text input, pointer constraints, tablet, session lock, etc.)

Each protocol implementation follows a pattern: a `*State` struct that registers globals, a `*Handler` trait for compositor-specific behavior, and `delegate_*!` macros for wiring. The compositor author implements the handler traits and smithay handles protocol dispatch.

**Desktop module** — higher-level window management helpers.

**Event loop**: smithay is built on **calloop**, a callback-oriented event loop. All I/O sources (Wayland client connections, DRM events, libinput events, timers) are integrated into a single event loop. The smithay documentation notes: "Using a callback-heavy structure...allows you to provide a mutable reference to a value that will be passed down to most callbacks." This enables centralized state management without heavy synchronization.

For pane-comp: smithay provides the Wayland protocol handling, DRM backend, input integration, and rendering infrastructure. Pane-comp adds: the pane protocol server (session-typed connections for native clients), chrome rendering (tag lines, borders, focus indicators), layout tree management, and input routing. The Interface Kit (client-side library) adds: cell grid rendering (GPU text rasterization, glyph atlas), widget rendering, and the pane visual language. Smithay handles the "be a Wayland compositor" part; pane-comp adds the "be the pane desktop" part; the Interface Kit provides the "build a pane application" programming model.

---

## 4. Client-Compositor Relationship

### 4.1 Connection and bootstrapping

A client connects to the compositor via a Unix domain socket (typically `$XDG_RUNTIME_DIR/wayland-0`). The connection gives the client `wl_display` (ID 1). The client calls `wl_display.get_registry` to obtain `wl_registry`, which immediately emits `global` events for every available global object.

The client scans the globals, binds to the ones it needs (`wl_compositor`, `wl_shm`, `wl_seat`, `xdg_wm_base`, etc.), and begins creating surfaces.

### 4.2 Creating a window (xdg-shell toplevel)

1. `wl_compositor.create_surface` → get a `wl_surface`
2. `xdg_wm_base.get_xdg_surface(wl_surface)` → get an `xdg_surface`
3. `xdg_surface.get_toplevel` → get an `xdg_toplevel`
4. Set title, app_id, min/max size on the toplevel
5. `wl_surface.commit` — this initial commit signals that the surface is ready to receive configure events
6. Wait for `xdg_toplevel.configure` + `xdg_surface.configure` events
7. `xdg_surface.ack_configure(serial)` — acknowledge the configuration
8. Create a buffer matching the configured size
9. Write pixels
10. `wl_surface.attach(buffer)`, `wl_surface.damage_buffer(...)`, `wl_surface.commit`

The first commit with actual content displays the window.

### 4.3 Frame callbacks (client pacing)

The frame callback mechanism prevents clients from rendering faster than the compositor can display. A client calls `wl_surface.frame` to request a `wl_callback`. The compositor fires the callback's `done` event when it's ready for a new frame — typically after presenting the previous frame and starting the next composition cycle.

The client render loop:
1. Request frame callback
2. Commit surface
3. Wait for frame callback
4. Render new content
5. Request next frame callback
6. Commit surface
7. (repeat)

This naturally throttles to the display refresh rate without busy-waiting. If the compositor can't keep up, callbacks arrive slower. If the client is slow, it just misses frames.

The callback also carries a timestamp (milliseconds since an arbitrary epoch), which clients can use for animation timing.

### 4.4 Input event dispatch

Input events flow from the compositor to the client that has focus:

**Pointer events**: `wl_pointer.enter` with surface-local coordinates when the pointer enters a client surface. Subsequently: `motion` (coordinates), `button` (press/release with Linux input event codes), `axis` (scroll), grouped by `frame` events. `leave` when the pointer exits. Coordinates are in the surface's local coordinate system — the compositor handles the global-to-local transform.

**Keyboard events**: `wl_keyboard.enter` with an array of currently pressed keys when a surface gains keyboard focus. Subsequently: `key` (scancode + state), `modifiers` (depressed, latched, locked modifier masks + group). `leave` on focus loss. The compositor sends the xkb keymap via fd on keyboard bind and whenever the keymap changes.

Key detail: "The scancode from this event is the Linux evdev scancode. To translate this to an XKB scancode, you must add 8 to the evdev scancode."

**Key repeat**: the compositor sends `repeat_info` with delay (ms before repeat starts) and rate (repeats/second). The _client_ implements key repeat — the compositor does not send repeated key events. This is a departure from X11 where the server generated key repeats.

**Implicit grab**: when a pointer button is pressed, the compositor maintains a grab on the surface that received the press event. All subsequent motion and the button release go to that same surface, even if the pointer moves outside it. This prevents broken interactions where a button-up lands on a different surface than the button-down.

### 4.5 Popup and subsurface management

**Popups** (xdg_popup) are positioned relative to their parent using a **positioner** that defines:
- Anchor rectangle on the parent surface
- Anchor point within the rectangle
- Gravity (which direction the popup expands from the anchor)
- Constraint adjustment (what happens when the popup would go off-screen: slide, flip, resize)
- Offset from the anchor

Popups can grab input — an exclusive grab means all input goes to the popup until it's dismissed. Popups form a stack: the topmost popup gets the grab, and destroying it passes the grab to the parent popup.

**Subsurfaces** (wl_subsurface) are child surfaces within a parent surface's coordinate space. They have:
- A position relative to the parent
- Z-ordering (above or below the parent, above or below siblings)
- Synchronization mode: **sync** (child commits only take effect when parent commits) or **desync** (child commits independently)

Subsurfaces are used for video overlays (video in a subsurface, UI in the main surface), embedded compositor content, and complex widget hierarchies.

### 4.6 The client rendering model

Wayland imposes one constraint on clients: produce a buffer of pixels. How the client renders those pixels is entirely its business:

- **CPU rendering** (Cairo, Skia, direct pixel manipulation) → shared memory buffers via wl_shm
- **OpenGL/GLES** (via EGL) → DMA-BUF handles via linux-dmabuf
- **Vulkan** (via VK_KHR_wayland_surface) → DMA-BUF handles via linux-dmabuf
- **Software fallback** → shared memory

The compositor doesn't know or care how the client rendered. It receives a buffer handle and composites from it. This complete freedom means clients can use any rendering technology — but it also means the compositor has no semantic understanding of client content. A buffer is just pixels.

Pane embraces this model: each pane renders its own content. The Interface Kit provides shared rendering infrastructure (glyph atlas, layout primitives, the pane visual language) so native clients produce visually consistent output without the compositor needing to render on their behalf. The compositor composites client buffers and renders chrome (borders, tag lines, focus indicators). Visual consistency comes from the kits, not from centralized rendering.

---

## 5. XWayland

### 5.1 How XWayland works

XWayland is a complete X11 server implementation that acts as a Wayland client. It bridges X11 applications into the Wayland world. The Wayland documentation states: "Xwayland is a complete X11 server, just like Xorg is, but instead of driving the displays and opening input devices, it acts as a Wayland client."

Architecture: XWayland presents an X11 socket to X11 applications and a Wayland connection to the compositor. X11 applications connect to XWayland thinking it's a normal X server. XWayland renders their content into buffers, creates Wayland surfaces for each X11 window, and presents those surfaces to the compositor.

### 5.2 Rootless vs rootful mode

**Rootful mode**: all X11 applications live inside a single Wayland surface — a virtual X11 desktop. A traditional X11 window manager can run inside this container. The downside: X11 windows can't intermingle with Wayland windows in the compositor's stacking order.

**Rootless mode**: each X11 top-level window gets its own Wayland surface. X11 windows seamlessly intermix with native Wayland windows. But the compositor must act as the X11 window manager (via XWM — XWayland Window Manager), because X11 window managers can't know about Wayland windows.

Rootless mode is the standard for desktop use. The compositor runs XWM to bridge X11 window management state (title, class, size hints, transient relationships) to its Wayland window management.

### 5.3 Window identification

The critical technical challenge: matching X11 windows to Wayland surfaces. XWayland creates a wl_surface for each X11 window, but the surfaces arrive on the Wayland connection independently of the X11 window creation.

The original mechanism: XWayland sends an X11 `ClientMessage` of type `WL_SURFACE_ID` to the X11 window, carrying the wl_surface's Wayland object ID. The compositor (via XWM) sees this message and correlates the X11 window with the Wayland surface.

The newer mechanism: `xwayland-shell` protocol (staging) provides a more robust way to associate X11 windows with wl_surfaces.

The surfaces may arrive asynchronously in any order, requiring the compositor to handle both "X11 window arrives first" and "Wayland surface arrives first" cases.

### 5.4 Limitations and impedance mismatches

The Wayland documentation acknowledges: "Xwayland compatibility compared to a native X server will probably never reach 100%."

Key impedance mismatches:

**Security model**: "Xwayland is an X server, so it does not have the security features of Wayland." All X11 clients within XWayland share the X11 security model — they can read each other's windows, snoop input, etc. The Wayland isolation boundary is between XWayland-as-a-whole and native Wayland clients, not between individual X11 applications.

**Global coordinates**: X11 applications expect global screen coordinates. XWayland provides them (it knows where the compositor placed each surface, via wl_surface.enter/configure), but the abstraction is imperfect — coordinate updates may lag, and some X11 tricks that rely on global coordinate knowledge don't work.

**Scaling**: X11 has no concept of fractional scaling. XWayland can scale up X11 content (the compositor renders the surface at 1x and scales it up, leading to blurriness), or X11 applications can be made DPI-aware with environment variables, but it's not seamless.

**Input**: Key repeat, pointer warping, and input method integration work differently. Some X11 input protocols (XInput2 features, pointer barriers) may not fully translate.

**Window management**: X11 applications expect ICCCM/EWMH semantics (WM_TRANSIENT_FOR, _NET_WM_STATE, etc.). XWM translates these into xdg-shell operations, but the mapping is imperfect.

**Clipboard**: X11 clipboard (XCLIPBOARD, PRIMARY) is bridged to Wayland's wl_data_device, but the mechanisms are different and edge cases exist (e.g., INCR transfers, custom targets).

**Deadlock risk**: "The compositor becomes an X11 client of XWayland, requiring careful implementation to avoid deadlocks." The compositor talks to XWayland over both the Wayland connection (as server) and the X11 connection (as client/XWM), and both are asynchronous. The documentation strongly recommends making "all X11 communications asynchronous."

**What can't work at all**: X11 desktop environment components (traditional WMs, system trays expecting the EWMH system tray protocol, screen savers using XScreenSaver protocol) can't coexist with XWM. They don't know about Wayland windows and can't manage them.

For pane-comp: XWayland support is build sequence item 11 — late in the pipeline, after native functionality is solid. The compositor needs XWM integration (smithay provides utilities), rootless mode, and surface matching. Legacy X11 apps get a pane wrapper with compositor-rendered chrome, same as legacy Wayland apps.

---

## 6. The Broader Linux Graphics Stack

### 6.1 The full stack

From hardware to pixels-on-screen:

```
User Applications
  |-- Wayland clients (EGL/Vulkan -> DMA-BUF -> compositor)
  +-- Pane-native clients (render via Interface Kit -> buffers -> compositor composites)

Compositor (pane-comp / smithay)
  |-- Rendering: OpenGL/GLES via EGL, or Vulkan
  |-- Buffer management: GBM for allocation, DMA-BUF for sharing
  +-- Display: DRM/KMS for mode setting and page flipping

Mesa / GPU Driver Stack
  |-- EGL: context creation, buffer management
  |-- GLES/GL: rendering API
  |-- Vulkan: rendering API
  +-- GBM: buffer allocation API

Linux Kernel
  |-- DRM: Direct Rendering Manager (GPU access)
  |-- KMS: Kernel Mode Setting (display configuration)
  |-- evdev: input events from hardware
  +-- GPU driver: i915, amdgpu, nouveau, etc.

Hardware
  |-- GPU
  |-- Display (HDMI, DP, eDP)
  +-- Input devices (keyboard, mouse, touchpad, etc.)
```

### 6.2 DRM (Direct Rendering Manager)

The kernel subsystem providing userspace access to GPUs. Two functions:
- **GEM (Graphics Execution Manager)**: manages GPU memory objects (buffers)
- **KMS (Kernel Mode Setting)**: configures displays

Compositors interact with DRM via `/dev/dri/card*` for KMS (privileged: display control) and `/dev/dri/renderD*` for rendering (unprivileged: GPU compute/render). The login session manager (logind/seatd) mediates access to card nodes, granting the compositor access when it's the active session and revoking it on VT switch.

### 6.3 Mesa and GPU drivers

Mesa implements the userspace portion of the GPU driver stack:
- OpenGL/GLES drivers for each GPU (i965, radeonsi, iris, panfrost, etc.)
- Vulkan drivers (anv for Intel, radv for AMD, etc.)
- EGL for context management
- GBM for buffer allocation

Mesa is the interface between compositor rendering code and the GPU hardware. When smithay's GLES renderer draws a textured quad, Mesa translates that into GPU commands for the specific hardware.

### 6.4 GBM (Generic Buffer Management)

GBM allocates GPU-compatible buffers that can be used for both rendering and display. Key operations:
- Create a buffer object (BO) with a format, modifier, and usage flags
- Export as DMA-BUF fd for sharing
- Import DMA-BUF fd from a client
- Create a framebuffer from a BO for KMS scanout

GBM is the glue between the compositor's rendering output and KMS display: the compositor renders into a GBM buffer, wraps it as a DRM framebuffer, and submits it to KMS.

### 6.5 The relationship between compositor, GPU driver, and kernel

```
Compositor <-> Mesa (EGL/GLES/GBM) <-> GPU kernel driver <-> GPU hardware
    |                                        |
  DRM/KMS ioctls <------------------------> Display hardware
```

The compositor uses Mesa for rendering (compositing client buffers into a final image) and DRM/KMS for display (presenting the final image). Mesa and the kernel driver share GPU command submission and memory management. The compositor doesn't talk to the GPU directly — it goes through Mesa's abstraction.

For direct scanout, the path is shorter: client buffer (DMA-BUF) -> compositor imports -> submits directly to KMS as a plane, bypassing the compositing step entirely.

---

## 7. What Wayland Doesn't Provide

### 7.1 No built-in window management policy

The core protocol provides surfaces and buffers. xdg-shell provides the vocabulary for desktop windows (toplevel, popup, configure). But the compositor decides: where windows go, how they're stacked, whether they tile or float, how focus works, what decorations look like. There is no protocol-level equivalent of X11's EWMH that defines "how a desktop works."

This is by design: the compositor owns the policy. But it means every compositor reimplements window management from scratch. For pane-comp, this is a feature — pane's layout tree (recursive tiling with tag-based visibility) is compositor policy that the Wayland protocol doesn't constrain.

### 7.2 No global hotkeys protocol (standard)

In X11, any client could grab a key combination globally. Wayland's security model prevents this — a client only receives input when it has focus. There is no standardized protocol for registering global keyboard shortcuts.

The Hyprland project has an `ext-global-shortcuts` protocol, but it hasn't been adopted into wayland-protocols. The workaround is compositor-specific configuration (keybindings defined in compositor config) or D-Bus-based portal APIs (xdg-desktop-portal's GlobalShortcuts interface).

For pane-comp: key bindings are compositor-native (defined in config, processed by the compositor before any client dispatch). This is the right design — the compositor owns input dispatch and should own shortcut resolution. No protocol needed for pane-native clients; for legacy clients that expect global hotkeys, the answer is "that's not how Wayland works."

### 7.3 Screen recording still evolving

The `wlr-screencopy` protocol is deprecated. Its replacement, `ext-image-copy-capture`, is staging. Both require explicit compositor support and typically route through xdg-desktop-portal for access control. There's no equivalent of X11's `XCompositeGetOverlayWindow` or `XShmGetImage` that any client can call.

PipeWire + xdg-desktop-portal-wlr is the current practical solution for screen recording and screen sharing (used by OBS, browser WebRTC, etc.).

### 7.4 Network transparency is not a goal

X11's network transparency (any client can display on any server) was a core design feature. Wayland explicitly does not provide this. The protocol uses Unix domain sockets with fd passing — neither of which works over a network.

Remote display on Wayland requires external solutions: VNC/RDP servers (which are themselves Wayland clients using screencopy + virtual input), or something like waypipe (which proxies Wayland traffic over SSH, including fd-passing via serialization).

This is a real loss for environments that valued network transparency (Plan 9, NX, traditional X11 thin clients). For pane, which draws from Plan 9's distributed philosophy: the network transparency that matters is at pane's own protocol layer (session-typed conversations over Unix sockets, which _can_ be tunneled over the network), not at the display protocol layer. Pane-fs (FUSE at `/srv/pane/`) provides network-accessible state via 9P-style filesystem export. The display protocol is local; the semantic protocol can be remote.

### 7.5 Clipboard limitations

The Wayland clipboard (`wl_data_device`) has a fundamental architectural constraint: data transfers happen via file descriptors. The source client creates an fd, the destination reads from it. When the source application exits, the fd is gone. There is no clipboard daemon in the core protocol.

This means:
- Copy text in an app, close the app, paste fails (unless a clipboard manager is running)
- Large clipboard content is streamed, not buffered (good for large objects, bad for reliability)
- MIME type negotiation happens per-transfer (source offers types, destination picks one)
- No built-in primary selection (middle-click paste) — that's a separate protocol (`wp-primary-selection`, unstable)

The `ext-data-control` protocol (staging) addresses this by allowing clipboard manager applications to intercept and persist clipboard content. Practically, most desktop environments ship a clipboard manager daemon.

For pane-comp: implement wl_data_device for legacy client interop. Consider running a built-in clipboard buffer or integrating with a pane service that persists clipboard state (the clipboard is a natural fit for pane's filesystem interface — clipboard contents as files in `/srv/pane/clipboard/`).

### 7.6 No window inspection

In X11, any client could enumerate windows, read window properties, and query the window tree. Wayland provides no such mechanism — each client only knows about its own surfaces. This is a deliberate security improvement (no keyloggers, no window snooping) but means:
- Screen readers can't inspect window content via the display protocol
- Automation tools (xdotool-style) can't manipulate other clients' windows
- Taskbars need special protocols (ext-foreign-toplevel-list) to learn about other windows

### 7.7 No standard for desktop integration

X11 had freedesktop.org specifications for desktop integration: system tray, taskbar, desktop icons, notifications (via D-Bus), startup notification, clipboard management. Wayland has no equivalent standard suite. Each desktop environment implements its own integration, leading to fragmentation.

Layer-shell (wlr) addresses panels/wallpapers but isn't in wayland-protocols stable. Output management, foreign toplevel management, and other desktop-level concerns remain in wlr/KDE-specific protocols or staging.

---

## 8. Accessibility on Wayland

### 8.1 The current state

Linux desktop accessibility is built on **AT-SPI2** (Assistive Technology Service Provider Interface), which communicates over **D-Bus**. Toolkit libraries (GTK, Qt) expose UI element roles, values, labels, and actions via AT-SPI2. Screen readers (Orca) and other assistive tools consume this information.

AT-SPI2 is independent of the display protocol — it works over D-Bus regardless of whether the application runs on X11 or Wayland. The basic pipeline works: Orca can read GTK/Qt applications on Wayland.

### 8.2 What's broken or missing

The GNOME accessibility wiki documents several gaps:

**Mouse interaction**: "Mouse events cannot be synthesized via AT-SPI2" (Bug 709999). Orca's mouse routing and click synthesis don't work. "Mouse-moved events are not being emitted. This functionality was/is coming from X" (Bug 710012). Orca's mouse review is broken.

**Window hierarchy**: Getting the Z-order of top-level windows "in a libwnckless world" is unsolved. X11 provided global window hierarchy; Wayland doesn't expose this to assistive tools.

**On-screen keyboards**: Struggle with Wayland-native text input. The input method protocol provides a path forward but adoption is incomplete.

**Screen magnification**: Crashes when typing in Wayland text views (at least as of the documented state).

The fundamental problem: X11 gave assistive tools global knowledge — read any window, inspect any pixel, synthesize any input event. Wayland removes all of this for security reasons. AT-SPI2 provides an alternative path (toolkit-level accessibility tree via D-Bus), but that path is less complete than what X11 provided, and it requires every toolkit to correctly implement AT-SPI2 — there's no fallback.

### 8.3 What the compositor knows vs doesn't know

In standard Wayland, the compositor knows:
- Where surfaces are positioned (the scenegraph)
- Which surface has focus
- Input events (it dispatches them)
- Surface size and scale
- Window titles and app_ids (from xdg-shell)

The compositor does NOT know:
- What text is in a client's window
- What buttons or controls exist
- The semantic structure of client content
- Anything about client content beyond "here's a pixel buffer"

This is the fundamental accessibility gap: the compositor is the one entity that has global knowledge (all windows, all positions, all focus state), but it doesn't have semantic knowledge of content. The toolkit has semantic knowledge (it built the UI) but communicates it via a side channel (AT-SPI2/D-Bus), not through the compositor.

### 8.4 How pane's architecture changes this

Pane-native clients render their own content via the Interface Kit (client-side rendering, same as standard Wayland). But unlike standard Wayland clients, native pane clients also send semantic metadata through the pane protocol:
- **Cell grid text**: the compositor knows the characters in every cell, their attributes, and their positions — not from rendering them, but from the protocol.
- **Widget structure**: semantic elements (buttons, labels, lists, text inputs) with roles, values, and actions are described in the protocol alongside the rendered output.
- **Tag line content**: always compositor-known, since the compositor renders the tag line chrome.

The compositor can directly answer accessibility queries about native pane content:
- "What text is at position (x, y)?" → look up the cell grid metadata from the pane protocol
- "What widget is at position (x, y)?" → traverse the widget tree from the pane protocol
- "What are the actions available on this element?" → the widget's session type defines them

This eliminates the AT-SPI2 indirection for native clients. The compositor has the accessibility tree because the pane protocol carries it — the semantic metadata travels alongside the rendered buffers, giving the compositor knowledge that standard Wayland compositors structurally lack. For legacy Wayland clients, the standard AT-SPI2/D-Bus path remains the only option — the compositor only has their pixel buffers.

The implication: pane-comp should expose an accessibility interface that unifies the native semantic model (from the pane protocol) with the AT-SPI2 model (for legacy clients), presenting a consistent view to screen readers and assistive tools.

Cell grid accessibility remains a research challenge — a grid of characters doesn't inherently convey structure (where's the prompt? where's the output? what's a URL?). Terminal semantics need to be layered on top.

---

## 9. Implications for Pane

### 9.1 Client-side rendering with shared infrastructure

Pane embraces the Wayland rendering model: each pane renders its own content into buffers, and the compositor composites those buffers together with its own chrome (borders, tag lines, focus indicators) for final display output.

Visual consistency across native pane clients comes from the **kits**, not from the compositor rendering on behalf of clients. The Interface Kit is a substantial UI programming model — comparable to BeOS's Interface Kit — that provides shared rendering infrastructure: glyph atlas, layout primitives, the pane visual language (beveled borders, Frutiger Aero aesthetic). A client that uses the Interface Kit produces output that looks like a pane application because the kit encodes the visual language, not because the compositor drew the pixels.

This is the standard Wayland relationship (client renders, compositor composites) with a twist: the kits ensure that "client renders" produces coherent results across the desktop, without requiring the compositor to understand or intervene in client content.

The compositor's rendering responsibilities are limited to:
- Compositing all client buffers (native and legacy) into the output framebuffer
- Rendering chrome: tag lines, borders, focus indicators, split handles
- Layout: positioning pane buffers according to the layout tree
- Presenting the final framebuffer to DRM/KMS

### 9.2 The pane protocol harmonizes, not replaces

The pane protocol does not replace Wayland for legacy apps. It provides the mechanism by which native pane clients and legacy Wayland clients coexist coherently in the UX.

**The Wayland protocol** — the standard display protocol. All clients can speak it. Legacy clients (GTK, Qt, Electron apps) connect via the Wayland socket, render their own pixels, and the compositor composites them. Pane-comp must implement this fully (via smithay) to be a functional Linux compositor.

**The pane protocol** — the native session-typed protocol that enables full pane integration. Native clients speak it to get pane-specific features (tag lines, routing, filesystem exposure, session-typed interactions). The protocol coordinates with Wayland rather than bypassing it — the Interface Kit uses Wayland surfaces and buffers under the hood. The protocol is an implementation detail inside the kit.

The compositor harmonizes these two worlds:
- Legacy Wayland clients produce pixel buffers → the compositor composites them, adds chrome (tag line, borders)
- Pane-native clients render via the Interface Kit → the compositor composites their buffers identically, adds chrome

The output path converges: regardless of whether content came from a legacy Wayland client or a native pane client using the Interface Kit, it ends up as a buffer in the same DRM/KMS framebuffer submission. The input path also converges: the compositor receives input from libinput and routes it to either Wayland clients (via wl_seat events) or pane clients (via pane protocol input events), depending on which pane is focused.

### 9.3 Kits are the programming model

The Interface Kit is not a thin wrapper around the pane protocol. It is a substantial UI programming model — the thing a developer actually programs against to build a pane application.

What the Interface Kit provides:
- Shared rendering infrastructure (glyph atlas, GPU-accelerated text, layout primitives)
- The pane visual language (so applications look right without per-app effort)
- Cell grid rendering for terminal-style content
- Widget tree rendering for structured UI
- Input handling (keyboard, pointer, focus)
- Frame pacing and buffer management
- Accessibility metadata emission

What the pane protocol provides (hidden inside the kit):
- Session-typed connection to the compositor
- Tag line content and routing
- Filesystem exposure coordination
- Frame timing signals

A developer building a pane application uses the Interface Kit. They do not speak the pane protocol directly, any more than a macOS developer speaks Mach messages to WindowServer. The kit is the API; the protocol is plumbing.

### 9.4 The two client classes

**Pane-native clients**:
- Program against the Interface Kit
- Render their own content via kit-provided infrastructure
- Speak the pane protocol (as an implementation detail inside the kit)
- Multiple panes per connection (sub-sessions)
- Get full pane features: tag line (editable, executable text), routing, filesystem exposure
- Visual consistency comes from the shared kit, not compositor intervention
- Frame pacing: compositor signals readiness, kit manages buffer lifecycle

**Legacy Wayland clients**:
- Connect via the standard Wayland socket (`$XDG_RUNTIME_DIR/wayland-0`)
- Speak xdg-shell, wl_shm, linux-dmabuf, etc.
- One surface tree per toplevel
- Render their own pixels into buffers
- Get a pane wrapper: compositor-rendered tag line and borders (chrome), but the body is their opaque surface
- Standard Wayland frame callbacks for pacing

**Legacy X11 clients (via XWayland)**:
- Connect to XWayland's X11 socket
- XWayland translates to Wayland surfaces
- Get the same pane wrapper as legacy Wayland clients
- Additional impedance mismatches (scaling, input, security boundary)

### 9.5 What pane-comp needs from smithay

Smithay provides the infrastructure for the "be a Wayland compositor" half:

**Must use**:
- DRM backend for display output
- Libinput for input
- wl_compositor, wl_surface, wl_subsurface
- xdg-shell (toplevel, popup)
- wl_seat (keyboard, pointer, touch)
- wl_shm for shared-memory buffers
- linux-dmabuf for GPU buffers
- wl_data_device for clipboard/DnD
- wl_output for display advertisement
- xdg-decoration (server-side decorations for legacy clients)
- calloop event loop integration

**Should use**:
- wlr-layer-shell (for any external shell components)
- Fractional-scale and viewporter (HiDPI)
- Presentation-time (precise frame timing)
- Input method protocols (IME)
- Pointer constraints and relative-pointer (games)
- Session lock (screen locking)
- ext-image-copy-capture (screen capture/sharing)
- XWayland integration (later phase)

**Pane-comp adds on top**:
- Pane protocol server (Unix socket, session types, kit coordination)
- Chrome rendering (tag lines, beveled borders, focus indicators, split handles)
- Layout tree (recursive tiling, tag-based visibility)
- Input routing (key bindings, focus policy, tag switching)
- Connection to pane-route (for tag line route actions on legacy panes)

**The Interface Kit provides (client-side)**:
- Cell grid renderer (GPU-accelerated text: glyph atlas, instanced rendering)
- Widget tree renderer (layout, styling)
- The pane visual language (shared aesthetics)
- Pane protocol client implementation (hidden from the developer)
- Buffer management and frame pacing

### 9.6 Alignment and divergence with Wayland's assumptions

**Where pane aligns with Wayland:**
- Client-side rendering: Wayland's model is that clients render their own content. Pane embraces this — the Interface Kit renders on the client side. The compositor composites, it does not render on behalf of clients.
- Compositor-as-authority: the compositor owns display, input, and policy. Pane's chrome rendering (tag lines, borders) and layout tree are compositor policy — exactly where Wayland expects this to live.
- Security through isolation: Wayland clients can't inspect each other's content. Pane's per-pane session types enforce protocol-level isolation on top of this.
- No built-in policy: Wayland doesn't dictate window management. Pane's layout tree, tag system, and chrome are compositor policy, unconstrained by the protocol.
- Double-buffered atomicity: Wayland's pending/current state model matches pane's desire for "every frame is perfect."

**Where pane diverges from Wayland's assumptions:**
- **Visual consistency through kits**: Wayland assumes each client chooses its own toolkit and visual language. Pane provides the Interface Kit as the native toolkit, encoding a shared visual language. Clients are free to render however they want (it's still client-side rendering), but the kit makes it natural to produce coherent output. This is how BeOS achieved visual consistency — not by centralizing rendering, but by providing a good enough kit that developers used it.
- **Protocol relationship**: Wayland assumes one protocol between compositor and clients. Pane adds a second protocol for native integration (session-typed, providing tag lines, routing, filesystem exposure). But the pane protocol harmonizes with Wayland rather than replacing it — the Interface Kit uses Wayland surfaces under the hood.
- **Content semantics**: for native clients, the compositor has semantic knowledge because the pane protocol carries it (tag line content, widget structure, cell grid text). This enables accessibility, routing, and integration that Wayland's opaque-buffer model structurally cannot provide. But this semantic richness flows through the protocol, not through the compositor rendering client content.
- **Session types**: Wayland's protocol is typed (interfaces, requests, events with signatures) but not session-typed — there's no compile-time guarantee that the conversation follows a valid sequence. Pane's native protocol is session-typed, with the compiler enforcing protocol correctness.
- **Network aspirations**: Wayland is explicitly local. Pane's pane protocol, being over Unix sockets with session types, can in principle be tunneled over the network for remote semantic access (not remote rendering — remote content description and state access). The filesystem interface (`/srv/pane/`) provides a separate network path via 9P or NFS export.

### 9.7 The critical integration points

**Frame timing**: the compositor must coordinate frame timing across all client buffers — both native (rendered by the Interface Kit) and legacy (rendered by arbitrary toolkits) — plus its own chrome rendering. All content must be composited in the same frame and submitted to KMS together. The calloop event loop drives this: DRM vblank events trigger composition.

**Input dispatch**: a single input event pipeline (libinput -> compositor) must route to either a pane-native client or a Wayland client depending on focus. The compositor's layout tree determines focus — the focused pane might contain a legacy Wayland surface or a native pane surface. Input goes to whichever protocol the focused pane speaks.

**Damage tracking**: for all clients (native and legacy), the compositor uses reported surface damage to optimize compositing. The Interface Kit reports damage through standard Wayland surface damage. The compositor additionally tracks its own chrome damage (tag line text changes, focus indicator moves). All damage feeds into the same output damage calculation for KMS.

**Buffer management**: all clients provide buffers via standard Wayland mechanisms (wl_shm or linux-dmabuf). The Interface Kit manages buffer allocation and lifecycle for native clients, hiding the details. The compositor handles all buffers uniformly, importing them as GPU textures for compositing.

**Clipboard**: wl_data_device handles clipboard for all clients. Native pane clients participate through the Interface Kit, which implements wl_data_device internally. Cross-world clipboard (copy from a native pane, paste in Firefox) works through standard Wayland clipboard because both sides speak wl_data_device. The filesystem interface (`/srv/pane/clipboard/`) can additionally persist clipboard state, surviving application exit.

### 9.8 What Wayland's gaps mean for pane

Several of Wayland's gaps are actually opportunities for pane:

**No window management policy** -> pane's layout tree is unconstrained. The tag-based tiling system with recursive splits is pure compositor policy. No protocol fights it.

**No global hotkeys** -> pane-comp owns all key bindings. They're compositor-level, configured via filesystem-as-config. No protocol negotiation needed.

**Poor accessibility** -> pane-native clients emit semantic metadata through the pane protocol (tag line content, cell grid text, widget structure). The compositor knows the semantic structure of native content, unlike standard Wayland compositors that only see pixel buffers. This enables a compositor-native accessibility provider for pane content, unified with the AT-SPI2 model for legacy clients.

**No network transparency** -> pane's semantic protocol layer provides its own remote story. The loss of display-level network transparency (X11-style) is compensated by state-level network access (filesystem interface, protocol tunneling). The right things to make remote are the semantic objects (pane state, file content, commands), not the pixels.

**Clipboard dies with source app** -> pane's filesystem interface can persist clipboard state. Content copied into `/srv/pane/clipboard/` survives application exit.

**No standard desktop integration** -> pane doesn't need freedesktop desktop integration protocols. The desktop IS the compositor. Tag lines replace taskbars. Routing replaces application associations. The filesystem replaces desktop files. Pane builds its own integration layer from its own principles, using Wayland for what Wayland is good at — the client/compositor buffer dance — and the kits for what the kits are good at — a coherent programming model and visual language.

---

## Sources

- Wayland Protocol book: https://wayland-book.com/
- Wayland documentation: https://wayland.freedesktop.org/docs/html/
- Wayland architecture: https://wayland.freedesktop.org/architecture.html
- Wayland protocol explorer: https://wayland.app/protocols/
- Smithay GitHub: https://github.com/Smithay/smithay
- Smithay docs: https://docs.rs/smithay/latest/smithay/
- Linux kernel DRM/KMS docs: https://kernel.org/doc/html/latest/gpu/drm-kms.html
- Arch Wiki — Wayland: https://wiki.archlinux.org/title/Wayland
- GNOME Accessibility/Wayland: https://wiki.gnome.org/Accessibility/Wayland
