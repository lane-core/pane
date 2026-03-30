# Per-Pane Server-Side Threading: Feasibility and Architecture

## 1. How Haiku's app_server Actually Threads

Verified in the Haiku source at `~/src/haiku/src/servers/app/`.

### The hierarchy: three levels of MessageLooper

Every threaded entity in app_server inherits from `MessageLooper`, which spawns a thread at `B_DISPLAY_PRIORITY` and runs a blocking message loop: read from port, lock self, dispatch, unlock, repeat. Three things are MessageLoopers:

1. **Desktop** — one per logged-in user. Owns the `MultiLocker fWindowLock` (reader-writer lock) that governs all window state. Also a MessageLooper but primarily serves as the shared-state coordinator.

2. **ServerApp** — one per client application (one per BApplication). Its thread handles app-level messages: create window, destroy window, manage bitmaps, manage pictures, manage fonts. When it receives `AS_CREATE_WINDOW`, it creates a `ServerWindow` and calls `window->Run()`, which spawns the window's own thread.

3. **ServerWindow** — one per client window (one per BWindow). Its thread handles ALL per-window protocol messages: drawing commands, view hierarchy changes, state queries, input responses.

This is confirmed by the source comment in `ServerWindow.cpp`:

> "There is one ServerWindow per BWindow."

And by `ServerApp::_CreateWindow()` which creates a `ServerWindow` object and calls `Run()` on it, spawning its own thread. The thread name includes the window title for debugging.

### The locking model: reader-writer on Desktop, per-window on ServerWindow

The critical insight is the two-tier locking:

**Desktop's `fWindowLock` (MultiLocker = reader-writer lock):**
- `LockSingleWindow()` = `ReadLock()` — multiple ServerWindow threads can hold this simultaneously
- `LockAllWindows()` = `WriteLock()` — exclusive, blocks all other window threads

**ServerWindow's own BLocker (inherited from MessageLooper):**
- Protects per-window state during message dispatch
- Held only by the window's own thread during processing

The message loop in `ServerWindow::_MessageLooper()` is instructive. On each message, it checks `_MessageNeedsAllWindowsLocked(code)`. Most messages (all drawing commands, state changes, view manipulation) only need `LockSingleWindow()` (read lock). Only structural changes (set title, create views, change window feel/look, move/resize) need `LockAllWindows()` (write lock).

This means: **multiple ServerWindow threads process drawing commands concurrently**, with no contention between them. Only structural operations serialize.

The inner loop also has a budget: process up to 70 messages or 10ms worth, then release the Desktop lock. This prevents a flood of drawing commands from one window starving others.

### The drawing pipeline: per-window DrawingEngine, shared HWInterface

Each `Window` object owns its own `DrawingEngine` instance (created via `fDesktop->HWInterface()->CreateDrawingEngine()`). The `DrawingEngine` wraps a `Painter` that does the actual rendering into the framebuffer.

The `HWInterface` (hardware abstraction) extends `MultiLocker` itself:
- `LockParallelAccess()` = `ReadLock()` — multiple windows draw to the framebuffer simultaneously
- `LockExclusiveAccess()` = `WriteLock()` — only for mode changes, buffer swaps, etc.

So in BeOS/Haiku: multiple ServerWindow threads draw to the framebuffer at the same time, each through their own DrawingEngine, clipped to their own visible region. The clipping ensures they don't interfere. This is true concurrent rendering at the server level.

### Summary of BeOS's per-window server threading

```
Client Process          app_server
─────────────          ──────────
BApp looper    ←port→   ServerApp thread (1 per app)
BWindow looper ←port→   ServerWindow thread (1 per window)
BWindow looper ←port→   ServerWindow thread (1 per window)
                                ↓
                         Desktop (shared state, RW lock)
                                ↓
                         DrawingEngine (per-window) → HWInterface (shared, RW lock)
```

## 2. What Pane's Architecture Spec Currently Says

The architecture spec (§3, pane-comp) says:

> "Per-client session handling runs on dedicated threads (one thread per pane-native connection, matching the app_server pattern of one ServerWindow thread per client)."

And in §6:

> | Each pane-native connection (server-side) | Dedicated thread |

But the spec also says:

> "Pane protocol server: accepts pane-native client connections over a unix socket, **multiple panes per connection**"

