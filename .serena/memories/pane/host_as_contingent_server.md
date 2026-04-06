# Host as Contingent Server

The local machine is one server among many. Its privilege as the user's interface is contingent (display, keyboard, low latency) not architectural. The hardware is just a server the UX runs on.

Unifies BeOS messaging discipline with Plan 9 location independence:
- BeOS: everything communicates the same way (BMessage everywhere)
- Plan 9: nothing is special because it's local (cpu, import, per-process namespaces)
- Pane: both — universal messaging + location independence

Consequences:
- Unified namespace is the default, not a feature
- Local/remote is metadata, not a type distinction
- The compositor is a server your eyes happen to be connected to
- A headless instance is the same thing without the display
- pane-fs computed views (by-sig, by-type, local, remote) are equivalent filters

Origin: Lane's deeply held design conviction predating pane. BeOS's app_server was architecturally special (couldn't treat it as just another server). Pane corrects this.
