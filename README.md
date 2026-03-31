pane
====

> *What are we to do with these spring days that are now fast coming on?
> Early this morning the sky was grey, but if you go to the window now
> you are surprised and lean your cheek against the latch of the casement.*
>
> — Franz Kafka, "Absent-minded Window-gazing"

pane is an operating environment for linux (and elsewhere).
a desktop, a compositor, a distribution — and a distributed
computing foundation that runs on any unix-like via nix.

the design recovers what made BeOS work — message-passing
discipline, per-component threading, infrastructure-first
composition — and what Plan 9 proved — protocol uniformity,
location independence, the network as a transparent extension
of the local namespace — on a modern linux base with session
types providing compile-time verification of the protocol
discipline BeOS achieved by convention.

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

the local machine has no architectural privilege. a headless
pane instance in the cloud runs the same protocol as the
desktop compositor — it's just a server without a display.
the unified namespace at /pane/ shows all panes, local and
remote, as computed views over the same indexed state.

the system is extended through the same interfaces it uses
internally. a routing rule is a file. a pane mode wraps a
library with domain-specific semantics. more generally, add
new functionality by dropping a declarative specification in
the relevant directory. removing it is deleting the file.

agents are system users, not applications. they have accounts,
home directories, .plan files. they communicate through the
same protocols and filesystem interfaces as human users —
whether they run locally or on a remote headless instance.

adoption
--------

you don't reinstall your OS to try pane. add a nix flake.

    nix flake on any unix-like
      → headless pane: server, kits, protocol, filesystem
      → configuration accumulates in nix expressions
      → upgrade to compositor, then desktop
      → the flake IS the seed of a full pane linux config

settings transfer because they were always nix expressions.
nixos, darwin, pane linux — same `pane.services.*` options,
different platform backend (systemd, launchd, s6-rc).

protocol
--------

every interaction is a session — a typed conversation verified
at compile time. the session type describes what each party sends
and receives, in what order, with what branches. deadlock freedom
is guaranteed structurally. async by default; sync only when a
response is needed.

the protocol is transport-agnostic: unix sockets for local,
tcp/tls for remote. same session types, same messages, same
guarantees. adding a network transport required zero protocol
changes — the transport trait is the hinge.

routing is a kit-level concern, not a central server. the kit
evaluates rules locally and dispatches directly — sender to
receiver, the way BeOS's BMessenger worked. no intermediary.

architecture
------------

the system has two layers: servers and kits.

servers are small processes, each doing one thing. the
compositor owns the display. the headless server speaks
the same protocol without rendering. other servers handle
lifecycle, storage, filesystem projection, and health
monitoring. configuration is files. plugin discovery is
directories. no config parsers, no SIGHUP, no restart.

kits are the programming model, not wrappers over a protocol.
they provide wire types, session-typed channels, the application
framework (application, pane, messenger, handler), composable
optics for structured state access, filesystem notification,
rendering, text, input, and media. a kit is the right abstraction
for its domain — not a lowest-common-denominator binding.

pane linux is a sixos flake (s6 + nix). sixos provides the
init/service substrate, nixpkgs provides packages, pane
provides the personality.

the crate layout under `crates/` is the source of truth for
what exists and what each component does.

status
------

what exists today:

    pane-proto       wire types, session-typed handshake, active-phase enums
    pane-session     session type primitives (Chan, Send, Recv, Branch),
                     transports (unix, tcp, tls), calloop integration
    pane-server      compositor protocol server — handshake, client routing,
                     identity validation, rejection. no rendering deps.
    pane-headless    headless server binary. full protocol, dual listeners
                     (unix + tcp), no gpu. runs on any unix-like.
    pane-app         application kit: App, Pane, Messenger, Handler,
                     MessageFilter, PaneCreateFuture, quit protocol,
                     timers, shortcuts, crash monitoring, send-reply
    pane-optic       optics: Getter/Setter/PartialGetter/PartialSetter,
                     FieldLens/FieldAffine/FieldTraversal, composition, laws
    pane-notify      filesystem change notification (inotify abstraction)
    pane-comp        wayland compositor (smithay). renders blank windows.
                     input routing and chrome rendering not yet wired.
    pane-hello       canonical first application

what's next: pane-store (attribute storage), pane-fs (computed-view
namespace), pane-roster (federation), compositor rendering + input.

building
--------

    direnv allow               # activate nix dev shell (first time)
    just build                 # build all crates
    just test                  # run all tests
    just test-crate <name>     # test a specific crate
    just lint                  # clippy
    just fmt                   # rustfmt + nixfmt
    just doc                   # generate API docs

running headless:

    cargo run -p pane-headless                     # unix socket
    cargo run -p pane-headless -- --tcp 0.0.0.0:7070  # + tcp

    # in another terminal:
    cargo run -p pane-app --example hello          # basic lifecycle
    cargo run -p pane-app --example handler        # stateful Handler
    cargo run -p pane-app --example worker         # worker thread → handler
    cargo run -p pane-app --example monitor        # crash monitoring

on methodology
---------------

pane's design is human-authored (the fruit of my lifelong
fascination with operating system design, going back to using
BeOS and Plan 9 as a young hacker), standing on the shoulders
of giants. the decision to ground the architecture in BeOS's
systems engineering and Plan 9's distributed computing model
was deliberate — these were the best ideas of a golden age of
operating system design, carried out by intuition and refined
technique by engineers whose work has aged remarkably well.

the Haiku project deserves special gratitude. their decades-long
marathon to reimplement BeOS from scratch kept the spirit of Be
alive when it would otherwise have been lost. their source code,
their documentation, their Haiku Book — these are invaluable to
any effort that seeks to carry on what Be started. the author is
a lifelong fan of the project and is deeply grateful for what
they have built and continue to build.

Plan 9's ideas were largely compatible with Be's. where Be
proved that message-passing discipline and per-component
threading produce responsive, stable desktops, Plan 9 proved
that protocol uniformity and location independence produce
systems whose capabilities exceed what any designer anticipated.
session types, optics, and related formal frameworks give us
tools to carry out with compiler verification what the best
systems designers of that era achieved by skill and convention.
better type systems, more refined theoretical guidance for hard
problems, more elegant formalizations of principles that were
previously folk wisdom — these don't replace the original
insights. they let us build on them with greater confidence.

the implementation is developed with substantial assistance from
AI coding agents. this is, in a sense, an adversarial test of
the design thesis: if sound architecture constrains implementation
toward correct outcomes, then it should do so regardless of who
— or what — is writing the code.

the least charitable reading is that this is a mean vehicle for
an ambitious project. we think the evidence points otherwise.
the synthesis required — session type theory, optics, BeOS
systems engineering, Plan 9 distributed computing, nix
packaging, Wayland protocol, s6 service management — spans
enough domains that no single developer could hold it all in
working memory simultaneously. an AI agent that can reference
the Be Newsletter archive, read Haiku source, consult Plan 9
papers, verify session type soundness, and write the code in
a single conversation may be more than mean for this particular
task. the architecture is the prompt, in the deepest sense:
well-chosen constraints lead to well-shaped implementations.

we are explicit about this because the result should be judged
on its merits, not on assumptions about how it was produced.
read the code. run the tests. the design either holds or it
doesn't.

license
-------

    protocol and client kits    BSD-3-Clause
    servers                     AGPL-3.0-only

    see LICENSE-BSD3 and LICENSE-AGPL3
