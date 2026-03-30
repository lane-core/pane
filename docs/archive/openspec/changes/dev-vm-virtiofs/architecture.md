## Context

We've spent hours fighting 9p permission issues. The root cause: QEMU's 9p `security_model=none` doesn't map UIDs correctly between macOS (uid 501) and the NixOS VM (uid 1000). `access=any` doesn't help. The systemd mount races with boot. Multiple workarounds (sway exec mount, tmpfiles, mapped-xattr) all failed.

Reference projects:
- **phaer/nixos-vm-on-macos**: Uses vfkit (Apple Virtualization.framework) + virtiofs. Works correctly because virtiofs handles permissions natively through Apple's framework. Not QEMU-compatible.
- **jordanisaacs/kernel-development-flake**: Uses virtiofs with virtiofsd daemon. Builds on host, mounts into QEMU VM via virtiofsd.

## Goals / Non-Goals

**Goals:**
- Working dev iteration loop: edit → build → test in VM
- Under 30 seconds from edit to seeing the result
- Clean VM config without hacks

**Non-Goals:**
- Switching from QEMU to vfkit (bigger change, evaluate later)
- Cargo incremental builds inside the VM (nice-to-have, not required)

## Decisions

### 1. Tier 1: scp-based deployment (immediate)

`nix build .#packages.aarch64-linux.pane-comp` already works (~2 min clean, faster with cache). The binary is at `./result/bin/pane-comp`. We `scp` it to the running VM and run it there.

Iteration cycle:
```
edit code → just dev → see result in QEMU
```

Where `just dev` does:
1. `nix build .#packages.aarch64-linux.pane-comp` (cached: ~10s, clean: ~2min)
2. `scp result/bin/pane-comp pane@localhost:~/pane-comp` (via port 2222)
3. `ssh pane@localhost "WAYLAND_DISPLAY=wayland-0 ... ~/pane-comp"` (run it)

This is slower than cargo incremental but it *works* and requires zero filesystem sharing hacks.

### 2. Remove all 9p project mount hacks

Delete: the `project` virtfs in run-vm-macos.sh, the fileSystems entry, the tmpfiles rule, the sway exec mount command, the `access=any` flag. Clean the VM config back to what it was before the 9p adventure.

### 3. Tier 2: virtiofs via virtiofsd (future)

QEMU supports virtiofs through the `virtiofsd` daemon (separate process). The host runs `virtiofsd` sharing the project directory, QEMU connects to it via a vhost-user socket. This gives correct permissions and good performance. But it requires:
- virtiofsd on macOS (available via nixpkgs or homebrew)
- QEMU flags: `-chardev socket,id=vfs,path=/tmp/vfs.sock -device vhost-user-fs-pci,chardev=vfs,tag=project`
- NixOS guest: standard virtiofs mount

This is the proper solution but needs testing. Deferred to after Tier 1 is working.

## Risks / Trade-offs

**[nix build latency]** → ~2 min for clean builds, ~10s when cached. Acceptable for renderer debugging where you make a change, build, look at the result. Not acceptable for tight shader iteration loops. Tier 2 (virtiofs + cargo incremental) solves this.

## Open Questions

- Does virtiofsd work on macOS? It's primarily a Linux tool. May need to run it inside the linux-builder VM.
