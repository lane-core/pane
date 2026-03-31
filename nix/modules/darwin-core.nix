# pane.darwinModules.core — headless pane on macOS (launchd backend)
#
# Consumes pane.lib.services, generates launchd plist.
# Requires nix-darwin.
{ self, ... }:
{
  flake.darwinModules.core = { config, lib, pkgs, ... }:
    let
      cfg = config.pane.services.headless;
      pane-headless = self.packages.${pkgs.system}.pane-headless;
    in
    {
      imports = [ ../../nix/lib/services.nix ];

      config = lib.mkIf cfg.enable {
        environment.systemPackages = [ pane-headless ];

        launchd.user.agents.pane-headless = {
          serviceConfig = {
            Label = "com.pane.headless";
            ProgramArguments = [
              "${pane-headless}/bin/pane-headless"
              "--socket" cfg.unixSocket
              "--cols" (toString cfg.cols)
              "--rows" (toString cfg.rows)
              "--log" cfg.logLevel
            ] ++ lib.optionals (cfg.tcpListen != null) [
              "--tcp" cfg.tcpListen
            ];

            RunAtLoad = true;
            KeepAlive = true;

            StandardOutPath = "/tmp/pane-headless.log";
            StandardErrorPath = "/tmp/pane-headless.log";
          };
        };
      };
    };
}