This is the gap. A single connection can carry multiple panes (sub-sessions). If threading is per-connection, a slow pane blocks its sibling panes on the same connection. BeOS didn't have this problem because each BWindow had its own kernel port — connections were inherently per-window.

## 3. The Translation to Wayland

### What BeOS had vs. what Wayland provides

In BeOS, drawing commands flowed from client to server over kernel ports. The ServerWindow thread received and executed them, writing pixels to a shared framebuffer. The server did the rendering.

In Wayland, the client renders its own buffer. The compositor receives a committed `wl_surface` — a completed frame. The compositor's job per-surface is:
- Accept the committed state (buffer, damage, scale, transform)
- Incorporate it into the layout/compositing tree
- During the composite pass, sample from the buffer

This means **the server-side work per pane is different from BeOS's**. There's no stream of drawing commands to execute. The per-pane work is:

1. **Protocol message processing** — session type operations: tag line updates, state changes, attribute modifications, routing events
2. **Input event dispatch** — sending keyboard/mouse events to the focused pane's client
3. **Surface state management** — handling wl_surface.commit: pending → committed state transition, damage region accumulation
4. **Lifecycle management** — creation, destruction, reparenting, workspace assignment

The actual compositing (sampling all buffers into the output framebuffer) and the actual page flip are inherently serialized — one pass per frame, one flip per vblank. This is the compositor main thread's job regardless of threading model.

### What a per-pane thread does

For pane-native clients, the per-pane thread handles the session-typed protocol: tag line content, routing events, attribute queries, scripting protocol (optic-addressed state access). This is the direct analogue of what ServerWindow's thread did — handling the per-window protocol messages.

For legacy Wayland clients, the per-pane thread handles wl_surface state management. But here's the constraint: **smithay's Wayland protocol handling is single-threaded by design** (see §4 below). Legacy surfaces must be processed on the calloop thread.

This creates a natural split:
- **Pane-native panes**: per-pane thread handles the pane protocol; results are sent to the compositor main thread via channel
- **Legacy Wayland panes**: protocol handling stays on calloop; no per-pane thread

This is not a compromise — it's the correct separation. Legacy Wayland clients speak a different protocol that's designed for single-threaded dispatch. Pane-native clients speak a protocol we control, designed for per-pane threading.

## 4. smithay's Threading Model

smithay is built around calloop (callback-oriented event loop). The core design:

- `Display` (wayland-server) processes all client protocol messages on a single thread
- `DisplayHandle` is the interface for interacting with protocol state — it is `!Send` in the default configuration
- Callbacks from protocol events receive a `&mut State` reference, relying on Rust's single-mutable-reference guarantee for safety rather than locks
- smithay explicitly recommends a centralized mutable state accessed through callbacks, "providing easy access to a centralized mutable state without synchronization as the callback invocation is always sequential"

**Implication**: smithay's Wayland protocol handling cannot be distributed across threads. This is a hard constraint from the library design, not a soft preference. The `wayland-server` crate's object model (`Resource`, `DisplayHandle`) is not designed for concurrent access.

