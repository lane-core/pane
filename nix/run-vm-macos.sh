#!/usr/bin/env bash
# Run the pane test VM on macOS using QEMU with HVF acceleration
#
# Usage:
#   ./nix/run-vm-macos.sh
#
# SSH: ssh -p 2222 pane@localhost  (password: pane)
# Quit: close the QEMU window

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
RESULT="$PROJECT_DIR/result"

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

# Locate UEFI firmware
EFI_CODE="$(dirname "$(which qemu-system-aarch64)")/../share/qemu/edk2-aarch64-code.fd"
if [ ! -f "$EFI_CODE" ]; then
    # Try nix store path
    EFI_CODE=$(find /nix/store -name "edk2-aarch64-code.fd" -maxdepth 3 2>/dev/null | head -1)
fi
if [ ! -f "$EFI_CODE" ]; then
    echo "UEFI firmware not found (edk2-aarch64-code.fd)"
    echo "Falling back to direct kernel boot (no UEFI)"
    EFI_CODE=""
fi

# Create disk image if needed
DISK_IMAGE="$PROJECT_DIR/nixos.qcow2"
if [ ! -e "$DISK_IMAGE" ]; then
    echo "Creating disk image..."
    qemu-img create -f qcow2 "$DISK_IMAGE" 1024M
fi

# Temp dirs for VM exchange
TMPDIR=$(mktemp -d)
mkdir -p "$TMPDIR/xchg"
trap "rm -rf $TMPDIR" EXIT

echo "pane test VM"
echo "  SSH:  ssh -p 2222 pane@localhost (password: pane)"
echo "  Quit: close window"

BIOS_ARGS=()
if [ -n "$EFI_CODE" ]; then
    BIOS_ARGS=(-bios "$EFI_CODE")
fi

exec qemu-system-aarch64 \
    -M virt \
    -accel hvf \
    -cpu host \
    -smp 4 \
    -m 4G \
    -device virtio-gpu-gl-pci,xres=1280,yres=720 \
    -display cocoa,gl=on \
    -device qemu-xhci \
    -device usb-kbd \
    -device usb-tablet \
    -nic user,model=virtio-net-pci,hostfwd=tcp::2222-:22 \
    -device virtio-rng-pci \
    -virtfs local,path=/nix/store,security_model=none,mount_tag=nix-store,readonly=on \
    -virtfs local,path="$TMPDIR/xchg",security_model=none,mount_tag=xchg \
    -drive cache=writeback,file="$DISK_IMAGE",id=drive1,if=none,index=1,werror=report \
    -device virtio-blk-pci,bootindex=1,drive=drive1,serial=root \
    -kernel "$KERNEL" \
    -initrd "$INITRD" \
    -append "$KERNEL_PARAMS" \
    "${BIOS_ARGS[@]}" \
    "$@"
