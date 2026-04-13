---
type: reference
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [pane-kernel, device_traits, platform, thin_kernel, Plan9_Dev, translator, namespace_binding, cfg_target_os]
related: [reference/plan9/foundational, reference/plan9/divergences, decision/host_as_contingent_server, architecture/fs, architecture/session, agent/plan9-systems-engineer/consultation_router_translator_2026_04_12]
agents: [plan9-systems-engineer]
---

# pane-kernel Design Consultation (2026-04-12)

Six-section analysis mapping Plan 9 kernel architecture to pane-kernel as a userspace trait suite.

## Core factoring

pane-kernel = Plan 9 Dev struct (device drivers) extracted from the kernel. The kernel's other responsibilities are already handled:
- Protocol multiplexer → pane-session
- Namespace / mount table → pane-fs
- Event loop → pane-app (calloop)
- Process lifecycle → pane-server (future)

## Key decisions proposed

1. **Device traits, not file servers.** pane-kernel defines `DisplayTarget`, `InputSource`, `AudioSink`/`AudioSource` as Rust traits, not as 9P-style file servers. pane-fs projects device state into `/pane/dev/` as computed directories. Follows three-tier access model.

2. **Platform trait as top-level abstraction.** One `Platform` trait with associated types for each device class. Implementations: `LinuxPlatform`, `DarwinPlatform`, `HeadlessPlatform`. HeadlessPlatform returns None for all devices (headless = base case per `decision/headless_strategic_priority`).

3. **cfg(target_os) modules, not separate crates or features.** Layout: `src/platform/{linux,darwin,headless}/`. Platform-specific code compiled conditionally. Same pattern as Plan 9's `sys/src/9/{pc,arm,mips}/` vs `port/`.

4. **peer_cred migrates from pane-session.** Platform-specific syscall wrapping belongs in pane-kernel. Migration deferred until pane-kernel crate is created.

5. **No network device trait.** rustix handles sockets. Plan 9's #I clone/ctl/data pattern doesn't map to pane's needs.

6. **No storage device trait Phase 1.** Host filesystem access goes through std::fs/rustix.

7. **Translator sub-module inside pane-kernel.** BeOS Translation Kit heir. Codec traits + TranslatorRoster. OS-independent but foundational.

8. **DeviceRegistry as pane-fs integration point.** pane-kernel exposes registry; pane-fs iterates it for /pane/dev/ directory listings. One-way dependency: pane-fs → pane-kernel.

## Boundary principle

pane-kernel owns anything that changes when you change the host OS, and nothing else. This is the Plan 9 `port/` vs `pc/` split: pane-kernel is `pc/`, everything else is `port/`.

## Size target

Trait definitions ~500-800 LOC. Platform backends ~500-2000 LOC each. Headless ~100 LOC. Total Phase 1: 2000-5000 LOC. Growing past this signals scope creep.

## Plan 9 sources cited

- `port/portdat.h`: Dev struct, devtab[], Pgrp (mount table)
- `port/syspath.c`: namec() path resolution through mount table
- `pc/main.c`: platform-specific init
- `#c` devcons.c, `#i` devdraw.c, `#A` devaudio.c, `#S` devsd.c, `#I` devip.c
- Inferno `emu/port/pgrp.c`: userspace namespace without kernel support
- Pike 1995 paper Discussion section: what to do differently (merge file server kernel)
