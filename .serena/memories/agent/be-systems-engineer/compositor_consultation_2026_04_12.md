---
type: agent
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [compositor, app_server, rio, synthesis, threading, drawing, decoration, SSD, buffer_model, window_management, pane-compositor]
sources: [Desktop.h, ServerWindow.h, ServerApp.h, EventDispatcher.h, DrawingEngine.h, HWInterface.h, Decorator.h, Window.h, WindowBehaviour.h, ServerProtocol.h, MessageLooper.cpp, ServerWindow.cpp:2479, Window.cpp:770, EventDispatcher.cpp:752, Desktop.cpp:2622, benewsletter-wisdom.md]
verified_against: [~/src/haiku/ as of 2026-04-12]
related: [agent/be-systems-engineer/pane_kernel_design_consultation, reference/haiku/appserver_concurrency, reference/haiku/decorator_architecture, reference/haiku/internals, architecture/kernel, decision/host_as_contingent_server]
agents: [be-systems-engineer]
---

# pane-compositor Design Consultation (2026-04-12)

Seven-section analysis: app_server synthesized with rio for
pane-compositor.

## Core findings

### Thread model: single calloop actor + optional render thread

app_server needed N threads for N windows because the server
rendered content for each window. Wayland buffer model eliminates
this — clients render, compositor composites. Single-threaded
calloop actor owns all window management state (no MultiLocker
needed). Optional render thread for chrome + compositing if
frame budget requires it.

### Drawing model: buffer submission, not command protocol

app_server's ~370-opcode command model was right for vertical
integration but wrong for GPU-capable clients. pane adopts
Wayland buffer model (clients render to their own buffers).
Command protocol survives only inside the compositor for
decorator chrome rendering (vello/GLES, not a wire protocol).

### Decoration: SSD by default, escape hatch for borderless

Decorator trait maps directly from Haiku: footprint(), 
content_rect(), region_at(), render(). SSD gives consistent
chrome, responsive even when app hangs. NO_BORDER_LOOK for
fullscreen/custom-chrome applications.

### Compositor is a normal pane (option a)

Per host_as_contingent_server: no architectural privilege. Its
authority is contingent on access to real display/input hardware
(device registry) and namespace position (child panes receive
virtual devices from compositor).

### Key differences from app_server

1. No server-side view tree — clients own their rendering
2. No ~370 drawing opcodes — buffer submission only
3. No per-window server thread — single calloop actor
4. No shared framebuffer — each pane has its own buffer
5. No architectural privilege — normal pane
6. Decorator renders into compositor's render target, not shared fb

### Key inheritances from app_server

1. SSD decoration with pluggable Decorator trait
2. Workspace management (bitmask per window)
3. Window stacking (z-order, modal, floating, subset)
4. Focus policy + input routing via EventFilter
5. Window look/feel/flags vocabulary
6. Update session model (dirty regions, expose regions)

### Haiku source paths verified

All thread counts, class hierarchies, and data flows verified
against ~/src/haiku/src/servers/app/ headers and implementations.
MessageLooper pattern: Lock → _DispatchMessage → Unlock loop
in _MessageLooper() (MessageLooper.cpp:140-165). Drawing flow:
BView → AS_STROKE_LINE → ServerWindow::_DispatchViewDrawingMessage
→ DrawingEngine::LockParallelAccess → Painter → framebuffer.
Redraw flow: Window::RedrawDirtyRegion → _DrawBorder (decorator)
→ _TriggerContentRedraw (client update session).
