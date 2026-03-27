# Rust project configuration via rust-flake (crane)
{ inputs, ... }:
{
  imports = [
    inputs.rust-flake.flakeModules.default
    inputs.rust-flake.flakeModules.nixpkgs
  ];

  perSystem = { ... }: {
    # rust-flake auto-discovers crates from Cargo.toml workspace
    # and reads the toolchain from rust-toolchain.toml
  };
}
