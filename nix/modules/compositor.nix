# pane-comp: Linux-only compositor package
# Built via the linux-builder from macOS:
#   nix build .#packages.aarch64-linux.pane-comp
{ inputs, ... }:
{
  perSystem = { pkgs, lib, system, self', ... }:
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
        pane-comp = pkgs.rustPlatform.buildRustPackage {
          pname = "pane-comp";
          version = "0.1.0";
          src = lib.fileset.toSource {
            root = ../../.;
            fileset = lib.fileset.unions [
              ../../Cargo.toml
              ../../Cargo.lock
              ../../crates
            ];
          };

          cargoLock.lockFile = ../../Cargo.lock;

          # Add pane-comp to the workspace for the Linux build
          postPatch = ''
            substituteInPlace Cargo.toml \
              --replace-fail \
              'members = ["crates/pane-proto", "crates/pane-session", "crates/pane-notify", "crates/pane-app"]' \
              'members = ["crates/pane-proto", "crates/pane-session", "crates/pane-notify", "crates/pane-app", "crates/pane-comp"]'
          '';

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = linuxBuildInputs;

          cargoBuildFlags = [ "-p" "pane-comp" ];
          cargoTestFlags = [ "-p" "pane-comp" ];
        };
      };
    };
}
