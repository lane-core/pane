First visual output achieved 2026-03-16. Yellow tag bar, light/dark beveled borders, dark blue body rendered via smithay's GlesFrame::draw_solid in a QEMU VM (NixOS + cage + llvmpipe).

Key learnings:
- smithay draw_solid damage parameter must be full-window rect, not per-rect (clips to intersection)
- GL coordinates: (0,0) = bottom-left, Y increases upward
- Static mut for frame damage is a temporary hack — needs proper threading through render calls
- The winit backend is a Wayland client, not a compositor — needs cage as a host compositor for testing
