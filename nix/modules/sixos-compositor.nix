# pane.sixosModules.compositor — adds pane-comp on Pane Linux
#
# Depends on sixos-core. Adds the graphical compositor as an s6-rc
# service with Wayland deps, seatd, libinput, and elogind dependency.
#
# Specification skeleton — requires sixos flake input.
{ self, ... }:
{
  flake.sixosModules.compositor = { config, lib, pkgs, ... }:
    {
      # imports = [ sixos-core ];

      # TODO: s6-rc service definition for pane-comp
      # /etc/s6-rc/source/pane-comp/type    → "longrun"
      # /etc/s6-rc/source/pane-comp/run     → execlineb script
      # /etc/s6-rc/source/pane-comp/notification-fd → "3"
      # /etc/s6-rc/source/pane-comp/dependencies.d/pane-roster
      # /etc/s6-rc/source/pane-comp/dependencies.d/elogind
      #
      # Wayland environment: WAYLAND_DISPLAY, XDG_RUNTIME_DIR
      # seatd for seat management, libinput for input devices
    };
}
