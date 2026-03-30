# Nix Base Layer Feasibility Research

Research for pane spec-tightening. Question: Can you fork nixpkgs and rewrite only the base layer (init, service management, boot, system configuration) while keeping the ~120,000 application packages unchanged?

Primary sources: nixpkgs repository (github.com/NixOS/nixpkgs), sixos project (codeberg.org/amjoseph/sixos), NixNG project (github.com/nix-community/NixNG), not-os project (github.com/cleverca22/not-os), NixOS Discourse discussions, Gentoo wiki systemd dependency analysis, 38C3 conference talk on sixos. Additional sources: Haiku documentation, GNU Guix project, Tvix/Snix projects, Cachix/Attic documentation, Graham Christensen's "erase your darlings" blog post, NixOS SAL proposal (nixpkgs #26067).

---

## 1. The nixpkgs architecture: where the boundary actually is

### The two halves of nixpkgs

The nixpkgs repository contains two largely independent subsystems:

**`nixpkgs/pkgs/`** — The package collection. ~120,000+ packages as of NixOS 25.11 (7,002 new packages added in that release alone, 25,252 updated). These are derivations: source + build instructions + dependencies -> store path. They define how to build Firefox, Python, GCC, etc. The vast majority are init-system-agnostic — they don't care whether systemd or s6 is PID 1.

**`nixpkgs/nixos/`** — The NixOS system configuration layer. This is the module system: ~2,022 modules (per module-list.nix), of which ~1,200+ are in the `services/` directory defining systemd service units. This layer handles `/etc` generation, boot stages, service management, user management, system activation. It is thoroughly coupled to systemd.

This split is confirmed by sixos, which explicitly depends on `nixpkgs/pkgs` while sharing zero code with `nixpkgs/nixos`:

> "Sixos is not a fork of NixOS. It shares no code with nixpkgs/nixos, nor is any part of it derived from NixOS. Sixos and NixOS both depend on nixpkgs/pkgs."

This is the key architectural fact. The package ecosystem (`pkgs/`) is a distinct, reusable asset. The system integration layer (`nixos/`) is where all the systemd coupling lives.

### The base layer, concretely

In NixOS, "the base layer" consists of:

- **Stage 1 (initrd)**: `nixos/modules/system/boot/stage-1.nix`, `stage-1-init.sh` — mounts root filesystem, loads kernel modules, hands off to stage 2. NixOS is currently migrating the initrd itself to systemd.
- **Stage 2 (init)**: `nixos/modules/system/boot/stage-2-init.sh` — mounts /proc, /dev, /sys, activates the system configuration, execs systemd as PID 1.
- **Service management**: `nixos/modules/system/boot/systemd.nix` and the entire `nixos/modules/system/boot/systemd/` tree (coredump, journald, logind, nspawn, tmpfiles, etc.)
- **System activation**: Scripts that build `/etc`, create users/groups, set up networking, apply the system configuration. Deeply intertwined with systemd unit activation ordering.
- **~1,200+ service module definitions**: Each one wraps an application package with a systemd unit definition. These are the `services.nginx.enable = true` options.
- **Boot loaders**: GRUB, systemd-boot, limine, refind — ~60 modules in `system/boot/`.

Total code: The NixOS module system is substantial. The ~2,022 modules represent tens of thousands of lines of Nix. The systemd coupling permeates: "Most of the NixOS options provides only a very slim layer of abstraction above systemd and many are 1:1 mapped to systemd options." Many services depend on systemd-specific security features like `DynamicUser`, `ProtectSystem`, `PrivateTmp`, `ProtectHome`.

---

## 2. The systemd dependency surface in application packages

### Where the coupling actually lives

There are three distinct kinds of systemd coupling:

**A. NixOS module wrappers (~1,200 modules)** — These are the `services.*.enable` options. They live in `nixpkgs/nixos/modules/services/` and are pure NixOS configuration. They do not affect the package derivations in `pkgs/`. If you replace the module system, you lose these service definitions but the underlying packages still build fine. You just need to write your own service management for each.

**B. Packages that link `libsystemd` or `libudev` at build time** — This is the real coupling surface. In nixpkgs:
- `libudev` is built as part of the systemd derivation (`systemd.lib` output). It is *statically linked against libsystemd*, making separation difficult.
- A "large number of applications link directly to systemd libraries for various purposes, and they have to be (re-)built without it" (Lobsters discussion of sixos).
- The sixos six-demo README states: "Almost everything in nixpkgs depends on `systemd` because of `libudev`."
- Packages commonly linking `systemd.lib`: PulseAudio/PipeWire, dbus, polkit, NetworkManager, Wayland compositors, Bluetooth stack, USB hotplug-dependent packages, most desktop environment components.
- Separating libudev.so was attempted in nixpkgs PR #97051 but concluded that libudev is "more tied to systemd than expected" — it statically links libsystemd internally.

**C. Packages with hard build-time systemd dependencies (no optional flag)** — Per Gentoo's analysis (which maps well to upstream behavior), truly hard dependencies are rare: ~8 packages (abrt, snapd, libreport, dbus-broker with launcher, gnome-user-share, office-runner, krdp, profile-sync-daemon). Most packages that use systemd features do so optionally via build flags.

### The libudev problem is the bottleneck

The key insight: the vast majority of "systemd dependencies" in application packages are actually `libudev` dependencies. Applications that do USB hotplug detection, input device enumeration, or hardware discovery typically link libudev. Since nixpkgs builds libudev from the systemd source tree, this creates a transitive dependency from many packages to systemd.

**Solutions that exist:**
- `libudev-zero` — a daemonless drop-in replacement for libudev. Written from scratch, no systemd dependency. Used by Void Linux and others. Limitation: hwdb interface not implemented, some functions missing.
- `eudev` — Gentoo's standalone fork of udev. More complete than libudev-zero but maintenance has been declining.
- Nix overlay approach: override `systemd.lib` in nixpkgs to provide libudev from an alternative source. This would require rebuilding all packages that transitively depend on it (a large rebuild), but it's a one-time override, not per-package patching.

### Packages with optional systemd support

Many packages in nixpkgs have `withSystemd` or `systemdSupport` flags that can be toggled. The GitHub search is rate-limited, but known examples include Waybar, PulseAudio, and many server daemons (PostgreSQL, MariaDB, Apache, Postfix all link systemd for `sd_notify` support but can be built without it).

### Quantitative estimate

- **Packages that are completely init-agnostic**: The vast majority. CLI tools, libraries, compilers, interpreters, most server software, most development tools — probably 95%+ of the ~120,000 packages. These have zero systemd coupling at build time.
- **Packages that link `systemd.lib` (mostly via libudev)**: Hundreds. This includes the desktop stack (Wayland, PipeWire, dbus, polkit, desktop environments). These need rebuilding with an alternative libudev.
- **Packages with hard, non-optional systemd dependencies**: Under 20. Mostly GNOME/KDE session management components and a few system tools.

---

## 3. Precedents: who has done this

### Sixos — the direct precedent (codeberg.org/amjoseph/sixos)

The most relevant project by far. Sixos is a two-year effort (first public release January 2025) that does *exactly* what the proposal describes:

- Uses `nixpkgs/pkgs` for all application packages
- Replaces `nixpkgs/nixos` entirely with a custom system layer using s6
- Replaces the NixOS module system with the `infuse` combinator (a "deep" version of `.override`/`.overrideAttrs` that generalizes `lib.pipe` and `recursiveUpdate`)
- Services are Nix expressions (`svcs/by-name/.../service.nix`) that parallel the `pkgs/by-name/.../package.nix` pattern
- Targets (instantiated services) are derivations, the target set is a scoped fixpoint — same algebra as nixpkgs packages. `override`, `callPackage`, `overrideAttrs` all work on services.
- Uses ownerboot for verified boot (coreboot loads immutable pre-kexec kernel from write-protected SPI flash)
- ~709 KiB of Nix code, 428 commits, GPLv3
- Author runs it on "workstations, servers, twelve routers, stockpile of disposable laptops, and on his company's 24-server/768-core buildfarm"

**Key architectural insight from sixos**: The NixOS module system is not the only way to compose system configuration in Nix. The `infuse` combinator provides a simpler alternative that treats services exactly like packages — same override semantics, same fixpoint structure, same composition model. This eliminates the "module system vs package system" bifurcation that NixOS has.

**What sixos proves**: You can build a production Linux system on top of `nixpkgs/pkgs` without touching `nixpkgs/nixos`. The package ecosystem is genuinely separable from the system integration layer.

### NixNG (github.com/nix-community/NixNG)

A lighter NixOS derivative using runit instead of systemd:
- Uses its own module system (fully structured, no `extraConfig` strings)
- "Minimal by default" package set vs NixOS's "full featured by default"
- 83.6% Nix, 14.4% Haskell, 419 commits
- **Critical limitation: cannot boot on real hardware.** Operates exclusively as LXC/OCI containers. No kernel, no initramfs.
- Useful as reference for module system design but not a precedent for full system replacement.

### not-os (github.com/cleverca22/not-os)

Minimal Nix-based Linux using runit:
- Produces a kernel + initrd + 48MB squashfs
- Based heavily on NixOS infrastructure but stripped down
- 98 commits, 97.6% Nix
- Supports QEMU, Raspberry Pi, Zynq
- iPXE support with signed boot
- Demonstrates that Nix can build a complete bootable system from scratch without NixOS's full module system.

