# pane.nixosModules.core — headless pane on NixOS (systemd backend)
#
# The adoption on-ramp: users try pane on their existing NixOS system.
# Consumes pane.lib.services, generates systemd unit files.
{ self, ... }:
{
  flake.nixosModules.core = { config, lib, pkgs, ... }:
    let
      cfg = config.pane.services.headless;
      pane-headless = self.packages.${pkgs.system}.pane-headless;
    in
    {
      imports = [ ../../nix/lib/services.nix ];

      config = lib.mkIf cfg.enable {
        environment.systemPackages = [ pane-headless ];

        # Ensure the socket directory exists
        systemd.tmpfiles.rules = [
          "d /run/pane 0755 root root -"
        ];

        systemd.services.pane-headless = {
          description = "Pane headless server";
          wantedBy = [ "multi-user.target" ];
          after = [ "network.target" ];

          serviceConfig = {
            ExecStart = let
              args = [
                "--socket" cfg.unixSocket
                "--cols" (toString cfg.cols)
                "--rows" (toString cfg.rows)
                "--log" cfg.logLevel
              ]
              ++ lib.optionals (cfg.tcpListen != null) [
                "--tcp" cfg.tcpListen
              ];
            in "${pane-headless}/bin/pane-headless ${lib.concatStringsSep " " args}";

            Restart = "on-failure";
            RestartSec = "2s";

            # Hardening
            DynamicUser = false;
            ProtectSystem = "strict";
            ProtectHome = true;
            ReadWritePaths = [ "/run/pane" ];
            NoNewPrivileges = true;
          };
        };

        # Open firewall for TCP if requested
        networking.firewall = lib.mkIf (cfg.openFirewall && cfg.tcpListen != null) {
          allowedTCPPorts =
            let
              # Extract port from "host:port" string
              parts = lib.splitString ":" cfg.tcpListen;
              port = lib.toInt (lib.last parts);
            in [ port ];
        };
      };
    };
}
