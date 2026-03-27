# pane desktop environment
# run `just` to see all recipes

default:
    @just --list

# --- build & test (runs inside direnv shell) ---

# Build all workspace crates
build:
    cargo build

# Run all tests
test:
    cargo test

# Check all crates without building
check:
    cargo check

# Run clippy
lint:
    cargo clippy -- -D warnings

# Format all Rust and Nix code
fmt:
    cargo fmt
    find . -name '*.nix' -not -path './result*' | xargs nixpkgs-fmt

# Generate API documentation
doc:
    cargo doc --no-deps --workspace

# --- specific crates ---

# Test a specific crate
test-crate crate:
    cargo test -p {{crate}}

# Build a specific crate
build-crate crate:
    cargo build -p {{crate}}

# --- nix ---

# Build pane-comp (Linux only, via nix)
build-comp:
    nix build .#packages.aarch64-linux.pane-comp --print-build-logs

# Build for Linux from macOS
build-linux:
    nix build .#packages.aarch64-linux.pane-comp --print-build-logs

# --- vm ---

# Build the test VM
vm-build:
    nix build .#nixosConfigurations.pane-test-vm.config.system.build.vm

# Boot a fresh VM (build + boot)
vm-fresh:
    just vm-build
    ssh-keygen -R "[localhost]:2222" 2>/dev/null || true
    rm -f nixos.qcow2
    ./nix/run-vm-macos.sh

# SSH into the test VM
vm-ssh:
    ssh -p 2222 pane@localhost

# Dev iteration in VM (build + run compositor)
vm-dev:
    ssh -p 2222 pane@localhost "cd ~/pane && cargo build -p pane-comp && WAYLAND_DISPLAY=wayland-0 XDG_RUNTIME_DIR=/run/user/1000 ~/pane/target/debug/pane-comp"

# --- clean ---

# Clean cargo artifacts
clean:
    cargo clean

# Clean everything (cargo + nix + vm)
clean-all:
    cargo clean
    rm -f nixos.qcow2 result result-disk

# --- openspec ---

# List specs and changes
spec-list:
    openspec list

# Validate all specs
spec-validate:
    openspec validate --all

# --- utility ---

# Regenerate Cargo.lock
lock:
    cargo generate-lockfile

# Push to origin
push:
    git push
