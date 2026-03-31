# pane.sixosModules.core — headless pane on Pane Linux (s6-rc backend)
#
# Native Pane Linux service layer. Consumes pane.lib.services,
# generates s6-rc service directories compiled by s6-rc-compile
# into a binary service database.
#
# Uses s6-fdholder for pre-registered socket endpoints and
# readiness notification via fd write.
#
# This module requires sixos as a flake input. Until sixos is
# integrated, this is a specification skeleton.
{ self, ... }:
{
  flake.sixosModules.core = { config, lib, pkgs, ... }:
    let
      cfg = config.pane.services.headless;
      pane-headless = self.packages.${pkgs.system}.pane-headless;
    in
    {
      imports = [ ../../nix/lib/services.nix ];

      config = lib.mkIf cfg.enable {
        environment.systemPackages = [ pane-headless ];

        # TODO: s6-rc service definition
        # When sixos is integrated, this generates:
        #
        # /etc/s6-rc/source/pane-headless/type    → "longrun"
        # /etc/s6-rc/source/pane-headless/run     → execlineb script
        # /etc/s6-rc/source/pane-headless/notification-fd → "3"
        # /etc/s6-rc/source/pane-headless/dependencies.d/ → (none for core)
        #
        # The run script:
        #   #!/bin/execlineb -P
        #   fdmove -c 2 1
        #   s6-fdholder-retrieve /run/s6-fdholder/s <socket fd>
        #   exec pane-headless --socket-fd 3 [--tcp ...]
        #
        # s6-fdholder creates the unix socket at boot; pane-headless
        # retrieves it on startup. Zero-downtime restarts.
      };
    };
}
