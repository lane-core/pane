# nix-darwin module: linux-builder for cross-platform pane builds
#
# Configures a QEMU-backed NixOS VM as a remote nix builder, enabling
# aarch64-linux pane-comp builds from macOS.
#
# Usage in nix-darwin config:
#   imports = [ pane.darwinModules.linux-builder ];
{ ... }:

{
  nix.linux-builder = {
    enable = true;
    config = {
      virtualisation.cores = 4;
      virtualisation.memorySize = 4096;
    };
  };
}
