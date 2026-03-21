# Smithay Viability Assessment — Should Pane Build On It?

Research for pane spec-tightening. Primary sources: smithay source code and documentation (github.com/Smithay/smithay, docs.rs/smithay/0.7.0), smithay issue tracker (especially #928 on wgpu), wayland-server crate documentation (docs.rs/wayland-server), drm crate documentation, lamco-wgpu project (wgpu renderer for smithay), Haiku app_server source code (~/src/haiku/src/servers/app), pane-comp current implementation, niri compositor (github.com/YaLTeR/niri), COSMIC compositor (github.com/pop-os/cosmic-comp), pane architecture spec and dependency philosophy, existing pane research (research-wayland.md, research-wio.md).

---

## 1. What Smithay Actually Provides

Smithay v0.7.0 is a modular Wayland compositor library. It is not a compositor and it is not a framework — it is a collection of building blocks that a compositor author selects and integrates. The distinction matters: smithay does not impose a window management model, a rendering strategy, or an application lifecycle. You pick the modules you need.

### What you get

**Wayland protocol handling.** This is the big one. Smithay implements ~40+ Wayland protocol interfaces through a delegate pattern: `*State` structs register globals on the display, `*Handler` traits define compositor-specific behavior, `delegate_*!` macros wire the dispatch. The protocol implementations cover:

- Core: wl_compositor, wl_surface, wl_subsurface, wl_seat, wl_output, wl_shm, wl_data_device
- Shell: xdg-shell (toplevel, popup, positioner), wlr-layer-shell
- Buffers: linux-dmabuf (zero-copy GPU buffer sharing), wl_shm_pool
- Input: pointer constraints, relative pointer, input method, virtual keyboard, tablet
- Display: fractional-scale, viewporter, presentation-time
- Desktop: session lock, idle notify, foreign toplevel list, xdg-activation
- Data: data control (clipboard manager access)
- X11: XWayland integration, xwayland-shell

Each protocol implementation is a self-contained module. You don't get xdg-shell handling by accident — you enable the feature and implement the handler trait. The implementations are tested against real clients (GTK, Qt, Firefox, Electron).

**Backend abstraction.** The backend module handles OS interaction:

- `drm`: DRM/KMS mode setting, atomic commits, page flipping, plane management
- `libinput`: input device discovery, event normalization, pointer acceleration
- `egl`: OpenGL context creation and management
- `gbm`: GPU buffer allocation
- `session`: login session management (libseat), VT switching
- `udev`: device discovery and hotplug
- `winit`: development backend (run as a window)
- `x11`: development backend (run as an X11 client)
- `allocator`: buffer allocation traits with GBM and Vulkan implementations

**Renderer.** Smithay provides a `Renderer` trait with these implementations:

- `GlesRenderer`: OpenGL ES 2 rendering — the primary path
- `PixmanRenderer`: CPU-based software rendering (testing/fallback)
- `MultiRenderer`: multi-GPU support (different renderer per output)

The `Renderer` trait requires: `render()` (begin a frame), `wait()` (sync), texture import from DMA-BUF/SHM/EGL, cleanup. The `Frame` trait requires: `clear()`, `draw_solid()`, `render_texture_at()`, `render_texture_from_to()`, `finish()`. These are the compositing primitives — the compositor draws client buffers as textures onto the output framebuffer.

**Desktop helpers.** Higher-level window management helpers for surface tracking, layer management, and space organization. Optional.

**Event loop.** Built on calloop (callback-oriented epoll wrapper). All I/O sources — Wayland client connections, DRM events, libinput events, timers — are integrated into a single calloop event loop.

### What you don't get

Smithay does not provide: a window manager, a layout algorithm, a decoration renderer, a configuration system, a shell UI, an application launcher, or any opinion about what the desktop should be. It gives you protocol plumbing and hardware abstraction. The personality is yours.

### The size of what smithay replaces

The Haiku app_server is ~80K lines (60K .cpp + 20K .h, 253 files). But the app_server is not comparable to smithay alone. The app_server combines:

1. **Display server protocol handling** (ServerWindow, ServerApp — the equivalent of smithay's Wayland protocol modules): ~12K lines (ServerWindow.cpp: 4631, ServerApp.cpp: 3828, Desktop.cpp: 3931)
2. **Input handling** (InputManager, etc.)
3. **Drawing engine** (Painter, AGG renderer, drawing modes, hardware interface): ~20K+ lines
4. **Window management** (Desktop, Workspace, Window, decorations)
5. **Font management** (GlobalFontManager, etc.)
6. **Bitmap management** (BitmapManager, overlays)

Smithay replaces items 1-3 almost entirely. In BeOS terms: smithay is the app_server's BPortLink protocol layer + the HWInterface + the InputManager + the DrawingEngine's hardware abstraction. What's left for pane-comp is the Desktop/Workspace/Window management logic, the decoration rendering (chrome), and the pane-specific protocol.

The honest comparison: smithay replaces maybe 30-40K lines of C++ equivalent work. The remaining 40-50K lines of app_server-equivalent functionality — window management, decoration rendering, layout — are what pane-comp needs to write regardless.

---

## 2. What Smithay Constrains

### The `!Send` question

This is the most important architectural constraint. Smithay's Wayland protocol state types are `!Send` — they cannot be moved across thread boundaries. This is not an accident. The underlying wayland-server `Display` is `Send + Sync`, but smithay's protocol handler state wraps Wayland resources (which are `!Send` because libwayland-server's object model is single-threaded). All protocol dispatch, all surface state access, all Wayland event sending must happen on the main calloop thread.

Pane's architecture wants per-pane server-side threads (the ServerWindow model). How does this interact?

The spec already addresses this correctly (section 3, pane-comp threading model):

> Per-pane threads process protocol messages and communicate with the main thread via channels; they never touch smithay objects directly (smithay is `!Send` by design, which correctly confines Wayland protocol handling to the main thread).

This is the right architecture. The three-tier model works:

| Tier | Thread | Touches smithay? |
|---|---|---|
| Compositor main (calloop) | Single thread | Yes — all Wayland protocol handling, compositing, DRM submission |
| Dispatcher (1 per connection) | Dedicated thread | No — reads from unix socket, demuxes to per-pane channels |
| Pane thread (1 per pane) | Dedicated thread | No — processes pane protocol messages, communicates with main via channels |

The `!Send` constraint does not conflict with pane's threading model because the threading boundary is clean: pane protocol messages flow on dedicated threads, Wayland protocol handling stays on the main thread, and channels bridge the two. This is exactly how Haiku's app_server worked — ServerWindow threads processed client messages and communicated with the Desktop thread (shared state coordinator) via messages, never touching the display hardware directly.

**Verdict on !Send:** Non-issue. The constraint correctly reflects that Wayland protocol state is inherently single-threaded, and pane's architecture already accounts for this.

### The calloop coupling

Smithay requires calloop for its event loop. Every I/O source (DRM page flip completion, libinput events, Wayland client connections, timers) is registered as a calloop source. The compositor's heartbeat is `calloop::EventLoop::dispatch()`.

The pane spec says calloop is "scoped to the compositor only" and "does not define the system-wide concurrency model." This is feasible because:

1. Only pane-comp uses calloop. Other servers (pane-roster, pane-store, pane-watchdog) use std::thread + channels.
2. The pane-app kit (client-side) uses a threaded looper, not calloop.
3. calloop is confined to the single main thread of pane-comp.

The only potential friction is integrating par session type futures with calloop. calloop has a futures executor module that can drive async futures within its event loop. The spec flags this as an open question (Phase 2 milestone). This is a real integration challenge but it's not a smithay-specific problem — it's a calloop + par problem, and calloop is a much simpler and more replaceable dependency than smithay.

**Verdict on calloop:** Acceptable scope. calloop is small (~3K lines), well-maintained, and confined to one process.

### The GLES renderer

Smithay's primary renderer is GLES 2. There is also a Pixman (software) renderer and a multi-GPU renderer. There is no Vulkan renderer and no wgpu renderer in smithay itself.

The renderer is used for compositing — taking all client buffers (imported as textures), drawing compositor UI elements, and producing the output framebuffer. The compositor needs to:

1. Import client buffers as textures (from SHM or DMA-BUF)
2. Draw each buffer at the right position, with the right transform and scale
3. Draw compositor chrome (tag lines, borders, focus indicators)
4. Submit the result to DRM/KMS

GLES 2 is sufficient for all of this. It's what sway/wlroots use (wlroots' primary renderer is also GLES 2). GLES 2 is universally supported on Linux GPUs. It is not technically ambitious — it's the conservative, proven choice.

The question is whether GLES 2 is the *right* choice for pane's chrome rendering, which includes:

- Beveled borders with gradients
- Tag line text rendering (editable, with cursor and selection)
- Focus indicators
- Translucent floating elements
- The Frutiger Aero aesthetic

GLES 2 can do all of this. It's not the most elegant way — you're writing or using shaders for rounded corners, gradients, text from an atlas texture. But it works. niri renders its entire UI (animations, rounded corners, drop shadows, window close animations) through smithay's GLES renderer. COSMIC renders its entire desktop through smithay's GLES renderer.

---

## 3. The Rendering Split Problem

Here is the real tension.

### The current state

Pane has two rendering contexts:

1. **Compositor compositing** (pane-comp): smithay's GlesRenderer. Composites client buffers, draws chrome. GLES 2.
2. **Widget rendering** (pane-ui, client-side): Vello via wgpu. GPU-compute 2D rendering for widgets, the Frutiger Aero controls.

These are different GPU APIs in different processes. This is actually fine from a Wayland perspective — every Wayland compositor has this split. The compositor uses one rendering approach; GTK uses Cairo/Vulkan; Qt uses its own renderer; Firefox uses WebRender. Each client renders into buffers; the compositor composites those buffers. The rendering APIs don't need to match.

### Where it gets interesting

The chrome — tag lines, borders, focus indicators — is rendered by the compositor, not by clients. This chrome is pane's visual identity. It needs to look good. The question is whether GLES 2 (smithay's renderer) is adequate for the chrome, or whether the chrome rendering should use the same Vello/wgpu stack as the widget rendering.

Arguments for keeping GLES chrome rendering:
- Simpler architecture (one renderer in the compositor)
- Proven path (niri, COSMIC do this)
- Chrome is geometrically simple (rectangles, borders, text from atlas)
- Avoids wgpu integration complexity in the compositor

Arguments for wgpu chrome rendering:
- Unified rendering stack (same shaders, same API for chrome and widgets)
- Vello is designed for exactly this — 2D vector rendering with GPU compute
- Gradients, rounded corners, translucency, text rendering are all Vello strengths
- Future-proofs against GLES deprecation (Vulkan is the future; wgpu abstracts over it)
- The Frutiger Aero aesthetic benefits from a high-quality 2D renderer

### What the lamco-wgpu project tells us

smithay issue #928 discusses wgpu as a compositor renderer. The key findings:

**2023 assessment (smithay maintainer i509VCB):** "wgpu lacks what is needed to be a first class renderer that I'd consider suitable for inclusion into Smithay." Missing: colorspace management, YUV buffer support, DMA-BUF import, sync fd export, low-level Vulkan extension control.

**2026 state (lamco-wgpu project):** A working implementation that demonstrates:
- Vulkan-based DMA-BUF importing (via raw Vulkan, VK_EXT_external_memory_dma_buf)
- Smithay Renderer trait implementation
- Explicit sync support (wgpu PR #6813, merged January 2025)
- NV12/P010 multi-planar format support

The blockers from 2023 are largely resolved. DMA-BUF import works. Explicit sync works. The remaining gap is that wgpu's API for these features requires reaching through to wgpu-hal (the hardware abstraction layer) for raw Vulkan access, rather than using wgpu's safe high-level API. This is workable but requires Vulkan expertise.

**smithay maintainer response (Victoria Brekenfeld, 2026):** Raised concerns about code quality and AI-generated code in lamco-wgpu. The project exists and works but is not production-quality.

### The compositing/chrome split option

There's a middle path that avoids replacing smithay's compositor renderer entirely:

1. Use smithay's GlesRenderer for **compositing** — importing client buffers, positioning them, submitting to DRM/KMS
2. Use wgpu/Vello for **chrome rendering** — pre-render chrome elements (tag lines, borders) into textures, import them into the GLES compositor as textures

This is how a game engine might work: render UI elements with one API, composite the scene with another. The chrome rendering happens off the main compositing path, produces textures, and those textures are just more inputs to the GLES compositor.

Cost: two GPU contexts in the compositor process. Benefit: Vello-quality chrome without replacing the entire compositing pipeline.

---

## 4. The wlroots Comparison

wlroots is the C library that powers sway, river, labwc, wayfire, and most of the non-GNOME/KDE Wayland compositor ecosystem. It's the established alternative to smithay.

### wlroots advantages
- More mature (started 2017, powers daily-driver compositors since 2018)
- Larger contributor base (sway alone has 300+ contributors)
- Battle-tested on diverse hardware
- Powers the largest Wayland tiling compositor (sway, ~14K stars)
- More complete protocol coverage
- Direct scanout optimization is well-implemented

### smithay advantages
- Rust (memory safety, Send/Sync enforcement, no UB in compositor code)
- More modular (you import traits and modules, not a monolithic library)
- Growing rapidly (niri: 21.6K stars; COSMIC: System76's entire desktop)
- Active development (v0.7.0, 106 contributors, ~4K commits)
- Better fit for pane (Rust codebase, Cargo ecosystem, par integration)

### smithay's bus factor

This is a legitimate concern. Examining the top contributors:

- **Drakulix (Victor Berger)**: original creator, appears to be less active recently
- **cmeissl (Christian Meissl)**: very active, appears to be the current primary maintainer
- **elinorbgr**: early contributor
- **PolyMeilex**: significant recent contributor
- **ids1024**: significant contributor (also works on COSMIC)

The bus factor is approximately 2-3. If cmeissl and PolyMeilex left, smithay would be in serious trouble. However:

1. System76 depends on smithay for COSMIC. They have paid engineers working on it (ids1024 at minimum). This is institutional investment, not just volunteer labor.
2. niri is a high-profile user that drives bug reports and testing.
3. The Rust Wayland ecosystem has no alternative — if smithay falters, the entire Rust compositor ecosystem has a problem, which creates collective maintenance pressure.

Compare to wlroots: bus factor is also small (Drew DeVault stepped back; Simon Ser is the primary maintainer). All compositor libraries have this problem because the domain is niche and the expertise is specialized.

**Verdict on bus factor:** Real risk, mitigated by System76's institutional investment and the "no alternative" dynamic in the Rust ecosystem. Not worse than the alternatives.

---

## 5. Building From Scratch

What would pane need to implement without smithay?

### The components

**Wayland protocol handling.** wayland-server (the Rust crate, maintained by the same people as smithay) provides the low-level protocol dispatch: Display, Dispatch trait, Global registration, client management. You still need to implement every protocol interface yourself.

What smithay provides that wayland-server alone doesn't:

- wl_compositor + surface state tracking (buffer management, damage accumulation, double-buffered state, subsurface tree, synchronized/desynchronized commit handling)
- xdg-shell (toplevel lifecycle, popup positioning with constraint adjustment, configure/ack negotiation)
- wl_seat (keyboard focus tracking, pointer enter/leave, input frames, cursor management, key repeat info, modifier tracking)
- wl_shm (buffer pool management, format negotiation)
- linux-dmabuf (multi-plane buffer import, modifier negotiation, feedback tranches)
- wl_data_device (clipboard source/offer lifecycle, drag-and-drop grab semantics, MIME type negotiation)
- All extension protocols (layer-shell, session-lock, fractional-scale, viewporter, presentation-time, input-method, pointer-constraints, etc.)

This is thousands of lines of protocol implementation per interface. The xdg-shell configure/ack state machine alone is subtle and full of edge cases. Clipboard is notorious. DMA-BUF modifier negotiation is a minefield.

Estimated effort to reimplement smithay's Wayland module: **20-40K lines of Rust**, 6-12 months of focused work for an experienced developer. Not including testing against real clients.

**DRM/KMS backend.** The drm-rs crate provides raw ioctl wrappers. You need to build:

- Device enumeration (via udev or scanning /dev/dri)
- Connector/CRTC/plane pipeline configuration
- Atomic commit assembly and submission
- Page flip event handling (vblank callbacks)
- Multi-monitor layout management
- VT switching (session management via libseat)
- Buffer allocation (via gbm-rs or Vulkan allocator)
- Framebuffer creation from allocated buffers

Estimated effort: **5-10K lines**, 2-4 months.

**Input handling.** input.rs (libinput bindings) exists. You need:

- Device discovery and lifecycle management
- Event dispatch (pointer, keyboard, touch, tablet)
- xkbcommon integration for keyboard layout processing
- Pointer acceleration configuration
- Cursor management and rendering
- Seat abstraction (grouping related input devices)

Estimated effort: **3-5K lines**, 1-2 months.

**Rendering.** If going wgpu:

- wgpu device/surface setup for DRM output
- Texture import from DMA-BUF (requires wgpu-hal Vulkan interop, as lamco-wgpu demonstrates)
- Texture import from SHM (straightforward)
- Compositing shader (position, transform, blend client textures)
- Chrome rendering (can use Vello)
- Output submission to DRM scanout

Estimated effort: **5-8K lines**, 3-6 months. The DMA-BUF import is the hardest part — it requires raw Vulkan and is the area where lamco-wgpu's existence is most valuable.

### Total from-scratch estimate

**33-63K lines of Rust, 12-24 months** for a single experienced developer to reach feature parity with what smithay provides. And "feature parity" means "handles the common cases" — the long tail of hardware quirks, client misbehavior, and protocol edge cases takes years of real-world deployment to flush out.

For comparison, pane-comp currently exists as ~450 lines rendering a hardcoded pane via smithay's winit backend. The gap between that and a functioning compositor is enormous regardless of whether smithay is used.

---

## 6. The Be Engineers' Perspective

At Be, we built the app_server from scratch. We had no choice — we were building a kernel, a display driver API (accelerant), a client-server protocol (BPortLink), a threading model (one thread per window), and a rendering engine (originally software, later hardware-accelerated through the accelerant API). There was no existing compositing infrastructure to build on because we were the infrastructure.

But here's the thing people forget: we didn't build everything from scratch by preference. We used a software rendering library (AGG — Anti-Grain Geometry) for the 2D drawing engine. We used FreeType for font rasterization. We used the kernel's virtual memory system for shared-memory buffer passing between clients and the server. We built what we had to build and used existing components where they fit.

The question for pane is: **what does pane need to control, and what can it delegate?**

What pane needs to control:
- The threading model (per-pane threads — this is a core design requirement)
- The layout tree (tiling, tag-based visibility — this is pane's window management policy)
- The chrome rendering (tag lines, borders — this is pane's visual identity)
- The pane protocol (session-typed communication — this is pane's programming model)
- The input dispatch (key binding grammar, modal input — this is pane's interaction model)

What pane does NOT need to control:
- Wayland protocol parsing and dispatch (this is plumbing, not personality)
- DRM/KMS mode setting (this is hardware abstraction, not design)
- libinput event normalization (this is device driver territory)
- DMA-BUF import mechanics (this is GPU buffer plumbing)
- xdg-shell configure state machine (this is Wayland compliance, not innovation)

The personality of a compositor is in its window management, its shell UI, its input handling philosophy, and its integration model. None of these are constrained by smithay. All of them are things pane needs to build regardless.

At Be, we would have used smithay. We would have treated it exactly the way we treated AGG in the drawing engine or FreeType in the font system — a solid implementation of a well-defined problem that we could rely on while focusing our energy on the things that made the system distinctive.

---

## 7. The Vello/wgpu Question Specifically

The spec says Vello (wgpu) for widget rendering. The compositor uses GLES via smithay. This split deserves specific analysis.

### What Vello is

Vello is a 2D vector graphics renderer that uses GPU compute shaders (via wgpu) instead of the traditional rasterization pipeline. It's from the Linebender project (Raph Levien and team). It's designed for high-quality 2D rendering: text, paths, gradients, blending, clipping. Think "what if the GPU rendered 2D graphics the way a GPU should, not the way OpenGL bolted onto a 3D pipeline."

### The rendering split is normal

On Wayland, the compositor renderer and client renderers are always different. Firefox uses WebRender (now Wr), GTK4 uses Vulkan or GL, Qt uses its own RHI. The compositor uses GLES to composite their output. Nobody expects these to be the same renderer.

The chrome rendering is the only place where the question matters, because the chrome is rendered by the compositor. Options:

**Option A: Chrome via GLES (current smithay path).** Tag lines, borders, and focus indicators are drawn using smithay's `Frame::draw_solid()`, `Frame::render_texture_from_to()`, and custom GLES shaders. Text rendering uses a glyph atlas uploaded as a texture.

Pros: Simple. One GPU context. Proven.
Cons: GLES 2 shader authoring for the Frutiger Aero aesthetic (gradients, rounded corners, translucency) is manual and fiddly. No GPU-compute text rendering.

**Option B: Chrome via Vello, composited via GLES.** A wgpu context in the compositor renders chrome elements to textures. Those textures are imported into the GLES renderer and composited alongside client buffers.

Pros: Vello-quality chrome. Unified visual language between chrome and widget rendering. Better text rendering for tag lines.
Cons: Two GPU contexts in one process. Texture sharing between wgpu and GLES (possible via DMA-BUF but requires careful setup). More complex initialization.

**Option C: Everything via wgpu.** Replace smithay's GLES renderer with a wgpu-based renderer (a la lamco-wgpu). Both compositing and chrome use the same wgpu context.

Pros: One rendering API for everything. Maximum coherence. Vulkan underneath (future-proof).
Cons: lamco-wgpu is immature. DMA-BUF import via raw Vulkan is complex. Losing smithay's tested GLES path means taking on all the GPU edge cases yourself. Multi-GPU support is harder.

### My recommendation on rendering

**Start with Option A. Plan for Option B. Keep Option C in your back pocket.**

For Phase 4 (minimal compositor), Option A is the only sane choice. You need to render a tag line and some borders. GLES draw_solid and a glyph atlas texture are sufficient. This is where pane-comp is today, and it's the right level of ambition for getting to a usable shell.

When Phase 7 arrives (widget rendering, Frutiger Aero controls), the chrome quality question becomes real. At that point, evaluate whether the GLES chrome is good enough or whether the Vello-rendered widgets make the GLES chrome look cheap by comparison. If they do, Option B (Vello for chrome, GLES for compositing) is a clean upgrade path that doesn't require replacing the compositing infrastructure.

Option C only makes sense if smithay's GLES renderer becomes a bottleneck or if wgpu's compositor support matures significantly (lamco-wgpu or equivalent reaches production quality). Check the landscape in 2027.

---

## 8. Assessment and Recommendation

### The options, evaluated

**(a) Use smithay as-is and work within its constraints.**

This is what niri and COSMIC do. It works. The constraints (!Send, calloop, GLES) are not actually in conflict with pane's architecture because:

- The threading model works: per-pane threads handle pane protocol; main thread handles Wayland protocol
- calloop is scoped to the compositor as specified
- GLES is adequate for compositing

Risk: smithay's bus factor. Mitigation: System76's institutional investment.

**(b) Use smithay but plan to replace components as needs diverge.**

This is Option B from the rendering section applied more broadly. Use smithay's protocol handling and backends, but own the renderer and potentially the input pipeline.

The problem: smithay's modules are integrated. Replacing the renderer means implementing `Renderer` and `Frame` traits yourself — which is what lamco-wgpu does. This is feasible but you're maintaining a fork-like relationship with smithay's rendering expectations.

**(c) Build directly on wayland-rs + drm-rs + wgpu.**

12-24 months of infrastructure work before you can render a window with a tag line. Every month spent on Wayland protocol compliance is a month not spent on what makes pane distinctive. The dependency philosophy says "choose the best option for our design model with confidence in future support or maintainability." Building from scratch is not the best option when smithay exists — it's the most expensive option for the least architectural benefit.

The Be comparison is instructive: we built app_server from scratch because *no compositor library existed*. If AGG had been a full display server library with protocol handling, input dispatch, and hardware abstraction, we would have used it and focused our energy on the things that made the system special.

**(d) Something else.**

The real answer is a variant of (a) with a clear boundary:

### Recommendation

**Use smithay fully for what it's good at. Build the pane-specific layers on top. Don't fight the framework; focus energy on the personality layer.**

Concretely:

1. **Wayland protocol handling:** smithay, completely. Every protocol module. This is pure infrastructure that pane needs but should not build. Accept the delegate pattern, implement the handler traits, move on.

2. **Backend (DRM, input, session, udev):** smithay, completely. This is hardware abstraction. Pane has no reason to own this.

3. **Compositing renderer:** smithay's GlesRenderer for now (Phase 4-6). Evaluate wgpu compositing (Option C) when the ecosystem matures, but do not block progress on it.

4. **Chrome rendering:** GLES via smithay initially. Upgrade to Vello/wgpu if chrome quality demands it (Option B, Phase 7+). The chrome rendering is behind the `Frame` trait — it can be swapped without restructuring the compositor.

5. **Per-pane threads:** pane's own implementation, communicating with the main thread via channels. This is the architectural layer that makes pane pane, and smithay doesn't constrain it.

6. **Pane protocol server:** pane's own implementation, on the dispatcher/pane threads. Completely independent of smithay.

7. **Layout tree, input grammar, tag system:** pane's own implementation. This is the personality.

The boundary is clean: smithay owns the Wayland-facing side. Pane owns the pane-facing side. The main thread is the bridge — it runs calloop, handles smithay protocol dispatch, and communicates with per-pane threads via channels.

### What this means for the dependency philosophy

The dependency philosophy says: "choose the best option for our design model with confidence in future support or maintainability."

smithay is the best option for Wayland compositor infrastructure in Rust. It has institutional backing (System76), active development, and a growing user base. The constraints it imposes (!Send, calloop, GLES) are either non-issues or manageable.

The philosophy also says: "We are futureproofing, not backward-compatible." The futureproofing argument for building from scratch would be: "we'll need to replace smithay eventually, so start without it." But this argument is wrong. Pane's innovations are not in the Wayland protocol layer. They're in the pane protocol, the threading model, the layout system, the input grammar, the kit programming model. None of these are constrained by smithay. Building from scratch to own the Wayland plumbing is optimizing the wrong thing.

If smithay ever becomes a constraint — if the !Send boundary makes the threading model unworkable, if the GLES renderer blocks a critical rendering feature, if the maintainers abandon the project — pane will know, and the wayland-rs + drm-rs + wgpu path will still be there. But starting there is premature optimization at the infrastructure level, and it would consume the project's limited engineering bandwidth on exactly the work that doesn't differentiate it.

**Build pane, not a Wayland compositor library.**

---

## 9. Specific Spec Changes Implied

Based on this analysis, the architecture spec's treatment of smithay is largely correct. Specific adjustments:

1. **Technology choices table (section 11):** Add a note that the GLES renderer is the Phase 4-6 choice, with wgpu compositing (via lamco-wgpu or equivalent) as a Phase 7+ evaluation point.

2. **Open questions (section 13):** The "Widget rendering performance" question should be reframed. The question is not "can Vello integrate with smithay's GLES compositor" (it can — they're in different processes, communicating via buffers). The real question is whether the *chrome rendering* should use Vello, and the answer is "evaluate when you get there."

3. **The rendering model (section 10):** Add explicit acknowledgment of the compositing/chrome rendering distinction. The compositor composites via GLES (smithay). Chrome rendering may eventually use Vello/wgpu. Client widget rendering uses Vello/wgpu (in-process, via the kit). These are three rendering contexts, not one, and that's fine — Wayland was designed for this.

4. **Build sequence:** No changes needed. Phase 4 (minimal compositor with smithay) is the right starting point. The transport bridge (Phase 2) is correctly prioritized over renderer questions.

---

## Sources

- smithay repository: https://github.com/Smithay/smithay (v0.7.0, ~4K commits, 106 contributors)
- smithay docs: https://docs.rs/smithay/0.7.0/smithay/
- smithay issue #928: wgpu backend support (discussion of blockers and lamco-wgpu implementation)
- wayland-server crate: https://docs.rs/wayland-server/0.31.12/wayland_server/ (Display is Send+Sync, dispatch requires &mut self)
- drm crate: https://docs.rs/drm/latest/drm/
- lamco-wgpu: https://github.com/lamco-admin/lamco-wgpu (wgpu Renderer trait impl for smithay)
- niri: https://github.com/YaLTeR/niri (21.6K stars, smithay-based tiling compositor, uses git HEAD of smithay)
- COSMIC compositor: https://github.com/pop-os/cosmic-comp (System76's smithay-based desktop)
- Haiku app_server: ~/src/haiku/src/servers/app/ (253 files, ~80K lines C++/headers)
- Haiku ServerWindow.cpp (4631 lines), ServerApp.cpp (3828 lines), Desktop.cpp (3931 lines)
- Pane architecture spec: openspec/specs/architecture/spec.md
- Pane Wayland research: openspec/changes/spec-tightening/research-wayland.md
- Pane wio research: openspec/changes/spec-tightening/research-wio.md
