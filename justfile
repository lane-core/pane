# pane desktop environment
# run `just` to see all recipes

# Target Linux system for cross-builds (change for x86_64)
linux_system := "aarch64-linux"

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
    find . -name '*.nix' -not -path './result*' -exec nixpkgs-fmt {} +

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

# --- nix (cross-build for linux) ---

# Build pane-comp for Linux (via nix remote builder)
build-comp:
    nix build .#packages.{{linux_system}}.pane-comp --print-build-logs

# Build any crate for Linux (via nix, --impure for ad-hoc builds)
build-linux crate:
    nix build --impure --expr ' \
      let pkgs = (builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.aarch64-linux; \
      in pkgs.rustPlatform.buildRustPackage { \
        pname = "{{crate}}"; version = "0.1.0"; src = ./.; \
        cargoLock.lockFile = ./Cargo.lock; \
        cargoBuildFlags = [ "-p" "{{crate}}" ]; \
        cargoTestFlags = [ "-p" "{{crate}}" ]; \
      }' --print-build-logs -o result-{{crate}}

# --- vm ---

# Build the test VM image (separate from ./result to avoid conflict with cross-builds)
vm-build:
    nix build .#nixosConfigurations.pane-test-vm.config.system.build.vm -o result-vm

# Boot a fresh VM (build image + clean state + boot)
vm-fresh:
    just vm-build
    ssh-keygen -R "[localhost]:2222" 2>/dev/null || true
    rm -f nixos.qcow2
    ./nix/run-vm-macos.sh

# SSH into the test VM
vm-ssh:
    ssh -p 2222 pane@localhost

# Push a nix build result to the running VM's nix store
vm-push path="./result":
    NIX_SSHOPTS="-p 2222" nix copy --no-check-sigs --to ssh://pane@localhost {{path}}

# --- dev iteration (the fast path) ---

# Build compositor + push to VM + restart it
# Note: $(readlink ./result) resolves the nix store path on the HOST.
# This works because the VM mounts the host /nix/store read-only via 9p.
dev-comp:
    just build-comp
    just vm-push ./result
    ssh -p 2222 pane@localhost "pkill pane-comp 2>/dev/null; sleep 0.5; \
      WAYLAND_DISPLAY=wayland-1 XDG_RUNTIME_DIR=/run/user/1000 \
      $(readlink ./result)/bin/pane-comp &"
    @echo "compositor restarted"

# Build pane-hello + push to VM
dev-hello:
    just build-linux pane-hello
    scp -P 2222 result-pane-hello/bin/pane-hello pane@localhost:/tmp/pane-hello
    @echo "pane-hello pushed — run with: just vm-run-hello"

# Run pane-hello in the VM
vm-run-hello:
    ssh -p 2222 pane@localhost "XDG_RUNTIME_DIR=/run/user/1000 /tmp/pane-hello"

# Full dev cycle: build both + push + restart compositor + run hello
dev-cycle:
    just dev-comp
    just dev-hello
    @echo "sleeping 2s for compositor startup..."
    sleep 2
    just vm-run-hello

# --- clean ---

# Clean cargo artifacts
clean:
    cargo clean

# Clean everything (cargo + nix + vm)
clean-all:
    cargo clean
    rm -f nixos.qcow2 result result-disk result-vm result-hello
    rm -f result-pane-*

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
