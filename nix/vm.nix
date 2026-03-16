# NixOS VM for testing and developing pane-comp
#
# Build:
#   just vm-build
#
# Run:
#   just vm-fresh   (or ./nix/run-vm-macos.sh)
#
# Fast iteration:
#   just dev-build  # cargo build inside VM via SSH
#   just dev-run    # run freshly built binary in VM
#
# SSH:
#   ssh -p 2222 pane@localhost  (password: pane)
{ pane-comp }:
{ pkgs, lib, ... }:

let
  # Build dependencies for pane-comp (needed for cargo build inside VM)
  buildDeps = with pkgs; [
    wayland
    wayland.dev
    wayland-protocols
    wayland-scanner
    libinput.dev
    libxkbcommon.dev
    libdrm.dev
    mesa
    libgbm
    libglvnd.dev
    udev.dev
    pixman
    fontconfig.dev
    freetype.dev
    seatd.dev
    pkg-config
  ];
in
{
  system.stateVersion = "25.05";

  # Disable binfmt (builder has Rosetta configured, breaks in QEMU sandbox)
  boot.binfmt.registrations = lib.mkForce {};
  nix.settings.extra-platforms = lib.mkForce [];
  environment.etc."binfmt.d/nixos.conf".enable = lib.mkForce false;

  # Mount host project directory for fast iteration
  fileSystems."/mnt/pane" = {
    device = "project";
    fsType = "9p";
    options = [ "trans=virtio" "version=9p2000.L" "msize=104857600" ];
  };

  hardware.graphics.enable = true;
  services.seatd.enable = true;

  # SSH for dev access
  services.openssh = {
    enable = true;
    settings.PasswordAuthentication = true;
  };

  # Auto-login into cage + foot
  services.greetd = {
    enable = true;
    settings = {
      default_session = {
        command = "${pkgs.cage}/bin/cage -- ${pkgs.foot}/bin/foot";
        user = "pane";
      };
    };
  };

  # Runtime library path for wayland/GL (winit dlopen)
  environment.variables.PATH = [ "${pane-comp}/bin" ];
  environment.variables.LD_LIBRARY_PATH = lib.makeLibraryPath (with pkgs; [
    wayland
    libxkbcommon
    libglvnd
    mesa
  ]);

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
    pkgs.ncurses
    pkgs.less
    pkgs.gcc        # linker for cargo
    pkgs.rustup     # rust toolchain
    pane-comp
  ] ++ buildDeps;
}