### nixos-init-freedom (sr.ht/~guido/nixos-init-freedom)

Earlier, less mature attempt to use s6 as PID 1 on NixOS:
- Hooks into NixOS's boot process via `boot.systemdExecutable = "${s6-init}"`
- Converts NixOS systemd service definitions to s6 service directories
- Stub services for unresolved dependencies (one-shot returning exit 0)
- Early stage, tested only with unbound and sshd
- Approach: shim on top of NixOS rather than clean replacement. Fragile.

### Chimera Linux (chimera-linux.org)

Not Nix-based, but relevant as a reference for the "replace systemd with alternative init" approach:
- musl libc + FreeBSD userland + dinit init + apk package manager
- Custom Python-based build system (cports/cbuild), not Nix
- Proves that the application ecosystem is init-system-agnostic at the source level.

---

## 4. The practical work involved

### Strategy A: Fork nixpkgs (not recommended)

Hard fork the repo, delete `nixos/`, keep `pkgs/`, add your own system layer.

**Maintenance burden**: nixpkgs has ~4,000 commits per week. Keeping `pkgs/` in sync requires periodic rebases/merges. The GitHub issue #27312 discussion found that maintaining a long-lived fork is painful: "nixpkgs is mostly setup for people working on the master branch and somewhat optimized for people who have commit access," leaving downstream maintainers doing "porting/rebasing busy-work."

Full git merges work smoothly if you never touch `pkgs/` files. If you confine your changes to a separate directory tree (your system layer), merge conflicts with upstream should be near-zero. The risk is upstream refactoring `pkgs/` infrastructure files (like `all-packages.nix`, the stdenv, or the Nix language constructs in `lib/`).

### Strategy B: Overlay + flake on top of upstream nixpkgs (recommended)

This is what sixos actually does. Don't fork — depend on nixpkgs as an input:

```nix
{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  # your system layer is a separate repo
}
```

Your code provides:
1. An overlay that replaces `systemd.lib` with a libudev alternative (libudev-zero or custom eudev build)
2. A custom system builder (your equivalent of `nixos/`) that generates boot images using s6
3. Service definitions for the services you need (parallel to NixOS modules but for s6)
4. System activation scripts that handle `/etc`, users, networking

**Advantages**: Zero merge conflicts with upstream. You get upstream package updates for free by bumping your nixpkgs input. You only maintain what you change.

**The libudev overlay**: This is the single largest build impact. Overriding `systemd.lib` to provide libudev from an alternative source triggers rebuilds of everything that transitively depends on it. On first build, this is substantial (probably thousands of packages). But Nix's content-addressed store means subsequent builds only rebuild what actually changed.

### Strategy C: The sixos approach — depend on nixpkgs/pkgs, write everything else from scratch

This is what sixos chose. You don't even use NixOS's module system — you write your own composition model. The infuse combinator is ~709 KiB total for the entire system layer.

**What you'd write:**
1. Boot stage scripts (initrd, init) — a few hundred lines of shell
2. Service definitions for core services (networking, logging, tty, nix-daemon) — sixos has these
3. The composition layer (how services combine into a system) — infuse.nix or your own
4. Per-application service definitions as you need them

**What you'd reuse from nixpkgs:**
- All of `pkgs/` — every application package
- `lib/` — Nix utility functions
- stdenv — the build infrastructure

### The binary cache problem

If you override `systemd.lib` or use a different libudev, you lose the official NixOS binary cache for every package that transitively depends on it. You'd need your own build farm. Sixos runs a 24-server/768-core buildfarm for this purpose. For a smaller operation, you could:
- Use Cachix or similar for your custom cache
- Accept longer initial builds
- Scope the libudev override narrowly (only for packages that actually need it)

### Quantitative estimate of work

| Component | Effort | Notes |
|-----------|--------|-------|
| Boot chain (initrd + init) | 1-2 weeks | Well-understood problem, not-os and sixos are references |
| s6 service management integration | 2-4 weeks | s6-rc has clean semantics, main work is the Nix integration |
| libudev replacement overlay | 1 week | Mechanical: override systemd.lib, point at libudev-zero |
| Core service definitions (20-30 services) | 2-4 weeks | Networking, logging, tty, nix-daemon, dbus, elogind, etc. |
| System activation scripts | 2-3 weeks | /etc generation, user management, filesystem setup |
| Composition layer / module system | 2-4 weeks | Or adopt sixos's infuse combinator |
| elogind integration (for desktop use) | 1-2 weeks | elogind is a standalone package, well-tested on Gentoo/Void |
| Testing + stabilization | 4-8 weeks | The long tail |
| **Total** | **~3-6 months** | For one experienced person, to a usable-for-daily-driving state |

---

## 5. Using Nix without NixOS

### What Nix provides independent of NixOS

Nix (the package manager) is completely independent of NixOS (the Linux distribution). You can use:

- **nix-build / nix build** — build any derivation
- **/nix/store** — the content-addressed store
- **Derivations** — the build model (source + deps + build script -> output)
- **nixpkgs as a library** — import it and use any package
- **Flakes** — dependency management and reproducibility
- **Binary cache protocol** — download pre-built packages

None of this requires NixOS. Nix runs on any Linux (and macOS). You can use Nix to build a custom Linux system that has nothing to do with NixOS's module system.

### Minimal Nix infrastructure for a custom distro

1. **The Nix daemon** — manages the store, runs builds
2. **A flake.nix** — declares your system, pins nixpkgs
3. **A system builder** — Nix expression that assembles kernel + initrd + rootfs into a bootable image
4. **A deployment mechanism** — something equivalent to `nixos-rebuild switch` that activates a new system generation

not-os proves this is viable with ~30 Nix files. Sixos proves it scales to production.

---

## 6. Application package compatibility

### If you keep glibc, replace systemd

**What breaks at build time**: Very little. The packages that explicitly `buildInputs = [ systemd ]` for sd_notify or journal logging will fail, but most of these have configure flags to disable systemd support. The five identified in nixpkgs issue #330821 (postgres, mariadb, apacheHttpd, redict, postfix) are all fixable with overrides.

**What loses functionality at runtime**: Services that use sd_notify for readiness signaling won't notify the service manager. Services that log to journald will need a different logging target. Services that use systemd socket activation need s6's equivalent (s6-ipcserverd or s6-tcpserver). These are all solvable — s6 has equivalents for all of them.

**What needs an alternative component**:
- logind -> elogind (standalone, well-tested, packaged in nixpkgs)
- journald -> syslog-ng, rsyslog, or s6-log
- tmpfiles -> s6-tmpfiles or a custom script (simple)
- networkd -> dhcpcd, connman, or NetworkManager (all in nixpkgs)
- resolved -> unbound, knot-resolver, or systemd-resolved standalone (it can run without systemd as PID 1)

### If you also replace glibc with musl

**Don't do this initially.** nixpkgs has a `pkgsMusl` overlay but it's not well-tested — many packages fail to build, there's no binary cache for it, and the debugging surface area is enormous. The glibc-to-musl switch is orthogonal to the init system switch. Do one at a time. The systemd replacement is far more tractable.

### The libudev dependency chain

This is worth visualizing:

```
Many desktop packages
    -> PipeWire, libinput, Mesa, Wayland
        -> libudev (from systemd.lib)
            -> libsystemd (statically linked inside libudev)
```

Replacing libudev with libudev-zero breaks this chain cleanly. libudev-zero provides the same `.so` interface without any systemd dependency. The main risk is libudev-zero's incomplete hwdb support, which matters for some input device quirks but not for most use cases.

---

## 7. How this informs pane's design

### The verdict: highly feasible

The sixos project proves this is not theoretical — someone has done it, runs it in production, and the codebase is ~709 KiB. The key facts:

1. **nixpkgs/pkgs is genuinely separable from nixpkgs/nixos.** The package ecosystem doesn't care about your init system.
2. **The overlay approach avoids forking entirely.** You depend on upstream nixpkgs, override what you need, write your own system layer. Zero merge conflicts.
3. **The libudev problem is solved.** libudev-zero exists, works, and is a clean override target.
4. **s6 is a mature init system** with well-understood semantics that map cleanly to Nix's derivation model (service directories are derivation outputs).
5. **The binary cache loss is real but manageable.** You need your own build infrastructure, but not a Google-scale operation.
6. **The NixOS module system is optional.** sixos's infuse combinator, NixNG's structured modules, or a custom approach all work.

### Alignment with pane's philosophy

| Pane principle | Nix alignment |
|---------------|---------------|
| **Compact, efficient, bulletproof core** | Nix builds exactly what's declared — no package manager state drift, no dependency rot |
| **Ship of Theseus** | Generations. Each system rebuild produces a new generation; old generations preserved. The system's identity is its configuration, not its specific package versions |
| **Declarative configuration** | This is Nix's defining feature |
| **Reproducible builds** | Nix's content-addressed store guarantees reproducibility |
| **Atomic upgrades and rollback** | Nix generations provide this natively |
| **Filesystem-based configuration** | Compatible — pane's `/etc/pane/` is writable state, Nix manages the immutable base |

### What this means for pane

