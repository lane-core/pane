# Builds an ext4 disk image labeled 'nixos' for the test VM.
# Runs on the linux-builder via nix build.
{ pkgs }:

pkgs.runCommand "pane-vm-disk" {
  nativeBuildInputs = [ pkgs.e2fsprogs pkgs.qemu_kvm ];
} ''
  truncate -s 1G raw.img
  mkfs.ext4 -q -L nixos raw.img
  mkdir -p $out
  qemu-img convert -f raw -O qcow2 raw.img $out/nixos.qcow2
''
