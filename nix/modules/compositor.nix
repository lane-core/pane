# pane-comp: Linux-only compositor package
# Uses rustPlatform.buildRustPackage directly (not rust-flake/crane)
# because the Wayland deps need explicit pkg-config configuration.
#
# Build from macOS via linux-builder:
#   nix build .#packages.aarch64-linux.pane-comp
{ inputs, ... }:
{
  perSystem = { pkgs, lib, system, ... }:
    let
      isLinux = pkgs.stdenv.isLinux;

      linuxBuildInputs = with pkgs; [
        wayland wayland-protocols wayland-scanner
        libinput libxkbcommon
        seatd
        libdrm mesa libgbm libglvnd
        udev pixman
        fontconfig freetype
      ];
    in
    {
      packages = lib.optionalAttrs isLinux {
        pane-comp = lib.mkForce (pkgs.rustPlatform.buildRustPackage {
          pname = "pane-comp";
          version = "0.1.0";
          src = ../../.;
          cargoLock.lockFile = ../../Cargo.lock;

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = linuxBuildInputs;

          cargoBuildFlags = [ "-p" "pane-comp" ];
          cargoTestFlags = [ "-p" "pane-comp" ];
        });
      };
    };
}
