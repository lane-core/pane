# pane

> _What are we to do with these spring days that are now fast coming on?
> Early this morning the sky was grey, but if you go to the window now
> you are surprised and lean your cheek against the latch of the casement._
>
> — Franz Kafka, "Absent-minded Window-gazing"

pane is an operating environment for Linux (and darwin hosts), and a
distributed computing foundation that runs on any unix-like supporting nix.
Used in all its faculties, pane will extend to a desktop environment and Linux
distribution, though for now the focus is on the core architecture which can be
run across multiple hosts without installing a new OS.

The design recovers what made BeOS work, the message-passing discipline,
per-component threading, interfaces emerging out of composing native
infrastructure (in our opinion, a logical extension of the spirit of unix
design principles), and also what Plan 9 proved: that protocol uniformity and
conceiving of networked applications as a transparent extension of the local
namespace leads to compelling user experiences emergent from combining system
infrastructure in ways the original developers need not anticipate. Our
ambition is to achieve this on a contemporary Linux base, which has come a
long way in catching up to the features that made these iconic operating system
design patterns possible.

There is not much that is conceptually new in terms of the specific design
principles guiding pane; that's by intention. The systems we are drawing upon
(remixing, I might put it) were crafted by the best and most prescient systems
engineers of the era, the ex-Apple developers who bet that media applications
would define the future of personal computing, and the Bell Labs researchers
who proved that a distributed system could be built on the same principles as a
local one. The design is new in how it synthesizes these ideas with modern
tools and techniques, in particular work in type systems seen from experimental
programming language design\* that formalize the principles of good protocol
design (session types), principled treatment of state access (lenses), that
these talented developers were able to achieve by intuition, skill, and
practical experience alone. We use type theory to validate the design
principles they already practically verified, which also allows us to enforce
our adherence to such core design principles in the course of pane's
development.

There will be some interesting ideas we bring to the table as well, particularly
in offering a new word on the conversation regarding operating design, distributed
computing systems, which we hope will lead to novel techniques and usage patterns
such as providing novel interpretations of the role of AI in software development
and in personal computing experiences more generally.

Our love of the 90s systems design canon is not surface level; the new
possibilities and progressive refinement of technique over the last decades
compel us to animate the spirit of that era anew; we can and should be just as
daring as the engineers of this era, while learning from both their lessons and
mistakes.

## Design

Everything is a pane. Every pane has a tag (title), a command vocabulary
(discoverable via the command surface or `/pane/<n>/commands/`), a body
(text, widgets, or a legacy surface), and a session-typed protocol connection. Panes compose spatially and through the protocol. The
system's power derives from the uniformity of this single object.

Every pane is one object with many views: a visual display to the user, a
protocol endpoint to other components, a filesystem node at `/pane/` for scripts
and tools, a semantic object for accessibility. The views are projections of
the same state, kept consistent by optics discipline.

The local machine has no architectural privilege. A headless pane instance in
the cloud runs the same protocol as the desktop compositor — it's just a server
without a display. The unified namespace at `/pane/` shows all panes, local and
remote, as computed views over the same indexed state.

The system is extended through the same interfaces it uses internally. A
routing rule is a file. A pane mode wraps a library with domain-specific
semantics. More generally, add new functionality by dropping a declarative
specification in the relevant directory. Removing it is deleting the file.

Agents are system users, not applications. They have accounts, home
directories, `.plan` files. They communicate through the same protocols and
filesystem interfaces as human users — whether they run locally or on a remote
headless instance.

## Agents

`finger ada` shows what your agent is working on. `mail -s
"review this PR" ada` sends it a task. `write ada` opens a
real-time conversation on the terminal. `cat /pane/5/body`
shows its live output. `ls /pane/by-sig/com.pane.ai.agent.ada/`
lists every pane it's running. `cat ~/.access` shows exactly
what it's allowed to touch.

The agent isn't a chatbot in a sidebar. It's a peer on the
system — a unix user with a home directory, a login session,
mail, cron, and the full pane protocol. It communicates through
the same interfaces humans use, its work is inspectable through
the same namespace, and its permissions are governed by the same
kernel mechanisms. When it needs to delegate, it deploys a VM
with sub-agents that coordinate among themselves using the same
tools. The infrastructure that made unix a multi-user system
in 1971 turns out to be exactly the infrastructure agents need
in 2026 — pane just recovers it.

