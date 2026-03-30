# Research: Cross-Platform Rust Development (macOS host, Linux target)

**Date:** 2026-03-27
**Context:** pane is developed on aarch64-darwin (Apple Silicon Mac) but targets aarch64-linux exclusively for pane-comp (Smithay/Wayland compositor). The workspace already excludes pane-comp from `[workspace.members]` and has a NixOS test VM. This research evaluates options for tightening the development loop.

## Current State

The project already has:
- A nix-darwin linux-builder module (`nix/darwin-linux-builder.nix`) with 4 cores / 4GB RAM
- A full NixOS test VM (`nix/vm.nix`) with Sway, SSH, 9p mount for live iteration
- `just build-linux` delegates `nix build .#packages.aarch64-linux.pane-comp` through the builder
- `just vm-dev` SSHes into the VM and runs `cargo build -p pane-comp` natively
- Workspace `Cargo.toml` excludes pane-comp from default members; macOS builds the cross-platform crates only

The gap: there is no way to **compile** pane-comp or **run its tests** from macOS without either (a) a full VM boot or (b) waiting for `nix build` to do a clean derivation build. Fast iteration on pane-comp requires SSHing into the VM.

---

## Option 1: nix-darwin linux-builder (current approach, tuned)

**How it works:** A QEMU-backed NixOS VM runs as a launchd service. The Nix daemon automatically delegates aarch64-linux builds to it via SSH (`ssh-ng://builder@linux-builder`). When you run `nix build .#packages.aarch64-linux.foo`, the derivation is built inside the VM and the result is copied back.

**Setup (already done, but tuneable):**
```nix
# nix-darwin configuration
{
  nix.linux-builder = {
    enable = true;
    ephemeral = true;   # fresh disk on restart
    maxJobs = 4;
    config = {
      virtualisation = {
        darwin-builder = {
          diskSize = 40 * 1024;  # 40 GB
          memorySize = 8 * 1024; # 8 GB
        };
        cores = 6;
      };
    };
  };
  nix.settings.trusted-users = [ "@admin" ];
}
```

**What it gives us:**
- `nix build .#packages.aarch64-linux.pane-comp` works from macOS
- NixOS integration tests can run on macOS (needs `system-features = [ "nixos-test" "apple-virt" ]`)
- No Docker dependency

**Limitations:**
- Performance is "relatively slow" -- QEMU on Apple Silicon without Rosetta in the VM means no hardware acceleration for the guest. Builds are significantly slower than native.
- Cannot run `cargo build` / `cargo test` interactively inside the builder -- it is a headless Nix build sandbox.
- The binary cache cannot be used for cross-compiled outputs, so every dependency compiles from source on first build.
- Not suitable for rapid iteration; better for CI-style "does it build?" checks.

**Verdict:** Already in place. Good for gating, not for development flow.

