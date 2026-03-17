# pane desktop environment

# --- build ---

# Build a target (default: pane-proto)
build target="proto":
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{target}}" in
        proto) cargo build ;;
        comp)  nix build .#packages.aarch64-linux.pane-comp --print-build-logs ;;
        all)   just build proto && just build comp ;;
        *)     echo "usage: just build <proto|comp|all>" ;;
    esac

# --- test ---

# Test a target (default: pane-proto)
test target="proto":
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{target}}" in
        proto) cargo test ;;
        linux) nix build .#packages.aarch64-linux.pane-proto --print-build-logs ;;
        *)     echo "usage: just test <proto|linux>" ;;
    esac

# --- clean ---

# Clean artifacts
clean target="cargo":
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{target}}" in
        cargo) cargo clean ;;
        disk)  rm -f nixos.qcow2 result-disk ;;
        nix)   rm -f result result-disk ;;
        all)   just clean cargo && just clean disk && just clean nix ;;
        *)     echo "usage: just clean <cargo|disk|nix|all>" ;;
    esac

# --- run ---

# Run pane-comp in the VM
run:
    ssh -p 2222 pane@localhost \
        "WAYLAND_DISPLAY=wayland-0 XDG_RUNTIME_DIR=/run/user/1000 ~/pane/target/debug/pane-comp"

# --- dev ---

# Dev iteration (default: build + run)
dev verb="build-run":
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{verb}}" in
        setup)     ssh -p 2222 pane@localhost "rustup default stable" ;;
        build)     ssh -p 2222 pane@localhost "cd ~/pane && cargo build -p pane-comp" ;;
        run)       just run ;;
        build-run) ssh -p 2222 pane@localhost "cd ~/pane && cargo build -p pane-comp && WAYLAND_DISPLAY=wayland-0 XDG_RUNTIME_DIR=/run/user/1000 ~/pane/target/debug/pane-comp" ;;
        shell)     ssh -t -p 2222 pane@localhost "cd ~/pane && exec bash" ;;
        *)         echo "usage: just dev <setup|build|run|build-run|shell>" ;;
    esac

# --- vm ---

# VM lifecycle
vm verb:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{verb}}" in
        build)        nix build .#nixosConfigurations.pane-test-vm.config.system.build.vm ;;
        boot)         just vm reset-ssh && rm -f nixos.qcow2 && ./nix/run-vm-macos.sh ;;
        fresh)        just vm build && just vm boot ;;
        run)          ./nix/run-vm-macos.sh ;;
        ssh)          ssh -p 2222 pane@localhost ;;
        disk)         nix build .#packages.aarch64-linux.vm-disk -o result-disk ;;
        refresh-disk) just clean disk && just vm disk ;;
        reset-ssh)    ssh-keygen -R "[localhost]:2222" 2>/dev/null || true ;;
        clean)        rm -f nixos.qcow2 result-disk result ;;
        *)            echo "usage: just vm <build|boot|fresh|run|ssh|disk|refresh-disk|reset-ssh|clean>" ;;
    esac

# --- spec ---

# OpenSpec workflow
spec verb:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{verb}}" in
        list)     openspec list ;;
        validate) openspec validate --all ;;
        *)        echo "usage: just spec <list|validate>" ;;
    esac

# --- standalone ---

# Regenerate Cargo.lock with all deps
lock:
    #!/usr/bin/env python3
    import subprocess, os
    with open("Cargo.toml") as f: t = f.read()
    with open("Cargo.toml", "w") as f: f.write(t.replace('members = ["crates/pane-proto"]', 'members = ["crates/pane-proto", "crates/pane-comp"]'))
    subprocess.run([os.path.expanduser("~/.cargo/bin/cargo"), "generate-lockfile"], check=True)
    with open("Cargo.toml") as f: t = f.read()
    with open("Cargo.toml", "w") as f: f.write(t.replace('members = ["crates/pane-proto", "crates/pane-comp"]', 'members = ["crates/pane-proto"]'))
    print("Cargo.lock regenerated with all deps")

# SSH into VM
ssh:
    ssh -p 2222 pane@localhost

# Push to origin
push:
    git push
