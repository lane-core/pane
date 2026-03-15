# NixOS VM for testing pane-comp
#
# Build and run:
#   nix build .#nixosConfigurations.pane-test-vm.config.system.build.vm
#   QEMU_OPTS="-display cocoa" ./result/bin/run-nixos-vm
#
# The VM boots NixOS, auto-logs in, and launches pane-comp
# inside cage (a single-window Wayland compositor wrapper).
{ pane-comp }:
{ pkgs, ... }:

{
  system.stateVersion = "24.11";

  hardware.graphics.enable = true;

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
