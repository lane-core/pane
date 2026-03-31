# pane.sixosModules.desktop — full Pane Linux desktop environment
#
# Depends on sixos-compositor. The complete vertically-integrated
# system: compositor + greetd + fonts + default applications.
#
# This is the end state of the adoption funnel:
#   flake on any unix → headless → compositor → this
#
# Specification skeleton — requires sixos flake input.
{ self, ... }:
{
  flake.sixosModules.desktop = { config, lib, pkgs, ... }:
    {
      # imports = [ sixos-compositor ];

      # TODO: Full desktop configuration
      # - greetd for session management (auto-login or greeter)
      # - Fonts: inter (UI), monoid (monospace), noto-fonts (fallback)
      # - Default applications: pane-shell, file manager, etc.
      # - User session setup
      # - The complete Pane Linux experience
    };
}
