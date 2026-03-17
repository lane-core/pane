pane
====

> *What are we to do with these spring days that are now fast coming on?
> Early this morning the sky was grey, but if you go to the window now
> you are surprised and lean your cheek against the latch of the casement.*
>
> — Franz Kafka, "Absent-minded Window-gazing"

pane is a desktop environment for Linux.

it draws from BeOS (integrated servers, kits, replicant-style
composition), Plan 9 (text as interface, content routing, filesystem
as API), and modern tiling window managers. the aesthetic is Frutiger
Aero — the polished evolution of 90s desktop design. what if Be had
continued into the 2000s.

design
------

everything is a pane. every pane has a tag line (title, command bar,
and menu in one) and a body (text, widgets, or a legacy surface).
there are no toolbars, no menus, no button widgets in the traditional
sense. text is the interface. middle-click executes. right-click routes.

the compositor renders everything. clients describe content; the
compositor draws it. one renderer, one visual language, one interaction
model. that is where the integrated feel comes from.

every interface — filesystem, tag line, protocol — presents the
abstraction level relevant to its consumer. a human user sees commands
and output. a system service sees state and capabilities. the compositor
sees cells and surfaces. a debugger sees byte streams and buffers.
the level is not fixed; it is determined by who is looking and what
they need.

the system is extended through the same interfaces it uses internally.
a routing rule is a file. a translator is a binary in a directory.
a pane mode wraps a library with domain-specific semantics. plugins
compose because they operate on the public interface surface. adding
a plugin is dropping a file. removing it is deleting the file.

servers
-------

small processes, each doing one thing. integrated behavior emerges
from their sequential composition.

    pane-comp       compositor, layout, rendering
    pane-route      pattern-match text, route to handlers
    pane-roster     app lifecycle, service registry
    pane-store      index file xattrs, emit change notifications
    pane-fs         expose state as filesystem at /srv/pane/

configuration is files. plugin discovery is directories. the
filesystem is the database, the registry, and the configuration
format. servers cache state in memory and update on change
notifications. no config parsers, no SIGHUP, no restart.

protocol
--------

typed messages over unix sockets. algebraic types (Rust enums)
wrapped in a message envelope with an open key-value attributes bag.
the typed core gives compile-time exhaustiveness. the attrs bag gives
BMessage-style extensibility. inter-server communication uses the
same envelope with typed views for field access.

protocol composition is grounded in sequent calculus: Value types
(constructed data) and Compute types (observed behavior) compose
with polarity-aware rules enforced at compile time.

fonts
-----

    Inter       proportional, ui chrome
    Monoid      monospace, cell grids and code

requirements
------------

    Linux (latest stable kernel)
    Rust (stable)
    wayland, libinput, libxkbcommon, mesa (for pane-comp)

building
--------

    just build              # pane-proto (any platform)
    just build comp         # pane-comp (linux, via nix)
    just test               # run tests
    just vm fresh           # boot NixOS test VM
    just dev                # build + run in VM

status
------

    pane-proto      done    wire types, state machine, typed views
    pane-comp       wip     compositor skeleton, renders pane chrome
    pane-shell      spec    VT bridge, semantic text interface
    pane-route      spec    content routing, pattern matching
    pane-roster     spec    app lifecycle, service registry

license
-------

    protocol and client kits    MIT
    servers                     AGPL-3.0-only

    see LICENSE-MIT and LICENSE-AGPL3
