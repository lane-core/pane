#!/usr/bin/env bash
# Run the pane test VM on macOS using QEMU with HVF acceleration
#
# Usage:
#   nix build .#nixosConfigurations.pane-test-vm.config.system.build.vm
#   nix build .#packages.aarch64-linux.vm-disk -o result-disk
#   ./nix/run-vm-macos.sh
#
# SSH: ssh -p 2222 pane@localhost  (password: pane)
# Quit: close the QEMU window

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
RESULT="$PROJECT_DIR/result"
DISK_RESULT="$PROJECT_DIR/result-disk"

if [ ! -d "$RESULT" ]; then
    echo "Build the VM first:"
    echo "  nix build .#nixosConfigurations.pane-test-vm.config.system.build.vm"
    exit 1
fi

if ! command -v qemu-system-aarch64 &>/dev/null; then
    echo "qemu-system-aarch64 not found"
    exit 1
fi

# Parse paths from the generated run-nixos-vm script
SYSTEM=$(sed -n 's/.*init=\(\/nix\/store\/[^ ]*\)\/init.*/\1/p' "$RESULT/bin/run-nixos-vm")
KERNEL="$SYSTEM/kernel"
INITRD=$(sed -n 's/.*-initrd \(\/nix\/store\/[^ ]*\).*/\1/p' "$RESULT/bin/run-nixos-vm")
REGINFO=$(sed -n 's/.*regInfo=\(\/nix\/store\/[^ ]*\).*/\1/p' "$RESULT/bin/run-nixos-vm")
KERNEL_PARAMS="$(cat "$SYSTEM/kernel-params") init=$SYSTEM/init regInfo=$REGINFO console=ttyAMA0,115200n8 console=tty0"

# Get or create disk image
DISK_IMAGE="$PROJECT_DIR/nixos.qcow2"
if [ ! -e "$DISK_IMAGE" ]; then
    if [ ! -d "$DISK_RESULT" ]; then
        echo "Building disk image via linux-builder..."
        nix build "$PROJECT_DIR#packages.aarch64-linux.vm-disk" -o "$DISK_RESULT"
    fi
    echo "Copying disk image (so it's writable)..."
    cp "$DISK_RESULT/nixos.qcow2" "$DISK_IMAGE"
    chmod 644 "$DISK_IMAGE"
fi

# Temp dirs for VM exchange
TMPDIR=$(mktemp -d)
mkdir -p "$TMPDIR/xchg"
trap "rm -rf $TMPDIR" EXIT

echo "pane test VM"
echo "  SSH:  ssh -p 2222 pane@localhost (password: pane)"
echo "  Quit: close window"

exec qemu-system-aarch64 \
    -M virt \
    -accel hvf \
    -cpu host \
    -smp 4 \
    -m 4G \
    -device virtio-gpu-pci,xres=1280,yres=720 \
    -display cocoa \
    -device qemu-xhci \
    -device usb-kbd \
    -device usb-tablet \
    -nic user,model=virtio-net-pci,hostfwd=tcp::2222-:22 \
    -device virtio-rng-pci \
    -virtfs local,path=/nix/store,security_model=none,mount_tag=nix-store,readonly=on \
    -virtfs local,path="$TMPDIR/xchg",security_model=none,mount_tag=shared \
    -virtfs local,path="$TMPDIR/xchg",security_model=none,mount_tag=xchg \
    -virtfs local,path="$PROJECT_DIR",security_model=mapped-xattr,mount_tag=project \
    -drive cache=writeback,file="$DISK_IMAGE",id=drive1,if=none,index=1,werror=report \
    -device virtio-blk-pci,bootindex=1,drive=drive1,serial=root \
    -kernel "$KERNEL" \
    -initrd "$INITRD" \
    -append "$KERNEL_PARAMS" \
    "$@"