**What this means for per-pane threading**: the per-pane threads handle the **pane protocol** (our protocol, over our socket), not the Wayland protocol. The compositor main thread (calloop) handles:
- Wayland protocol for legacy clients
- DRM page flip and presentation
- libinput events
- Frame timing and compositor rendering pass
- Receiving results from per-pane threads via channel (calloop's channel source)

Per-pane threads do NOT touch smithay objects. They manage pane-protocol state and communicate with the main thread via typed channels. This is the same boundary that Haiku has: ServerWindow threads don't touch the HWInterface directly for mode changes — they acquire the Desktop lock and go through Desktop's interface.

## 5. The Sub-Session Demultiplexing Question

### The problem

One unix socket connection carries multiple panes (sub-sessions). With per-pane threading, we need to route incoming messages from a shared socket to the correct pane's thread.

### The solution: dispatcher thread per connection

```
Unix Socket
    │
    ▼
Dispatcher Thread (1 per connection)
    │  reads frames, inspects pane-id header
    │
    ├──► Pane Thread A (via channel)
    ├──► Pane Thread B (via channel)
    └──► Pane Thread C (via channel)
```

Each connection gets a dispatcher thread that:
1. Reads framed messages from the socket (postcard-serialized, with a pane-id header)
2. Routes each message to the correct pane thread's channel
3. Handles connection-level messages (new sub-session, close sub-session)

Outbound messages go the other direction: each pane thread sends to a shared write channel, and the dispatcher thread serializes writes to the socket.

This is directly analogous to BeOS's ServerApp thread. ServerApp was the per-application thread that handled app-level messages and created ServerWindow threads for each window. Here, the dispatcher is the per-connection thread that handles connection-level messages and routes per-pane messages to pane threads.

**The hierarchy maps cleanly:**

| BeOS | Pane |
|---|---|
| ServerApp thread (1 per application) | Dispatcher thread (1 per connection) |
| ServerWindow thread (1 per window) | Pane thread (1 per pane) |
| Desktop (shared state, RW lock) | Compositor main thread (calloop) |
| kernel port (1 per window) | channel (1 per pane) |

### Why not one socket per pane?

Pane could give each pane its own unix socket (making the connection-per-pane problem go away). But:
- Unix sockets are file descriptors; fd limits are finite (though generous — 1024 default, easily raised)
- A single connection carries application-level context (authentication, client identity, shared resources)
- Sub-sessions over a single connection are cheaper than N connections
- The dispatcher thread is trivial code — a loop that reads frames and routes by pane-id

The sub-session model is right. The dispatcher thread handles the demux.

## 6. Cost Analysis

### Thread memory

Eli Bendersky's measurements (2018, still relevant — the NPTL model hasn't changed): each Linux thread requires:
- `task_struct` in the kernel: ~5-8KB
- Stack: 8KB minimum with `ulimit -s`, but glibc defaults to 8MB *virtual*. Only 1-2 pages (4-8KB) are physically allocated initially (demand paging)
- Thread-local storage: minimal unless heavily used
- pthread metadata: ~1-2KB

