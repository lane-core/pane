# Smithay Viability Assessment (2026-03-20)

Assessed smithay v0.7.0 as pane's compositor framework.

## Key findings

1. **!Send constraint is non-issue.** Pane's three-tier threading model already accounts for it. Wayland protocol state stays on main thread; pane protocol messages flow on dedicated threads; channels bridge the two. Same architecture as Haiku's app_server.

2. **Building from scratch: 12-24 months** for ~33-63K lines, mostly Wayland protocol compliance that doesn't differentiate pane. The personality (layout tree, input grammar, pane protocol, chrome) must be built regardless.

3. **Rendering split (GLES compositor + Vello/wgpu client widgets) is normal** for Wayland. GLES for Phase 4-6, evaluate Vello for chrome in Phase 7+.

4. **wgpu as compositor renderer becoming viable** (lamco-wgpu demonstrates DMA-BUF import, explicit sync) but not mature enough to bet on today.

5. **Bus factor ~2-3** mitigated by System76's institutional investment (COSMIC desktop depends on it).

## Recommendation

Use smithay fully for Wayland infrastructure. Build pane's personality on top. The boundary is clean: smithay owns Wayland-facing side, pane owns pane-facing side.

Pane's innovations (pane protocol, threading model, layout system, input grammar, kit programming model) are unconstrained by smithay. Don't spend engineering bandwidth on Wayland plumbing.

**Reference:** Haiku app_server ~80K lines. smithay replaces ~30-40K lines equivalent. Remaining ~40K lines (window management, decoration, layout) is what pane-comp builds regardless.