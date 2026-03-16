## Context

pane-comp requires Linux system libraries (wayland, libinput, mesa) to compile. The macOS host can't build it natively. Currently we use `nix build .#packages.aarch64-linux.pane-comp` which does a clean build on the linux-builder (~2min), then boot a QEMU VM to test. Every code change requires this full cycle.

## Goals / Non-Goals

**Goals:**
- Incremental cargo builds of pane-comp (~5-10s per change)
- Built binary immediately available in the running VM
- Simple justfile recipes for the workflow

**Non-Goals:**
- Native macOS builds of pane-comp (impossible — needs wayland)
- Replacing the nix build for CI/release (nix stays for reproducible builds)
- GUI forwarding from VM to host (QEMU window is fine)

## Decisions

### 1. SSH cargo build on the linux-builder

The linux-builder is already running and has all the build dependencies. We can SSH in (as root, using the builder key), clone/mount the source, and run `cargo build` directly. This gives us incremental builds — only changed files recompile.

The challenge: the builder SSH key is root-only (`/etc/nix/builder_ed25519`). We need sudo for SSH. Alternatively, we could set up a user-accessible SSH key for the builder.

### 2. virtio-9p shared directory

The QEMU VM already supports virtfs. We mount the host project directory into the VM. The built binary (compiled on the builder or cross-compiled) is placed in the shared directory and immediately available in the VM.

Current VM config already has `xchg` and `shared` mount tags. We add the project directory as another mount.

### 3. Workflow

```
Terminal 1: QEMU VM running (cage + foot)
Terminal 2: just build-fast    # SSH cargo build on builder
Terminal 3 (in VM): /mnt/pane/target/.../pane-comp   # run the binary
```

Or simpler: `just build-fast && just vm-run-comp` which builds then SSHs into the VM and runs the binary.

## Risks / Trade-offs

**[Builder SSH access]** → The builder key is root-only. Using sudo for every build is awkward. Alternative: copy the builder's public key to a user-accessible location, or set up a dedicated build user.

**[Cross-compilation]** → cargo cross-compile from macOS to aarch64-linux is possible (via cross or zigbuild) but requires the Linux sysroot with wayland/mesa headers. More complex than SSH builds. Defer unless SSH approach has issues.

## Open Questions

- Can we run cargo directly on the linux-builder via SSH, or does it not have a full build environment (only nix-build sandbox)?
- Would a persistent Linux dev VM (not just for testing) be better than using the builder?