See `docs/ai-kit.md`.

## Adoption

You don't reinstall your OS to try pane. You can integrate it in your nix flakes.

    nix flake on any unix-like
      → headless pane: server, kits, protocol, filesystem
      → configuration accumulates in nix expressions
      → upgrade to compositor, then desktop
      → the flake IS the seed of a full pane linux config

Settings transfer because they were always nix expressions. NixOS, darwin, pane
linux — same `pane.services.*` options, different platform backend (systemd,
launchd, s6-rc).

## Protocol

Every interaction is a session — a typed conversation verified at compile time.
The session type describes what each party sends and receives, in what order,
with what branches. Deadlock freedom is guaranteed structurally. Async by
default; sync only when a response is needed.

The protocol is transport-agnostic: unix sockets for local, TCP/TLS for remote.
Same session types, same messages, same guarantees. Adding a network transport
required zero protocol changes — the transport trait is the hinge.

Routing is a kit-level concern, not a central server. The kit evaluates rules
locally and dispatches directly — sender to receiver, the way BeOS's BMessenger
worked. No intermediary.

## Architecture

The system has two layers: servers and kits.

Servers are small processes, each doing one thing. The compositor owns the
display. The headless server speaks the same protocol without rendering. Other
servers handle lifecycle, storage, filesystem projection, and health
monitoring. Configuration is files. Plugin discovery is directories. No config
parsers, no SIGHUP, no restart.

Kits are the programming model, not wrappers over a protocol. They provide wire
types, session-typed channels, the application framework (Application, Pane,
Messenger, Handler), composable optics for structured state access, filesystem
notification, rendering, text, input, and media. A kit is the right abstraction
for its domain — not a lowest-common-denominator binding.

pane linux is a sixos flake (s6 + nix). sixos provides the init/service
substrate, nixpkgs provides packages, pane provides the personality.

The crate layout under `crates/` is the source of truth for what exists and
what each component does.

## Status

The project is in a redesign phase. The architecture spec
(`docs/architecture.md`) is the source of truth. A prototype
validated the API vocabulary and subsystem landscape; the
codebase has been struck for reimplementation against the
tightened spec.

What exists today:

    pane-session     session type primitives (Chan, Send, Recv, Branch),
                     transports (unix, tcp, tls, memory, proxy, reconnecting),
                     calloop integration. ~520 LOC, 40 tests. Orthogonal
                     to the redesign — carries forward unchanged.
    pane-optic       optics: Getter/Setter/PartialGetter/PartialSetter,
                     FieldLens/FieldAffine/FieldTraversal, composition,
                     optic law tests. carries forward unchanged.
    pane-notify      filesystem change notification (inotify/fanotify
                     abstraction). carries forward unchanged.
    pane-proto       struck — reimplementing per architecture spec
    pane-app         struck — reimplementing per architecture spec
    pane-server      struck — reimplementing per architecture spec
    pane-headless    struck — reimplementing per architecture spec
    pane-comp        struck — reimplementing per architecture spec
    pane-hello       struck — reimplementing per architecture spec

What's next: Phase 1 (Core) — Protocol trait, ServiceId,
Handler + Handles<P> (Display, Clipboard, etc.), Message (base-protocol-
only), Flow, Dispatch<H>, ConnectionSource, filter chain,
Messenger + ServiceRouter. See `PLAN.md`.

## Building

    direnv allow               # activate nix dev shell (first time)
    cargo check                # check workspace (surviving crates)
    cargo test                 # test surviving crates
    just lint                  # clippy
    just fmt                   # rustfmt + nixfmt
    just doc                   # generate API docs

## On methodology

> It takes an idiot to do something cool, that's why it's cool.
>
> — Mamimi, FLCL

pane's design is human-authored (the fruit of my lifelong fascination with
operating system design, going back to using BeOS and Plan 9 as a young teenage
hacker), standing on the shoulders of giants. The decision to ground the
architecture in BeOS's systems engineering and Plan 9's distributed computing
model was deliberate — these were the best ideas of a golden age of operating
system design, carried out by intuition and refined technique by engineers
whose work has aged remarkably well.

