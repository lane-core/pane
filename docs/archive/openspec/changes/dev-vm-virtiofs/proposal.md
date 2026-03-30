## Why

The current dev VM uses QEMU's virtio-9p for host→guest file sharing. This has been unreliable: UID mapping issues cause permission errors, the NixOS systemd mount races with boot, and multiple hack attempts have failed to produce a working build-inside-VM workflow. The dev iteration loop (edit on macOS → build → test in VM) doesn't work.

Real projects solving this problem (phaer/nixos-vm-on-macos, jordanisaacs/kernel-development-flake) use either **vfkit with virtiofs** or **build on host + copy binary to VM**. We need to pick one and implement it properly.

## What Changes

Two-tier approach:

**Tier 1 (immediate, works today):** Build pane-comp via `nix build` on the linux-builder (already works), then `scp` the binary into the running VM. No shared filesystem needed. Iteration cycle: edit → `just build comp` → `just deploy` → run in VM.

**Tier 2 (better, needs research):** Evaluate vfkit + virtiofs as a replacement for QEMU + 9p. vfkit uses Apple's Virtualization.framework natively, virtiofs handles permissions correctly. The phaer/nixos-vm-on-macos project demonstrates this working. This would give us true shared-directory incremental builds.

- Remove all 9p hacks from the VM config and QEMU script
- Add `just deploy` recipe: scp nix-built binary to VM
- Add `just dev` recipe: build + deploy + run in one step
- Document the workflow clearly

## Specs Affected

### New
- None

### Modified
- None

## Impact

- nix/vm.nix simplified (remove 9p mount, tmpfiles, sway mount hack)
- nix/run-vm-macos.sh simplified (remove project virtfs)
- justfile updated with deploy-based workflow
- AGENT.md updated with correct iteration instructions
