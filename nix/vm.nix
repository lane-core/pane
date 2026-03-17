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

  # Use Lix instead of CppNix — the host runs Lix, and Lix-produced .drv
  # files crash CppNix's daemon on the builder, corrupting its SQLite cache.
  nix.package = pkgs.lix;

  # Mount host project directory for fast iteration
  systemd.tmpfiles.rules = [ "d /home/pane/pane 0755 pane users -" ];
  fileSystems."/home/pane/pane" = {
    device = "project";
    fsType = "9p";
    options = [ "trans=virtio" "version=9p2000.L" "msize=104857600" "nofail" "x-systemd.after=systemd-tmpfiles-setup.service" ];
  };

  # Passwordless sudo for dev
  security.sudo.wheelNeedsPassword = false;

  hardware.graphics.enable = true;
  services.seatd.enable = true;

  # SSH for dev access
  services.openssh = {
    enable = true;
    settings.PasswordAuthentication = true;
  };

  # Auto-login into sway
  services.greetd = {
    enable = true;
    settings = {
      default_session = {
        command = "${pkgs.sway}/bin/sway";
        user = "pane";
      };
    };
  };

  # Sway config: vertical split with foot terminal
  environment.etc."sway/config".text = ''
    # Default to vertical split (foot left, pane-comp right)
    default_orientation horizontal

    # No window decorations (pane has its own chrome)
    default_border pixel 2
    gaps inner 8
    gaps outer 8

    # Launch foot on startup
    exec ${pkgs.foot}/bin/foot

    # Keybindings
    set $mod Mod4
    bindsym $mod+Return exec ${pkgs.foot}/bin/foot
    bindsym $mod+Shift+q kill
    bindsym $mod+d exec pane-comp
    bindsym $mod+h splith
    bindsym $mod+v splitv
    bindsym $mod+Left focus left
    bindsym $mod+Right focus right
    bindsym $mod+Up focus up
    bindsym $mod+Down focus down
    bindsym $mod+Shift+Left move left
    bindsym $mod+Shift+Right move right

    # Use virtio-gpu output
    output * bg #6644aa solid_color
  '';

  # Runtime environment
  environment.variables = {
    PATH = [ "${pane-comp}/bin" ];
    TERMINFO_DIRS = "${pkgs.ncurses}/share/terminfo";
    LD_LIBRARY_PATH = lib.makeLibraryPath (with pkgs; [
      wayland
      libxkbcommon
      libglvnd
      mesa
    ]);
  };

  users.users.pane = {
    isNormalUser = true;
    password = "pane";
    extraGroups = [ "video" "render" "input" "wheel" ];
    openssh.authorizedKeys.keys = [
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIKVSbEBgxXTt3tVDpg98EKR+sTCmacdATBiIQXDZvwi3"
    ];
  };

  fonts.packages = with pkgs; [
    monoid        # monospace (cell grids, tag line text regions)
    inter         # proportional (widget labels, UI chrome)
    noto-fonts    # fallback/unicode coverage
  ];

  environment.systemPackages = [
    pkgs.foot
    pkgs.sway
    pkgs.htop
    pkgs.ncurses
    pkgs.less
    pkgs.gcc        # linker for cargo
    pkgs.rustup     # rust toolchain
    pane-comp
  ] ++ buildDeps;
}
