# pane-headless package — builds on all platforms (no Wayland/Linux deps)
{ inputs, ... }:
{
  perSystem = { pkgs, lib, system, ... }:
  {
    # mkForce: rust-flake auto-discovers pane-headless from workspace
    # members but doesn't know about our specific build config.
    # Override to match the compositor.nix pattern.
    packages.pane-headless = lib.mkForce (pkgs.rustPlatform.buildRustPackage {
      pname = "pane-headless";
      version = "0.1.0";
      src = ../../.;
      cargoLock.lockFile = ../../Cargo.lock;

      cargoBuildFlags = [ "-p" "pane-headless" ];
      cargoTestFlags = [ "-p" "pane-headless" ];
    });
  };
}
