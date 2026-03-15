pane
====

> *What are we to do with these spring days that are now fast coming on?
> Early this morning the sky was grey, but if you go to the window now
> you are surprised and lean your cheek against the latch of the casement.*
>
> — Franz Kafka, "Absent-minded Window-gazing"

pane is a desktop environment for Linux.

it combines ideas from BeOS (integrated feel, server/kit decomposition,
replicant-style embedding), Plan 9 (text as interface, content routing,
filesystem as API), and modern tiling window managers (tree-based layout,
tag-based visibility).

the pane is the universal UI object. every pane has a tag line (editable
text that serves as title, command bar, and menu) and a body (cell grid
or wayland surface). there are no toolbars, no menus, no button widgets.
text is the interface. middle-click executes. right-click routes.

the compositor owns all rendering. clients send cell data; the compositor
rasterizes. this produces consistent fonts and styling everywhere and
makes terminal-derived widgets first-class citizens.

servers are small processes that each do one thing. integrated behavior
emerges from their sequential composition:

    pane-comp       compositor, layout, cell grid rendering
    pane-route      pattern-match text, route to handlers
    pane-roster     track running apps, service registry
    pane-store      index file xattrs, emit change notifications
    pane-fs         expose state as FUSE filesystem at /srv/pane/

configuration is files. plugin discovery is directories. the filesystem
is the database, the registry, and the configuration format. servers
cache state in memory and update on filesystem change notifications.
no config parsers, no SIGHUP, no restart.

requirements
------------

    Linux (latest stable kernel)
    Rust (stable)
    wayland, libinput, libxkbcommon, mesa (for pane-comp)

building
--------

    cargo build                         # pane-proto (any platform)
    cargo build -p pane-comp            # compositor (linux only)
    cargo test                          # all tests
    nix develop                         # reproducible dev shell

status
------

    pane-proto      done    wire types, protocol state machine, typed views
    pane-comp       wip     smithay skeleton, blocked on linux build
    pane-shell      next    PTY bridge, first usable terminal

license
-------

    protocol crates and client kits    MIT
    servers (compositor, router, etc)  AGPL-3.0-only

    see LICENSE-MIT and LICENSE-AGPL3
