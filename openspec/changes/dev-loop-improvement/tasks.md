## 1. VM as dev environment

- [x] 1.1 Add Rust toolchain (rustc, cargo) to the test VM's systemPackages
- [x] 1.2 Add pane-comp build dependencies (wayland-dev, libinput-dev, mesa-dev, etc.) to the VM
- [x] 1.3 Mount host project directory into VM via virtio-9p (mount tag `project`, mounted at `/mnt/pane`)
- [x] 1.4 Verify `cargo build -p pane-comp` works inside the VM with the mounted source
- [x] 1.5 Verify incremental builds (~5-10s) after a single file change

## 2. Justfile recipes

- [x] 2.1 Add `just dev-build` recipe: SSH into VM, cargo build the mounted source
- [x] 2.2 Add `just dev-run` recipe: SSH into VM, run the freshly built pane-comp with correct env vars
- [x] 2.3 Add `just dev` recipe: combined build + run
- [x] 2.4 Update AGENT.md with fast iteration workflow

## 3. Host-side tooling

- [x] 3.1 Update run-vm-macos.sh to mount the project directory via virtio-9p
- [x] 3.2 Add ssh-keygen setup for passwordless SSH to VM (or document using `pane` password)
