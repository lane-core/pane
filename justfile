# pane desktop environment

# --- Build ---

# Build pane-proto (works on any platform)
build:
    cargo build

# Build pane-comp via nix (clean build on linux-builder)
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

# Rebuild VM from scratch (fresh disk + reset SSH key)
vm-fresh: vm-build vm-reset-ssh
    rm -f nixos.qcow2
    ./nix/run-vm-macos.sh

# SSH into the running VM
vm-ssh:
    ssh -p 2222 pane@localhost

# --- Fast iteration (VM must be running) ---

# Set up rust toolchain in VM (run once after first boot)
dev-setup:
    ssh -p 2222 pane@localhost "rustup default stable"

# Incremental cargo build of pane-comp inside the VM
dev-build:
    ssh -p 2222 pane@localhost "cd /mnt/pane && cargo build -p pane-comp"

# Run freshly built pane-comp in the VM
dev-run:
    ssh -p 2222 pane@localhost "WAYLAND_DISPLAY=wayland-0 XDG_RUNTIME_DIR=/run/user/1000 /mnt/pane/target/debug/pane-comp"

# Build and run in one step
dev: dev-build dev-run

# --- Lockfile ---

# Regenerate Cargo.lock with all deps (needed after adding pane-comp deps)
lock-regen:
    #!/usr/bin/env python3
    import subprocess, re
    with open("Cargo.toml") as f: t = f.read()
    with open("Cargo.toml", "w") as f: f.write(t.replace('members = ["crates/pane-proto"]', 'members = ["crates/pane-proto", "crates/pane-comp"]'))
    import os; cargo = os.path.expanduser("~/.cargo/bin/cargo")
    subprocess.run([cargo, "generate-lockfile"], check=True)
    with open("Cargo.toml") as f: t = f.read()
    with open("Cargo.toml", "w") as f: f.write(t.replace('members = ["crates/pane-proto", "crates/pane-comp"]', 'members = ["crates/pane-proto"]'))
    print("Cargo.lock regenerated with all deps")

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

# Remove stale VM SSH host key (after VM rebuild)
vm-reset-ssh:
    ssh-keygen -R "[localhost]:2222"
