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
    # Note: this requires aarch64-linux packages which only build on Linux.
    # Access via: nix build .#nixosConfigurations.pane-test-vm.config.system.build.vm
    nixosConfigurations.pane-test-vm = inputs.nixpkgs.lib.nixosSystem {
      system = "aarch64-linux";
      modules = [
        (import ../vm.nix {})
      ];
    };
  };
}
