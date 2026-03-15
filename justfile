# pane desktop environment

# --- Build ---

# Build pane-proto (works on any platform)
build:
    cargo build

# Build pane-comp (requires linux-builder)
build-comp:
    nix build .#packages.aarch64-linux.pane-comp --print-build-logs

# Build everything for linux
build-linux:
    nix build .#packages.aarch64-linux.pane-comp --print-build-logs
    nix build .#packages.aarch64-linux.pane-proto --print-build-logs

# --- Test ---

# Run pane-proto tests
test:
    cargo test

# Run pane-proto tests via nix (linux)
test-linux:
    nix build .#packages.aarch64-linux.pane-proto --print-build-logs

# --- VM ---

# Build the test VM image
vm-build:
    nix build .#nixosConfigurations.pane-test-vm.config.system.build.vm

# Build VM disk image (first time only)
vm-disk:
    nix build .#packages.aarch64-linux.vm-disk -o result-disk

# Run the test VM (builds disk if needed)
vm: vm-build
    ./nix/run-vm-macos.sh

# Rebuild VM from scratch (fresh disk)
vm-fresh: vm-build
    rm -f nixos.qcow2
    ./nix/run-vm-macos.sh

# SSH into the running VM
vm-ssh:
    ssh -p 2222 pane@localhost

# --- Lockfile ---

# Regenerate Cargo.lock with all deps (needed after adding pane-comp deps)
lock-regen:
    #!/usr/bin/env bash
    set -euo pipefail
    sed -i '' 's/members = \["crates\/pane-proto"\]/members = ["crates\/pane-proto", "crates\/pane-comp"]/' Cargo.toml
    cargo generate-lockfile
    sed -i '' 's/members = \["crates\/pane-proto", "crates\/pane-comp"\]/members = ["crates\/pane-proto"]/' Cargo.toml
    echo "Cargo.lock regenerated with all deps"

# --- Spec ---

# List active changes
spec-list:
    openspec list

# Show change status
spec-status change:
    openspec status --change "{{change}}"

# Validate all specs
spec-validate:
    openspec validate --all

# --- Utilities ---

# Push to origin
push:
    git push

# Check what would be committed
status:
    git status
