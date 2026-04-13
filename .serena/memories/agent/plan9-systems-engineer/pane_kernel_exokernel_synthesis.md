---
type: reference
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [pane-kernel, exokernel, Dev_trait, typed_api, namespace, event_model, Inferno_emu, DeviceRegistry, PaneDeviceView, calloop]
related: [agent/plan9-systems-engineer/pane_kernel_design_consultation, reference/plan9/foundational, reference/plan9/divergences, decision/host_as_contingent_server, architecture/fs, architecture/proto, architecture/session]
agents: [plan9-systems-engineer]
---

# pane-kernel Exokernel Synthesis (2026-04-12)

Five-section analysis: Plan 9 kernel architecture married to Be's typed API design for pane-kernel as userspace exokernel.

## 1. Dev trait + typed APIs

Two-layer model: Dev trait (Plan 9 Dev struct vtable) as universal file-protocol foundation, typed domain traits (Translator, InputSource, DisplayTarget) extending Dev as ergonomic application APIs. The typed API is what application code uses; the Dev read/write path is the scripting/automation/cross-machine fallback. Validated by Plan 9 practice: libdraw was a typed wrapper over structured reads/writes to /dev/draw. Inferno made this explicit with Limbo modules over Styx.

Key design: typed traits extend Dev (`trait Translator: Dev`). Compile-time proof that a typed API has a file-protocol backing. Plan 9 enforced this by convention; pane enforces it by trait bounds.

Risk: two interfaces to maintain per device class. Mitigation: derive Dev text format from typed API return types (`#[derive(DevTextFormat)]`).

## 2. Namespace model

pane-kernel provides DeviceRegistry (devtab[] equivalent, flat HashMap name->Dev). pane-fs mounts it at /pane/dev/. One-way dependency: pane-fs -> pane-kernel. pane-kernel has no path resolution, no bind/mount, no union semantics.

Per-pane device visibility via PaneDeviceView — a predicate (HashSet of visible device names), not a mount table. Headless = empty set. Simpler than Plan 9's Pgrp/mount-table but upgradeable. Inferno's Pgrp was 10-30 entries per group (~1-2 KB per copy), confirming full mount tables are cheap if needed later.

No namec() in pane-kernel — pane-fs owns path resolution. No pipe/dup/rfork — pane creates panes, doesn't fork them.

## 3. Event model reconciliation

Devices produce fds (event_fd() -> Option<RawFd>). calloop polls them via DeviceEventSource (same pattern as ConnectionSource). Dev::read() is the fallback for fd-less devices. FUSE blocking-read files use broadcast channels bridging looper events to FUSE threads. Both paths consume from the same device.

Typed calloop events for application code, blocking-read files for scripts. Broadcast channel is the bridge between looper-side drain_events() and FUSE-side blocking reads.

## 4. Essential vs optional Plan 9 kernel concepts

Essential: Dev trait, DeviceRegistry (devtab[]), Qid-like DevId (path+version+kind for FUSE inode/cache).

Deferrable: Chan-like per-open state (start with stateless DevHandle), full Pgrp (start with PaneDeviceView predicate).

Not needed: namec() (pane-fs owns this), pipe/dup (OS-level), rfork (pane doesn't fork), full mount table in pane-kernel, network device trait (rustix handles sockets).

## 5. Inferno/emu lessons

Successes: Dev in userspace worked fine. Pgrp was lightweight. Thread-per-process scheduling validated pane's looper-per-pane model.

Awkward: draw device translation to X11/Win32 was painful (don't replicate — pane's Display should be compositor-native, not draw-protocol-over-files). Audio timing issues (use buffer-pull APIs, not streaming writes). Network device was replaced by native sockets (validates no network Dev trait). Two-worlds problem (Dis namespace invisible to host processes — same situation for pane, acceptable).

Lessons: keep Dev thin, platform backends as compile-time modules not dynamic dispatch, device hot-plug was never solved (static registration Phase 1, event channel Phase 2), typed API path must bypass Dev/pane-fs for performance (direct call, no path resolution).