The Haiku project deserves special gratitude. Their decades-long marathon to
reimplement BeOS from scratch kept the spirit of Be alive when it would
otherwise have been lost. Their source code, their documentation, their Haiku
Book — these are invaluable to any effort that seeks to carry on what Be
started. The author is a lifelong fan of the project and is deeply grateful for
what they have built and continue to build.

Plan 9's ideas were largely compatible with Be's. Where Be proved that
message-passing discipline and per-component threading produce responsive,
stable desktops, Plan 9 proved that protocol uniformity and location
independence produce systems whose capabilities exceed what any designer
anticipated. Session types, optics, and related formal frameworks give us tools
to carry out with compiler verification what the best systems designers of that
era achieved by skill and convention. Better type systems, more refined
theoretical guidance for hard problems, more elegant formalizations of
principles that were previously folk wisdom — these don't replace the original
insights. They let us build on them with greater confidence.

Combining BeOS and Plan 9 is not eclecticism — it's hybrid vigor. These two
traditions optimized for different things (local responsiveness vs. location
transparency) and their "incompatible" design pressures produce solutions that
neither lineage alone could reach. The session types and optics act as the
recombination mechanism: they force the implementation to find the common
structure that satisfies both traditions, rather than retreating into pure
BeOS or pure Plan 9 patterns. When the two lineages contradict, the formal
methods often reveal a third option that transcends the contradiction entirely.

The implementation is developed with substantial assistance from Claude
(Anthropic) via the Claude Code CLI. The relationship is explicitly dialectical: the human provides
architectural synthesis — recognizing cross-lineage unifications (e.g., that
BeOS live queries and Plan 9 synthetic filesystems are dual expressions of
"namespace as indexed state with materialized views") and setting selection
pressures. The AI handles rapid prototyping, cross-referencing theoretical
literature against reference implementations (particularly Haiku's source),
and navigating constraints to discover lawful solutions. Session types and
protocol boundaries serve as caching layers for implementation knowledge,
allowing the human to maintain focus on architectural integration while the
AI handles syntactic recombination.

The development process functions as empirical systems design — each subsystem
extension is a controlled experiment in compositional behavior. Session-type
boundaries prevent bad mutations from propagating (a flawed component cannot
violate the protocol contract with its neighbors). Testing validates not just
local correctness but emergent system properties — does this component
introduce impedance mismatches that ripple through the protocol graph? And
checking against BeOS/Plan 9 conventions maintains lineage purity, preventing
drift toward "GitHub average" solutions that would fracture the system's
coherent personality. `pane-session` was established as the foundational
substrate before any other subsystems, ensuring that subsequent components are
necessary implications of the session constraints rather than arbitrary choices.

The least charitable reading is that this is a mean vehicle for an ambitious
project. We think the evidence points otherwise. The synthesis required —
session type theory, optics, BeOS systems engineering, Plan 9 distributed
computing, nix packaging, Wayland protocol, s6 service management — spans
enough domains that no single developer could hold it all in working memory
simultaneously. An AI agent that can reference the Be Newsletter archive, read
Haiku source, consult Plan 9 papers, verify session type soundness, and write
the code in a single conversation may well be above the mean for this particular
task. The architecture is the prompt, in the deepest sense: well-chosen design
constraints correlate with well-shaped implementations — and our bet is that
correlation is sufficient in our case. The modular nature of the architecture
also entails that the code surface of the most mission-critical components of
our infrastructure can be readily vetted by human eyes if need be.

Individual commits record human design direction and agent execution steps,
including the model used. We are explicit about this process because the result
should be judged on its merits, not on assumptions about how it was produced. Read the code. Run the
tests. The design either holds or it doesn't. That being said, if you believe
enough in what you see here and don't want matters to be left to chance, we
welcome your help in building, testing, and verifying pane's architecture with
us.

## License

    protocol and client kits    BSD-3-Clause
    servers                     AGPL-3.0-only

    see LICENSE-BSD3 and LICENSE-AGPL3
