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
    maxJobs = 4;
    config = {
      virtualisation.cores = 4;
      virtualisation.memorySize = 4096;
      # Rosetta disabled — we build native aarch64-linux, not x86_64 via
      # translation. Enabling Rosetta causes sandbox errors because
      # /run/rosetta is inaccessible inside the nix build sandbox.
    };
  };
}
