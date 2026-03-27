# Additional packages beyond what rust-flake provides
# (VM config, disk images, darwin modules)
{ inputs, self, ... }:
{
  perSystem = { pkgs, lib, system, ... }:
    let
      isLinux = pkgs.stdenv.isLinux;
    in
    {
      packages = lib.optionalAttrs isLinux {
        vm-disk = import ../../nix/make-disk-image.nix { inherit pkgs; };
      };
    };

  flake = {
    # nix-darwin module for linux-builder
    darwinModules.linux-builder = import ../darwin-linux-builder.nix;

    # NixOS VM for testing pane-comp visually
    # Defined lazily — only evaluates when accessed, not during flake check
  };
}
