# Phase 4: Minimal Compositor Implementation Plan

**Date:** 2026-03-27
**Author:** Be Systems Engineer (consultant)
**Milestone:** pane-shell running inside pane-comp — a pane-native terminal emulator connected to the compositor over the pane protocol, rendering in a pane with a tag line, processing keyboard input, running a real shell.

---

## Current State

**What exists:**
- `pane-proto` — wire types, protocol enums (`ClientToComp`, `CompToClient`), handshake types, event types, tag vocabulary types. Solid.
- `pane-session` — typestate `Chan<S, T>`, `UnixTransport`, `MemoryTransport`, `SessionSource` (calloop integration), length-prefixed framing. Reviewed and correct.
- `pane-app` — `App`, `Pane`, `PaneProxy`, `Handler`, `FilterChain`, `Tag` builder, routing stubs, scripting stubs. Running against `MockCompositor` in tests. API reviewed and approved.
- `pane-comp` — demo-only. Hardcoded single pane rendered with smithay's winit backend. Glyph atlas (cosmic-text), beveled borders, tag line background, no protocol server, no input handling, no calloop event loop. Not in the workspace (Linux-only).

**What exists but is stale:**
- `pane-comp`'s `PaneRenderer` renders from a hardcoded `CellRegion`. No dynamic content.
- `pane-comp`'s main loop is a raw `while running` + `thread::sleep(16ms)`. No calloop. No fd polling.
- The glyph atlas rasterizes glyphs but never uploads them to GPU as textures — cell backgrounds render but text doesn't.

**What doesn't exist:**
- Compositor calloop event loop
- Pane protocol server (unix socket listener)
- Session-typed handshake on the compositor side
- Per-pane server-side threads
- Layout tree
- Input handling (libinput, xkbcommon)
- Chrome rendering driven by protocol state
- pane-shell (PTY bridge)

---

## Architecture Mapping

The spec defines a three-tier threading model for the compositor, mapped from Haiku's app_server:

| Haiku | Pane | Role |
|---|---|---|
| `Desktop` (MessageLooper) | Compositor main thread (calloop) | Shared state, compositing, Wayland protocol, input dispatch |
| `ServerApp` (MessageLooper) | Dispatcher thread (1 per connection) | Demuxes incoming socket messages to per-pane threads |
| `ServerWindow` (MessageLooper) | Pane thread (1 per pane) | Handles session protocol for a single pane |

In Haiku, all three inherit `MessageLooper` — each has its own thread and message port. `Desktop` owns the window list and coordinates compositing. `ServerApp` owns one client connection and its `ServerWindow` children. `ServerWindow` owns the server-side representation of a single window and processes drawing commands on its own thread.

Pane's compositor follows this structure but adapts to the unix socket model:
- **Main thread**: calloop event loop. Owns smithay state, the unix socket listener, all registered `SessionSource` fds. Drives compositing. Dispatches input.
- **Dispatcher thread** (1 per connection): reads from the unix socket, deserializes, routes messages to per-pane channels. This is the `ServerApp` equivalent — it doesn't process messages itself, it demuxes.
- **Pane thread** (1 per pane, server-side): processes protocol messages for a single pane. Communicates with the main thread via channels. Never touches smithay objects directly (smithay is `!Send`).

