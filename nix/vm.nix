# NixOS VM for testing pane-comp
#
# Build:
#   nix build .#nixosConfigurations.pane-test-vm.config.system.build.vm
#
# Run:
#   nix shell nixpkgs#qemu -c ./result/bin/run-nixos-vm
#
# SSH in (after boot):
#   ssh -p 2222 pane@localhost  (password: pane)
#
# The VM boots NixOS, auto-logs in via greetd, and launches pane-comp
# inside cage (single-window Wayland compositor wrapper).
#
# Display: virtio-gpu-gl-pci for virgl GLES passthrough (falls back to
# llvmpipe if virgl unavailable on the host).
{ pane-comp }:
{ pkgs, ... }:

{
  system.stateVersion = "24.11";

  # VM configuration
  virtualisation = {
    qemu.options = [
      # GPU: virgl for OpenGL ES passthrough
      "-device" "virtio-gpu-gl-pci,xres=1280,yres=720"
      # Display: cocoa with GL context for virgl
      "-display" "cocoa,gl=on"
      # Network: SSH port forward
      "-nic" "user,hostfwd=tcp::2222-:22"
    ];
    memorySize = 2048;
    cores = 2;
  };

  hardware.graphics.enable = true;

  # SSH for debugging
  services.openssh = {
    enable = true;
    settings.PasswordAuthentication = true;
  };

  # Auto-login, launch pane-comp via cage
  services.greetd = {
    enable = true;
    settings = {
      default_session = {
        command = "${pkgs.cage}/bin/cage -- ${pane-comp}/bin/pane-comp";
        user = "pane";
      };
    };
  };

  users.users.pane = {
    isNormalUser = true;
    password = "pane";
    extraGroups = [ "video" "render" "input" ];
  };

  fonts.packages = with pkgs; [
    iosevka
    noto-fonts
  ];

  environment.systemPackages = with pkgs; [
    foot
    cage
    htop
  ];
}
