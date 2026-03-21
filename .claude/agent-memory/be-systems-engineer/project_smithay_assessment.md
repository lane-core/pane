---
name: Smithay viability assessment (2026-03-20)
description: Comprehensive evaluation of smithay as pane's compositor framework — architecture constraints, rendering split, build-vs-buy analysis, recommendation
type: project
---

Assessed smithay (v0.7.0) as pane's compositor framework. Written to `openspec/changes/spec-tightening/research-smithay-viability.md`.

Key findings:

1. **smithay's !Send constraint is non-issue.** Pane's three-tier threading model (calloop main thread, dispatcher threads, per-pane threads) already accounts for it. Wayland protocol state stays on main thread; pane protocol messages flow on dedicated threads; channels bridge the two. This is the same architecture as Haiku's app_server (ServerWindow threads + Desktop coordinator thread).

2. **Building from scratch would cost 12-24 months** for ~33-63K lines of Rust, mostly in Wayland protocol compliance that doesn't differentiate pane. The personality (layout tree, input grammar, pane protocol, chrome) must be built regardless.

3. **The rendering split (GLES compositor + Vello/wgpu client widgets) is normal and fine** for Wayland. The only question is chrome rendering quality. Recommended: GLES for Phase 4-6, evaluate Vello for chrome in Phase 7+.

4. **wgpu as compositor renderer is becoming viable** (lamco-wgpu demonstrates DMA-BUF import, explicit sync, Renderer trait impl) but not mature enough to bet on today. Keep in back pocket.

5. **smithay bus factor ~2-3** but mitigated by System76's institutional investment (COSMIC desktop depends on it) and "no alternative" pressure in Rust Wayland ecosystem.

6. **Recommendation: Use smithay fully for Wayland infrastructure. Build pane's personality on top.** The boundary is clean: smithay owns Wayland-facing side, pane owns pane-facing side.

**Why:** Pane's innovations are in the pane protocol, threading model, layout system, input grammar, and kit programming model — none constrained by smithay. Spending engineering bandwidth on Wayland plumbing is optimizing the wrong thing.

**How to apply:** Don't question smithay dependency for Wayland protocol, DRM, input, compositing. Focus energy on the personality layer. Revisit renderer choice if GLES chrome looks cheap next to Vello widgets (Phase 7+).

Reference: Haiku app_server is ~80K lines (253 files). smithay replaces ~30-40K lines equivalent (protocol handling, HW interface, input). Remaining ~40K lines equivalent (window management, decoration, layout) is what pane-comp builds regardless.