**Effective physical cost per thread: ~20KB** (Pierre Raynaud-Richard's 1999 measurement for BeOS is remarkably still in the right ballpark for Linux).

At 50 panes: 50 pane threads + 5-10 dispatcher threads = 55-60 server-side threads. Physical memory: ~1.2MB. Negligible.

For the Rust thread, we can also set explicit stack sizes via `std::thread::Builder::new().stack_size()`. 64KB stacks are generous for message-dispatch loops. At 50 panes: 50 × 64KB = 3.2MB of virtual address space (physical use: far less due to demand paging).

### Context switch cost

Bendersky measured ~1.3-1.6μs per context switch on 2018 hardware. 2026 hardware is comparable or better. Threads within the same process (as these are — all in the compositor process) share the address space, so no TLB flush is needed. Cost is dominated by register save/restore and cache effects.

At a desktop workload: per-pane threads wake on incoming messages, process them (typically microseconds of work), and go back to sleep. The scheduler handles this trivially — these are mostly-sleeping threads with brief wakeups, which is exactly the workload CFS/EEVDF optimizes for.

### Comparison: 50 threads vs. single-threaded dispatch

**50 threads:** each pane sleeps independently, wakes only on its own messages. A slow pane (expensive protocol operation, blocking filesystem query) doesn't affect other panes. The scheduler distributes work across cores naturally.

**Single thread with multiplexing:** all 50 panes share one dispatch loop. A 5ms blocking operation in one pane's handler delays all 49 others by 5ms. To avoid this, you'd need to either (a) never block (requires async everything) or (b) offload blocking work to a thread pool (which reintroduces threading complexity without the clean per-pane model).

The per-pane model is simpler in the sense that matters: each pane's state is thread-local, no shared mutable state between pane threads, and blocking is a per-pane concern.

### Scheduling overhead

The Linux EEVDF scheduler (which replaced CFS in kernel 6.6) handles hundreds of threads routinely. Desktop environments already run 50-200 threads across their process tree. Adding 50 more in the compositor is not a concern.

## 7. What Needs to Be Serialized vs. What Can Be Parallel

### Must be serialized (compositor main thread only)

1. **Wayland protocol dispatch** — smithay's `Display::dispatch_clients()`, `Display::flush_clients()`. Single-threaded by library design.
2. **Compositing pass** — sampling all surface buffers, rendering chrome, producing the output frame. One pass per vblank. OpenGL/Vulkan command submission from one thread.
3. **DRM page flip** — one atomic commit per frame per output. Inherently serial.
4. **Input routing** — determining which pane is focused, which pane the pointer is over. Must be consistent with layout state.
5. **Layout tree mutations** — reparenting, splitting, resizing. Affects all panes' positions.

### Can be parallel (per-pane threads)

1. **Pane protocol message processing** — tag line updates, attribute changes, routing events. Each pane's state is independent.
2. **Session type state machine** — advancing the per-pane session. No cross-pane dependencies.
3. **Scripting protocol** — optic-addressed state access. Per-pane handler chain resolution.
4. **Damage calculation** — computing what region changed per-pane (though the compositor merges these during the compositing pass).
5. **Input event formatting** — packaging input events for the specific pane's protocol. The routing decision is on the main thread; the formatting and sending is per-pane.

### Cross-thread communication

Per-pane threads communicate with the compositor main thread via `calloop::channel`. This is a typed MPSC channel that integrates with calloop's event loop — the main thread gets woken when a pane thread sends a message.

Messages from pane threads to main thread:
- `TagLineUpdated { pane_id, content }` — triggers chrome re-render
- `PaneStateChanged { pane_id, change }` — triggers layout/visibility update
- `SurfaceReady { pane_id }` — pane has new content to composite

Messages from main thread to pane threads (via per-pane channels):
- `InputEvent { event }` — keyboard/mouse/touch event
- `LayoutChanged { rect }` — pane's position/size changed
- `FocusChanged { focused }` — pane gained/lost focus

## 8. The Concrete Threading Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  Compositor Process                       │
│                                                          │
│  ┌──────────────────────────────────────────────┐       │
│  │  Main Thread (calloop)                        │       │
│  │  • Wayland protocol (smithay Display)         │       │
│  │  • DRM/KMS page flip                          │       │
│  │  • libinput event processing                  │       │
│  │  • Compositing pass (GLES renderer)           │       │
│  │  • Chrome rendering (tag lines, borders)      │       │
│  │  • Layout tree management                     │       │
│  │  • Input routing (focus, pointer)             │       │
│  │  • calloop::channel receivers from pane       │       │
│  │    threads                                    │       │
│  └──────┬───────────┬──────────────┬─────────────┘       │
│         │           │              │                      │
│    ┌────▼────┐ ┌────▼────┐  ┌─────▼─────┐              │
│    │Dispatch │ │Dispatch │  │ Dispatch   │              │
│    │Thread A │ │Thread B │  │ Thread C   │              │
│    │(conn 1) │ │(conn 2) │  │ (conn 3)  │              │
│    └──┬──┬───┘ └────┬────┘  └──┬────┬───┘              │
│       │  │          │          │    │                    │
│    ┌──▼┐┌▼──┐   ┌──▼──┐   ┌──▼┐ ┌─▼──┐               │
│    │P1 ││P2 │   │P3   │   │P4 │ │P5  │               │
│    │thr││thr│   │thr  │   │thr│ │thr │               │
│    └───┘└───┘   └─────┘   └───┘ └────┘               │
│                                                          │
│  Legacy Wayland clients: handled entirely on main        │
│  thread via smithay. No per-pane threads.                │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### Thread roles

**Main thread (calloop):**
- Sole owner of smithay `Display` and all Wayland protocol objects
- Sole owner of the GLES rendering context
- Sole owner of DRM/KMS backend
- Sole owner of libinput
- Owns the layout tree (read by pane threads via shared `Arc<RwLock<LayoutTree>>` for position queries, mutated only on main thread)
- Receives `PaneEvent` messages from pane threads via `calloop::channel`
- Sends `CompEvent` messages to pane threads via per-pane `std::sync::mpsc::Sender`

**Dispatcher threads (1 per pane-native connection):**
- Owns the unix socket read half
- Reads framed messages, inspects pane-id header
- Routes to the correct pane thread's channel
- Handles connection-level protocol (new sub-session → spawn pane thread, close sub-session → signal pane thread)
- Owns the socket write half (serializes outbound messages from pane threads)

**Pane threads (1 per pane-native pane):**
- Owns per-pane session state (session type machine, tag line content, attributes, scripting handlers)
- Processes pane protocol messages from its dispatcher
- Processes compositor events from the main thread (input, layout changes, focus)
- Sends state change notifications to the main thread via `calloop::channel`
- Sends responses to the client via the dispatcher's write channel
- **Does not touch**: smithay objects, GLES context, DRM, libinput, layout tree mutation

### The RW lock parallel: `Arc<RwLock<LayoutTree>>`

Haiku's Desktop used a `MultiLocker` (RW lock) for the window list. Most operations took the read lock (drawing commands, state queries); only structural changes took the write lock.

Pane's equivalent: the layout tree is wrapped in `Arc<RwLock<LayoutTree>>`. Pane threads can read-lock it to determine their own position/size (for event coordinate transformation, etc.). Only the main thread write-locks it for mutations (splits, resizes, workspace changes).

This is the same concurrency pattern, mapped to Rust's `std::sync::RwLock` (which uses futex on Linux — fast uncontested path, kernel involvement only on writer contention).

### What about Haiku's 70-message / 10ms budget?

Haiku's ServerWindow limited itself to processing 70 messages or 10ms of work before releasing the Desktop lock, preventing one window from hogging the lock. In pane, this concern manifests differently:

- Pane threads don't hold a shared lock during message processing (their state is thread-local)
- The only shared resource is the `calloop::channel` to the main thread, which is non-blocking
- A pane thread that sends a burst of `TagLineUpdated` events won't block other pane threads
- The main thread processes channel events in its own loop and can rate-limit if needed

The per-pane model actually eliminates the need for the budget — there's no shared lock to hog.

## 9. Recommendation

### Adopt per-pane server-side threading, faithful to BeOS

The architecture spec should be updated from "one thread per pane-native connection" to:

1. **One dispatcher thread per pane-native connection** — handles socket I/O and message routing
2. **One pane thread per pane-native pane** — handles pane protocol, session state, scripting
3. **Compositor main thread (calloop)** — handles Wayland protocol, rendering, input, DRM
4. **Legacy Wayland panes** — no per-pane thread, fully handled on main thread

This is faithful to BeOS's model:
- BeOS: ServerApp (per-app) → ServerWindow (per-window) → Desktop (shared)
- Pane: Dispatcher (per-connection) → Pane Thread (per-pane) → Main Thread (shared)

### Why this is the right answer

1. **Isolation.** A slow pane (complex scripting query, blocking attribute lookup) cannot block other panes. This was the whole point of BeOS's per-window threading.

2. **Simplicity.** Each pane thread owns its state outright. No shared mutable state between pane threads. Message passing is the only communication. This is the actor model that BeOS proved works.

3. **Rust fit.** Rust's ownership model makes this safer than BeOS's C++ implementation. Each pane thread owns its `PaneState` struct; `Send` guarantees that inter-thread messages are safe; the borrow checker prevents accidental sharing.

4. **Cost.** 50 pane threads: ~3MB virtual, ~1MB physical, negligible scheduling overhead. This is a rounding error on 2026 hardware.

5. **smithay compatibility.** Per-pane threads never touch smithay objects. The main thread is the sole smithay consumer. This respects smithay's single-threaded design while recovering BeOS's threading granularity for the pane-native protocol layer.

### What the spec should say

The threading table in §6 should change from:

| Component | Thread model |
|---|---|
| Each pane-native connection (server-side) | Dedicated thread |

To:

| Component | Thread model |
|---|---|
| Each pane-native connection (server-side) | Dispatcher thread (socket I/O, message routing) |
| Each pane-native pane (server-side) | Dedicated pane thread (session state, protocol handling) |

And §3 (pane-comp) should describe the three-tier hierarchy: main thread → dispatcher threads → pane threads, with explicit documentation of what each tier owns and what communication channels connect them.

### Open prototype question

The dispatcher-thread-per-connection model needs a prototype to validate. Specifically:
- Frame format for the pane protocol (pane-id header + postcard payload)
- Demux performance under realistic message rates
- Backpressure handling when a pane thread falls behind
- Graceful handling of pane thread crash (dispatcher catches the channel disconnect, notifies main thread)

This is tractable engineering, not a research problem. The architecture is sound; the details need implementation.