The shared state is the layout tree — accessed by pane threads (read position/size) and the main thread (composite, structural changes). The spec calls for a reader-writer lock (Haiku's `Desktop::fWindowLock` pattern).

---

## Stages

### Stage 1: Calloop Skeleton

**Goal:** Replace the raw `while` loop with a calloop event loop. No protocol changes. The compositor renders the same hardcoded pane but is now event-driven.

**Why first:** Everything else (socket listening, input handling, timer-driven compositing) plugs into calloop. Without this, nothing else works. This is the `Desktop::Init()` equivalent — setting up the event loop before registering any sources.

**Files to modify:**
- `crates/pane-comp/src/main.rs` — gut the `while running` loop; create a `calloop::EventLoop`; register the winit backend's fd for readiness; register a timer source for frame scheduling (16ms / 60fps); move rendering into the timer callback
- `crates/pane-comp/Cargo.toml` — add `pane-session` dependency (brings calloop integration types)

**Files to create:**
- `crates/pane-comp/src/state.rs` — `CompState` struct holding all compositor state: output info, glyph atlas, pane renderer, current window size, running flag. Passed as calloop's shared data. This is the `Desktop` equivalent — the single struct that the event loop's callbacks mutate.

**Key decisions:**
- Frame scheduling: calloop `Timer` source at ~16ms. Winit redraws on this timer, not on every event. This avoids the pathological case where input events trigger redundant redraws.
- The winit event dispatch stays synchronous within the calloop callback — `dispatch_new_events` is called from the calloop handler for the winit fd source, not from a separate thread.
- `CompState` is `&mut` in calloop callbacks, not `Arc<Mutex<>>`. calloop's design gives exclusive access to shared data in callbacks. This is deliberate — the main thread owns compositor state exclusively. Pane threads communicate with it via channels, not shared references.

**Acceptance criteria:**
1. `pane-comp` starts, opens a winit window, renders the same hardcoded pane as before.
2. The main loop is a `calloop::EventLoop::run()`, not a `while` + `sleep`.
3. Close button works (winit CloseRequested propagates through calloop to set running = false).
4. Resize works (winit Resized events update CompState, next frame renders at new size).
5. No `thread::sleep` anywhere.

**Tests:** Manual only at this stage (requires Linux + display). The acceptance criteria are visual + log-based.

---

### Stage 2: Pane Protocol Server

**Goal:** The compositor listens on a unix socket, accepts connections, performs session-typed handshakes, and enters the active phase. No per-pane threading yet — everything runs on the main thread via calloop. A connected pane-app client can create a pane and see it in the compositor's log output.

**Why second:** This is the first real protocol integration. It validates that `SessionSource` works in production, that the handshake types from `pane-proto` round-trip correctly, and that the active-phase enum dispatch works from calloop callbacks. This is the equivalent of getting `ServerApp` to accept its first `BApplication` connection.

**Files to create:**
- `crates/pane-comp/src/protocol.rs` — pane protocol server logic:
  - `PaneServer::new(socket_path)` — creates and binds the unix socket listener
  - `PaneServer` as calloop event source (wraps `Generic<UnixListener>` for accept readiness)
  - Handshake handling: on accept, perform the session-typed handshake (server side of `ClientHello` / `ServerHello` / `ClientCaps` / `Accepted`|`Rejected`). After handshake, extract the transport, wrap the socket in a `SessionSource`, register it with calloop for active-phase message dispatch.
  - Active-phase dispatch: `handle_client_message(state: &mut CompState, client_id: ClientId, msg: ClientToComp)` — processes each message variant
- `crates/pane-comp/src/client.rs` — `ConnectedClient` struct: client ID, signature, `SessionSource` handle, list of pane IDs owned by this client. One per accepted connection.
- `crates/pane-comp/src/layout.rs` — `LayoutTree` stub: for now, a flat `Vec<PaneEntry>` where `PaneEntry` holds `PaneId`, geometry, title, content. No actual tree structure yet — that comes with tiling. The stub provides `insert`, `remove`, `get`, `get_mut`, `iter`, `focused`.

**Files to modify:**
- `crates/pane-comp/src/main.rs` — create `PaneServer`, register it with calloop. Pass `CompState` (now containing the layout) to all handlers.
- `crates/pane-comp/src/state.rs` — add `LayoutTree`, `HashMap<ClientId, ConnectedClient>`, pane ID counter, socket path. Add `send_to_client(client_id, CompToClient)` method.
- `crates/pane-comp/Cargo.toml` — ensure `pane-session` dependency includes calloop feature

**Key decisions:**
- **Socket path:** `$XDG_RUNTIME_DIR/pane-0` (the `0` is the display number, matching Wayland convention). Env var `PANE_SOCKET` overrides.
- **Handshake is blocking on accept.** When a new connection arrives, the compositor performs the full handshake synchronously before returning to the calloop. This is acceptable because the handshake is 4 messages (~microseconds) and new connections are rare. If this becomes a bottleneck (it won't — BeOS never had more than ~50 applications), the handshake moves to a spawned task.
- **PaneId allocation:** monotonic `AtomicU64` counter. IDs are never reused within a compositor session. This simplifies cleanup — a stale reference to a dead pane simply finds nothing.
- **Active-phase dispatch in calloop callbacks.** Each `SessionSource` fires its callback when a complete message arrives. The callback deserializes `ClientToComp`, dispatches to `handle_client_message`. Responses are sent synchronously via `write_message` on the client's stream. This is the correct architecture: the main thread processes protocol messages sequentially, never concurrently. Pane thread parallelism comes in Stage 3.
- **`App::connect()` comes alive.** The `pane-app` crate's `App::connect()` currently returns `Err(ConnectError::NotRunning)`. Stage 2 implements the real connection path: connect to the unix socket, perform the client-side handshake, set up the dispatcher thread. `connect_test()` continues to work for unit tests.

**Acceptance criteria:**
1. `pane-comp` starts and creates a unix socket at `$XDG_RUNTIME_DIR/pane-0`.
2. A pane-app client (`App::connect("com.example.hello")`) connects successfully, handshake completes.
3. `CreatePane` from the client produces a `PaneCreated` response with a valid `PaneId` and geometry.
4. The compositor logs the connection and pane creation.
5. Client disconnect (drop the `App`) produces `SessionEvent::Disconnected`, the compositor cleans up the client's panes without crashing.
6. Multiple clients can connect simultaneously.

**Tests:**
- Integration test: spawn `pane-comp` as a subprocess, connect with `pane-app`, create a pane, verify `PaneCreated` response, disconnect cleanly. Requires the unix socket, so Linux-only. This is the first test that exercises the real protocol path (not `MockCompositor`).
- Unit test: `SessionSource` + calloop + `MemoryTransport` — verify handshake round-trip and active-phase message dispatch in a single-threaded calloop loop. This can run anywhere.

---

### Stage 3: Per-Pane Threading

**Goal:** Each pane gets its own server-side thread. The dispatcher thread demuxes incoming messages from the connection socket to per-pane channels. Pane threads process protocol messages and communicate with the main thread via channels. The main thread composites.

**Why third:** Stages 1 and 2 proved that the protocol works end-to-end on a single thread. Stage 3 adds the threading model that makes it scale. This is the `ServerWindow` thread — the guarantee that one slow pane can't block its siblings.

The spec is explicit: "Each pane gets its own server-side thread — not one thread per connection. A slow pane cannot block its siblings." This is the BeOS guarantee that we verified in the Haiku source — `ServerWindow` inherits `MessageLooper`, spawns a dedicated thread on `Run()`.

**Files to create:**
- `crates/pane-comp/src/pane_thread.rs` — `PaneThread`: spawns a dedicated thread for one pane. Owns the pane's session state (title, content, dirty flag). Receives `ClientToComp` messages (filtered by PaneId) on its `mpsc::Receiver`. Sends state updates to the main thread via an `mpsc::Sender<PaneUpdate>` where `PaneUpdate` is an enum:
  - `TitleChanged { pane: PaneId, title: PaneTitle }`
  - `ContentChanged { pane: PaneId, content: Vec<u8> }`
  - `VocabularyChanged { pane: PaneId, vocabulary: CommandVocabulary }`
  - `CloseRequested { pane: PaneId }`
  - `ClientDisconnected { pane: PaneId }`

  The main thread reads from this channel (registered as a calloop `channel::Channel`) and updates the layout tree + triggers recomposite.

- `crates/pane-comp/src/dispatcher.rs` — `Dispatcher`: one thread per client connection. Reads from the `SessionSource` (blocking mode — the dispatcher thread is dedicated to this connection). Demuxes `ClientToComp` messages by PaneId to the right pane thread's channel. Handles `CreatePane` by signaling the main thread (which allocates the pane ID and spawns the pane thread). Handles connection death by signaling the main thread.

**Files to modify:**
- `crates/pane-comp/src/protocol.rs` — after handshake, hand the socket to a new `Dispatcher` thread instead of registering it directly with calloop. The calloop no longer reads client messages directly — dispatchers do.
- `crates/pane-comp/src/state.rs` — add `calloop::channel::Channel<PaneUpdate>` as the pane-thread-to-main-thread communication path. Add `CompState::process_pane_update(update: PaneUpdate)`.
- `crates/pane-comp/src/layout.rs` — `PaneEntry` now holds live state: title, content bytes, dirty flag. The main thread updates these when processing `PaneUpdate` messages. The renderer reads them during compositing.
- `crates/pane-comp/src/client.rs` — `ConnectedClient` now holds the `Dispatcher`'s join handle and per-pane `mpsc::Sender<ClientToComp>` channels.

**Key decisions:**
- **Dispatcher reads in blocking mode on its own thread.** This is the `ServerApp` pattern — one thread per connection, blocking on the port. The dispatcher is cheap (it just deserializes and forwards) and the thread cost is negligible. The alternative (calloop + non-blocking) was Stage 2's model and is correct but doesn't scale to per-pane threading cleanly — the pane thread needs its own blocking recv, not a callback.
- **Pane thread blocking recv, not calloop.** Pane threads use `mpsc::Receiver::recv()` — blocking, sequential message processing. This is the `BLooper::task_looper()` model. calloop is scoped to the compositor main thread, as the spec requires.
- **Main thread never blocks on pane threads.** Communication is one-directional per channel: pane threads send `PaneUpdate` to the main thread via calloop channel (non-blocking, fd-signaled). The main thread sends `CompToClient` responses directly on the socket (via the dispatcher's write half) — this is a `write()` on a non-blocking fd, not a channel send to the pane thread.
- **Response routing.** When a pane thread decides to send a `CompToClient` message (e.g., in response to a future scripting query), it sends a `PaneResponse { pane: PaneId, msg: CompToClient }` to the main thread, which writes it to the correct client's socket. The pane thread never writes to the socket directly. This keeps socket I/O on the main thread (or the dispatcher), matching smithay's `!Send` constraint.
- **Layout tree locking.** Not needed yet — the layout tree lives in `CompState`, which is exclusively owned by the main thread's calloop callbacks. Pane threads don't read the layout tree directly; they receive geometry via messages. When tiling arrives (Phase 5) and pane threads need to read their own geometry for damage calculation, a `RwLock<LayoutTree>` enters the picture.

**Acceptance criteria:**
1. A pane-app client creates 3 panes. Each gets its own server-side thread (visible in thread names / logs).
2. A pane thread processing a slow handler (simulated with `thread::sleep`) does not block other panes from receiving messages.
3. `SetTitle` from the client updates the pane's title in the compositor's layout state.
4. `SetContent` from the client updates the pane's content.
5. Client disconnect kills the dispatcher thread and all pane threads for that connection.
6. Pane close (`RequestClose` -> `Close` -> `CloseAck`) works end-to-end.

**Tests:**
- Integration test: connect two clients, each creates 2 panes. Client A's pane sleeps in its handler. Client B's panes continue receiving events without delay. Verify via message timestamps.
- Unit test: `Dispatcher` with mock socket, verify demux to correct pane channels.
- Unit test: `PaneThread` with mock channels, verify `PaneUpdate` sent to main thread on `SetTitle`/`SetContent`.

---

### Stage 4: Chrome Rendering

**Goal:** The compositor renders panes dynamically from protocol state — tag lines with real titles, beveled borders with focus indicators, body content from `SetContent`. The hardcoded `PaneRenderer` is replaced by a layout-driven renderer that iterates the layout tree and renders each pane.

**Why fourth:** Stages 1-3 gave us protocol + threading. Stage 4 gives us visual output driven by that protocol. This is the equivalent of Haiku's `Desktop::draw_window_list()` — the compositor renders what the layout tree says, not hardcoded geometry.

**Files to create:**
- `crates/pane-comp/src/chrome.rs` — chrome rendering logic extracted from (and replacing) `pane_renderer.rs`:
  - `render_pane(frame, atlas, pane: &PaneEntry, geometry: Rect, focused: bool)` — renders one pane: tag line, borders, body background
  - `render_tag(frame, atlas, title: &PaneTitle, geometry: Rect, focused: bool)` — renders the tag line with actual text from the protocol. Focused panes get the warm yellow (`TAG_BG`). Unfocused get a muted variant.
  - `render_borders(frame, geometry: Rect, focused: bool)` — beveled borders. Light edge top/left, dark edge bottom/right. Focus affects border color intensity.
  - `render_body(frame, atlas, content: &[u8], geometry: Rect)` — renders body content. For Stage 4, content is treated as a `CellRegion` (text grid). The content model negotiated during handshake determines interpretation — but for now, it's cells.
  - `render_text(frame, atlas, text: &str, position: Point, color: Color32F)` — renders a string of text using the glyph atlas. This is the missing piece from the current code — the atlas rasterizes glyphs but never uploads them as GPU textures.

- `crates/pane-comp/src/texture.rs` — glyph atlas GPU texture management:
  - Upload atlas bitmap to a GL texture
  - `render_glyph(frame, texture, glyph_info, position, color)` — render a single glyph from the atlas texture at a given position with a given foreground color
  - The current `GlyphAtlas` rasterizes to CPU memory. This module handles the CPU-to-GPU upload and the textured quad rendering. smithay's `GlesRenderer` provides `import_memory` for texture upload and `render_texture_from_to` for textured rendering.

**Files to modify:**
- `crates/pane-comp/src/main.rs` — rendering loop iterates `layout.iter()` and calls `render_pane` for each entry. The hardcoded `PaneRenderer::new` / `PaneRenderer::render` calls are replaced.
- `crates/pane-comp/src/glyph_atlas.rs` — add `upload_to_gpu(renderer: &mut GlesRenderer) -> GlesTexture` method. May need refactoring to separate the CPU-side atlas (rasterization) from the GPU-side texture (rendering). The atlas itself is fine; it just needs a GPU upload path.
- `crates/pane-comp/src/layout.rs` — `PaneEntry` gains `focused: bool`. `LayoutTree` gains focus tracking: `focus(pane_id)`, `focused() -> Option<PaneId>`, `cycle_focus()`.

**Files to remove:**
- `crates/pane-comp/src/pane_renderer.rs` — replaced by `chrome.rs`. The color conversion utilities (`color_to_rgba`, `named_to_rgba`, `indexed_to_rgba`) move to a shared location (either `chrome.rs` or a `color.rs` utility module).

**Key decisions:**
- **Text rendering via atlas texture.** The glyph atlas is already rasterized to CPU memory. Stage 4 uploads it once (and re-uploads on atlas growth) as a GL texture, then renders textured quads per glyph. This is the standard approach for GPU text rendering in compositors. smithay's `GlesRenderer::import_memory` handles the upload; `Frame::render_texture_from_to` handles the draw.
- **Body content interpretation.** For Stage 4, body content (`SetContent`) is postcard-serialized `CellRegion` from `pane-proto`. The renderer deserializes and renders the cell grid. This is sufficient for pane-shell (which sends cell grids). Rich content models come later.
- **Focus rendering.** Focused pane: bright tag, full-intensity borders. Unfocused: dimmed tag, muted borders. No animation yet. Focus is tracked per `PaneEntry` in the layout tree, updated via a channel message from the pane thread when the compositor dispatches focus events.
- **Layout is still flat.** Panes are positioned by the layout tree, but the layout tree is still a flat list with manual positioning (stacked or tiled into a simple grid). Real tiling is Phase 5. Stage 4's layout just needs to place N panes on screen without overlap.

**Acceptance criteria:**
1. Connected pane-app clients with `SetTitle` see their title rendered in the tag line.
2. Connected pane-app clients with `SetContent` (CellRegion) see their content rendered in the body.
3. Text is legible — glyphs are rendered from the atlas texture, not just solid backgrounds.
4. Focus is visually distinguished (bright vs dim tag line, border intensity).
5. Multiple panes render simultaneously at different positions.
6. The hardcoded `PaneRenderer` is gone. All rendering is driven by `LayoutTree` state.

**Tests:**
- Integration test: connect, create pane, set title to "Test Pane", set content to a CellRegion with "Hello, pane!", verify rendering (screenshot comparison or log-based geometry verification).
- Unit test: `chrome::render_pane` with a mock frame — verify correct draw calls for tag, borders, body. (Requires a smithay test renderer or a mock frame trait.)
- The texture upload path is hard to unit test — it requires GL context. Acceptance is visual.

---

### Stage 5: Input and Layout

**Goal:** The compositor handles keyboard and mouse input, dispatches to the focused pane, and provides basic layout (focus cycling, pane positioning). pane-shell connects, creates a pane, spawns a PTY, bridges input/output, and provides a working terminal.

**Why last:** Input requires everything else to be working — protocol, threading, rendering, focus tracking. pane-shell requires input to be useful. This stage brings it all together.

**Files to create:**
- `crates/pane-comp/src/input.rs` — input handling:
  - Keyboard: smithay's `KeyboardHandle` + xkbcommon for keymap processing. Translates raw keycodes to `KeyEvent` (from `pane-proto`). Dispatches to the focused pane via its pane thread channel.
  - Mouse: smithay's `PointerHandle`. Hit testing against the layout tree to determine which pane receives the event. Translates pixel coordinates to cell coordinates for text-mode panes. Click on tag line activates command surface.
  - Compositor bindings: `Super+Return` = spawn new pane-shell. `Super+Tab` = cycle focus. `Super+Q` = close focused pane. `Super+Shift+Q` = quit compositor. These are hardcoded for Stage 5; the Input Kit (pane-input) provides configurability later.
  - Seat management: one `wl_seat` for the winit backend. Multi-seat is a Phase 5+ concern.

- `crates/pane-comp/src/focus.rs` — focus management extracted from layout:
  - Focus stack (MRU order). `focus(pane_id)` pushes to top. `cycle_focus()` rotates.
  - Focus events: when focus changes, send `Focus` to the newly focused pane and `Blur` to the previously focused pane.
  - Integration with input: keyboard events go to the focused pane. Mouse events go to the clicked pane (which also receives focus).

- `crates/pane-shell/` — new crate. The milestone application. Minimal terminal emulator as a pane-native client:
  - `src/main.rs` — `App::connect("org.pane.shell")`, create a pane with `Tag::new("~")`, spawn a PTY, bridge I/O.
  - `src/pty.rs` — PTY management: `openpty()`, `fork()`, `exec()` the user's shell. Read PTY output on a background thread, write to pane content via `PaneProxy::set_content()`. Read keyboard input from pane events, write to PTY input.
  - `src/grid.rs` — terminal grid: maintains a cell grid (`CellRegion`), processes VT100/xterm escape sequences to update cells. Serializes the grid as `SetContent` payload. For Stage 5, a basic VT100 implementation (cursor movement, color, clear screen) is sufficient. Full xterm compliance is future work.
  - `Cargo.toml` — depends on `pane-app`, `pane-proto`, `rustix` (for PTY operations).

**Files to modify:**
- `crates/pane-comp/src/main.rs` — register input sources with calloop (smithay's `Libinput` backend for DRM, or winit's input for the winit backend). Wire input dispatch to `input.rs` handlers.
- `crates/pane-comp/src/layout.rs` — add simple positioning: first pane gets full screen minus margins. Additional panes split the screen horizontally (50/50, 33/33/33, etc.). This is throwaway layout — real tiling is Phase 5 — but it's enough to demonstrate multiple panes.
- `crates/pane-comp/src/protocol.rs` — add `CompToClient::Key`, `CompToClient::Mouse`, `CompToClient::Focus`, `CompToClient::Blur` dispatch from the main thread to pane threads (these messages are already defined in `pane-proto`).
- `Cargo.toml` (workspace) — add `pane-shell` to members (Linux-only, like `pane-comp`).

**Key decisions:**
- **Winit input for the dev environment.** The winit backend provides keyboard and mouse events. We translate these to `pane-proto` events. When we move to DRM/KMS (Phase 5+), we switch to libinput. The abstraction boundary is `input.rs` — it produces `pane-proto` events regardless of source.
- **pane-shell is minimal.** It's not a full terminal emulator — it's a PTY bridge with enough VT100 to run bash/ksh and see output. `ls`, `cd`, `cat` should work. vim probably won't (it needs many more escape sequences). The goal is proving the system works end-to-end, not terminal completeness.
- **VT100 parsing: use a library or hand-roll?** The recommendation is to start hand-rolled for the subset we need (CSI cursor movement, SGR color, ED/EL clear), then evaluate `vte` crate for full coverage if pane-shell grows into the primary terminal. Hand-rolling the subset keeps dependencies minimal and teaches the team the protocol.
- **PTY resize.** When the compositor sends `CompToClient::Resize` (from window resize or layout change), pane-shell translates to `TIOCSWINSZ` on the PTY. This closes the resize loop: compositor -> pane-app kit -> pane-shell handler -> PTY -> shell.
- **pane-shell's grid is server-side content.** pane-shell maintains the cell grid and sends the full grid (or dirty regions) to the compositor via `SetContent`. The compositor renders it. This is the Wayland model inverted — in standard Wayland, the client renders to a buffer and the compositor composites. In pane's text model, the client sends structured content and the compositor renders. Both models coexist: legacy Wayland clients use buffers; pane-native text clients use cell grids.

**Acceptance criteria:**
1. Typing in the compositor window produces characters in pane-shell's pane body.
2. Running `ls` in pane-shell produces visible output with correct line breaks.
3. Running `echo $TERM` shows a recognized terminal type.
4. `Ctrl+C` interrupts a running command.
5. Creating a second pane-shell (via compositor binding) produces a second pane on screen, independently interactive.
6. Focus cycling (`Super+Tab`) switches which pane receives keyboard input.
7. Closing a pane (`Super+Q`) terminates the PTY and removes the pane from the layout.
8. Resizing the compositor window causes pane-shell to receive `Resize`, update the PTY, and re-render at the new size.

**Tests:**
- Integration test (scripted): spawn compositor, spawn pane-shell, send keystrokes via the protocol, read content back, verify output. This is the first fully automated end-to-end test.
- Unit test: VT100 parser — feed known escape sequences, verify cell grid state.
- Unit test: PTY bridge — mock PTY fd, verify read/write plumbing.
- Manual test: actually use pane-shell to navigate the filesystem and edit a file with ed(1). If ed works, the terminal is usable.

---

## Dependency Graph

```
Stage 1: Calloop Skeleton
    |
    v
Stage 2: Pane Protocol Server
    |
    v
Stage 3: Per-Pane Threading
    |
    +---> Stage 4: Chrome Rendering (can start in parallel with Stage 3
    |     completion — needs layout tree from Stage 2, benefits from
    |     Stage 3's dynamic content but can use hardcoded content initially)
    |         |
    v         v
Stage 5: Input and Layout (requires Stage 3 threading + Stage 4 rendering)
    |
    v
pane-shell (requires Stage 5 input + Stage 4 rendering + Stage 3 threading)
```

In practice, Stages 4 and 5 have a soft dependency on Stage 3. Stage 4's chrome rendering can begin as soon as Stage 2's layout tree exists, using test content. Stage 3's per-pane threading and Stage 4's chrome rendering can be developed in parallel by different people. Stage 5 and pane-shell require everything.

---

## The Milestone

**pane-shell running inside pane-comp.** Concretely:

1. `pane-comp` starts, opens a window, listens on a unix socket.
2. `pane-shell` connects via `App::connect()`, creates a pane with a tag line.
3. The compositor renders the pane: yellow tag line with the current directory as title, beveled borders, white body with the shell prompt.
4. The user types commands. Characters appear in the body. The shell executes them. Output appears.
5. `Super+Return` spawns a second pane-shell. Both are visible, both are interactive, focus switches between them.
6. Closing a pane terminates its shell cleanly.

This is the moment pane becomes usable. Not useful — usable. A person could, in theory, do real work in it. Everything before this is infrastructure; everything after builds on it. It's the equivalent of getting Terminal running in BeOS R3's app_server — the proof that the rendering pipeline, the messaging system, the threading model, and the application kit all work together.

---

## Risks and Mitigations

**Risk: smithay texture API.** The glyph-to-GPU-texture path in Stage 4 depends on smithay's `import_memory` and `render_texture_from_to` working for our use case (small RGBA textures, many draw calls per frame). If performance is bad, we may need to batch glyph draws into a single textured quad draw call using instanced rendering.

*Mitigation:* Profile early in Stage 4. If per-glyph texture draws are too slow, switch to a glyph atlas texture + instanced vertex buffer approach (standard technique, well-documented). cosmic-text's swash already produces the atlas bitmap; the question is only how we submit it to GL.

**Risk: VT100 subset scope creep.** pane-shell's terminal emulator could expand indefinitely. Every new escape sequence is a rabbit hole.

*Mitigation:* Define the target precisely: bash/ksh interactive session with colored `ls`, line editing (readline), and process control (Ctrl+C/Ctrl+Z). Nothing more for Stage 5. If vim is needed, that's a future stage.

**Risk: Main thread contention.** All pane threads send `PaneUpdate` to the main thread's calloop channel. At 50 panes sending content updates, the calloop wakes on every message.

*Mitigation:* `PaneUpdate::ContentChanged` carries dirty flags, not full content. The main thread reads the dirty flag and re-renders only dirty panes. Content updates are batched: the pane thread accumulates changes and sends one `ContentChanged` per frame interval (signaled by a timer or by the compositor's frame callback). This is George Hoffman's batching from Be Newsletter #2-36, applied to the server side.

**Risk: Handshake blocking on the main thread.** Stage 2 performs the handshake synchronously in the calloop callback. If a client connects and then hangs during the handshake, the compositor stalls.

*Mitigation:* Accept with a timeout. If the handshake doesn't complete within 1 second, drop the connection. Malicious or broken clients don't get to stall the compositor. Alternatively, move the handshake to the dispatcher thread (which blocks on its own thread, not the main thread) — this is the cleaner long-term approach and should replace the synchronous handshake by the end of Stage 3.

---

## Open Questions for Lane

1. **Content model negotiation.** The handshake `ClientCaps` can negotiate content model (cells vs. buffer vs. semantic). For Stage 5, everything is cells. Should the handshake enforce this, or should we keep the negotiation stub and assert cells on the compositor side?

2. **pane-shell scope.** Should pane-shell be its own crate in the workspace, or a binary target within pane-comp? Separate crate is cleaner (it's a client, not part of the compositor). But it means another crate to maintain.

3. **Glyph atlas sharing.** The spec mentions shared memory glyph caching across pane-native processes. For Phase 4, each process has its own atlas. Should we plan the shared memory layout now, or defer entirely?

4. **DRM backend.** The winit backend is development-only. When does DRM/KMS/libinput become a priority? Phase 5 (tiling desktop) seems like the natural point. Should Stage 5 here prepare for it, or stay on winit?
