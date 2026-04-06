# Headless Development Unblocking

pane-headless eliminates the compositor as development bottleneck. Before headless, every subsystem required pane-comp in a VM. Now:

- pane-roster, pane-store, pane-fs, scripting, AI kit, routing — all develop against pane-headless
- pane-shell's protocol side works headless; only rendering needs compositor
- The compositor becomes the last mile: chrome, input dispatch, layout, Wayland legacy

When planning work on any subsystem, default to developing against pane-headless. Only pull in pane-comp when the feature specifically requires rendering or input.