**Sources:**
- [Nixcademy: Build and Deploy Linux Systems from macOS](https://nixcademy.com/posts/macos-linux-builder/)
- [nixpkgs darwin-builder docs](https://ryantm.github.io/nixpkgs/builders/special/darwin-builder/)
- [Adrian Hesketh: Setting up a NixOS remote builder for M1 Mac](https://adrianhesketh.com/2024/04/20/setting-up-nixos-remote-builder-m1-mac/)

---

## Option 2: Determinate Nix native Linux builder

**How it works:** Determinate Nix 3.8.4+ uses macOS's built-in Virtualization.framework (the same hypervisor used by Docker Desktop and UTM) to run Linux derivations natively. No separate VM management -- `determinixd` spawns a lightweight Linux environment on demand.

**Configuration** (`/etc/determinate/config.json`):
```json
{
  "builder": {
    "state": "enabled",
    "memoryBytes": 8589934592,
    "cpuCount": 1
  }
}
```

**Key detail:** The docs explicitly warn that `cpuCount > 1` is *slower* than 1 due to Virtualization.framework overhead. This is a significant limitation.

**What it gives us:**
- Zero-config `nix build nixpkgs#legacyPackages.aarch64-linux.foo` from macOS
- Supports both aarch64-linux and x86_64-linux targets
- No nix-darwin module needed -- built into the Nix daemon

**Limitations:**
- Requires Determinate Nix (not upstream Nix or Lix). Pane uses Lix.
- Single-CPU performance constraint makes it no faster than the nix-darwin builder for large Rust compilations.
- Same "Nix sandbox only" limitation -- no interactive cargo sessions.
- Still in relatively early stages; rolled out to all Determinate users as of late 2025.

**Verdict:** Incompatible with Lix. If the project ever moved to Determinate Nix, this would replace Option 1 with less config, but the performance characteristics are equivalent.

**Sources:**
- [Determinate Systems: Native Linux Builder for macOS](https://determinate.systems/blog/changelog-determinate-nix-384/)
- [Determinate Nix docs](https://docs.determinate.systems/determinate-nix/)

---

## Option 3: True cross-compilation (nix crossSystem)

**How it works:** Nix instantiates nixpkgs with `localSystem = "aarch64-darwin"` and `crossSystem = "aarch64-unknown-linux-gnu"`, producing a cross-toolchain that runs on macOS but emits Linux binaries.

**Example with crane:**
```nix
let
  pkgs = import nixpkgs {
    localSystem = "aarch64-darwin";
    crossSystem = { config = "aarch64-unknown-linux-gnu"; };
    overlays = [ rust-overlay.overlays.default ];
  };
  craneLib = (crane.mkLib pkgs).overrideToolchain
    (p: p.rust-bin.stable.latest.default.override {
      targets = [ "aarch64-unknown-linux-gnu" ];
    });
in craneLib.buildPackage { ... }
```

**What it gives us:**
- Compilation runs on the Mac's native CPU -- no VM overhead
- Can potentially be very fast for the Rust compilation step itself

**Limitations -- and this is where it breaks for pane-comp:**
- Darwin-to-Linux cross-compilation in nixpkgs is poorly supported. The nix.dev documentation explicitly states: "macOS/Darwin is a special case, as not the whole OS is open-source. It's only possible to cross compile between aarch64-darwin and x86_64-darwin."
- pane-comp depends on `wayland`, `libinput`, `libdrm`, `mesa`, `seatd`, `udev`, `pixman`, `libgbm`, `libglvnd` -- all Linux-only C libraries. Cross-compiling these from Darwin requires a complete Linux cross-sysroot, which nixpkgs does not provide for darwin hosts.
- Even if you could get the C libraries to cross-compile, Smithay's build.rs scripts run host-side tools (wayland-scanner, pkg-config) that expect Linux paths.
- The official Nix binary cache has no cross-compiled outputs, so you'd rebuild the entire dependency closure.
- N8henrie's successful example was for a pure Rust binary with no native dependencies. Once you add C libraries that depend on Linux kernel headers, it falls apart.

**Verdict:** Not viable for pane-comp. The native C dependency chain (wayland, libinput, mesa, udev) makes darwin-to-linux cross-compilation a non-starter with current nixpkgs. Would work for pure-Rust crates only.

**Sources:**
- [nix.dev cross-compilation tutorial](https://nix.dev/tutorials/cross-compilation.html)
- [Crane cross-compilation example](https://crane.dev/examples/cross-rust-overlay.html)
- [N8henrie: Cross-Compile Rust for x86 Linux from M1 Mac](https://n8henrie.com/2023/09/crosscompile-rust-for-x86-linux-from-m1-mac-with-nix/)
- [Ayats: Cross-compilation with Nix](https://ayats.org/blog/nix-cross)

---

## Option 4: cross-rs (Docker-based)

**How it works:** `cross` is a drop-in replacement for `cargo` that transparently runs builds inside Docker containers with pre-configured cross-toolchains. `cross build --target aarch64-unknown-linux-gnu` pulls a Docker image with the right gcc, sysroot, and linker.

**What it gives us:**
- Interactive `cargo build` and `cargo test` semantics
- The Docker image contains a full Linux sysroot, so C dependencies can be satisfied
- Supports running tests via QEMU inside the container

**Limitations:**
- Requires Docker (or Podman) on macOS -- adds a runtime dependency outside the Nix ecosystem
- The pre-built cross-rs images do NOT include Wayland/Smithay dependencies (wayland-scanner, libinput-dev, mesa, etc.). You'd need a custom Dockerfile that adds all of pane-comp's deps.
- Maintaining a custom Docker image is ongoing work and duplicates what the Nix devShell already defines.
- Docker on macOS runs a Linux VM anyway (Docker Desktop uses Virtualization.framework), so this is ultimately "VM with extra steps."
- No Nix integration -- the Docker build wouldn't use the Nix store or benefit from Nix caching.

**Verdict:** Technically possible but creates a parallel dependency management system alongside Nix. The custom Dockerfile maintenance burden is significant and the performance is no better than the VM approach since Docker on macOS *is* a VM.

**Sources:**
- [cross-rs/cross GitHub](https://github.com/cross-rs/cross)
- [cross-rs Getting Started](https://github.com/cross-rs/cross/wiki/Getting-Started)

---

## Option 5: Flakebox

**How it works:** Flakebox wraps crane with opinionated cross-compilation support. It provides `nix develop` shells that include cross-toolchains and lets you run `cargo build --target aarch64-unknown-linux-gnu` inside a Nix devShell.

**What it gives us:**
- Dev shell with cross-toolchain pre-configured
- Supports "build on Linux and Darwin, targeting Linux, Android, Darwin and iOS"
- Handles non-Rust dependencies through Nix

**Limitations:**
- Same fundamental darwin-to-linux cross-compilation problem as Option 3: the Linux C sysroot issue. Flakebox can set up the Rust cross-target, but it cannot conjure wayland-dev headers for a Linux target on a Darwin host.
- The project is maintained by the Fedimint team and is oriented toward their use case (pure Rust + OpenSSL). Complex compositor deps like Smithay + mesa + libinput are not in their test matrix.
- Adds another layer of abstraction on top of crane.

**Verdict:** Same blocker as Option 3. Useful for projects with simpler native deps, but pane-comp's dependency chain defeats it.

**Sources:**
- [Flakebox GitHub](https://github.com/rustshop/flakebox)
- [Flakebox announcement on NixOS Discourse](https://discourse.nixos.org/t/rustshop-flakebox-rust-dx-we-can-share-and-love/33361)

---

## How comparable projects handle this

### cosmic-comp (System76 COSMIC compositor)

- **Systems supported in flake.nix:** `aarch64-linux` and `x86_64-linux` only. No Darwin targets.
- **Development workflow:** Developers work on Linux machines. The flake provides a devShell with all Wayland/Smithay deps.
- **CI:** GitHub Actions on Linux runners. No macOS cross-compilation.
- **Nix integration:** Uses `rustPlatform.buildRustPackage` with a custom `rustPlatformFor` function. The nixos-cosmic community flake packages all COSMIC components for NixOS.
- **Takeaway:** System76 does not attempt macOS development of the compositor at all. It is a Linux-only development workflow.

### niri (scrollable-tiling Wayland compositor)

- **Systems supported in flake.nix:** `lib.intersectLists lib.systems.flakeExposed lib.platforms.linux` -- all Linux platforms, no Darwin.
- **Development workflow:** Linux-only devShell with nightly Rust, all Wayland deps, cargo-insta for snapshot testing.
- **Build outputs:** Standard and debug builds; debug builds used in CI checks for speed.
- **Takeaway:** Same pattern as cosmic-comp. No macOS development path for the compositor.

**Sources:**
- [cosmic-comp flake.nix](https://github.com/pop-os/cosmic-comp/blob/master/flake.nix)
- [nixos-cosmic community flake](https://github.com/lilyinstarlight/nixos-cosmic)
- [niri flake.nix](https://github.com/niri-wm/niri/blob/main/flake.nix)
- [niri-flake (sodiboo)](https://github.com/sodiboo/niri-flake)

---

## Recommendation: Tuned VM with persistent cargo cache

The industry answer is clear: **Wayland compositor development happens on Linux.** cosmic-comp and niri don't even try to support macOS. The fundamental problem is that pane-comp's native dependencies (wayland, libinput, mesa, udev, libdrm, seatd) are Linux kernel-coupled libraries that cannot be cross-compiled from Darwin.

Given that pane *is* developed from macOS, the best path is to optimize the VM-based workflow rather than fight cross-compilation:

### Short-term: Tune the existing VM (`nix/vm.nix`)

1. **Persistent cargo registry + target dir.** The 9p mount already shares the source tree. Add a second virtio-fs or 9p mount for `~/.cargo/registry` and a persistent `target/` directory on the VM's disk. This avoids re-downloading crates and re-compiling unchanged deps on every VM boot.

2. **Tune linux-builder resources.** Bump to 8 GB RAM and 6 cores (matching the `nixcademy` recommendation). The current config has 4 cores / 4 GB.

3. **Add `just` recipes for the common loop:**
   ```
   # Watch pane-comp source, rebuild on change (inside VM)
   vm-watch:
       ssh -p 2222 pane@localhost "cd ~/pane && cargo watch -p pane-comp -x build"
   ```

4. **Pre-warm the VM's Nix store.** The VM currently installs rustup + gcc. Switch to a pinned Rust toolchain from nixpkgs (matching `rust-toolchain.toml`) so the VM boots with the exact toolchain ready, no rustup dance needed.

### Medium-term: Remote builder for CI checks

5. **Enable NixOS integration tests from macOS.** Add `system-features = [ "nixos-test" "apple-virt" ]` to the nix-darwin config. This lets `nix build .#checks.aarch64-linux.pane-comp-test` run from macOS, delegating to the linux-builder automatically.

6. **Add a `nix flake check` target** that builds pane-comp and runs its tests as a Nix derivation. This gives a single command (`nix flake check`) that verifies everything -- cross-platform crates on Darwin, pane-comp on Linux via the builder.

### Long-term: Dedicated Linux dev machine or cloud builder

7. **Remote Linux builder via SSH.** A dedicated aarch64-linux machine (Ampere cloud instance, Raspberry Pi 5, or Hetzner ARM box) as a Nix remote builder would be dramatically faster than QEMU. Configure in `/etc/nix/machines`:
   ```
   ssh-ng://builder@linux-box aarch64-linux /path/to/key 8 - - nixos-test
   ```
   This makes all `nix build .#packages.aarch64-linux.*` commands use native hardware.

### What NOT to do

- Do not attempt darwin-to-linux cross-compilation for pane-comp. Every project in this space (cosmic-comp, niri, Smithay examples) treats this as a Linux-only build.
- Do not maintain a parallel Docker-based build system. It duplicates the Nix dependency management without adding capability.
- Do not switch to Determinate Nix just for the builder. The Lix ecosystem alignment is more valuable, and the performance characteristics are equivalent.

---

## Summary table

| Approach | Builds pane-comp? | Runs tests? | Interactive? | Speed | Maintenance |
|---|---|---|---|---|---|
| nix-darwin linux-builder | Yes | Yes (nix check) | No | Slow | Low |
| Determinate native builder | Yes | Yes (nix check) | No | Slow | Low (but requires Determinate Nix) |
| Cross-compilation (nix) | **No** -- C deps | No | N/A | N/A | N/A |
| cross-rs (Docker) | Maybe (custom img) | Maybe | Partial | Slow (VM under Docker) | High |
| Flakebox | **No** -- C deps | No | N/A | N/A | N/A |
| NixOS test VM (current) | Yes | Yes | Yes | Medium | Low |
| Remote Linux builder (SSH) | Yes | Yes | No | **Fast** | Medium |
