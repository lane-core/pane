pane
====

> *What are we to do with these spring days that are now fast coming on?
> Early this morning the sky was grey, but if you go to the window now
> you are surprised and lean your cheek against the latch of the casement.*
>
> — Franz Kafka, "Absent-minded Window-gazing"

pane is a desktop environment, compositor, and distribution for
Linux. one thing, not three.

the design recovers what made BeOS work — message-passing discipline,
per-component threading, infrastructure-first composition — on a
modern Linux base with session types providing compile-time
verification of the protocol discipline BeOS achieved by convention.

everything is a pane. every pane has a tag line (title, command
bar, and menu in one), a body (text, widgets, or a legacy
surface), and a session-typed protocol connection. panes compose
spatially and through the protocol. the system's power derives
from the uniformity of this single object.

design
------

every pane is one object with many views: a visual display
to the user, a protocol endpoint to other components, a
filesystem node at /pane/ for scripts and tools, a
semantic object for accessibility. the views are projections
of the same state, kept consistent by optics discipline.

the system is extended through the same interfaces it uses
internally. a routing rule is a file. a pane mode wraps a
library with domain-specific semantics. more generally, add
new functionality by dropping a declarative specification in
the relevant directory. removing it is deleting the file.

agents are system users, not applications. they have accounts,
home directories, .plan files. they communicate through the
same protocols and filesystem interfaces as human users.

protocol
--------

every interaction is a session — a typed conversation verified
at compile time. the session type describes what each party sends
and receives, in what order, with what branches. deadlock freedom
is guaranteed structurally. async by default; sync only when a
response is needed.

routing is a kit-level concern, not a central server. the kit
evaluates rules locally and dispatches directly — sender to
receiver, the way BeOS's BMessenger worked. no intermediary.

servers
-------

small processes, each doing one thing.

    pane-comp       compositor, layout, chrome
    pane-roster     app lifecycle, service registry
    pane-store      attribute indexing, change notifications
    pane-fs         FUSE at /pane/
    pane-watchdog   heartbeat monitor, escalation

configuration is files. plugin discovery is directories.
no config parsers, no SIGHUP, no restart.

kits
----

kits are the programming model, not wrappers over a protocol.

    pane-proto      wire types, protocol enums, session definitions
    pane-session    session-typed channels (Chan<S, Transport>)
    pane-app        application kit: App, Pane, Messenger, Handler
    pane-notify     filesystem notification (fanotify/inotify)
    pane-ui         text rendering, widgets, styling, layout
    pane-text       text buffers, structural regular expressions
    pane-input      generalized keybinding grammar
    pane-media      PipeWire abstraction
    pane-ai         agent infrastructure

stack
-----

    Rust            core system layer
    smithay         Wayland compositor
    s6              init and service supervision
    Nix             build system and package management
    btrfs           target filesystem
    Landlock        agent sandboxing
    PipeWire        audio and media
    Vello           widget rendering (wgpu)

fonts
-----

    Inter           proportional, ui chrome
    Monoid          monospace, text content and code

building
--------

    direnv allow            # activate nix dev shell (first time)
    just build              # build all crates
    just test               # run all tests (120+)
    just test-crate pane-app   # test a specific crate
    just lint               # clippy
    just fmt                # rustfmt + nixfmt
    just doc                # generate API docs

license
-------

    protocol and client kits    MIT
    servers                     AGPL-3.0-only

    see LICENSE-MIT and LICENSE-AGPL3
