## 1. Remove 9p hacks

- [ ] 1.1 Remove `project` virtfs line from nix/run-vm-macos.sh
- [ ] 1.2 Remove fileSystems, tmpfiles, and sway mount hack from nix/vm.nix
- [ ] 1.3 Remove rustup/gcc/build-deps from VM systemPackages (not building inside VM anymore)
- [ ] 1.4 Verify `just vm fresh` boots cleanly without 9p errors

## 2. Deploy-based workflow

- [ ] 2.1 Update `just dev build` to run `nix build .#packages.aarch64-linux.pane-comp`
- [ ] 2.2 Add `just deploy` recipe: scp result/bin/pane-comp to VM
- [ ] 2.3 Update `just dev run` to run ~/pane-comp in VM with correct env vars
- [ ] 2.4 Update `just dev` default to: build + deploy + run
- [ ] 2.5 Remove `just dev setup` (no longer needed — no rustup in VM)
- [ ] 2.6 Update `just dev shell` to just SSH in (no /mnt/pane)

## 3. Documentation

- [ ] 3.1 Update AGENT.md with the deploy-based workflow
- [ ] 3.2 Verify full cycle: edit → just dev → see result in QEMU
