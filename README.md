pane
====

> *What are we to do with these spring days that are now fast coming on?
> Early this morning the sky was grey, but if you go to the window now
> you are surprised and lean your cheek against the latch of the casement.*
>
> — Franz Kafka, "Absent-minded Window-gazing"

pane is a desktop environment for Linux.

pane is about expressive ways to compose ideas. the system is
built from a small, principled core — typed protocols, filesystem
interfaces, composable servers — from which the entire experience
is derived by first principles. the core can be understood,
modified, and extended. guard rails for new users. a ladder for
power users.

the design bet: if the protocol is right — if each component's
operational semantics are local and sound — then coordination
is emergent and the system sustains stability in the face of
complexity. BeOS proved this: pervasive multithreading forced
self-contained components, which produced a system that could
play 30 videos on a Pentium 3 and still respond instantly to
input. not because it was simple, but because each piece
reasoned locally while the whole composed globally. the protocol
was the operating principle, not a global coordinator.

pane extends this with Plan 9's text-as-interface and filesystem-
as-API, modern tiling, and a Frutiger Aero aesthetic — the polished
evolution of 90s desktop design. what if Be had continued into the
2000s.

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

communication
-------------

pane-route is the communication infrastructure. data flows from
sources to handlers based on content. clicking text, receiving a
dbus signal, watching a file change, handling a network request —
these are all instances of the same thing: content arrives, matches
a rule, dispatches to a handler.

foreign protocols are integrated via bridges — small daemons that
translate between a foreign protocol and pane's native message
model. pane-dbus translates dbus signals and method calls. pane-9p
serves pane state to plan 9 systems. each bridge is a plugin. the
pane side is always the same typed interface.

start small. the first bridge is text routing from a mouse click.
the protocol itself is the experiment platform for discovering
what other bridges make sense.

servers
-------

small processes, each doing one thing. integrated behavior emerges
from their sequential composition.

    pane-comp       compositor, layout, rendering
    pane-route      communication infrastructure, content routing
    pane-roster     app lifecycle, service registry, session state
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
    pane-shell      spec    textual interface layer, terminal bridge
    pane-route      spec    communication infrastructure, protocol bridges
    pane-roster     spec    app lifecycle, service registry

license
-------

    protocol and client kits    MIT
    servers                     AGPL-3.0-only

    see LICENSE-MIT and LICENSE-AGPL3
