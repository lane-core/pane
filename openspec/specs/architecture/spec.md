# Pane — Architecture Specification

## Vision

Pane is a Wayland compositor and desktop environment for Linux. It is the foundation for a complete desktop distribution — a unified OS design philosophy applied over the Linux base, analogous to what Apple did with Mac OS X over Unix in the 2000s, but grounded in BeOS's design convictions rather than NeXT's.

Pane is about expressive ways to compose ideas. The system is built from a small, principled core — typed protocols, filesystem interfaces, composable servers — from which the entire experience is derived by first principles. The core can be understood, modified, and extended. Guard rails for new users. A ladder for power users.

The design bet: if the protocol is right — if each component's operational semantics are local and sound, if interfaces are semantic, if composition rules are principled — then the system will sustain stability in the face of emergent complexity. BeOS proved this. In the 1990s, Be Inc. built for the hardest case: pervasive concurrency, symmetric multiprocessing, media as first-class. The BMessage/BLooper model forced every component to implement self-contained operational semantics — each piece handled its own messages, managed its own state, and only accounted for side effects at explicit message boundaries. No global coordinator was needed because the protocol was the coordination. The result was an OS leagues ahead of its contemporaries in stability and responsiveness, not because it was simple, but because overcoming the formidable challenges of concurrency could only be accomplished by superior systems design.

We know operationally that message-passing discipline produces stable, compelling systems. Session types — typed descriptions of entire conversations between components — formalize what BeOS's engineers achieved by skill, sensibility, and intuition alone. Two decades of theoretical development eventually caught up to what practice demonstrated. Pane stands on both: the empirical proof that it works, and the theoretical framework that lets the compiler verify it.

