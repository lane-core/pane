---
type: reference
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [pane-compositor, rio, draw_protocol, wctl, input_routing, nesting, compositor, Dev_multiplexer, buffer_based]
related: [architecture/kernel, architecture/router, architecture/fs, architecture/proto, reference/plan9/man_pages_insights, reference/plan9/papers_insights, agent/plan9-systems-engineer/pane_kernel_design_consultation]
agents: [plan9-systems-engineer]
---

# pane-compositor Design Consultation (2026-04-12)

Seven-section analysis mapping rio's architecture to pane-compositor on the pane-kernel substrate.

## Key decisions proposed

1. **Buffer-based compositor (option b), not command-based draw protocol.** Inferno lesson: draw device translation to X11/Win32 was painful. Modern GPU pipelines assume buffer submission. Draw protocol elegance doesn't survive commodity hardware. Command-based rendering lives inside future pane-kit toolkit, not at compositor interface. High confidence.

2. **Two-tier window management: WctlDev + WindowManagement Protocol.** Text commands via Dev trait (pane-fs `/pane/<id>/ctl`) for scripting, typed messages for applications. Same two-tier pattern as pane-kernel. Derives text format from typed API. High confidence.

3. **Compositor routes input, router filters.** Compositor decides routing target (focus, geometry). Router applies security policy (permit/deny/transform/audit). Input filter chain (BInputServerFilter heir) runs at compositor level before routing. Clean separation: compositor handles spatial routing, router handles policy. High confidence.

4. **Nesting works via Dev trait uniformity.** Inner compositor consumes virtual devices from outer compositor (same trait). Per-pane DeviceRegistry replaces per-process namespaces. Performance constraint: one buffer copy per nesting level. Medium-high confidence.

5. **Compositor IS a pane application + ProtocolServer.** Multiplexer-as-reproducer: consumes Dev devices, provides virtual Dev devices. Same calloop event loop, same Protocol messages. Other panes connect to it via ProtocolServer.

## Rio concepts preserved

- wctl text protocol for scriptable window management
- Blocking reads for state change notification (pane-fs event file)
- consctl close-to-revert lease pattern (RAII mode handles)
- Snarf delegation in nesting (Clipboard Locality::Federated)
- Minimal chrome (borders, tag lines, menus — compositor draws chrome, app draws content)
- Multiplexer-as-reproducer symmetry

## Rio concepts improved

- Structured error reporting from ctl (EINVAL + reason, not generic string)
- Compositor-level event stream (new pane, exit, focus, layout changes)
- Layout protocol for atomic constraint-based management (beyond rio's floating-only)
- Multi-monitor support from day one
- Damage tracking for power efficiency
- Focus stealing prevention (security policy via pane-router)
- Accessibility tree alongside visual surface
- Keyboard shortcuts for window management

## Crate structure proposed

compositor.rs (main struct), surface.rs (buffer/damage), layout.rs (geometry/z-order), input.rs (focus routing), chrome.rs (tag/borders), wctl.rs (WctlDev impl), virtual_display.rs, virtual_input.rs, protocols/ (WindowManagement, surface, layout), platform/ (wayland/cocoa/headless).