If pane needs a custom Linux base:
- **Use Nix as the build system.** The derivation model, the store, the reproducibility guarantees are the right foundation.
- **Use nixpkgs/pkgs for application packages.** 120,000+ packages, maintained by a large community, continuously updated.
- **Write a custom system layer** using s6 for service management. Don't try to use NixOS modules — write something simpler that fits pane's architecture (a small module system using `lib.evalModules` with pane-specific options, or adopt sixos's infuse combinator). Pane needs ~15-20 service definitions, not 2,000.
- **Use overlays, not a fork.** Override `systemd.lib`, add your packages, keep everything else upstream.
- **Use s6-linux-init as PID 1**, s6-svscan for supervision, s6-rc for dependency management. The entire skalibs/s6/execline ecosystem is already packaged in nixpkgs.
- **Handle `/etc/pane/` as writable state.** Generate defaults at build time, install to writable volume at first boot, let pane-notify watch for changes. The overlayfs or shine-through pattern (from Haiku's packagefs) solves the mutable/immutable tension.
- **Maintain a binary cache** via Cachix for open-source builds.
- **Track sixos** as the closest reference implementation.
- **Watch Tvix/Snix** for future Rust-native Nix tooling.
- **Look at sixos and not-os** as architectural references, not dependencies to adopt wholesale.

### The risk profile

- **Low risk**: Application packages working unchanged. This is proven.
- **Low risk**: Using Nix without NixOS. Multiple projects demonstrate this.
- **Medium risk**: The libudev replacement causing subtle breakage in hardware detection edge cases.
- **Medium risk**: The service migration work for desktop-oriented services (desktop session management, audio, display management).
- **Medium risk**: Binary cache logistics if desktop packages need rebuilding.
- **Low risk**: Boot chain and init replacement. Well-understood, multiple references.
- **Low risk**: Reconciling declarative base with writable pane config. NixOS has solved this pattern (overlayfs, "erase your darlings"). Haiku's shine-through directories validate it independently.

---

## 8. The NixOS module system and alternatives

### The Service Abstraction Layer (SAL) proposal

In 2017, Dan Peebles proposed a Service Abstraction Layer (nixpkgs issue #26067) to decouple NixOS service definitions from systemd. Goals: share service modules with nix-darwin (launchd), enable containerized deployment, support alternative init systems.

The proposed approach: modules could contain separate `systemd.services.*`, `docker.services.*`, `launchd.services.*` sections, sharing common logic while allowing platform-specific configuration. Eelco Dolstra responded positively. Status as of late 2025: still open, "needs: community feedback" label. No implementation completed. The scope of the change (~2,000 modules) makes incremental migration intractable at NixOS's scale.

**Lesson for pane:** Don't wait for SAL. Build init system abstraction for pane's scope (15-20 services, not 2,000). The SAL discussion validates the architecture but shows that the NixOS community can't execute it at NixOS's scale. Pane can execute it at pane's scale.

### Defining a custom module system

The Nix module system itself (`lib.evalModules`) is reusable infrastructure — it can evaluate any set of modules with option declarations and definitions. You don't need NixOS's module definitions to use the composition framework. A pane-specific module system could look like:

```nix
pane.services.<name> = {
  command = "/nix/store/.../bin/pane-comp";
  readiness.fd = 3;
  dependencies = [ "pane-route" ];
  environment = { WAYLAND_DISPLAY = "wayland-0"; };
  logging = true;
};
```

A backend translates these to s6 service directories and compiles them into an s6-rc database. This is the sixos approach scaled to pane's needs. The Nix module system provides the composition framework (option merging, type checking, defaults); pane defines the options and the backend.

### How tightly is the NixOS module system coupled to systemd?

Very. The coupling manifests at multiple levels:
- `systemd.services.<name>` is nearly 1:1 mapped to systemd unit directives
- Security hardening uses systemd-specific features: `DynamicUser`, `ProtectSystem`, `PrivateTmp`, `SystemCallFilter`
- Socket activation via `systemd.sockets.<name>`
- Timers via `systemd.timers.<name>`
- tmpfiles via `systemd.tmpfiles.rules`
- Targets for service grouping

The NixOS community consensus (from Discourse discussion): "The ship has sailed for NixOS to use anything other than systemd." Creating a viable alternative "would effectively have to be one or more new distros." This is a realistic assessment of coupling depth after 15+ years.

---

## 9. Immutable system design patterns

### How NixOS achieves immutability

- **The Nix store (`/nix/store/`)** is read-only, content-addressed by hash. Only the Nix daemon writes to it.
- **Symlink generations.** `/nix/var/nix/profiles/system` symlinks to the current generation's store path. Previous generations retained for rollback.
- **Boot entries.** Each generation gets a bootloader entry. Rollback means booting an older generation.
- **`/etc` as symlinks.** Most `/etc` files are symlinks into the store. On activation, scripts create/update these symlinks.
- **`/etc` overlay (recent).** `system.etc.overlay.enable` mounts `/etc` as overlayfs — read-only Nix-generated lower layer, writable upper layer. `system.etc.overlay.mutable` (default: true) controls writability.

### NixOS vs OSTree (Fedora Silverblue)

| Aspect | NixOS | OSTree/Silverblue |
|--------|-------|-------------------|
| **Granularity** | Per-package | Entire OS tree |
| **Customization** | Arbitrary Nix expression | Layering via rpm-ostree |
| **Rollback** | Per-generation (any previous config) | Per-deployment (previous tree) |
| **Build model** | Functional (content-addressed) | Git-like (filesystem tree commits) |
| **Developer experience** | Steep learning curve, maximum control | Familiar RPM/Flatpak, less control |

NixOS's advantage: more expressive declarative model. OSTree's advantage: simpler mental model, more accessible for desktop users.

### Reconciling declarative system config with writable `/etc/pane/`

Pane wants system immutability (reproducible, rollbackable base) AND live configuration (`/etc/pane/` writable, watched by pane-notify). This tension is solved.

**The overlayfs pattern.** Mount `/etc/pane/` as overlayfs: Nix-generated defaults as read-only lower layer, writable upper layer for runtime modifications. System rebuild updates the lower layer; the upper layer persists.

**The "erase your darlings" pattern (Graham Christensen).** Root filesystem is ephemeral (ZFS rollback to blank at each boot). Only explicitly listed mutable state persists via `/persist`. For pane:
1. System build generates default `/etc/pane/` configs in the Nix store
2. On boot, defaults are copied to `/etc/pane/` (a writable volume)
3. User modifications persist across reboots
4. `pane-rebuild` regenerates defaults but preserves user modifications

**Pane-specific resolution:** This is simpler than NixOS's full `/etc` management because pane only manages `/etc/pane/`, not the entire `/etc`. The rest of `/etc` can be fully declarative. Pane's filesystem-as-configuration model (each config key is a file, servers watch via pane-notify) is orthogonal to the build system — users configure pane by editing files, not by writing Nix.

---

## 10. Haiku's packagefs parallel

### How packagefs works

Haiku's packagefs is a virtual filesystem that presents a merged, read-only view of all activated HPKG packages. On boot, packagefs reads an activation list (`packages/administrative/activated-packages`) and activates only listed packages. Package management creates transaction directories, moves packages, updates the activation list.

**Writable state:** "Shine-through directories" — `settings`, `cache`, `var`, `non-packaged` — bypass the virtual layer and expose the underlying BFS volume directly. Mutable state coexists with immutable packages through explicit pass-throughs.

**Recovery:** Old packages are moved to timestamped directories. The boot loader can select an old activation list, booting into a previous system state.

### Comparison with Nix

| Aspect | Haiku packagefs | Nix store |
|--------|----------------|-----------|
| **Merging** | Virtual filesystem union at mount time | `buildEnv` or profile symlinks |
| **Writable state** | Shine-through directories | `/etc`, `/var`, etc. outside store |
| **Activation** | Activation list -> packagefs mount | Profile symlink -> generation |
| **Rollback** | Boot loader selects old activation list | Boot loader selects old generation |
| **Content addressing** | Not content-addressed | Hash-based content addressing |

**What Nix can learn from Haiku:** Haiku's shine-through directories are a cleaner solution to the mutable/immutable tension than overlayfs. The activation list is simpler than Nix's nested symlink chains.

**What Haiku could learn from Nix:** Content addressing (identical builds produce identical store paths). Declarative system configuration vs imperative install/remove. Reproducible builds.

**Relevance to pane:** Haiku's packagefs validates the architectural pattern pane wants: immutable package layer with explicit writable pass-throughs. Pane synthesizes: Nix's build model + Haiku's activation/writability model + pane's filesystem-as-configuration.

---

## 11. Other Nix-based projects and alternative approaches

### Tvix / Snix — Rust reimplementation of Nix

**Tvix** (https://tvix.dev/) — Rust reimplementation of the Nix evaluator and package manager by TVL. Not production-ready. Uses a bytecode VM instead of AST walking. Decomposes Nix into independently usable library components (evaluator, store, builder, daemon protocol).

**Snix** (https://git.snix.dev/snix/snix) — a fork of Tvix created March 2025 by the devenv team. Also not production-ready. devenv is building backend abstraction to support it.

**Relevance to pane:** Not immediately actionable. But the trajectory is clear: the Nix ecosystem is moving toward Rust-based tooling with library-oriented architecture. If pane (Rust) uses Nix for system management, a Rust Nix implementation usable as a library (rather than shelling out to the CLI) would be architecturally clean. Worth watching, not worth depending on yet.

### GNU Guix System

Uses Guile Scheme (not Nix language) following the same functional package management model. Uses GNU Shepherd as its init system — not systemd. Proves that a Nix-like system works without systemd. Grafting mechanism: substitute updated runtime dependencies without recompiling dependent packages (useful for security updates). Much smaller community and package set. Significantly slower than Nix (guix pull can take 30-50 minutes).

**Relevance:** Validates that declarative functional package management does not require systemd. Guix's grafting addresses a real pain point (security updates without full rebuilds) that pane would eventually face.

### Mobile NixOS

Superset of NixOS targeting mobile devices. Demonstrates heavy NixOS customization for specific hardware: custom kernels, device trees, firmware, SoC-specific modules. Keeps systemd and the full module system.

### The Nix Process Management Framework (s6-rc backend)

Sander van der Burg developed a process-manager-agnostic Nix framework generating configs for multiple backends: systemd, sysvinit, supervisord, Docker, s6-rc. Provides `createLongRunService`, `createOneShotService`, `createServiceBundle`. Key insight: s6-rc's compiled database model maps naturally to Nix's derivation model. Service definitions are data (directories with files), compilation is a derivation, activation is switching databases.

---

## 12. Cross-compilation considerations

### Building Linux targets from macOS

nixpkgs does not support true cross-compilation from darwin to Linux. The supported matrix is Linux->Linux (including cross-architecture) and darwin->darwin. This is a fundamental toolchain limitation (can't run Linux ELF binaries during build on macOS).

**Practical approaches:**
- Linux VM on Apple Silicon with Rosetta 2 — near-native aarch64-linux performance
- Remote Linux builder via `nix build --builders 'ssh://linux-machine ...'`
- NixOS in VM with `virtualisation.rosetta2.enable = true`

Pane already uses a VM-based workflow, so this is a known quantity.

### True cross-compilation within Linux

`pkgsCross.aarch64-multiplatform` provides a nixpkgs instance where all packages are cross-compiled for aarch64. Works well for most packages but can hit issues with packages that have build-time dependencies on target-architecture binaries.

---

## 13. Binary cache infrastructure

### Options

- **Cachix** (https://cachix.org) — hosted Nix binary cache. Free for open source. Easy to set up. Standard for Nix projects.
- **Attic** — self-hostable Nix binary cache backed by S3-compatible storage. More control, more maintenance.
- **nix-serve** — minimal self-hosted cache. Simple HTTP server serving the local Nix store.
- **GitHub Actions + Cachix** — CI builds packages and pushes to Cachix. The standard open-source workflow.

### The cost of diverging from standard nixpkgs

If pane overrides `systemd.lib` for libudev-zero, every transitively dependent package needs rebuilding. These won't be in the official NixOS binary cache. Options:
- Host a pane-specific cache via Cachix (free tier for open source)
- Accept long initial build times, cache locally
- Be strategic: many packages don't actually need libudev, only the desktop stack does

A minimal system (kernel, coreutils, s6, compositor) might be 5-10 GB cached. A full desktop environment: 50-100 GB.

---

### Sources

- [sixos repository](https://codeberg.org/amjoseph/sixos) — the direct precedent
- [sixos demo](https://codeberg.org/amjoseph/six-demo) — example configurations
- [infuse.nix](https://codeberg.org/amjoseph/infuse.nix) — the module system replacement
- [38C3 talk: sixos, a nix os without systemd](https://media.ccc.de/v/38c3-sixos-a-nix-os-without-systemd)
- [NixNG](https://github.com/nix-community/NixNG) — runit-based Nix distro (containers only)
- [not-os](https://github.com/cleverca22/not-os) — minimal Nix-based Linux with runit
- [nixos-init-freedom](https://sr.ht/~guido/nixos-init-freedom/) — s6 on NixOS (early stage)
- [NixOS issue #24346: Can I replace systemd?](https://github.com/NixOS/nixpkgs/issues/24346)
- [NixOS issue #126797: Make NixOS systemd-independent](https://github.com/NixOS/nixpkgs/issues/126797)
- [NixOS issue #330821: Service packages shouldn't depend on systemd on non-NixOS](https://github.com/NixOS/nixpkgs/issues/330821)
- [nixpkgs PR #97051: Build libudev.so separately](https://github.com/NixOS/nixpkgs/pull/97051)
- [NixOS Discourse: Restructuring NixOS to work without systemd](https://discourse.nixos.org/t/restructuring-nixos-to-work-without-systemd-e-g-with-sysvinit/21298)
- [Gentoo wiki: Hard dependencies on systemd](https://wiki.gentoo.org/wiki/Hard_dependencies_on_systemd)
- [Gentoo wiki: Gentoo without systemd](https://wiki.gentoo.org/wiki/Gentoo_without_systemd)
- [libudev-zero](https://github.com/illiliti/libudev-zero) — daemonless libudev replacement
- [elogind](https://github.com/elogind/elogind) — standalone logind
- [nixpkgs issue #27312: Maintaining a long-lived custom nixpkgs tree](https://github.com/NixOS/nixpkgs/issues/27312)
- [KDAB: Using Nix as a Yocto Alternative](https://www.kdab.com/using-nix-as-a-yocto-alternative/)
- [NixOS SAL proposal — nixpkgs issue #26067](https://github.com/NixOS/nixpkgs/issues/26067) — service abstraction layer
- [Erase your darlings — Graham Christensen](https://grahamc.com/blog/erase-your-darlings/) — immutable NixOS patterns
- [system.etc.overlay.mutable — MyNixOS](https://mynixos.com/nixpkgs/option/system.etc.overlay.mutable) — /etc overlayfs
- [Haiku Package Management Infrastructure](https://www.haiku-os.org/docs/develop/packages/Infrastructure.html) — packagefs architecture
- [Haiku Package Management — markround.com](https://www.markround.com/blog/2023/02/13/haiku-package-management/) — packagefs overview
- [Tvix — tvix.dev](https://tvix.dev/) — Rust Nix reimplementation
- [Snix — git.snix.dev](https://git.snix.dev/snix/snix) — Tvix fork by devenv team
- [devenv switching to Tvix](https://devenv.sh/blog/2024/10/22/devenv-is-switching-its-nix-implementation-to-tvix/)
- [Guix vs. Nix — Abilian Innovation Lab](https://lab.abilian.com/Tech/Linux/Packaging/Guix%20vs.%20Nix/)
- [A look at Nix and Guix — LWN.net](https://lwn.net/Articles/962788/)
- [Cachix — cachix.org](https://www.cachix.org) — hosted Nix binary cache
- [Attic — NixOS Discourse](https://discourse.nixos.org/t/introducing-attic-a-self-hostable-nix-binary-cache-server/24343) — self-hosted cache
- [NixOS cross-compilation — nix.dev](https://nix.dev/tutorials/cross-compilation.html)
- [Fedora Silverblue vs NixOS — Medium](https://thamizhelango.medium.com/immutable-linux-distributions-fedora-silverblue-vs-nixos-9a56693ebe54)
- [Mobile NixOS](https://mobile.nixos.org/)
- [nixos-generators — GitHub](https://github.com/nix-community/nixos-generators)
- [s6-rc backend for Nix process management — Sander van der Burg](http://sandervanderburg.blogspot.com/2021/02/developing-s6-rc-backend-for-nix.html)
- [NixOS Module System — nix.dev](https://nix.dev/tutorials/module-system/deep-dive.html)
- [Building a non-NixOS Linux image using Nix — NixOS Discourse](https://discourse.nixos.org/t/building-a-non-nixos-linux-image-using-nix/55652)
- [Void Linux](https://voidlinux.org/) — runit-based distribution reference

---

## Nix as Build Infrastructure for a NeXTSTEP-Style Distribution

The previous sections established technical feasibility: yes, you can build a production Linux system on nixpkgs/pkgs with s6 replacing systemd. This section addresses the deeper question: how does Nix serve a distribution that wants to be *one integrated thing* — not a package collection, but a complete system identity?

The analogy throughout is NeXTSTEP. Not "NeXTSTEP was cool and we want to be like it." Rather: NeXTSTEP is the clearest historical example of a system where the boundary between the OS personality and the underlying kernel was invisible to the user, and understanding *how* they achieved that informs how pane can achieve it over Linux with Nix as the build substrate.

Additional sources for this section:

- [Apple Kernel Programming Guide — Architecture](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/KernelProgramming/Architecture/Architecture.html)
- [NeXTSTEP technical review — paullynch.org](https://www.paullynch.org/NeXTSTEP/NeXTSTEP.TechReview.html)
- [OPENSTEP secrets — 3fingeredsalute.com](https://www.3fingeredsalute.com/untold-secrets-openstep/)
- [CHM: Steve Jobs, NeXTSTEP, and early OOP](https://computerhistory.org/blog/the-deep-history-of-your-apps-steve-jobs-nextstep-and-early-object-oriented-programming/)
- [s6-linux-init overview](https://skarnet.org/software/s6-linux-init/overview.html)
- [s6-rc overview](https://skarnet.org/software/s6-rc/overview.html)
- [infuse.nix](https://codeberg.org/amjoseph/infuse.nix)
- [six-demo](https://codeberg.org/amjoseph/six-demo)
- [38C3 sixos talk](https://media.ccc.de/v/38c3-sixos-a-nix-os-without-systemd)
- [Nix flakes — nix.dev](https://nix.dev/manual/nix/stable/command-ref/new-cli/nix3-flake.html)
- [Nix profiles — nix.dev](https://nix.dev/manual/nix/stable/package-management/profiles)
- [NixOS modules — wiki.nixos.org](https://wiki.nixos.org/wiki/NixOS_modules)
- [Erase your darlings — Graham Christensen](https://grahamc.com/blog/erase-your-darlings/)
- [Cachix](https://cachix.org/)
- [Attic](https://docs.attic.rs/)
- [nixos-generators (archived)](https://github.com/nix-community/nixos-generators)

---

### 1. NeXTSTEP's relationship to Mach/BSD as a model for pane's relationship to Linux

#### What NeXTSTEP controlled vs. delegated

NeXTSTEP was built on a Mach 2.5 kernel with a BSD 4.3 Unix layer. The architecture was layered but the layers were not visible to the user or the developer:

**Mach provided** (invisible infrastructure):
- Task and thread abstractions (preemptive multitasking, SMP)
- Inter-process communication via ports and messages
- Virtual memory management with demand paging
- Hardware abstraction — the reason NeXTSTEP could run on 68k, then Intel, then SPARC, then PA-RISC

**BSD provided** (familiar Unix surface):
- POSIX file I/O and process model (fork/exec, signals, PIDs)
- Socket-based networking (TCP/IP stack)
- Device driver interfaces
- The Unix security model (users, groups, permissions)

**NeXTSTEP provided** (the system identity):
- Display PostScript — all screen rendering through a unified imaging model
- The Objective-C runtime — dynamic dispatch as the substrate for everything
- AppKit/Foundation — the application frameworks that every program used
- Interface Builder — the development tool that enforced the programming model
- The Workspace Manager — the desktop shell
- The Services menu, Distributed Objects, the pasteboard — cross-app composition
- The .app bundle model — self-contained application packaging
- One font (Helvetica), one color scheme, one visual language

The key architectural fact: NeXTSTEP used Mach 2.5 in its "conventional kernel" form (not the microkernel architecture CMU later pursued). Paul Lynch's technical review notes that "each of these layers is relatively independent." Mach handled memory and scheduling; BSD handled files and networking; NeXTSTEP's personality layer handled everything the user actually saw and touched. The layers were distinct in implementation but unified in experience.

#### How the integration produced the "one thing" feel

Three mechanisms made NeXTSTEP feel like one thing rather than "a GUI on top of Unix":

**1. One runtime.** Objective-C's dynamic dispatch was the substrate for Interface Builder connections, Services menu discovery, Distributed Objects, responder chain traversal, and plugin loading. These weren't separate features — they were all instances of the same mechanism. When you use Interface Builder to wire a button to an action, the same runtime that handles that connection also handles the Services menu, also handles Distributed Objects. Shared mechanisms produce integrated behavior without coordination.

**2. One imaging model.** Display PostScript rendered everything — windows, text, graphics, print output. There was no "screen API" vs "print API" vs "widget API." The screen was a soft printer. Visual consistency went deeper than theming because the anti-aliasing, font rendering, and compositing were the same code everywhere.

**3. One application model.** Every app was a bundle. Every app had NIBs. Every app used AppKit. Every app got Services for free. The Workspace Manager could inspect any app. Developers couldn't opt out because the tools didn't offer an alternative. Constraint produced consistency.

The OPENSTEP evolution later separated this into two concepts: OPENSTEP *the API* (Foundation + AppKit, portable to other kernels) and OPENSTEP *the environment* (the complete system running on Mach/BSD). This separation is instructive — the API could theoretically run on any kernel, but the *experience* of NeXTSTEP came from the total integration of API + runtime + imaging model + development tools + visual design, all running on a kernel that provided the right primitives and stayed out of the way.

When Apple built macOS, this became XNU (Mach 3.0 + FreeBSD + I/O Kit), with the BSD layer providing POSIX compatibility, Mach providing IPC and memory management, and the I/O Kit providing device driver infrastructure. Darwin (the open-source kernel + core utilities) can run without macOS's proprietary layers, but nobody uses it that way — because Darwin without the Cocoa personality layer is just a Unix variant. The "one thing" feel is entirely in the personality layer.

#### What pane learns from this

The lesson is not "pane should have a custom kernel." The lesson is about *where the identity lives*.

NeXTSTEP's identity was not in Mach. Mach was infrastructure — it provided the right primitives (IPC, VM, threads) and was otherwise invisible. The identity was in the personality layer: the frameworks, the runtime, the imaging model, the development tools, the visual design. This personality layer was *opinionated and complete* — it didn't expose Mach to the developer or the user. It presented a coherent world.

For pane, Linux is the Mach equivalent. Linux provides:
- Process model, scheduling, SMP (like Mach's tasks and threads)
- Filesystem primitives, networking, device drivers (like BSD)
- Namespaces, cgroups, fanotify, inotify, xattrs, memfd, seccomp (modern capabilities NeXTSTEP didn't have)

Pane's identity lives *above* this, in:
- The pane protocol (session-typed conversations — pane's equivalent of NeXTSTEP's Objective-C runtime)
- The compositor and rendering model (pane's equivalent of Display PostScript / Quartz)
- The kits (pane's equivalent of AppKit/Foundation)
- The filesystem-as-interface model (pane's equivalent of the .app bundle model and the Workspace Manager)
- The routing infrastructure (pane's equivalent of the Services menu)
- The aesthetic (pane's equivalent of NeXTSTEP's opinionated visual identity)

The kernel provides primitives and stays out of the way. The personality layer presents a coherent world. The user never thinks about the kernel. **This is the architecture Nix needs to serve**: building the personality layer as one coherent artifact, on top of a Linux kernel that provides the right primitives.

---

### 2. How Nix builds a complete system image

#### The NixOS system closure

In NixOS, the "system closure" is the complete, transitively-closed set of Nix store paths required to run the system — from the Linux kernel down to the last library. NixOS builds this through:

1. **Configuration evaluation.** The module system (`lib.evalModules`) processes all modules, merges option definitions, resolves dependencies between options. High-level options (like `services.nginx.enable`) cascade down through intermediate options (like `systemd.services.nginx`) to low-level outputs (like the content of `/etc/systemd/system/nginx.service`). Everything eventually flows into `config.system.build.toplevel`.

2. **The toplevel derivation.** `config.system.build.toplevel` is a Nix derivation whose build output is a directory containing everything needed to activate the system: boot scripts, `/etc` generation scripts, the system profile, kernel, initrd, and references to every package in the closure. Building this derivation transitively builds every dependency.

3. **Boot image generation.** The toplevel derivation produces initrd and kernel references. Boot loader configuration (GRUB, systemd-boot) creates entries pointing to this toplevel. The system is bootable because the boot entry points to a self-contained closure in `/nix/store/`.

4. **Activation.** `nixos-rebuild switch` builds the new toplevel, creates a new generation symlink, updates the boot loader, and runs activation scripts that rebuild `/etc` (as symlinks into the store), create users, and switch services. The previous generation remains intact for rollback.

This is a complete pipeline from "declarative specification" to "bootable, activatable system image." Every piece — kernel, initrd, `/etc`, services, packages — is a Nix derivation whose output lives in the content-addressed store.

#### What pane needs from this (without NixOS's module system)

Pane doesn't need NixOS's 2,000+ modules. It needs the same *pipeline* — declarative spec → complete system closure → bootable image — with its own system layer in place of NixOS's.

The pipeline, concretely:

1. **A system builder.** A Nix function that takes pane's configuration as input and produces a derivation whose output contains: kernel, initrd, s6-linux-init boot scripts, s6-rc compiled service database, `/etc/pane/` defaults, the pane server binaries, the kit libraries, and a system profile linking to all installed packages. This is pane's equivalent of `config.system.build.toplevel`.

2. **An initrd builder.** Nix expression that assembles a minimal initrd: kernel modules for the root filesystem, the stage-1 init script (mount root, pivot, exec s6-linux-init). not-os and sixos both demonstrate this — it's a few hundred lines of Nix.

3. **A boot image builder.** Produces an ISO, VM image, or disk image containing the kernel, initrd, and root filesystem. nixos-generators (now upstreamed into nixpkgs as `nixos-rebuild build-image`) supported 30+ formats. Pane can reference these format modules or write simpler ones for the specific targets it needs (QEMU/KVM image for development, ISO for installation, disk image for deployment).

4. **An activation mechanism.** Pane's equivalent of `nixos-rebuild switch`: build the new system closure, create a new generation, update the boot loader, activate the new configuration. With s6, this means: compile a new s6-rc database from the new service definitions, run `s6-rc-update` to live-switch to the new database, update symlinks for `/etc/pane/` defaults.

The key insight from NixOS: *the system closure is just a derivation*. It's built the same way any Nix package is built — by declaring inputs and a build script. NixOS's module system is one way to compose the inputs; sixos's infuse combinator is another; a simpler approach (a single Nix function that takes a configuration attrset) is a third. The pipeline doesn't depend on the module system.

#### Building without NixOS

The NixOS Discourse thread on "Building a non-NixOS Linux image using Nix" identifies two key techniques:

- `buildEnv` merges multiple packages into a unified directory structure (a merged FHS tree via symlinks to store paths).
- `pkgs.closureInfo` computes the complete dependency closure of a derivation — every store path transitively required. This is how you go from "I want these packages" to "here is the complete set of files that must be on disk."

Combined with a custom initrd and boot chain, these are sufficient to produce a bootable system image entirely from Nix expressions, without touching NixOS's module system.

---

### 3. Sixos as starting point: architecture in detail

#### The sixos architecture

Sixos is the closest existing precedent for what pane needs. Key architectural decisions from the 38C3 talk and repository:

**Relationship to nixpkgs.** Sixos depends on `nixpkgs/pkgs` for all application packages. It shares zero code with `nixpkgs/nixos`. This is not a fork — sixos is a separate repository that imports nixpkgs as an input. The package ecosystem is treated as a reusable library; the system integration layer is written from scratch.

**The infuse combinator.** Sixos replaces NixOS's module system with `infuse` — a "deep" version of `.override`/`.overrideAttrs` that generalizes `lib.pipe` and `recursiveUpdate`. The key design insight: mark subtrees where automatic merging should stop by converting values into functions. This eliminates the module system's typed options and merge semantics in favor of a simpler combinator that treats all Nix non-finite types (attrsets, lists, functions) uniformly. Infuse satisfies identity and associativity laws, making composition predictable.

In practice: services in sixos's `svcs/by-name/` directory are Nix expressions, paralleling the `pkgs/by-name/` pattern in nixpkgs. The target set (instantiated services) is a scoped fixpoint — the same algebraic structure as nixpkgs's instantiated package set. Standard nixpkgs tools (`override`, `callPackage`, `overrideAttrs`) work on services the same way they work on packages.

**Tag-based configuration.** Rather than per-host customization (the NixOS model), sixos uses "tags" — boolean attributes applied to hosts. Examples: `has-hwclock`, `has-wifi`, `is-nix-builder`. A tag is declared, applied to a host, and customized. This avoids NixOS's pattern of 2,000+ option declarations and instead treats system properties as composable labels.

**The boot sequence with s6-linux-init.** s6-linux-init provides the PID 1 binary. The boot process:

1. The kernel execs `/sbin/init` (produced by `s6-linux-init-maker`).
2. Stage 1 init: sets global resource limits, mounts a tmpfs at `/run`, sets up the environment from `/etc/s6-linux-init/current/env`, and execs `s6-svscan` on the `/run/service` scan directory. **s6-svscan becomes PID 1** and remains PID 1 for the entire machine lifetime.
3. Early services start: s6-svscan scans `/run/service` and starts the services it finds — these are the "always up" services that constitute the init system itself, including the catch-all logger (`s6-svscan-log`).
4. The `rc.init` script runs as stage 2 — this is where system initialization happens: mounting filesystems, starting networking, bringing up the s6-rc service manager. The supervision tree is already in place when rc.init runs.
5. s6-rc takes over service management: `s6-rc-init` sets up the live state directory, then `s6-rc change` brings up the desired service set according to the compiled dependency database.

**Service definitions in s6-rc.** Services are defined as source directories with simple files:

- **Longruns** (daemons): a directory containing a `run` script, optionally `notification-fd`, `dependencies`, `producer-for`/`consumer-for`, etc. The run script execs the daemon.
- **Oneshots** (initialization tasks): a directory containing `up` and `down` scripts. Up runs during startup, down during shutdown.
- **Bundles**: a directory containing a list of service names. A bundle groups services under one name (like systemd's targets).

`s6-rc-compile` processes these source directories into a compiled database — a binary format optimized for fast service state transitions. This compilation step maps naturally to Nix: the source directories are derivation outputs, and the compiled database is another derivation that depends on them. The entire service graph is a Nix expression.

**Shutdown.** Shutdown runs rc.shutdown with the supervision tree still in place (services can still be managed). After rc.shutdown completes, s6-linux-init-shutdownd kills remaining processes, unmounts filesystems, and halts/reboots.

**Ownerboot.** On compatible hardware, sixos can manage all mutable firmware — "all the way back to the reset vector — versioned, managed, and built as part of the sixos configuration." This eliminates the artificial distinction between firmware and non-firmware software.

#### What pane needs on top of sixos's base

Sixos provides the system layer (boot chain, s6 integration, service definitions for core infrastructure). Pane needs to add:

1. **Pane-specific service definitions.** s6-rc service directories for: pane-comp (compositor), pane-router, pane-roster, pane-store, pane-fs, elogind (seat management), pipewire (audio), dbus. These are straightforward: each is a longrun with a `run` script that execs the server binary, a `notification-fd` for readiness signaling, and `dependencies` on prerequisite services.

2. **Desktop session management.** The transition from "system booted, services running" to "user logged in, compositor displaying panes." This is the display manager equivalent — the mechanism that starts pane-comp for a user session. With s6, this is a service that manages the user session lifecycle.

3. **The pane system builder.** A Nix function that composes: sixos's base system + pane's service definitions + pane's packages + pane's `/etc/pane/` defaults + the user's configuration → a complete system closure.

4. **The `/etc/pane/` configuration layer.** Pane's filesystem-based live configuration, generated at build time with Nix-provided defaults, writable at runtime for user modifications (see section 6 for the mutable/immutable reconciliation).

5. **Per-user environment management.** Nix profiles for user-installed applications, agent environments, and workspace-specific tooling.

---

### 4. The distribution philosophy: opinionated system, open application layer

#### The tension

Pane's foundations document articulates two seemingly contradictory positions:

- **Deep integration.** "The degree of integration between pane's user experience layer and the underlying system is deep and opinionated — closer to what NeXTSTEP was to Mach/BSD than to what a conventional desktop environment is to its host distribution."

- **Embrace Linux's modularity.** "Our aim is not to reinvent yet-another-suite of yet-another-* tools... The linux ecosystem has been modular since its inception, the way that ours is modular ought to conform with it, so that evolving alongside it is tractable."

This is not actually a contradiction. It is exactly the architecture macOS has maintained for 25 years.

#### The macOS model: opinionated frameworks, open application layer

macOS is deeply opinionated about:
- The kernel and driver model (XNU, I/O Kit — you don't bring your own)
- The display server and compositor (Quartz/WindowServer — there is no alternative)
- The application frameworks (Cocoa/SwiftUI — if you want native integration, you use these)
- The visual design language (the HIG — not enforced but strongly incentivized through the App Store and user expectations)
- The security model (sandboxing, notarization, SIP)
- The init system (launchd — not replaceable)

macOS is completely open about:
- What applications you run (anything that compiles for the platform)
- What languages you develop in (Swift, Objective-C, C, C++, Rust, Python, whatever)
- What tools you install (Homebrew, MacPorts, nix-darwin, direct downloads)
- What Unix tools you use (the BSD userland is all there)

The line is drawn at the *system layer*. Below the line: Apple's choices, not yours. Above the line: your choices, not Apple's. The system layer is opinionated so that the application layer can be free — because the opinionated system layer provides the guarantees (consistent rendering, reliable IPC, predictable lifecycle management) that applications depend on.

#### How Nix enables this for pane

Nix is the mechanism that lets pane draw this line:

**Below the line (the pane system layer):**
- Linux kernel (configured and built by pane's Nix expressions)
- s6 init system (configured by pane's service definitions)
- Pane's core servers (pane-comp, pane-router, pane-roster, pane-store, pane-fs)
- Pane's kits (pane-proto, pane-app, pane-ui, pane-text, pane-ai)
- Desktop infrastructure (elogind, pipewire, dbus)
- The pane aesthetic (built into the kits — the Interface Kit IS the visual design)
- `/etc/pane/` configuration defaults

This is the *opinionated layer*. Pane controls every piece. The Nix expressions that define this layer are pane's, not upstream's. The user doesn't pick an init system or a compositor or a kit — these are pane.

**Above the line (the open application layer):**
- All ~120,000+ packages in nixpkgs
- User-installed applications via Nix profiles
- Legacy Wayland applications (Firefox, Inkscape, etc.)
- TUI applications (which compose naturally with pane's textual interface)
- Per-user development environments (`nix develop`, devenv, etc.)
- Agent environments (running as separate Nix-managed user profiles)

This is the *open layer*. The user installs whatever they want from the vast nixpkgs ecosystem. Pane doesn't gatekeep application packages. The system layer provides the guarantees; the application layer provides the freedom.

**Nix makes this layering explicit.** The pane distribution is a flake. The flake's `inputs` include nixpkgs (for the package ecosystem). The flake's `outputs` include the system builder (the opinionated layer). The user's system configuration is another flake that imports the pane flake and adds their own packages, agents, and configuration. The boundary between pane's opinions and the user's choices is a flake input boundary — clean, explicit, versionable.

#### The source-based philosophy, reconciled

The foundations document references source-based distributions as a model: "the lack of opinionation was *their* uniform design principle." Gentoo and friends thrived because they let users configure everything.

Pane's position is more nuanced: opinionated about the *system layer* (you get s6, you get pane-comp, you get the kits), unopinionated about the *application layer* (install anything from nixpkgs). This is exactly what Nix enables: the system layer is a curated, tested, integrated whole; the application layer is nixpkgs's vast, permissive ecosystem. The user doesn't need to make system-level choices (which init system? which display server?) because those choices are pane's identity. But they have complete freedom in what they build on top.

The "embrace Linux's modularity" principle applies at the package level: pane composes with the Linux ecosystem's packages, tools, and libraries. The "deep integration" principle applies at the system level: pane's core is not a collection of interchangeable parts.

---

### 5. The ship of Theseus with Nix

#### Atomic upgrades and rollback

Nix's generation model directly supports the "every part can be rewritten as needs evolve" principle from the foundations:

- **Each system rebuild creates a new generation.** The previous generation is preserved in `/nix/store/` and accessible via the boot loader. The system's identity is its configuration, not its specific package versions.

- **Rollback is instant.** `pane-rebuild switch --rollback` (or selecting an older generation at boot) activates the previous system closure. No reinstallation, no package downgrade dance. The old closure is complete and self-contained.

- **Upgrades are atomic.** The switch from one generation to another is a symlink swap + service reload. There is no intermediate state where half the system is old and half is new. With s6-rc, the service transition is: compile new database → `s6-rc-update` → services restart against the new definitions.

- **Generations are cheap.** Because Nix's store is content-addressed, packages shared between generations occupy disk space only once. A system with 50 generations doesn't use 50x the disk — it uses 1x plus the delta for each generation.

This means pane can evolve aggressively. A pane release that rewrites the compositor from scratch doesn't risk the user's system — the previous generation's compositor is still there, one rollback away. The "ship of Theseus" is not just a metaphor; it's the operational model of how the system updates.

#### Per-user profiles and the multi-user architecture

Nix's per-user profile model maps directly to pane's multi-user (human + agent) architecture:

**How Nix profiles work.** Each user has an independent profile — a symlink chain: `~/.nix-profile` → current generation link → user environment (a merged symlink tree of activated packages) → individual package store paths. Users install, remove, and rollback packages independently. Packages shared between users are deduplicated in the store.

**What this means for pane's agent model:**

- Each agent is a system user (as described in the architecture spec's §pane-ai). Each agent has its own Nix profile — its own set of installed tools, its own generation history, its own rollback capability.
- An agent's environment is declaratively specified. The `.plan` file describes what the agent does; its Nix profile describes what tools it has. Both are versionable, shareable, reproducible.
- Agent environments are isolated. Agent A's installed packages don't affect Agent B's. The human user's packages don't affect agents. Nix's store deduplication means shared packages (coreutils, etc.) aren't duplicated on disk.
- `nix profile diff-closures` shows exactly what changed between two generations of any profile — human or agent. Full auditability of what tools each system participant has access to.

**Per-workspace environments.** Nix's `nix develop` (or devenv) provides ephemeral, project-scoped environments. A pane user working on a Rust project enters a development environment with the Rust toolchain, specific library versions, and project-specific tools — without installing anything globally. This composes with pane's tag-based workspace model: a tag set for "rust development" could include both the pane layout and the Nix development environment.

#### Flake-based system definitions

The entire pane distribution can be defined as a flake:

```nix
# pane/flake.nix — the distribution definition
{
  description = "Pane — a desktop environment and distribution for Linux";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # sixos or pane's own system layer as base
  };

  outputs = { self, nixpkgs, ... }: {
    # The system builder — produces a complete pane system
    lib.mkPaneSystem = { hostConfig }: ...;

    # Default system closure for a reference machine
    nixosConfigurations.pane-reference = ...;

    # Packages provided by pane (servers, kits, tools)
    packages.x86_64-linux = {
      pane-comp = ...;
      pane-router = ...;
      pane-roster = ...;
      # ...
    };

    # The overlay that replaces systemd.lib with libudev-zero
    overlays.default = final: prev: { ... };
  };
}
```

A user's system configuration is another flake that imports pane's:

```nix
# user/flake.nix — a user's system definition
{
  inputs = {
    pane.url = "github:pane-project/pane";
    nixpkgs.follows = "pane/nixpkgs";  # use pane's pinned nixpkgs
  };

  outputs = { self, pane, nixpkgs }: {
    paneConfigurations.my-machine = pane.lib.mkPaneSystem {
      hostConfig = {
        hostname = "my-machine";
        locale = "en_US.UTF-8";
        users.lane = {
          shell = "ksh";
          packages = with nixpkgs.legacyPackages.x86_64-linux; [
            firefox neovim ripgrep
          ];
        };
        agents.reviewer = {
          plan = ./agents/reviewer.plan;
          packages = [ /* agent tools */ ];
        };
      };
    };
  };
}
```

The flake.lock pins exact versions of pane and nixpkgs. `pane-rebuild switch` builds and activates. The entire system is reproducible from the flake definition.

---

### 6. The mutable/immutable tension: reconciling Nix with filesystem-based live configuration

#### The problem stated precisely

Pane wants two things simultaneously:

1. **Immutable, reproducible base.** The system closure should be deterministic. Given the same flake inputs, the same system is produced. Rollback should restore exactly the previous state. This is Nix's strength.

2. **Writable, live configuration.** `/etc/pane/` is writable. Each config key is a file. Servers watch via pane-notify and react to changes without restart. Users (and agents) configure pane by editing files, not by writing Nix. This is pane's filesystem-as-interface commitment.

These are not in conflict if the roles are clear:

**Nix manages the defaults.** The system build produces default configuration files in the Nix store (immutable, content-addressed). These are the "factory settings."

**The runtime manages the overrides.** User modifications to `/etc/pane/` persist across reboots as writable state. Servers read runtime config, not store paths.

#### Three patterns for reconciliation

**Pattern A: Overlayfs (NixOS's newer approach).**

```
/etc/pane/ = overlay(
  lower = /nix/store/<hash>-pane-config-defaults/  (read-only)
  upper = /persist/etc/pane/                        (writable)
)
```

The overlay presents a merged view: Nix-generated defaults for any key the user hasn't modified, user's values for any key they have. System rebuild updates the lower layer; user modifications in the upper layer are preserved. Unused upper-layer files (overrides of defaults that no longer exist) can be garbage-collected.

Advantages: clean separation, standard Linux mechanism, user modifications survive rebuild. Disadvantage: requires overlayfs support (universal on modern kernels), slightly complex mount setup.

**Pattern B: Erase-your-darlings with explicit persistence.**

The "erase your darlings" pattern (Graham Christensen) takes a more radical approach: the root filesystem is ephemeral — ZFS rolls it back to blank at every boot. Only explicitly listed state persists via bind mounts from `/persist`. For pane:

- `/persist/etc/pane/` — user's config modifications survive reboot
- On boot, Nix-generated defaults are written to `/etc/pane/`
- Persisted overrides are bind-mounted or symlinked on top

The advantage: forces all mutable state to be explicitly declared. Nothing accumulates silently. The system has "new computer smell on every boot" (Christensen) — every reboot tests the entire automation pipeline. The disadvantage: requires ZFS (or btrfs with equivalent snapshot semantics) and disciplined state management.

**Pattern C: Generation-aware config (pane-specific).**

Pane could implement its own pattern, tuned to the filesystem-as-configuration model:

1. At build time, Nix produces `/nix/store/<hash>-pane-config/` with all default configs.
2. On first boot (or after `pane-rebuild switch`), a lightweight activation script diffs the new defaults against `/etc/pane/`:
   - New keys: added to `/etc/pane/` with default values
   - Removed keys: optionally cleaned up (or flagged)
   - Changed defaults: if the user hasn't modified the key, update to new default; if the user *has* modified it, preserve their value
3. `/etc/pane/` is a regular writable directory on a persistent volume.
4. `pane-rebuild` records which keys the user has modified (via xattr `user.pane.modified = true` or a manifest file) so it can distinguish "user chose this value" from "user got this value from an older default."

This is closer to how Haiku's packagefs shine-through directories work: the package layer provides defaults, the writable layer provides overrides, and the system tracks what came from where.

#### Recommendation for pane

Start with Pattern C. It's the simplest, requires no special filesystem features, and aligns with pane's existing filesystem-as-interface model. The activation script is a small piece of infrastructure (<200 lines). The xattr or manifest approach for tracking user modifications is native to pane's attribute model.

Pattern A (overlayfs) is a clean upgrade path if the complexity is warranted. Pattern B (erase-your-darlings) is the gold standard for reproducibility but requires ZFS and a discipline that may be too aggressive for a desktop-oriented system where users expect their tweaks to survive.

The key principle: **Nix owns the defaults; the user owns the overrides; the activation script mediates between them.** This is analogous to how macOS handles preferences — the system provides defaults in `/Library/Preferences/`, applications read from `~/Library/Preferences/`, and `defaults write` modifies the user layer. The system update doesn't clobber user preferences.

---

### 7. Binary cache and distribution infrastructure

#### What needs rebuilding

The libudev replacement overlay (§4 of the existing research) triggers rebuilds of every package that transitively depends on `systemd.lib`. This is the desktop stack: PipeWire, libinput, Mesa, Wayland libraries, most GUI applications. The estimate from the existing research: hundreds to low thousands of packages.

Everything else — CLI tools, compilers, interpreters, server software, libraries that don't touch hardware — hits the standard nixpkgs cache as-is. This is the vast majority of nixpkgs's 120,000+ packages.

Pane's own packages (the servers, the kits) are novel derivations that obviously need building. These are small relative to the desktop stack.

#### Infrastructure requirements

**Option 1: Cachix (recommended for initial development).**

Cachix provides hosted Nix binary caches with CDN distribution via Cloudflare. Free for open source. Workflow: CI builds pane's overlay packages and pushes to a pane-specific cache. Users add the cache (`cachix use pane`) and get pre-built binaries.

Cachix reports 521 TB transferred monthly across 7,613 caches. The infrastructure is proven at scale. For pane's initial needs (a few thousand rebuilt packages), the free tier is likely sufficient.

**Option 2: Attic (for eventual self-hosting).**

Attic is a self-hostable Nix binary cache backed by S3-compatible storage. Features: global deduplication across caches, managed signing (users never see signing keys), garbage collection. The architecture supports both single-machine setups and distributed deployments.

For a mature pane distribution with its own build farm, Attic on S3-compatible storage (Backblaze B2, Cloudflare R2, self-hosted MinIO) provides full control over the binary distribution pipeline at modest cost.

**Option 3: Sixos's approach (768-core build farm).**

Sixos runs its own 24-server/768-core build farm, hosted by TVL (The Very Large?). This is the scale needed for a production distribution that rebuilds a significant portion of the desktop stack. Pane doesn't need this initially but might eventually if it diverges significantly from standard nixpkgs.

#### Build strategy to minimize cache divergence

Not everything needs to diverge from upstream. A strategic approach:

1. **Phase 1: Minimal overlay.** Override only `systemd.lib` → libudev-zero. Accept the rebuild of direct libudev dependents. Push these to Cachix. Everything else hits the upstream cache.

2. **Phase 2: Pane packages.** Add pane's own packages (servers, kits) to the cache. These are novel and small.

3. **Phase 3: Patched desktop stack.** As pane matures, some upstream packages may need patches (PipeWire configuration, Mesa options, etc.). Each patch adds to the rebuild set. Be strategic: only patch what you must.

4. **Phase 4: Dedicated build infrastructure.** When the rebuild set is large enough that Cachix is insufficient, stand up Attic on S3 with a dedicated build machine (or farm). At this point, pane is a real distribution with real infrastructure needs.

#### CI/CD for a Nix-based distribution

The standard pipeline:

1. **GitHub Actions or similar CI** runs `nix build` on every commit to the pane flake.
2. **Cachix push** uploads built artifacts to the binary cache.
3. **Periodic full rebuilds** (weekly or on nixpkgs input bump) rebuild the complete system closure to catch regressions.
4. **Per-PR builds** verify that changes don't break the system closure.
5. **Release branches** pin specific nixpkgs revisions and gate on full test suites.

Nix makes this tractable because rebuilds are incremental — only derivations whose inputs changed are rebuilt. A nixpkgs bump might rebuild hundreds of packages, but a pane-only change (new version of pane-comp) only rebuilds the pane packages.

---

### 8. Practical architecture: what the pane-on-nix layering looks like

#### The layer stack

```
Application packages (from nixpkgs — ~120,000 packages)
    ↓ installed via Nix profiles (per-user)
Pane kits (pane-proto, pane-app, pane-ui, pane-text, pane-ai)
    ↓ linked by pane-native applications
Pane core servers (pane-comp, pane-router, pane-roster, pane-store, pane-fs)
    ↓ managed as s6-rc longruns
Desktop infrastructure (elogind, pipewire, dbus)
    ↓ managed as s6-rc longruns
s6 init (s6-linux-init as PID 1, s6-svscan, s6-rc)
    ↓ the init system
Linux kernel (built by Nix, configured for pane's needs)
    ↓ the foundation
Hardware
```

Each layer is a set of Nix derivations. The system builder composes them into a closure.

#### Where configuration lives

| What | Where | Managed by |
|------|-------|------------|
| System closure definition | `flake.nix` + `flake.lock` | Nix (declarative, immutable) |
| Kernel configuration | Nix expression (kconfig) | Nix (declarative, immutable) |
| s6 service definitions | Nix derivations → s6-rc source dirs → compiled database | Nix (declarative, immutable) |
| Boot chain | Nix derivation (initrd, kernel, boot loader config) | Nix (declarative, immutable) |
| Pane server defaults | `/nix/store/<hash>-pane-config/` → activated to `/etc/pane/` | Nix provides defaults; user provides overrides |
| User packages | `~/.nix-profile` (per-user Nix profile) | Nix (per-user, rollbackable) |
| Agent environments | `/home/<agent>/.nix-profile` | Nix (per-agent, declared in `.plan`) |
| Runtime pane configuration | `/etc/pane/<server>/<key>` (writable files) | User/agent edits, pane-notify watches |
| Routing rules | `/etc/pane/route/rules/` and `~/.config/pane/route/rules/` | Filesystem (drop a file, gain behavior) |
| User data | `~/.local/`, `~/Documents/`, etc. | User (not managed by Nix) |

The clean separation: **Nix manages the reproducible base; the filesystem manages the mutable state; pane-notify bridges the two at runtime.**

#### The system builder sketch

```nix
# lib/mkPaneSystem.nix
{ nixpkgs, panePackages, s6Packages }:
{ hostConfig }:
let
  pkgs = import nixpkgs {
    system = "x86_64-linux";
    overlays = [ paneOverlay ];  # libudev-zero, pane packages
  };

  # Build the s6-rc service database
  serviceDatabase = pkgs.runCommand "pane-services" {} ''
    mkdir -p $out/sv

    # Core s6-rc service definitions (each a directory with run, dependencies, etc.)
    ${lib.concatMapStrings mkServiceDir paneServices}

    # Compile the database
    ${pkgs.s6-rc}/bin/s6-rc-compile $out/db $out/sv
  '';

  # Build the initrd
  initrd = pkgs.makeInitrd {
    contents = [ kernelModules stage1Init ];
  };

  # Build /etc/pane/ defaults
  paneConfigDefaults = pkgs.runCommand "pane-config-defaults" {} ''
    mkdir -p $out/comp $out/router $out/roster $out/store
    # Generate default config files from hostConfig
    ${lib.concatMapStrings mkConfigFile hostConfig.pane}
  '';

  # The system closure
  toplevel = pkgs.runCommand "pane-system" {} ''
    mkdir -p $out
    ln -s ${kernel} $out/kernel
    ln -s ${initrd} $out/initrd
    ln -s ${serviceDatabase} $out/services
    ln -s ${paneConfigDefaults} $out/config-defaults
    ln -s ${systemProfile} $out/profile
    # Boot loader configuration, activation script, etc.
  '';
in toplevel
```

This is a sketch, not production code. The point is that the system closure is *just a derivation* — it references other derivations (kernel, initrd, services, config), and Nix builds the whole graph transitively.

#### How `pane-rebuild switch` works

1. User runs `pane-rebuild switch` (or `nix build .#paneConfigurations.my-machine` + activate).
2. Nix evaluates the flake, builds the new system closure (or fetches from cache).
3. The activation script:
   a. Creates a new generation symlink (`/nix/var/nix/profiles/system-N-link → /nix/store/<hash>-pane-system`).
   b. Updates the boot loader (GRUB or systemd-boot entry).
   c. Activates new `/etc/pane/` defaults (diff + merge as in Pattern C above).
   d. Compiles and activates the new s6-rc database via `s6-rc-update`.
   e. Pane servers reload configuration via pane-notify (the config files changed on disk).
4. The system is now running the new closure. The previous closure remains available for rollback.

No reboot required for most changes. Kernel updates require reboot (as on any Linux system). s6-rc's live database update means service changes take effect immediately.

---

### 9. How this informs pane's design: the distribution-level answer

The question this research was meant to answer: how does Nix serve a distribution that wants to be *one integrated thing*?

The answer, informed by NeXTSTEP's architecture and Nix's capabilities:

**Nix is the build substrate, not the identity.** Just as Mach was NeXTSTEP's kernel but not its identity, Nix is pane's build system but not its personality. The user doesn't interact with Nix to use pane — they interact with pane's interfaces (the compositor, the kits, the filesystem, the routing infrastructure). Nix builds the system and manages its evolution; pane presents the experience.

**The system closure is one artifact.** Nix builds the entire pane system — kernel through desktop — as a single, transitively-closed derivation. This is the "one thing" property: not a kernel plus a compositor plus packages glued together at runtime, but a single build artifact where every dependency is accounted for and versioned. The system closure is pane's equivalent of a NeXTSTEP release — a complete, tested, integrated system image.

**Opinionated system layer, open application layer.** Nix's flake model makes the boundary explicit. Pane's flake defines the system layer (init, servers, kits, defaults). The user's flake adds applications from nixpkgs. The system layer is pane's opinion; the application layer is the user's choice. This is the macOS model, enabled by Nix's compositional dependency management.

**Evolution without fragility.** Nix's generations, atomic upgrades, and rollback mean pane can evolve aggressively — rewriting components, changing defaults, updating the system layer — without risking the user's ability to get back to a working state. The ship of Theseus sails with a Nix store full of previous generations.

**The multi-inhabitant model.** Nix's per-user profiles give each system participant (human, agent) an independent, rollbackable, declarative package environment. The infrastructure for "agents as system users" is not something pane needs to build — Nix already provides it.

**Infrastructure, not bureaucracy.** Nix provides the reproducibility, the versioning, the atomic upgrades, the binary caching, the dependency management. But it doesn't impose a particular experience. Pane's system builder translates pane's opinions into Nix derivations. The user configures pane through `/etc/pane/` (writable files, pane-notify, live updates) — not through Nix expressions. Nix is the backstage infrastructure; pane is the show.
