# pane desktop environment

# --- basics ---

# Build pane-proto (works on any platform)
build:
    cargo build

# Run pane-proto tests
test:
    cargo test

# Clean cargo build artifacts
clean:
    cargo clean

# Regenerate Cargo.lock with all deps
cargo-lock:
    #!/usr/bin/env python3
    import subprocess, os
    with open("Cargo.toml") as f: t = f.read()
    with open("Cargo.toml", "w") as f: f.write(t.replace('members = ["crates/pane-proto"]', 'members = ["crates/pane-proto", "crates/pane-comp"]'))
    subprocess.run([os.path.expanduser("~/.cargo/bin/cargo"), "generate-lockfile"], check=True)
    with open("Cargo.toml") as f: t = f.read()
    with open("Cargo.toml", "w") as f: f.write(t.replace('members = ["crates/pane-proto", "crates/pane-comp"]', 'members = ["crates/pane-proto"]'))
    print("Cargo.lock regenerated with all deps")

# --- just vm <verb> ---

# VM lifecycle management
vm verb:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{verb}}" in
        build)        nix build .#nixosConfigurations.pane-test-vm.config.system.build.vm ;;
        boot)         just vm reset-ssh && rm -f nixos.qcow2 && ./nix/run-vm-macos.sh ;;
        fresh)        just vm build && just vm boot ;;
        disk)         nix build .#packages.aarch64-linux.vm-disk -o result-disk ;;
        run)          ./nix/run-vm-macos.sh ;;
        ssh)          ssh -p 2222 pane@localhost ;;
        reset-ssh)    ssh-keygen -R "[localhost]:2222" 2>/dev/null || true ;;
        refresh-disk) rm -f nixos.qcow2 result-disk && just vm disk ;;
        clean)        rm -f nixos.qcow2 result-disk result ;;
        *)            echo "usage: just vm <build|boot|fresh|disk|run|ssh|reset-ssh|refresh-disk|clean>" ;;
    esac

# --- just dev <verb> ---

# Fast iteration (VM must be running)
dev verb="build-run":
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{verb}}" in
        setup)     ssh -p 2222 pane@localhost "rustup default stable" ;;
        build)     ssh -p 2222 pane@localhost "cd /mnt/pane && cargo build -p pane-comp" ;;
        run)       ssh -p 2222 pane@localhost "WAYLAND_DISPLAY=wayland-0 XDG_RUNTIME_DIR=/run/user/1000 /mnt/pane/target/debug/pane-comp" ;;
        build-run) ssh -p 2222 pane@localhost "cd /mnt/pane && cargo build -p pane-comp && WAYLAND_DISPLAY=wayland-0 XDG_RUNTIME_DIR=/run/user/1000 /mnt/pane/target/debug/pane-comp" ;;
        *)         echo "usage: just dev <setup|build|run|build-run>" ;;
    esac

# --- just spec <verb> ---

# OpenSpec workflow
spec verb:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{verb}}" in
        list)     openspec list ;;
        validate) openspec validate --all ;;
        *)        echo "usage: just spec <list|validate>" ;;
    esac

# --- just nix <verb> ---

# Nix builds (linux-builder)
nix verb:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{verb}}" in
        comp)   nix build .#packages.aarch64-linux.pane-comp --print-build-logs ;;
        proto)  nix build .#packages.aarch64-linux.pane-proto --print-build-logs ;;
        all)    just nix comp && just nix proto ;;
        clean)  rm -f result result-disk ;;
        *)      echo "usage: just nix <comp|proto|all|clean>" ;;
    esac
