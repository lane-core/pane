# Development shell — combines Rust toolchain with platform-specific deps
{ inputs, ... }:
{
  perSystem = { pkgs, lib, self', system, ... }:
    let
      isLinux = pkgs.stdenv.isLinux;
      isDarwin = pkgs.stdenv.isDarwin;

      # Linux-only deps for pane-comp (Wayland compositor)
      linuxDeps = with pkgs; [
        wayland wayland-protocols wayland-scanner
        libinput libxkbcommon
        seatd
        libdrm mesa libgbm libglvnd
        udev pixman
        fontconfig freetype
      ];

      darwinDeps = with pkgs; [
        libiconv
      ];
    in
    {
      devShells.default = pkgs.mkShell {
        name = "pane-dev";

        inputsFrom = [
          self'.devShells.rust
        ] ++ lib.optionals (self' ? devShells && builtins.hasAttr "pre-commit" (self'.devShells or {})) [
          self'.devShells.pre-commit
        ];

        nativeBuildInputs = [
          pkgs.pkg-config
          pkgs.just
        ]
        ++ lib.optionals isLinux linuxDeps
        ++ lib.optionals isDarwin darwinDeps;

        PKG_CONFIG_PATH = lib.optionalString isLinux
          (lib.makeSearchPath "lib/pkgconfig" linuxDeps);

        LD_LIBRARY_PATH = lib.optionalString isLinux
          (lib.makeLibraryPath [ pkgs.libglvnd pkgs.mesa pkgs.wayland ]);

        shellHook = ''
          echo "pane dev shell (${system})"
          ${if isLinux then ''
            echo "  cargo build  — builds all crates"
            echo "  cargo test   — runs all tests"
            echo "  just         — see all recipes"
          '' else ''
            echo "  cargo build  — builds pane-proto, pane-session, pane-notify, pane-app"
            echo "  cargo test   — runs all tests"
            echo "  just         — see all recipes"
          ''}
        '';
      };
    };
}
