## Why

The current dev iteration cycle for pane-comp is ~3 minutes: edit code → `nix build` (rebuilds from source on linux-builder) → `./nix/run-vm-macos.sh` → boot VM → run pane-comp. For renderer work (shader debugging, visual tweaks), this is unusably slow. We need the cycle under 30 seconds: edit → build → see result.

## What Changes

- Set up cargo cross-compilation to aarch64-linux from macOS, or SSH-based cargo build on the linux-builder, so pane-comp builds incrementally (~5s) instead of from scratch (~2min)
- Add a virtio-9p or virtiofs shared directory between host and VM so the binary is immediately available without VM rebuild
- Update the justfile with fast iteration recipes
- Document the workflow in AGENT.md

## Specs Affected

### New
- None (this is infrastructure, not behavioral)

### Modified
- None

## Impact

- justfile gains fast iteration recipes
- VM config may gain shared directory mount
- AGENT.md updated with iteration workflow
- No protocol or spec changes
