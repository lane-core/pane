# Pre-commit hooks for code quality
# git-hooks.nix is imported as a non-flake source
{ inputs, ... }:
{
  imports = [
    (inputs.git-hooks + "/flake-module.nix")
  ];

  perSystem = { ... }: {
    pre-commit.settings.hooks = {
      rustfmt.enable = true;
      nixpkgs-fmt.enable = true;
    };
  };
}
