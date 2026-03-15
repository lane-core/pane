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

  # QEMU options are set by nix/run-vm-macos.sh, not here.
  # This is a plain NixOS config, not a vmVariant.

  hardware.graphics.enable = true;

  # Seat management — needed for Wayland compositors to access input/display
  services.seatd.enable = true;

  # SSH for debugging
  services.openssh = {
    enable = true;
    settings.PasswordAuthentication = true;
  };

  # Auto-login, launch cage with foot terminal.
  # Run pane-comp from the terminal: it connects as a winit client.
  # (pane-comp winit backend needs a running Wayland session to connect to)
  services.greetd = {
    enable = true;
    settings = {
      default_session = {
        command = "${pkgs.cage}/bin/cage -- ${pkgs.foot}/bin/foot";
        user = "pane";
      };
    };
  };

  # Put pane-comp on PATH and ensure wayland libs are findable at runtime
  # (winit loads libwayland-client.so via dlopen)
  environment.variables.PATH = [ "${pane-comp}/bin" ];
  environment.variables.LD_LIBRARY_PATH = with pkgs; lib.makeLibraryPath [
    wayland
    libxkbcommon
    libglvnd
    mesa
  ];

  users.users.pane = {
    isNormalUser = true;
    password = "pane";
    extraGroups = [ "video" "render" "input" ];
  };

  fonts.packages = with pkgs; [
    iosevka
    noto-fonts
  ];

  environment.systemPackages = [
    pkgs.foot
    pkgs.cage
    pkgs.htop
    pane-comp
  ];
}
