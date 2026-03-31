{
  description = "pane — an operating environment for linux (and elsewhere)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    systems.url = "github:nix-systems/default";

    rust-flake = {
      url = "github:juspay/rust-flake";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      flake = false;
    };
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      systems = import inputs.systems;

      imports = [
        ./nix/modules/rust.nix
        ./nix/modules/devshell.nix
        ./nix/modules/pre-commit.nix
        ./nix/modules/packages.nix
        ./nix/modules/headless.nix
        ./nix/modules/compositor.nix
        # NixOS, Darwin, and sixos module exports
        ./nix/modules/nixos-core.nix
        ./nix/modules/darwin-core.nix
        ./nix/modules/sixos-core.nix
        ./nix/modules/sixos-compositor.nix
        ./nix/modules/sixos-desktop.nix
      ];
    };
}
