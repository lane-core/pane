# Target-agnostic pane service definitions.
#
# These options are consumed by platform-specific backends (systemd,
# launchd, s6-rc) to generate native service configurations. The user
# writes `pane.services.headless.enable = true` — it means the same
# thing regardless of which backend interprets it.
#
# This is the layer that makes the seed property work: settings
# transfer from NixOS to Darwin to Pane Linux because they were
# always expressed at this level.
{ lib, ... }:
let
  inherit (lib) mkEnableOption mkOption types;
in
{
  options.pane.services = {
    headless = {
      enable = mkEnableOption "pane headless server";

      unixSocket = mkOption {
        type = types.str;
        default = "/run/pane/compositor.sock";
        description = "Unix socket path for local client connections.";
      };

      tcpListen = mkOption {
        type = types.nullOr types.str;
        default = null;
        example = "0.0.0.0:7070";
        description = "TCP listen address for remote connections. Null to disable.";
      };

      enableTls = mkOption {
        type = types.bool;
        default = false;
        description = "Whether to require TLS for TCP connections.";
      };

      certFile = mkOption {
        type = types.nullOr types.path;
        default = null;
        description = "Path to TLS certificate file (PEM).";
      };

      keyFile = mkOption {
        type = types.nullOr types.path;
        default = null;
        description = "Path to TLS private key file (PEM).";
      };

      cols = mkOption {
        type = types.int;
        default = 80;
        description = "Default geometry columns for headless panes.";
      };

      rows = mkOption {
        type = types.int;
        default = 24;
        description = "Default geometry rows for headless panes.";
      };

      logLevel = mkOption {
        type = types.enum [ "error" "warn" "info" "debug" "trace" ];
        default = "info";
        description = "Log verbosity level.";
      };

      openFirewall = mkOption {
        type = types.bool;
        default = false;
        description = "Whether to open the TCP port in the firewall (NixOS only).";
      };
    };

    # Future services follow the same pattern:
    # roster = { enable = ...; federation = ...; };
    # store = { enable = ...; backend = ...; };
    # watchdog = { enable = ...; };
    # fs = { enable = ...; mountPoint = ...; };
  };
}