The experience emerges from composition of small, focused servers speaking a shared protocol. No single component implements "the desktop." As Jean-Louis Gassée distinguished: a system has _rendering power_ (raw capability — threading, compositing, filesystem) and _expressive power_ (the ability to write new applications that aren't easily written, or are impossible, on other platforms). Pane's Wayland compositor, session types, and threading model are rendering power. The communication infrastructure, the kits, and the extension model are expressive power. The goal is to enable applications that are impossible or impractical on conventional Linux desktops.

Pane draws from Plan 9's text-as-interface philosophy for interaction, from NeXTSTEP's Services model and design-as-developer-productivity insight, and from modern tiling window management. But BeOS is the north star — the proof that protocol discipline, infrastructure-first design, and building for the hardest case produce a system where integrated experiences emerge without being designed top-down. You know you've done something right when people use your platform in ways you didn't anticipate.

## The Pane Primitive

The **pane** is the universal object of the system. Everything — shells, editors, file managers, status widgets, configuration panels, notifications, legacy applications — is a pane. This is not a UI convention but a structural commitment: the pane is the unit of composition, and the system's power derives from the uniformity of that unit.

A pane is one thing with many views. The same pane is:

- A **visual display** to the user (tag line, body content, chrome)
- A **protocol endpoint** to other components (session-typed conversations)
- A **filesystem node** to scripts and tools (files representing state and operations)
- A **semantic object** to accessibility infrastructure (roles, values, actions)

When state changes through one view (user edits text), the other views reflect it (filesystem shows updated content, protocol notifies subscribers). The views are not separate systems bolted together — they are projections of the same object, kept consistent by the pane's internal state.

Every pane shares:

- A **tag line**: editable text that serves as title, command bar, and menu simultaneously (inspired by acme). Text is the interface.
- A **body**: the content area, whose representation depends on the pane's nature — text, widgets, a legacy Wayland surface, or a hybrid.
- A **protocol connection**: session-typed communication with the compositor and other servers.

Legacy applications participate through wrapping. A legacy app is wrapped in a pane that abstracts over its implementation — guided by an application directory (scripts, integration helpers, metadata) managed at the app level. There is no centralized registry for application capabilities. The integration metadata lives with the application and is discovered via the filesystem, the same way routing rules and plugins are discovered. The pane wrapper exposes as much of the legacy app's interface to pane's system services as can be extracted.

## Target Platform

Pane targets Linux exclusively, tracking the latest stable kernel release. The system leverages Linux-specific capabilities: mount namespaces, user namespaces, fanotify, inotify, xattrs, memfd, pidfd, and seccomp.

**Init system:** pane-init is an abstraction layer over the host init system. pane defines contractual guarantees it needs (process restart, readiness notification, dependency ordering) and pane-init maps these to the concrete init system (s6, runit, systemd). pane-roster is the app directory — it tracks who's alive and what they can do. It does not supervise processes directly. When a server dies and the init system restarts it, the server re-registers with roster. The init system is an implementation detail behind pane-init's contracts.

**Filesystem:** The target filesystem must support the `user.*` xattr namespace. ext4, btrfs, XFS, and bcachefs all qualify. Advanced filesystem features (snapshots, subvolumes, CoW) are available through an abstraction layer when the filesystem provides them, and degrade gracefully on filesystems that lack them.

## Design Pillars

### 1. Text as Action

Any visible text is potentially executable (this should be presented as hypertext, a concept users are already familiar with. Two fundamental actions exist on text: **execute** (run it as a command) and **route** (send it to the router for pattern-matched dispatch to the appropriate handler). Activate `Makefile:42` anywhere in the system and it opens in the editor at line 42. This collapses toolbars, menus, hyperlinks, and file associations into one mechanism: actionable text and pattern matching. The specific input gestures that trigger execute and route are a design detail — the principle is that these two actions are available on any text, anywhere.

### 2. Visual Consistency Through Shared Infrastructure

Each pane renders its own content — this is the Wayland model, and pane embraces it. The compositor composites the results and renders chrome (borders, tag lines, focus indicators), but body content is the client's responsibility. Visual consistency is not achieved by centralizing rendering in the compositor. It is achieved through the kits.

The Interface Kit provides the rendering infrastructure that all native pane clients use: text rendering, styling primitives, color management, layout. When every native application renders through the same kit, they produce the same fonts, the same styling, the same visual language — not because a central authority forces it, but because the kit makes it the path of least resistance. This is how BeOS achieved its integrated feel: not because app_server rendered everything (though it did), but because every application used the Interface Kit, and the Interface Kit enforced consistency through its API. Pane achieves the same result through the same mechanism — shared kit infrastructure — while working with Wayland's client-side rendering model rather than against it.

The compositor owns what only the compositor can own: window chrome, layout structure, compositing, and input dispatch. Everything else belongs to the client, mediated by the kits.

### 3. Modular Composition

The system decomposes into small servers (separate processes) and thin client kits (Rust crate libraries). Each server does exactly one thing. Integrated behavior emerges from composition of servers, not from any single server knowing about everything.

The architecture follows the pattern BeOS proved: client-side libraries convert API calls into messages sent to servers. Servers allocate a thread per client, providing concurrent access while keeping the developer's API simple and synchronous-feeling. The boundary between kit and server is the boundary between what the developer experiences and how concurrency works. Developers write against a clean Rust API; the system manages the threading, the message passing, and the protocol safety behind that API. This is how BeOS made concurrency invisible — and it's why Schillings could say "programming should be fun."

### 4. Session-Typed Protocols

All inter-component communication is described by session types — typed descriptions of entire conversations, not just individual messages. A session type specifies what each party sends and receives, in what order, with what choices. The compiler enforces that both parties follow complementary protocols. Deadlock freedom is guaranteed structurally.

BeOS's BMessage/BLooper model gave every component self-contained operational semantics: each looper had its own thread, its own message queue, and communicated only via asynchronous messages. A BMessage could carry any typed data — strings, integers, nested messages, raw bytes — and flow through the system without tight coupling between sender and receiver. This produced an extraordinarily stable and responsive system, but correctness was enforced by convention. A BMessage could contain anything. A BLooper could receive any message at any time. The protocol was in the engineers' heads, not in the types.

Session types formalize exactly this discipline. What BMessage enforced by API convention ("you send a B_REPLY after receiving a B_REQUEST_COMPLETED"), session types enforce at compile time ("after `Recv<Request>`, the type is `Send<Reply>`"). The conversation structure — what is sent, in what order, by whom — is verified before the program runs. The theoretical foundation is the Caires-Pfenning correspondence between linear logic and session types. The practical foundation is the `par` crate.

The specific message data model will be refined as the implementation develops. The commitment is to the discipline — typed conversations between components — not to a particular message format. The type system should guide behavior, not prevent it — strict enough to catch protocol errors at compile time, flexible enough that components can extend conversations without breaking the system. BeOS's MIME type philosophy applies: escape hatches for edge cases, not a rigid cage.

### 5. Semantic Interfaces

Every interface a pane exposes — filesystem, tag line, protocol messages — SHALL present the abstraction level semantically relevant to its consumer. The same object may be viewed at different levels by different consumers:

- A **human user** sees the semantic level: commands, files, directories, operations.
- A **pane application** sees a system-service level: state, exit codes, environment, capabilities.
- The **compositor** sees the rendering level: cells, regions, surfaces — because rendering IS its semantics.
- A **debugger or admin tool** sees the implementation level: byte streams, buffer state, protocol traces — because introspection IS its purpose.

The abstraction level isn't fixed — it's determined by who's looking and what they need. This operates over a permission gradient from system to user. Implementation details aren't hidden — they're available at the appropriate interface depth for consumers who need them. The principle is: match the interface to the consumer's purpose.

### 6. Filesystem as Interface

State and configuration are filesystem primitives — file content for values, xattrs for metadata, directories for structure. Plugin discovery is via well-known directories watched by pane-notify. The FUSE interface at `/srv/pane/` exposes server state for scripting and debugging. The filesystem is the database, the registry, and the configuration format.

**Caching invariant:** Servers cache filesystem state in memory at startup and update only in response to pane-notify events. The render loop and event dispatch never perform filesystem I/O.

### 7. Composable Extension

The system is extended through the same interfaces it uses internally. There should be a way to modularly compose modifications of behavior, given in a declarative specification, with good UX abstractions for modifying it in various ways as suits user preference. The ambition is a vast ecosystem of plugin design over the safe parts of the OS surface itself.

Extensions span a spectrum from pure data to code to full applications:

- **Routing rules** (pure data): a JSON file in a directory. Drop it, the system gains behavior. Delete it, the behavior disappears. No code, no compilation, no registration.
- **Translators** (plugins): a binary in a well-known directory, following the Translation Kit pattern — identify content, transform it, declare quality ratings. BeOS proved this model scales: drop a translator, the whole system gains a file format.
- **Pane modes** (composable code): a program that wraps pane-shell-lib with domain-specific semantics. A "git mode" transforms a shell pane's interface — custom tag line, custom routing patterns, custom filesystem endpoints. The terminal emulation is reused; the semantic layer is new. Like emacs modes, but with static types, OS-level composition, and no language runtime.
- **Protocol bridges** (services): a daemon that translates between a foreign protocol and pane's native message model. pane-dbus is a bridge. Each bridge is a plugin.

Plugins compose safely because they operate on the public interface surface, not on internal state. The extension surface is the same surface the system itself uses. Adding a plugin is dropping a file in a directory. Removing it is deleting the file. The type system is the safety guarantee — extensions operate on typed interfaces, not raw memory.

The goal is that the extension surface enables composition the designers didn't plan — applications and workflows that emerge from combining plugins, routing rules, pane modes, and bridges in ways nobody anticipated. If the interfaces are regular enough, this happens naturally.

### 8. Developer Experience as Design Philosophy

NeXTSTEP's deepest insight was that developer productivity and user experience are inseparable. If building an application is fast and the tools enforce consistency, more applications will exist and they'll be more consistent. The quality of the developer experience — Interface Builder, AppKit, dynamic loading — was inseparable from the quality of the user experience.

Pane inherits this conviction. The kits, the typed protocols, the session type definitions, the filesystem conventions — these are not just internal infrastructure. They are the developer-facing surface that determines whether people build pane-native applications or don't. If building a pane app requires fighting the framework, developers will write Wayland apps instead and pane will have the two-world problem forever. If building a pane app is the easiest way to build a desktop application on Linux — because the protocols are clear, the kits are ergonomic, the types catch mistakes early, and the result looks and feels integrated by default — then the native ecosystem grows and the integrated feel follows.

## Servers

Each server is a separate process that does exactly one thing. Servers communicate via session-typed protocols over unix sockets. Infrastructure servers are managed by the init system (via pane-init's contractual abstraction) and register with pane-roster on startup. Each server runs its own threaded looper — a thread with a message queue, processing messages sequentially (the BLooper model). The compositor's Wayland core uses calloop for fd polling; other servers use plain threads and channels.

The server architecture scales in both directions. Gassée observed that "most systems only 'scale' upwards — they bleed to death when a cut is attempted in order to downscale them." BeOS could fit on a 1.44MB floppy because the client-server architecture meant you could leave out servers you didn't need. Pane inherits this: the system works with any subset of its servers. If you don't need attribute indexing, don't start pane-store. If you don't need the filesystem interface, don't start pane-fs. The modularity is architectural, not bolted on.

### pane-comp — Compositor

Composites client surfaces, manages layout, renders chrome. Smithay-based Wayland compositor.

Responsibilities:

- Wayland protocol handling (xdg-shell, layer-shell, xwayland)
- Layout tree: tree-based tiling (recursive splits) with tag-based visibility (dwm-style bitmask)
- Surface compositing: composites buffers submitted by clients (both pane-native and legacy Wayland)
- Pane protocol server: accepts pane-native client connections (multiple panes per connection)
- Chrome rendering: borders, tag lines, split lines, focus indicators
- Input handling: libinput integration, xkbcommon keyboard layout, key binding resolution, pointer acceleration (in-process, not a separate server — latency-critical)
- Input dispatch: routes keyboard/mouse events to the focused pane

Does NOT contain: routing logic, app launch logic, file type recognition. For native panes, a route action sends a `TagRoute` event to the pane client; the pane-app kit evaluates routing rules locally and dispatches directly to the handler. For legacy Wayland panes, pane-comp handles route dispatch through its own kit integration.

### Routing — A Kit Concern, Not a Server

Content-based routing is built into the pane-app kit, not centralized in a separate server. Components communicate directly via the protocol — sender to receiver — the way BeOS's BMessenger carried messages directly between applications via kernel ports. There is no central routing server whose failure would break all communication.

The pane-app kit provides routing as part of its standard functionality:

- Loads routing rules from the filesystem (`/etc/pane/route/rules/`, `~/.config/pane/route/rules/`), one file per rule
- Watches rule directories via pane-notify for live addition/removal — drop a file, gain a behavior; delete it, lose it
- When a route action is triggered, the kit evaluates rules locally: matches content against the ordered rule set, transforms content (extracts substrings, adds attributes, validates paths), resolves the target
- Queries pane-roster's service registry for additional matching operations
- When multiple handlers match: presents options for the user to choose. Single match auto-dispatches.
- Dispatches directly to the handler — no intermediary

One communication model, not two — BeOS learned this the hard way when they tried heterogeneous DSP+CPU processing and abandoned it because "people developing the system now have to contend with two programming models and two pieces of system software and the coordination headaches between them."

**Protocol bridges** are standalone daemons, not part of a central router. Each bridge translates between a foreign protocol and pane's native message model. pane-dbus translates D-Bus signals and method calls. Each bridge is a plugin. The pane side is always the same typed interface. Bridges dispatch directly to handlers — they are clients of the protocol like any other participant.

### pane-watchdog — System Health Monitor

A minimal external process that monitors system health. Inspired by Erlang's `heart` — deliberately simple, with a trivial heartbeat protocol. The less it does, the harder it is to kill.

Responsibilities:
- Heartbeat monitoring of critical components (compositor, roster)
- Detecting unresponsive components via missed heartbeats
- Triggering escalation procedures on failure: journal flush, user state backup
- Notifying the init system to restart failed components

Does NOT contain: routing logic, message dispatch, application-level functionality. The watchdog is infrastructure in the way a hardware watchdog is infrastructure — it checks pulses and pulls the emergency brake. Nothing else.

### pane-roster — Roster

The component that makes the app ecology work. Roster tracks who's alive, knows what they can do, remembers what was running, and facilitates the protocol flows that launch and connect applications. It does this by implementing the same protocol every other component speaks — it's not special, it's just a server that happens to know about other servers.

**Service directory** (for infrastructure servers):

- Infrastructure servers register on startup. Roster records identity and capabilities.
- Answers queries: "where is the router?", "is the store running?"
- Roster does not restart servers. When an infrastructure server crashes, the init system (via pane-init) restarts it; the server re-registers with roster.

**App lifecycle** (for desktop applications):

- Facilitates launching desktop apps (shells, editors, user programs)
- Monitors running apps, distinguishes crash from clean exit
- Session save/restore: serializes running app state, restores on login

**Service registry** (for discoverable operations):

- Apps register `(content_type_pattern, operation_name, description)` tuples
- Router queries the registry for multi-match scenarios
- Answers: "what operations are available for this content type?"

Does NOT contain: process supervision of infrastructure servers (that's the init system behind pane-init's contracts).

### pane-store — Attribute Store

Indexes file attributes, emits change notifications.

Responsibilities:

- Reads and writes extended attributes on files (`user.pane.*` xattr namespace on Linux)
- Maintains an in-memory index over attribute values for fast queries (rebuilt from xattr scan on startup, like BFS)
- Uses pane-notify for filesystem change detection (fanotify for mount-wide xattr changes, inotify for targeted watches)
- Emits change notifications when watched files/attributes change
- Provides a query interface over the index

Does NOT contain: live query maintenance (that's a client-side composition of index reads + change notification subscriptions), file type recognition as a built-in (type recognition is a client of pane-store that sets type attributes based on sniffing rules).

### pane-fs — Filesystem Interface

Exposes system state as a FUSE filesystem at `/srv/pane/`. This is Plan 9's gift: if state is a file, any tool can access it — shell scripts, remote machines, programs in any language. The filesystem provides universality that typed protocols cannot; the typed protocol provides safety that the filesystem cannot. Both are needed.

The filesystem interface follows the semantic interfaces pillar: each pane's filesystem representation presents the abstraction level relevant to its consumer. A shell pane exposes output, working directory, commands — not cells and escape sequences. The specific filesystem tree structure will evolve with the implementation.

pane-fs is a translation layer between FUSE operations and the socket protocol — it is just another client of the pane servers, not a privileged component.

Does NOT contain: any server logic.

## Shared Infrastructure

### pane-notify — Filesystem Notification

An internal crate (not a standalone server) that abstracts over Linux filesystem notification interfaces.

- **fanotify** with `FAN_MARK_FILESYSTEM` for mount-wide watches (pane-store bulk xattr tracking)
- **inotify** for targeted watches (specific directories, config files, plugin directories)
- Consumers request watches by scope; pane-notify picks the right kernel interface
- Unified event stream integrating into the server's looper (calloop for the compositor, channel-based for other servers)

### Filesystem-Based Configuration

Server configuration is stored as files in well-known directories under `/etc/pane/<server>/`. Each config key is a separate file. File content is the value. xattrs carry metadata: `user.pane.type` (string, int, float, bool), `user.pane.description`, optionally `user.pane.range` and `user.pane.options`.

Servers watch their config directories via pane-notify. Config changes take effect without server restart, without SIGHUP, without manual reload commands. All available config keys are discoverable by listing the directory.

### Filesystem-Based Plugin Discovery

Servers that support extensibility discover plugins by scanning well-known directories:

- `~/.config/pane/translators/` — content translators (type sniffing, format conversion)
- `~/.config/pane/input/` — input method add-ons (IME, connected via Wayland IME protocols)
- `~/.config/pane/route/rules/` — routing rules (one file per rule)

System-wide equivalents exist under `/etc/pane/` with user directories taking precedence. pane-notify watches these directories for live addition/removal. Plugin metadata is carried in xattrs: `user.pane.plugin.type`, `user.pane.plugin.handles`, `user.pane.plugin.description`.

## How Composition Works

The design bet is that if the infrastructure is right, integrated experiences emerge without being designed top-down. No single component implements "the desktop" — the desktop is what happens when the servers compose.

The canonical proof is BeOS's email. No component in BeOS implemented email. The mail_daemon delivered messages as files with typed attributes (From, Subject, Date, Status — all indexed by BFS). Tracker displayed these files in its normal column view, because it already knew how to display files with attributes. Queries over those attributes became inboxes — a "live query" for `status == new && mailbox == inbox` was a folder that updated in real time. Composing a reply opened a file in the editor. The entire email experience emerged from the filesystem, the attribute indexing system, the file manager, and a small daemon — infrastructure that had no knowledge of email.

This is what pane aspires to. The servers provide infrastructure: routing, attribute indexing, filesystem exposure, application lifecycle, compositing. The experiences — file management, development workflows, communication, system administration — emerge from how that infrastructure composes.

**Routing composes content with handlers.** Text is activated, the router matches it against rules, and the matched handler receives a resolved message. The router transforms content (extracts filenames, line numbers, URLs), validates paths, and queries the service registry. The pipeline is: content → rules → transformation → dispatch. Whether the content came from a user action, a D-Bus signal, or a filesystem event, the routing is the same.

**Attribute indexing composes metadata with queries.** pane-store indexes file attributes, emits change notifications, and answers queries. A client that subscribes to change notifications and maintains a query result set has a live query — without pane-store implementing "live queries" as a feature. The composition is client-side.

**Filesystem exposure composes system state with tools.** Anything exposed at `/srv/pane/` is scriptable. A shell script that reads `/srv/pane/index` can list all panes. Writing to a configuration file triggers a live update via pane-notify. The filesystem is the universal FFI — any language, any tool can participate.

**Session persistence composes lifecycle with state.** The compositor serializes layout. The roster serializes the running app list. Each app serializes its own state. On restart, each component restores its part. No single component owns "the session."

## Kits

Kits are the BeOS kit concept: cohesive subsystems that each address a domain. The Application Kit is the messaging and lifecycle model. The Interface Kit is the UI subsystem — rendering, styling, layout. The Storage Kit is the filesystem data layer. Kits are not wrappers over protocols — they ARE the programming model. The protocol is an implementation detail inside the kit, just as it was in BeOS where libbe.so communicated with servers via kernel ports internally but presented a coherent, opinionated API surface to developers.

The kit structure mirrors BeOS's insight that kits should be layered: foundation types at the bottom, application lifecycle in the middle, domain-specific functionality at the top. The specific kits pane provides address contemporary needs — an AI Kit for agent infrastructure, a Media Kit abstracting over PipeWire — but follow the same pattern: cohesive, layered, forming an ecology. Because pane's servers communicate via a well-defined protocol, kits can be implemented in any language — but the kit in each language is a substantial programming model, not a thin binding.

The communication pattern is the one BeOS proved: the client-side library converts API calls into messages sent to a thread in the appropriate server. Servers allocate a thread per client, allowing concurrent access to the functionality each server provides. The developer sees an ergonomic Rust API but the implementation is message passing over typed channels. This is how BeOS made threading invisible — the system managed concurrency so the developer didn't have to.

BeOS's messaging architecture was built from four small, composable primitives: BLooper (a thread with a message queue), BMessage (the data container), BHandler (the message recipient), and BMessageFilter (intercept and modify messages before dispatch). These were simple individually but powerful in composition — a BMessageFilter could add cross-cutting behavior without subclassing, BHandlers could be moved between BLoopers, and any component could send a BMessage to any other via BMessenger. Pane's kit primitives should be similarly small and composable.

### pane-proto (foundation)

Wire types (message enums, session type definitions), inter-server protocol types, serde derivations, validation. Every other crate depends on this. No runtime dependencies — pure types and serialization. Analogous to BeOS's Support Kit — the foundation everything else builds on.

### pane-app (application lifecycle)

Application lifecycle management. Looper abstraction — a thread with a message queue, processing messages sequentially (the BeOS BLooper model, realized in Rust with std::thread + channels). Connection management for servers. Session type integration for typed conversations with the compositor and other servers. The developer's primary interface for building pane-native applications.

### pane-ui (interface)

Cell grid writing helpers. Tag line management. Styling primitives (colors, attributes). Coordinate systems and scrolling.

### pane-text (text manipulation)

Text buffer data structures. Structural regular expressions (sam-style x/pattern/command). Editing operations (insert, delete, transform). Address expressions.

### pane-store-client (store access)

Client library for pane-store. Attribute read/write. Query building. Change notification subscription. Reactive signal composition for live queries.

### pane-notify (filesystem notification)

Abstraction over fanotify and inotify. Integrates into a server's looper as an event source — delivers filesystem events through the same message queue the server uses for everything else. Used by pane-store, pane-comp (config), and any server that watches filesystem state.

### pane-ai (agent infrastructure)

AI agents are not applications — they are additional users of the system. They participate through the same protocols, the same filesystem interfaces, the same routing infrastructure as human users, but in sandboxed environments with permissions governed by declarative specification.

This takes direct inspiration from Plan 9's distributed computing model. Plan 9's `cpu` command let a remote user operate seamlessly across a network connection — the same namespace composition, the same file protocol, the same tools, just a different machine providing the computation. Pane's agent model extends this: an agent is a participant that operates within its own sandboxed user environment, communicating through the session-typed protocol and the filesystem, constrained by specifications that declare what it can see, what it can modify, and what actions it can take.

**Agents as system users.** Each agent runs as an actual user of the system — its own account, its own home directory, its own filesystem view (scoped via Linux user namespaces or equivalent isolation), its own set of permissions, its own protocol connections. The agent interacts with the system the way any user does: through the pane protocol, through the filesystem interface, through routing. It doesn't get a special API; it gets the same API everyone else gets, constrained by its declared capabilities. `who` shows which agents are active. `last` shows what they did. The permission model constrains what they can touch. Everything an agent does is visible through the same tools you use to inspect anything else on the system.

**The `.plan` file as behavioral specification.** An agent's behavior — what tools it can use, what panes it can observe, what files it can access, how it should respond to events — is declared in its `.plan` file: a human-readable specification in its home directory. `finger agent.reviewer` shows the specification and current task. The `.plan` is editable, version-controllable, shareable — transparent in the way pane demands all interfaces be transparent. A "code review agent" is a `.plan` file: watch for commits, read the diff, apply review criteria, post results. The `.plan` is a file; the specification IS the agent's identity. The same extension model as routing rules, translators, and pane modes.

**Communication through Unix infrastructure, revived.** The multi-user Unix communication primitives — designed for a world where multiple inhabitants shared a system — become the natural agent interaction layer:

- **`write`**: an agent that finishes a task sends a one-liner to your pane. Brief, one-directional, lightweight. Most agent communication should be this.
- **`talk`**: a focused interactive session with an agent — split-screen, real-time, bidirectional. This is what pair-working with an agent looks like.
- **`mail`**: asynchronous, persistent, queryable. Agents mail results to users, mail requests to other agents. When mail messages are files with typed attributes (the BeOS email model), agent communication becomes queryable by pane-store, filterable by routing rules, archivable. "You have mail." becomes "Your build agent has results."
- **`mesg y/n`**: the simplest possible availability protocol. One bit — am I available for interruption? An agent that respects `mesg n` queues its messages as mail instead of writing to your pane. The mechanism is a file permission, not a complex status system.
- **`wall`**: broadcast to all inhabitants. System events that every agent and the human should know about.

These patterns were designed for multi-inhabitant systems, abandoned when PCs became single-user, and are exactly right-sized for a person and their agents sharing a system. Pane doesn't need to invent agent communication infrastructure — it needs to recognize that Unix already built it.

**Seamless local and remote models.** The agent kit provides a uniform experience whether the underlying model runs locally on the user's hardware or is accessed via a remote API. The user should not need to think about where computation happens — the same interface, the same prompting patterns, the same tool use protocols, the same conversation history apply regardless. Switching between a local model and a remote API is a configuration change, not an application change. Users can optimize across models of different strengths — a fast local model for low-latency drafting, a strong remote model for complex reasoning, a private local model for sensitive data — moving fluidly between them within the same session. The routing infrastructure handles dispatch; the session protocol handles the conversation; the agent specification declares which models are available and when to prefer each. The experience is one continuous interaction with the system, not a collection of disconnected AI applications.

**AI-assisted system configuration.** A critical use case: agents helping users modify their own system. Pane's filesystem-as-configuration model, typed protocols, and declarative extension surface create interfaces that can be formally verified as safe configuration surfaces for AI-assisted design. An agent operating within its declared permissions can help a user customize routing rules, adjust aesthetic parameters, compose pane modes, or restructure their workspace — through the same typed, auditable interfaces the user would use directly. Because the configuration surface is the same filesystem and protocol surface the system uses internally, the agent's modifications are subject to the same validation and type checking as any other operation. This is qualitatively different from an AI with shell access — the agent operates on configuration surfaces that are designed to be safe, not on raw system internals.

**Ad-hoc construction.** Beyond configuration, agents can build new capabilities on request. A user says "I want shell output with TODO comments highlighted and routable to my task manager." The agent writes the routing rule, writes the output transformer, drops them in the right directories. The user describes intent; the agent produces the declarative specification; the system gains the behavior. This works because pane's extension surfaces — routing rules, translators, pane modes — are small, declarative, filesystem-native artifacts that an agent can produce. The agent doesn't modify pane's source code; it operates on the same extension surfaces human developers use. A user's collection of agent-built customizations becomes a shareable personal configuration — `.plan` files, routing rules, pane modes, all versionable as a directory. The emacs/neovim plugin ecosystem dynamic, with agents as contributors alongside humans.

**Why this matters.** No existing desktop environment treats AI agents as system-level participants with typed protocols, sandboxed environments, and declarative governance. Current AI tooling is ad-hoc precisely because operating systems provide no principled infrastructure for it. Pane's existing commitments — session-typed protocols, filesystem-as-interface, the routing infrastructure, per-user sandboxing, declarative extension — are exactly the infrastructure agents need. The AI kit doesn't require new architectural concepts; it's a concrete application of the architecture to a use case that demands all of it simultaneously. This is a potential killer app of pane — robust, expressive, and transparent AI usage patterns that are impossible with current tooling, emerging naturally from infrastructure designed for composability.

## Composition Model

Kit APIs compose at multiple levels, reflecting different needs:

**Session types for protocol structure.** The session type IS the protocol state machine — verified at compile time, not tracked at runtime. Protocol composition means composing session type fragments: a lifecycle phase, an active phase, a shutdown phase. Each is testable independently. The composition is type-checked. This replaces the runtime state machine approach with compile-time verification.

**Rust idioms for data composition.** `Result`/`?`, `Option` combinators, iterator chains — standard Rust, not a framework. Domain types carry derived combinators where their shape warrants it. The test: does it read more clearly than sequential statements?

**Reactive signals for observable state.** Change notifications from pane-store, UI state (focus, dirty, tag content), live queries — these compose via signals with `map`, `combine`, and friends. Specific crate choice deferred to when consuming code is built.

**Threaded loopers for runtime composition.** Components compose at runtime the way BeOS's BLoopers composed: each runs its own thread, processes messages sequentially, communicates via typed channels. The system's behavior emerges from many such loopers running concurrently, not from a single event loop orchestrating everything.

## Pane Protocol

### Session Types

Every interaction between components is a session — a typed conversation. The session type describes the entire protocol: what each party sends and receives, in what order, with what branches. The compiler enforces that both parties follow complementary protocols. Each side of a conversation has a dual type — derived automatically — ensuring structural compatibility.

Session types make the async/sync distinction visible in the protocol. A fire-and-forget operation (send content, continue without waiting) is a `Send` followed by continuation. A request-response (send a query, wait for the answer) is a `Send` followed by a `Recv`. The distinction is in the type, not in a runtime flag. This matters because fire-and-forget operations can be batched — the kit accumulates them and flushes in chunks, the same optimization BeOS's Interface Kit used for drawing commands. Request-response operations force a flush and a round-trip. The default should be async; sync only when a response is needed.

A single connection can host multiple panes. Each pane is a sub-session within the connection. Servers communicate via sessions too — each server pair defines their conversation type. The specific session type definitions for each protocol are in the respective component specs and will evolve with the implementation.

### Message Content

Pane messages travel along session-typed channels. The message model is influenced by BMessage — rich, composable, introspectable data that can flow through the system without tight coupling between sender and receiver. The specific serialization and data model will be refined alongside the session type integration.

### Resilience

Component failures are isolated and recoverable, not cascading. The guiding principle is monadic error composition — errors propagate through typed channels and are handled compositionally, not through ad-hoc catch blocks.

- **Session boundaries are error boundaries.** Each client session is wrapped so that a crashed client produces a "session terminated" event, not a panic in the compositor. The compositor cleans up the dead client's panes and continues serving others. This is analogous to how BeOS's app_server handled unresponsive windows — it discarded messages and continued.
- **Server crashes are recoverable.** pane-init's contractual guarantees mean the init system restarts crashed servers. The restarted server re-registers with roster. No component implements restart logic — that's the init system's job.
- **Failure does not cascade.** A crashed pane-store does not bring down pane-comp. A crashed client does not affect other clients. Each component's operational semantics are local — the same principle that produces stability in the happy path also contains failure in the unhappy path.

The specific mechanisms for crash handling (the affine/linear gap in Rust, catch strategies, heartbeat protocols) are open design questions to be resolved as the implementation develops. The commitment is to the principle: failures are protocol events, not system events.

## Client Classes

### Pane-native clients

Speak the pane protocol. Get full integration: tag line, cell grid body, routing, event streams, compositor-rendered chrome. Examples: shell (PTY bridge), editor, file manager, status widgets.

### Legacy Wayland clients

Speak standard xdg-shell (or xwayland for X11 apps). Get a pane wrapper: the compositor provides a tag line and borders, but the body is an opaque surface rendered by the client. Full desktop functionality (Firefox, Inkscape, etc.) works — just without routing or cell grid integration.

### The two-world problem

Mac OS X's Carbon/Cocoa split killed the Services menu and fragmented the integrated feel. This is pane's biggest risk: native pane clients and legacy Wayland apps are two worlds. The boundary is explicit — legacy apps don't pretend to be pane-native. The mitigation strategies: lean into the proliferation of good TUI applications (these compose seamlessly with pane's textual interface model), wrap non-pane applications to expose as much of their interfaces to pane's system services as is possible to extract, and provide visual theming that brings GUI toolkit apps toward uniformity with pane's aesthetic. Developer experience is the long-term answer — if building a pane-native app is fast and the tools enforce consistency (the NeXTSTEP lesson), the native ecosystem grows.

## pane-shell — The Textual Interface Layer

pane-shell is the most important pane client — it makes the system a daily driver. But it is not just a terminal emulator. Terminal emulation is commodity infrastructure (use existing libraries like vte or alacritty_terminal). The value of pane-shell is the semantic layer above the terminal and the extension model.

pane-shell is a library (pane-shell-lib) that other programs compose with. A "git pane" wraps pane-shell-lib with git-specific semantics: custom tag line showing branch/status, routing patterns for commit hashes, filesystem endpoints for staged files. The terminal emulation is reused; the semantic layer is new. This is the "emacs as an OS" idea — but interface-agnostic, statically typed, and composable via OS primitives rather than a language runtime.

The compositor does not know or care that it's compositing a terminal. pane-shell renders its content into a buffer using the Interface Kit (like any native pane client) and submits it to the compositor. It is just another Wayland client. The implementation details of terminal emulation (VT parsing, screen buffers, alternate screen) belong in the pane-shell spec, not here.

## Layout

Tree-based tiling with tag-based visibility, with floating panes supported as a complementary mode:

- The layout is a tree of containers. Leaf nodes hold panes. Branch nodes define splits (horizontal or vertical).
- Each pane has a tag bitmask. The compositor displays panes matching the currently selected tags. A pane can appear in multiple tag sets. Multiple tags can be viewed simultaneously (bitwise OR).
- Tiling splits are explicit visible lines on screen. The structure is always visible.
- Floating panes are supported as a separate layer. Some applications may use a mix of tiled and floating panes depending on their needs. Transient floating panes are used for choosers and popups.

## Aesthetic

Frutiger Aero — the polished evolution of 90s desktop design. The design philosophy: what if Be Inc. survived into the 2000s and refined their visual design alongside the early Aqua era? BeOS's information density and integration, Mac OS X Aqua 1.0's rendering refinement and warmth, combined into a power-user desktop that is both beautiful and dense. A computer interface should invite interaction rather than demand it — interfaces should have material qualities that make them feel real and approachable, with depth and texture serving comprehension rather than merely entertaining.

The visual consistency is architectural, not cosmetic. Every native pane renders through the same Interface Kit, producing the same fonts, the same styling, the same visual language — not because a central authority forces it, but because the kit makes consistency the path of least resistance. The compositor provides uniform chrome (borders, tag lines, focus indicators). The integrated feel comes from the shared kit infrastructure, not from apps individually following a style guide. BeOS achieved this because every app used the Interface Kit; pane achieves it the same way.

Reference points: BeOS R5 / Haiku (density, integration, matte bevels), Mac OS X 10.0–10.2 Aqua 1.0 (rendering quality, subtle translucency, warm palette), Frutiger Aero (the intersection: depth and warmth serving comprehension).

- **Depth through lighting**: subtle vertical gradients on controls (light top, darker bottom), 1px highlight/shadow edges. Matte and solid — not glossy Aqua gel, not flat Metro. Depth communicates hierarchy.
- **Beveled borders and visible chrome**: panes have real borders. Controls look like controls. Structure is always visible. Rounded corners (3-4px radius) — approachable without losing density.
- **Selective translucency**: floating elements (scratchpads, popups) are translucent to show context. Translucency where it's beautiful and aids comprehension, not universally.
- **Warm saturated palette**: warm grey base, saturated accent colors for focus/dirty/active states. The workspace feels well-lit — not a dark cave, not a white void.
- **Typography split**: proportional sans-serif for widget chrome (labels, buttons). Monospace for cell grid content and tag line text regions. Tag line stays monospace (it's executable text where column alignment matters).
- **Color as information**: dirty state, focus, errors. Not decoration.
- **Dense but refined**: closer to BeOS than Aqua in spacing. Smaller controls, tighter layout. Enough padding to be comfortable, not enough to waste space.
- **One opinionated look**: no theme engine, no theme selector. The aesthetic IS pane's identity. Individual properties configurable via filesystem-as-config (accent color, font size) but not wholesale theme replacement.

## Accessibility

Pane's semantic interfaces pillar connects directly to accessibility. Mac OS X's accessibility framework succeeded because every UI element exposed a role (what it is), a value (its current state), a label (what it's called), and actions (what can be done to it) — and because the compositing window manager maintained a model of the visual layout that accessibility could interrogate.

Pane has structural advantages here: pane-native clients describe their content to the compositor (cell grids, widget trees), so the compositor has a semantic model of what's on screen — not just opaque pixel buffers. Widget panes have semantic structure (buttons, labels, lists) that screen readers can interpret. Cell grid panes remain a challenge — addressing cell grid accessibility is a research problem for later phases.

## Technology

- **Language:** Rust — ownership and Send/Sync give compile-time guarantees for the threading model that BeOS engineers enforced by convention
- **Threading model:** per-component threads with message queues (BeOS's BLooper model). std::thread + channels for concurrency. No system-wide async runtime.
- **Compositor library:** smithay — Wayland compositor framework
- **Compositor event loop:** calloop — scoped to the Wayland core (fd polling for Wayland, DRM, input). Does not define the system-wide concurrency model.
- **Session types:** `par` crate — typed conversations between components, deadlock-free by construction. Driven per-thread via `block_on`, not via async runtime.
- **Wire format:** postcard (serde-based, varint-encoded, compact)
- **Filesystem notification:** pane-notify (fanotify + inotify abstraction)
- **FUSE:** pane-fs at `/srv/pane/`
- **Init abstraction:** pane-init — contractual interface over s6, runit, or systemd
- **Testing:** property-based (proptest) for protocol correctness, integration tests for server composition
- **Widget layout:** taffy (flexbox/grid layout engine, pure computation)
- **Widget rendering:** femtovg (2D vector graphics on OpenGL via glow — rounded rects, gradients, text)
- **Reactive signals (candidate):** `agility` crate — widget state bindings, store notifications. Decision deferred to when consuming code is built.

## Build Sequence

Each phase produces a testable, usable artifact:

1. **pane-proto** — message types, session type definitions, inter-server protocol, property tests ✓ (built, session type migration in progress)
2. **pane-notify** — fanotify/inotify abstraction, looper integration (calloop for compositor, channels for other servers)
3. **pane-comp skeleton** — smithay compositor, single hardcoded pane, tag line + cell grid rendering
4. **pane-shell** — PTY bridge client, first usable terminal
5. **Layout tree** — tiling with splits, multiple panes, tag-based visibility
6. **Routing integration** — routing rules, kit-level dispatch, protocol bridges, service-aware multi-match
7. **pane-roster** — service directory, app lifecycle, service registry, session management
8. **pane-store** — attribute indexing, change notifications, queries, in-memory index
9. **Widget rendering** — femtovg integration, taffy layout, Frutiger Aero controls
10. **pane-fs** — FUSE at `/srv/pane/`, format-per-endpoint
11. **Legacy Wayland/XWayland** — xdg-shell and xwayland support
