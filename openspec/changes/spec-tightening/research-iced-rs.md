# Research: iced-rs as Widget Rendering Backend for pane-ui

Initial assessment. Sources: iced GitHub ([repo](https://github.com/iced-rs/iced), [issues](https://github.com/iced-rs/iced/issues/1192), [discussions](https://github.com/iced-rs/iced/discussions/1214)), [DeepWiki architecture docs](https://deepwiki.com/iced-rs/iced/9.4-integration-and-custom-rendering), [libcosmic](https://github.com/pop-os/libcosmic), pane architecture spec.

---

## Quick answers

### 1. Can iced render into buffers for a compositor to composite?

**Not without significant surgery.** Iced's architecture assumes it owns the window via winit and composites to a display surface. There is no blessed path for "render this widget tree into an offscreen buffer I give you." The `iced_tiny_skia` backend does CPU rendering, which could theoretically target an arbitrary buffer, but the runtime/shell layer (`shell::run()`) wraps the entire lifecycle and expects to drive the event loop. You'd need to dismantle the shell layer and drive the renderer + layout manually.

The [screenshot API](https://docs.iced.rs/iced/type.Renderer.html) exists for snapshotting, but it's a capture mechanism, not a render-to-buffer pipeline.

### 2. Can iced be used as a library inside another event loop?

**This is the fundamental problem.** Iced wants to own `main()`. Its `iced::run()` / `iced::application()` entry points take over the event loop via winit. There is no `iced::update_once(state, events) -> frame` API that you call from your own loop.

People have [asked for this](https://github.com/iced-rs/iced/issues/1192). The answer is "not really, but there are workarounds." The workarounds involve either:
- Forking the shell layer (what COSMIC did — they wrote [cctk](https://github.com/pop-os/libcosmic) to bridge iced into their compositor's Wayland world, but even they use iced's event loop, not their own)
- Using `pump_app_events()` from winit's extension trait to manually pump, which is fragile and still assumes winit owns the window

For pane, this is a dealbreaker as currently architected. pane-ui clients render into wl_surface buffers managed by the kit. The kit controls buffer lifecycle (memfd/DMA-BUF allocation, double-buffering, wl_surface.attach/damage/commit). Iced would need to render into those buffers on demand, driven by pane's frame pacing, not by its own event loop. That's fighting the framework, not using it.

### 3. Can iced's look be customized to Frutiger Aero?

**Partially.** Iced has a [theming system](https://docs.rs/iced/latest/iced/theme/index.html) based on `Palette` + per-widget `StyleSheet` traits. You can customize colors, backgrounds, borders. There is a [custom shader widget](https://docs.rs/iced/latest/iced/widget/shader/index.html) for wgpu-level rendering.

But the Frutiger Aero spec requires beveled edges, directional gradients (light-top, dark-bottom), thin highlight/shadow lines — per-control depth cues. Iced's built-in widgets produce flat or minimally styled output. You'd need to reimplement every widget's `draw()` method to produce the right visual treatment. At that point, you're not using iced's widget library — you're using iced's layout engine (which is just flexbox, same as taffy) and its rendering abstraction, then doing all the visual work yourself.

Gradients exist in iced but are background fills, not the fine-grained edge lighting the aesthetic spec demands.

### 4. Does iced work with per-pane threading?

**No.** Iced assumes [one event loop per process](https://github.com/iced-rs/iced/issues/996), driven by winit, which requires `EventLoop::new` on the main thread. Multiple windows are supported within that single event loop (via `iced_multi_window`), but that's one iced instance managing N windows, not N independent iced instances on N threads.

Pane's model is: each pane is a separate Wayland client (or a sub-surface of one), potentially on its own thread, rendering independently. You'd need N iced runtimes, which means N winit event loops, which winit doesn't support cleanly. `EventLoopExtWindows::new_any_thread` exists but is explicitly "you're on your own" territory.

### 5. iced vs Vello+taffy: control and shipping speed

| | iced | Vello + taffy |
|---|---|---|
| **Layout** | flexbox (iced_core) | flexbox + grid (taffy) |
| **Rendering** | wgpu or tiny_skia | wgpu (GPU-compute 2D) |
| **Event loop** | Owns it (winit) | You own it |
| **Widget library** | ~30 built-in widgets | None (build from scratch) |
| **Buffer target** | Display surface | Arbitrary (you choose) |
| **Customization floor** | Restyle existing widgets | Build widgets with full control |
| **Threading model** | Single event loop | Whatever you want |

**Control:** Vello+taffy gives total control. You own the event loop, the buffer lifecycle, the rendering pipeline, the widget drawing. This is what pane needs — the kit IS the framework, not a layer on top of someone else's framework.

**Shipping speed:** Iced ships a TODO app faster. But pane isn't building a TODO app. The cost of fighting iced's assumptions (event loop ownership, buffer targets, threading model, visual customization) would exceed the cost of building widgets on Vello+taffy within the first month of real UI work. The widget set pane needs initially is small: button, text input, label, list, container, scroll. That's 2-3 weeks on Vello+taffy with full control over appearance from day one.

---

## The BeOS parallel

The Interface Kit was the rendering infrastructure AND the widget library AND the visual language, all in one package. It didn't wrap someone else's toolkit — it was the toolkit, built on top of app_server's drawing primitives. BView::Draw() gave you a BRect and a drawing context. You drew. The kit's built-in controls (BButton, BTextView, BListView) drew themselves using the same primitives any developer could use.

The key property: **the kit and the rendering infrastructure were the same thing.** There was no impedance mismatch between "what the kit wants" and "what the renderer does" because they were designed together.

Iced would introduce exactly that impedance mismatch. Iced has its own idea of what a widget is, how layout works, how events flow, how rendering happens. Pane has a different idea — one that's closer to BeOS. Using iced means constantly negotiating between two models. Using Vello+taffy means building one model that's exactly what pane needs.

COSMIC's experience is instructive: System76 built [libcosmic](https://github.com/pop-os/libcosmic) on top of iced and has spent substantial engineering effort bridging iced's model to their compositor's reality. They wrap every iced widget in COSMIC-specific styling. They maintain their own Wayland integration layer. They use iced's event loop. For them, this made sense — they were building desktop apps that happen to run on their compositor, and iced gave them cross-platform portability. Pane doesn't need cross-platform. Pane needs total control over the rendering pipeline for one platform.

---

## Verdict

**Don't use iced.** The architecture mismatch is fundamental, not incidental:

1. Iced owns the event loop; pane needs to own the event loop
2. Iced renders to display surfaces; pane needs to render to compositor-managed buffers
3. Iced's threading model is single-loop; pane's is per-component
4. Iced's visual language is flat/modern; pane's is beveled/depth-cued
5. Iced's widget model is its own; pane's widget model needs to be its own

Each of these could be worked around individually. Together, they mean you'd spend more effort adapting iced than building the widget layer you actually need on Vello+taffy.

**The Vello+taffy plan is correct.** It gives pane what the Interface Kit gave BeOS: a rendering infrastructure that's designed for the system, not adapted to it. The cost is building widgets from scratch. The payoff is that those widgets are exactly right — right rendering model, right threading model, right visual language, right buffer management — from the beginning.
