{
  description = "pane — Wayland compositor and desktop environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;

      pkgsFor = system:
        import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

      # Rust toolchain — latest stable, consistent across platforms
      rustToolchain = pkgs: pkgs.rust-bin.stable.latest.default.override {
        extensions = [ "rust-src" "rust-analyzer" ];
        targets = [ ];
      };

      # Common dev deps (all platforms)
      commonDeps = pkgs: [
        (rustToolchain pkgs)
        pkgs.pkg-config
      ];

      # Linux-only deps for pane-comp (Wayland compositor)
      linuxDeps = pkgs: [
        # Wayland
        pkgs.wayland
        pkgs.wayland-protocols
        pkgs.wayland-scanner
        # Input
        pkgs.libinput
        pkgs.libxkbcommon
        # Session
        pkgs.seatd
        # Display
        pkgs.libdrm
        pkgs.mesa
        pkgs.libglvnd
        # System
        pkgs.udev
        pkgs.pixman
        # Fonts
        pkgs.fontconfig
        pkgs.freetype
      ];
    in
    {
      devShells = forAllSystems (system:
        let
          pkgs = pkgsFor system;
          isLinux = pkgs.stdenv.isLinux;
        in
        {
          default = pkgs.mkShell {
            name = "pane-dev";

            nativeBuildInputs = commonDeps pkgs
              ++ pkgs.lib.optionals isLinux (linuxDeps pkgs);

            # pkg-config needs to find the linux libs
            PKG_CONFIG_PATH = pkgs.lib.optionalString isLinux
              (pkgs.lib.makeSearchPath "lib/pkgconfig" (linuxDeps pkgs));

            # For smithay's OpenGL
            LD_LIBRARY_PATH = pkgs.lib.optionalString isLinux
              (pkgs.lib.makeLibraryPath [
                pkgs.libglvnd
                pkgs.mesa
                pkgs.wayland
              ]);

            shellHook = ''
              echo "pane dev shell (${system})"
              ${if isLinux then ''
                echo "  Linux: pane-comp builds available"
                echo "  cargo build  — builds all crates"
              '' else ''
                echo "  macOS: pane-proto only (pane-comp requires Linux)"
                echo "  cargo build  — builds pane-proto"
                echo "  cargo test   — runs pane-proto tests"
              ''}
            '';
          };
        }
      );

      packages = forAllSystems (system:
        let
          pkgs = pkgsFor system;
          isLinux = pkgs.stdenv.isLinux;
        in
        pkgs.lib.optionalAttrs isLinux {
          pane-comp = pkgs.rustPlatform.buildRustPackage {
            pname = "pane-comp";
            version = "0.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;

            # Override workspace members to include pane-comp on Linux
            postPatch = ''
              substituteInPlace Cargo.toml \
                --replace 'members = ["crates/pane-proto"]' 'members = ["crates/pane-proto", "crates/pane-comp"]'
            '';

            nativeBuildInputs = [
              pkgs.pkg-config
            ];

            buildInputs = linuxDeps pkgs;

            # Only build pane-comp binary
            cargoBuildFlags = [ "-p" "pane-comp" ];
            cargoTestFlags = [ "-p" "pane-proto" "-p" "pane-comp" ];
          };

          pane-proto = pkgs.rustPlatform.buildRustPackage {
            pname = "pane-proto";
            version = "0.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;

            cargoBuildFlags = [ "-p" "pane-proto" ];
            cargoTestFlags = [ "-p" "pane-proto" ];
          };
        }
      );

      # Quick check: does pane-proto build and test?
      checks = forAllSystems (system:
        let
          pkgs = pkgsFor system;
        in
        {
          pane-proto = pkgs.rustPlatform.buildRustPackage {
            pname = "pane-proto-check";
            version = "0.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [ "-p" "pane-proto" ];
            cargoTestFlags = [ "-p" "pane-proto" ];
            doCheck = true;
          };
        }
      );
    };
}
