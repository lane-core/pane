# Haiku Project Research — 20+ Years Rebuilding BeOS

Research for pane spec-tightening. Primary sources: Haiku source code (local at /Users/lane/src/haiku/), haiku-os.org development blog and documentation, FOSDEM presentations, OSnews analysis articles, Haiku community forums (discuss.haiku-os.org), Haiku internals documentation, developer reports by waddlesplash, PulkoMandy, Axel Dorfler (axeld), and others.

Sources:

- Haiku project history: <https://www.haiku-os.org/about/history/>
- Wikipedia overview: <https://en.wikipedia.org/wiki/Haiku_(operating_system)>
- Haiku API messaging docs: <https://www.haiku-os.org/docs/api/app_messaging.html>
- BLooper reference: <https://www.haiku-os.org/docs/api/classBLooper.html>
- app_server architecture: <https://www.haiku-os.org/documents/dev/windows_and_views_in_the_haiku_app_server/>
- app_server compositing plan: <https://www.haiku-os.org/blog/stippi/2011-06-15_how_transform_app_server_code_use_compositing/>
- Package management design: <https://www.haiku-os.org/blog/zooey/2011-01-08_package_management_first_draft>
- Package management infrastructure: <https://www.haiku-os.org/docs/develop/packages/Infrastructure.html>
- Package management user perspective: <https://www.markround.com/blog/2023/02/13/haiku-package-management/>
- launch_daemon introduction: <https://www.haiku-os.org/blog/axeld/2015-07-17_introducing_launch_daemon/>
- launch_daemon API docs: <https://www.haiku-os.org/docs/api/launch_intro.html>
- Network stack architecture: <https://www.haiku-os.org/docs/develop/net/NetworkStackOverview.html>
- FreeBSD driver compat layer: <https://www.haiku-os.org/news/2007-05-08_haiku_getting_a_freebsd_network_driver_compatibility_layer>
- Binary compatibility strategy: <https://www.haiku-os.org/documents/dev/binary_compatibility_3_easy_steps/>
- API incompatibilities: <https://www.haiku-os.org/docs/api/compatibility.html>
- Media Kit updates: <https://www.haiku-os.org/blog/barrett/2015-07-29_media_kit_new_and_old_pieces>
- Xlibe X11 compatibility: <https://discuss.haiku-os.org/t/xlibe-an-xlib-x11-compatibility-layer-for-haiku/11692>
- Xlibe in contract report: <https://www.haiku-os.org/blog/waddlesplash/2022-01-10_haiku_contract_report_december_2021/>
- Replicants overview: <https://www.haiku-os.org/documents/dev/replicants_more_application_than_an_application/>
- Layout API introduction: <https://www.haiku-os.org/docs/api/layout_intro.html>
- Layout API history: <https://www.haiku-os.org/blog/yourpalal/2010-04-28_taking_haiku_layout_api_public/>
- Registrar protocols: <https://www.haiku-os.org/docs/develop/servers/registrar/Protocols.html>
- BFS attributes discussion: <https://discuss.haiku-os.org/t/bfs-attributes-what-are-the-most-popular-use-cases/18433>
- R1 Beta 5 release notes: <https://www.haiku-os.org/get-haiku/r1beta5/release-notes/>
- Beta 5 review: <https://hackaday.com/2024/10/30/haiku-oss-beta-5-release-brings-us-into-a-new-beos-era/>
- "Haiku isn't a BeOS successor anymore": <https://www.osnews.com/story/139634/haiku-isnt-a-beos-successor-anymore/>
- R2 speculation thread: <https://discuss.haiku-os.org/t/haiku-r2-what-would-beos-be-today/8211>
- Project direction discussion: <https://discuss.haiku-os.org/t/project-development-philosophies-and-direction/15512>
- Programmer's introduction: <https://www.osnews.com/story/24945/a-programmers-introduction-to-the-haiku-os/>
- POSIX compliance: <https://discuss.haiku-os.org/t/posix-compliance/1287>
- Security discussion: <https://discuss.haiku-os.org/t/haiku-security/707>
- Stack and Tile: <https://www.haiku-os.org/blog/czeidler/2012-05-14_status_report_stack_and_tile/>
- November 2025 report: <https://www.haiku-os.org/blog/waddlesplash/2025-12-12-haiku_activity_contract_report_november_2025>
- January 2026 report: <https://www.haiku-os.org/blog/waddlesplash/2026-02-12-haiku_activity_contract_report_january_2026/>
- FOSDEM 2016 package management: <https://archive.fosdem.org/2016/schedule/event/haikus_package_management/>
- FOSDEM 2017 desktop talk: <https://archive.fosdem.org/2017/schedule/event/desktops_haiku_desktop_still_learn_from/>
- HaikuPorts: <https://github.com/haikuports/haikuports>
- IME development docs: <https://www.haiku-os.org/documents/dev/developing_ime_aware_applications/>

---

## 1. History and Trajectory

### Origins (2001-2004)

Palm, Inc. announced the purchase of Be, Inc. on August 17, 2001. The next day, Michael Phipps started OpenBeOS. Palm refused to license the BeOS source code, meaning the entire system had to be reverse-engineered from the public API surface and documented behavior.

The initial scope was unambiguous: replicate BeOS R5 as an open-source, binary-compatible system. This was a strategic masterstroke. Phipps understood that an open-source project without a concrete, verifiable target tends to sprawl into permanent research. "Reproduce BeOS R5" gave every contributor a precise acceptance criterion: does the old binary run? This decision alone likely saved the project from the fate of countless other alternative OS efforts.

In 2002, Phipps forked the NewOS kernel (by Travis Geiselbrecht, a former Be engineer) as the kernel foundation. In 2003, Haiku, Inc. was incorporated as a nonprofit. In 2004, the project renamed from OpenBeOS to Haiku to avoid Palm's trademark.

### The long march (2005-2018)

- 2005: app_server completed, first native GUI rendering, Tracker file manager running
- 2007: FreeBSD network driver compatibility layer introduced (critical infrastructure decision)
- 2008: System achieved self-hosting (could compile itself)
- 2009: R1 Alpha 1 released. Qt4 ported. GCC4 compiler support.
- 2013: 64-bit support. Package management system. HaikuArchives project launched.
- 2014: New O(1) scheduler with CPU affinity
- 2018: R1 Beta 1 released — the first official release with full package management. LibreOffice available.

The gap between "Alpha 1" (2009) and "Beta 1" (2018) is instructive. Nine years. During that period the project built package management, the launch_daemon, the Layout API, 64-bit support, and dozens of subsystem rewrites. The alpha-to-beta gap is the cost of building infrastructure that BeOS never had — the stuff Be, Inc. ran out of money before implementing.

### Current state (2024-2026)

Beta 5 released September 2024. Nearly 350 tickets resolved. Key achievements: order-of-magnitude TCP performance improvement, USB audio support, FAT filesystem driver replaced with FreeBSD port, TUN/TAP driver for VPNs, dark mode, HiDPI improvements.

Beta 6 is imminent as of early 2026. The monthly activity reports show a project that has shifted from "building the OS" to "polishing the OS" — the majority of HaikuPorts commits now exceed Haiku core commits in most months, indicating the platform is stable enough that porters can work without constantly hitting OS bugs.

The January 2026 report documents: IPv6 path MTU discovery, multicast support, gradient stroke drawing in BView/app_server, BFS query optimizations, continued POSIX compliance improvements, and the monthly report itself being "largely written, previewed, and edited from within Haiku itself" — a quiet milestone of self-sufficiency.

### What the timeline tells us

Twenty-five years from inception to a system approaching daily-driver status. But the shape of the effort is revealing: the first ~7 years were about getting something that booted and rendered windows. The middle ~10 years were about building all the infrastructure BeOS never had (package management, modern networking, launch system, POSIX compliance). The final ~5 years have been about stability, polish, and ecosystem. The infrastructure decade is the part that's relevant to pane.

---

## 2. What Survived from BeOS

### BMessage/BLooper/BHandler threading model: survived intact

This is the most important finding. After 25 years of reimplementation, the BMessage/BLooper/BHandler architecture is essentially unchanged from BeOS R5. The Haiku source code in `src/kits/app/Looper.cpp` shows the same design: BLooper spawns a thread, runs a message loop, dispatches to handlers via the chain-of-responsibility pattern. The public headers in `headers/os/app/` — `Message.h`, `Looper.h`, `Handler.h`, `Messenger.h`, `Application.h`, `Roster.h` — map one-to-one to the BeOS originals.

The threading model survived because it was correctly designed for its purpose. The per-thread message loop with sequential message processing within each looper is an actor model. Actors don't share mutable state. Cross-looper communication is exclusively via message passing. The concurrency discipline — messages in, messages out, lock when accessing looper internals, never share data directly — scales naturally to modern multi-core hardware. Haiku's developers found nothing to change because the model was already right.

The one area where the model was extended (not changed) is `BServer`, a private subclass of `BApplication` used by system servers. This gives servers the same message-loop-per-instance pattern that applications get. The launch_daemon, registrar, and app_server all derive from it.

**Implication for pane:** The strongest possible endorsement of message-passing discipline as the concurrency model. Twenty-five years of reimplementation stress-tested it and it held. Pane's commitment to session-typed channels is the same idea taken further — where BeOS used untyped `uint32` `what` codes, pane can have the type system enforce protocol correctness.

### Kit structure: survived with additions

The kit hierarchy is faithfully preserved:

```
headers/os/
  app/       — Application Kit (BMessage, BLooper, BHandler, BApplication, BRoster)
  interface/ — Interface Kit (BWindow, BView, BControl + Layout API)
  storage/   — Storage Kit (BNode, BFile, BDirectory, BQuery, BMimeType)
  support/   — Support Kit (BArchivable, BLocker, BDataIO)
  translation/ — Translation Kit (BTranslatorRoster)
  media/     — Media Kit
  mail/      — Mail Kit
  network/   — Network Kit (new, replacing BeOS's net_server)
  locale/    — Locale Kit (new)
  package/   — Package Kit (new)
```

The kits that changed least are the ones closest to the core design: Application Kit, Support Kit, Storage Kit. The kits that changed most are the ones where BeOS was weakest: Network Kit (complete rewrite), Locale Kit (new), Package Kit (new).

**Implication for pane:** The kit structure proves that a well-designed kit hierarchy is durable. The additions Haiku made — locale, package, network — are exactly the things BeOS didn't have time to build. Pane's kit structure should start with the domains that are most fundamental (messaging, rendering, storage) and plan clean extension points for the ones that will come later.

### BFS: reimplemented faithfully, validated by 25 years of use

Haiku reimplemented BFS from scratch. The original specification was available through Dominic Giampaolo's "Practical File System Design" book. The implementation preserves:

- Extended attributes (arbitrary typed metadata on any file)
- Attribute indexing (system-wide indices enabling queries)
- BQuery live queries (queries that update in real-time as files change)
- Journaling
- MIME type association via filesystem attributes

BFS attributes remain the foundation of Haiku's identity. The registrar watches MIME database directories and updates caches when packages are activated/deactivated. The Tracker file manager displays attributes as columns. Email applications store messages as files with attributes.

But the community discussion on attribute use cases reveals an honest tension: many modern Haiku applications use SQLite databases instead of filesystem attributes, "drawing in completely unnecessary dependencies when native attributes could suffice." Attributes work best for Haiku-native applications; ported software ignores them. The canonical use cases — email metadata, music library organization, document ratings — remain compelling. But the portability problem (attributes don't survive copy to non-BFS filesystems) limits adoption.

**Implication for pane:** The concept of typed filesystem metadata is validated. But pane is on Linux, where extended attributes (xattr) exist but lack BFS's indexing and query infrastructure. Pane either needs to build that query infrastructure over xattr (significant work) or find another way to get the same UX benefit (attribute-based search and live queries). The Haiku experience shows that the concept is right but the ecosystem adoption depends on native applications actually using it.

### app_server: architecture preserved, compositing still pending

The app_server is the centralized rendering server. Its architecture is documented thoroughly in both the source code and development blog posts. The key structural elements from BeOS survived:

- **ServerWindow** per BWindow, managing the communication link
- **Window** (née WindowLayer) as the server-side on-screen representation
- **View** hierarchy mirroring client-side BView hierarchy
- **DrawingEngine** abstracting rendering (Painter class default implementation using AGG)
- **Desktop** managing all windows, clipping, workspaces
- **Two threads per window**: one in the client (BWindow's looper) and one in the server (ServerWindow)

The update session mechanism prevents flickering: when dirty regions are detected, the server notifies the client, the client's Draw() hooks fire, and only after all drawing is complete does the server copy from back-buffer to front-buffer. This is unchanged from BeOS.

The locking model uses a read-write lock (MultiLocker) on the Desktop: ServerWindow threads hold read locks (allowing concurrent operation), while global Desktop operations require write locks. This prevents deadlocks by ensuring threads release read locks before acquiring write locks for Desktop calls.

What has NOT been done: compositing. Stippi wrote a detailed plan in 2011 for transitioning from the current shared-framebuffer model to per-window buffers composited together. The plan is sound and would dramatically improve performance (window movement wouldn't require repaints of exposed regions). But as of 2026, it remains unimplemented. This is a 15-year-old TODO.

The absence of compositing means: no hardware-accelerated window rendering, no alpha-blended window effects, no smooth window animations, and inefficient handling of window overlaps. Every expose event triggers a full repaint of the affected region.

**Implication for pane:** Pane gets compositing for free via Wayland. This is the single largest advantage of building on Linux rather than rebuilding from scratch. Haiku has spent 15 years wanting compositing and not having the engineering bandwidth to implement it. Pane starts with it. But the app_server's client-server separation — the idea that rendering is a service, not a library call — is validated and pane should preserve that boundary.

### Translation Kit: survived, still works

The add-on model is alive. Translators in `/boot/system/add-ons/Translators/` handle format conversion. The current list of translators in the source (`src/add-ons/translators/`): AVIF, BMP, GIF, HVIF, ICNS, ICO, JPEG, JPEG2000, PCX, PNG, PPM, PSD, RAW, RTF, SGI, STxT, TGA, TIFF, WebP, WonderBrush.

The architecture is unchanged: BTranslatorRoster discovers add-ons, applications work with interchange formats (B_TRANSLATOR_BITMAP, B_STYLED_TEXT_FORMAT), and translators handle conversion. The extensibility model — drop a shared object in a directory — is simple and effective.

**Implication for pane:** The translator pattern validates the "format-neutral core + pluggable converters" design. Pane's type-dispatch system serves a similar role but with richer protocol awareness (session types on the conversion channels rather than just function calls).

### Replicants: survived technically, failed socially

Replicants exist in Haiku. The mechanism works: a BView with an Archive() function and a BDragger handle can be embedded into other applications, notably the Desktop for widgets. The ActivityMonitor, Workspaces applet, and DeskCalc are replicatable.

But the verdict from the community: "absolutely nothing happened to Replicants, and almost no one ever used them." They were touted as "the next ActiveX" and never caught on. In modern Haiku, their primary use is desktop widgets — a narrow application of a general mechanism.

Why the failure? Replicants require applications to be architected for embedding (implementing BArchivable correctly, designing views that make sense in isolation). The cost of supporting replication was borne by every application developer. The benefit was reaped by the rare user who wanted to embed calculator widgets on their desktop. The incentive structure was inverted.

**Implication for pane:** Composition mechanisms need to emerge naturally from the architecture rather than being opt-in features that applications must explicitly support. If pane's component embedding works through the same channels that normal inter-pane communication uses, the marginal cost of "making something embeddable" drops to zero. That's the difference between replicants (explicit opt-in) and Plan 9's filesystem interface (everything is already a file, so everything is already composable).

### Media Kit: preserved but modernized

The Media Kit's node-based architecture survived. BMediaNode, the producer/consumer/filter topology, shared-memory buffer passing, and the media_server coordinator are all present. The recent work consolidated functionality into three classes: BMediaRecorder, BMediaPlayer, BMediaFilter — reducing duplication while preserving the original design.

Key additions: automatic reconnection to the media_server after crashes (B_MEDIA_SERVICES_STARTED/B_MEDIA_SERVICES_QUIT notifications), progress callbacks for launch_media_server(), and integration with the launch_daemon for on-demand service startup.

The Media Kit's original design was BeOS's most ambitious subsystem and it has proven robust enough to survive reimplementation. The producer/consumer graph model for media processing was ahead of its time in the mid-1990s and remains a solid architecture.

**Implication for pane:** The media node graph is a natural fit for session-typed channels. Each connection between producer and consumer is a protocol: buffer format negotiation, flow control, error signaling. These are exactly what session types describe. Pane should study the Media Kit's connection protocol as input to its own media pipeline design.

---

## 3. What Had to Change

### Networking: complete replacement

BeOS's net_server was primitive — a single monolithic daemon handling all networking. Haiku replaced it with a modular kernel-level network stack resembling FreeBSD's design:

- Protocol chains are dynamically constructed per socket: Socket -> TCP -> IPv4 -> ARP -> Ethernet -> device
- Each address family (IPv4, IPv6) registers its own domain with protocol modules, interfaces, and routes
- net_buffer abstracts packets through 2048-byte shareable chunks
- net_protocol modules are socket-bound (adding headers, checksums)
- net_datalink_protocol modules are interface-bound (framing, deframing)

IPv6 was added via GSoC 2010, though finalization remained ongoing. The interface model was redesigned to support multiple addresses per interface (essential for IPv6, not needed for IPv4). WiFi support came through importing OpenBSD's WiFi stack and drivers, now supporting 802.11ac and Intel AX devices.

The TCP stack was rewritten for Beta 5, achieving 8-10x performance improvement through ACK coalescing, SACK, and window scaling.

**Implication for pane:** Pane dodges this entirely by running on Linux. Linux's network stack is one of the most battle-tested pieces of software ever written. This is the prototypical example of what pane avoids by not rebuilding the OS from scratch.

### Package management: invented from scratch

BeOS had no package management. This was one of its most painful omissions. Haiku's solution is architecturally innovative:

**HPKG format**: Archives optimized for fast random access. Contents are concatenated and compressed in 64 KiB chunks with a table of contents specifying each file's location. This enables mounting without full extraction.

**PackageFS**: A virtual filesystem that presents a union view of all activated packages. When you look at `/boot/system`, you're looking at the merged contents of hundreds of `.hpkg` files, all mounted read-only. Moving a package into the packages directory activates it live. Removing it deactivates it live. The filesystem hierarchy is immutable — you can't edit files in `/boot/system` directly.

**State management**: Before every transaction, packagefs creates a timestamped state directory with an `activated-packages` manifest. Users can boot into any previous state from the bootloader. This provides atomic rollback without the complexity of filesystem snapshots.

**Shine-through directories**: Special directories (settings, cache, var, non-packaged) that live on the underlying BFS volume rather than in packages. These handle the cases where mutability is essential — configuration files, caches, user-installed scripts.

**Separation of concerns**: The filesystem hierarchy separates system packages, user packages, and ported POSIX software into distinct trees with strict dependency ordering. System packages depend on nothing. User packages depend on system. This prevents user-installed software from breaking the boot process.

The design is conceptually similar to NixOS/Guix (immutable system state, atomic rollback) but implemented at the filesystem level rather than through symlink forests. It was designed years before NixOS became popular.

**Implication for pane:** Pane doesn't need its own package manager (it runs on Linux distributions that have them). But the philosophical idea — that system state should be declarative, immutable, and rollback-able — is worth studying. Pane could represent its own configuration and component state with similar properties. The "shine-through" pattern (mostly immutable + specific mutable escape hatches) is especially relevant for pane's settings management.

### POSIX compliance: gradual, ongoing, never complete

BeOS was not POSIX-compliant. Haiku has pursued POSIX compliance aggressively but incrementally:

- Libraries are reorganized (what's in libresolv elsewhere is in libnetwork on Haiku)
- exec() behavior adjusted to match POSIX for multi-threaded processes
- Signal masks preserved across forks (POSIX requirement, not BeOS behavior)
- POSIX-2024 functions being added as of late 2025
- SOCK_SEQPACKET, MSG_TRUNC, MSG_PEEK added for UNIX domain sockets
- Ongoing test suite runs identify and fix compliance gaps

Each porting effort (Go, Rust, .NET, major applications) surfaces new POSIX compliance issues. The Go port in November 2025 required multiple kernel changes. This is a never-ending treadmill.

The trade-offs: full POSIX compliance conflicts with BeOS's original API design in some areas. BeOS made deliberate choices that differed from POSIX (thread semantics, signal handling, process model). Haiku has generally chosen POSIX compatibility over BeOS purity when they conflict, because POSIX compatibility is what enables software porting.

**Implication for pane:** Pane runs on Linux, where POSIX compliance is the host's problem. But pane should be aware that the BeOS API design sometimes made choices that were better than POSIX for desktop use (the threading model, for instance). Where pane can improve on POSIX conventions while remaining compatible with them, it should.

### launch_daemon: replacing shell scripts with structured boot

BeOS used shell scripts for boot sequencing. This worked until Haiku's package management made the boot filesystem immutable — you could no longer customize boot scripts by editing files in place.

Axel Dorfler built the launch_daemon (2015), inspired by Apple's launchd and Linux's systemd but deeply integrated with Haiku's own infrastructure. Key design:

- **The kernel launches launch_daemon as the first userland process**
- Three concepts: **jobs** (one-shot), **services** (persistent), **targets** (groups triggered by events)
- Configuration via driver_settings format parsed into BMessages
- System servers launch in parallel without explicit dependencies
- Communication ports are pre-registered before processes start, so messages queue during startup
- On-demand launching: the print server's port exists at boot, but the actual server starts only when something sends a message to it
- Session management: when app_server creates a display session, launch_daemon forks a child with the user's UID

The configuration lives in `/system/data/launch/system` — a single file defining all system jobs, services, and targets. User-specific configuration goes in `/config/settings/launch/`.

Notable design choice: services don't need dependency declarations. Because communication ports are pre-established, any service can send messages to any other service's port immediately. The messages queue until the receiving service starts. This eliminates the dependency graph problem that plagues systemd units.

**Implication for pane:** The pre-registered port pattern is brilliant and directly applicable to pane's architecture. If pane-roster pre-creates communication channels for all known services, components can send messages immediately at startup without needing to wait for their dependencies to initialize. Messages just queue. This eliminates startup ordering as a concern.

### Layout API: filling BeOS's biggest GUI gap

BeOS had no layout management. GUI positioning was entirely manual — absolute pixel coordinates. This was painful even in the 1990s and became untenable with HiDPI displays, font size variations, and localization (text in different languages has different widths).

Haiku added the Layout API as a new layer on top of the existing Interface Kit:

- **BLayout** manages positioning of **BLayoutItem** objects
- **BGroupLayout** for horizontal/vertical stacking
- **BGridLayout** for grid placement
- **BCardLayout** for card/tab stacks
- **BSplitView** for user-adjustable dividers
- **BLayoutBuilder** provides a builder pattern for concise, chainable UI construction
- Recursive: a BLayout is itself a BLayoutItem, so layouts nest naturally

The API was designed by yourpalal and made public in 2010. It integrates with BView through an optional `SetLayout()` call — existing code using manual positioning continues to work.

The design philosophy: layouts handle positioning automatically in response to font changes, window resizing, and content changes. Applications specify relationships ("this goes next to that, this fills remaining space") rather than coordinates.

**Implication for pane:** This is a basic requirement for any modern GUI toolkit. Pane's Interface Kit must have layout management from day one — not as an afterthought bolted on decades later. The Haiku Layout API's recursive composition (layouts containing layouts) is the right model. The builder pattern for construction is a good ergonomic touch.

### Hardware support: the FreeBSD driver strategy

Haiku's kernel has its own driver framework, incompatible with Linux or FreeBSD drivers. Writing drivers from scratch for every piece of hardware would be impossible for a small team. The solution: a FreeBSD compatibility layer.

Starting in 2007, Hugo Santos built a generic compatibility layer that lets FreeBSD network drivers compile and run on Haiku with minimal modifications. The approach:

1. Copy the essential FreeBSD kernel functions that drivers call
2. Implement Haiku-specific versions of each function
3. FreeBSD drivers compile against the compatibility layer
4. Updates from FreeBSD upstream can be imported with minimal porting effort

This strategy has been expanded progressively. The ethernet and WiFi drivers are now from FreeBSD 15 (as of late 2025). The compatibility layer also covers USB networking. GPU drivers are a separate challenge — NVIDIA's open-source kernel driver was recently ported, and Intel GPU support comes through the accelerant interface.

Despite this, hardware support remains Haiku's biggest practical limitation. Multi-monitor support is experimental. GPU acceleration is limited. Many laptop features (power management, advanced touchpads) require per-device work.

**Implication for pane:** This is the single biggest reason pane is right to build on Linux rather than building its own OS. Linux's driver ecosystem represents millions of person-hours of work that no alternative OS project can replicate. Haiku's experience proves this — despite the FreeBSD compatibility layer being a clever shortcut, hardware support remains their most persistent challenge 25 years in.

---

## 4. Modern Display and Input Challenges

### HiDPI: late and incomplete

BeOS was designed for fixed-resolution CRT monitors. HiDPI support has been gradually added to Haiku, with improvements in Beta 4 and Beta 5. But the approach is font-size-based scaling rather than true resolution-independent rendering, and community members report the UI being "very tiny" on high-DPI screens even after adjustments.

The absence of compositing makes HiDPI harder — without per-window buffers, scaling operations need to happen at the drawing level rather than the compositing level.

**Implication for pane:** Wayland compositors handle HiDPI scaling at the protocol level. Pane gets per-output scale factors from Wayland. The challenge for pane is ensuring its Interface Kit's layout system and rendering pipeline handle fractional scaling cleanly from the beginning, not as a retrofit.

### Multi-monitor: barely functional

BeOS R5 had minimal multi-monitor support. Haiku has experimental support in some drivers, but applications aren't aware that the display spans multiple monitors. Windows can open in the gap between monitors. The Desktop class (`src/servers/app/Desktop.h`) handles multiple screens through the ScreenManager, but the integration is incomplete.

**Implication for pane:** Wayland's multi-output model is well-defined. Pane inherits proper multi-monitor support from the compositor (smithay). This is another major advantage of building on a modern graphics stack.

### Compositing: still missing after 15 years

Stippi's 2011 compositing plan laid out three phases: (1) per-window buffers, (2) a compositor merging them into the back-buffer, (3) BDirectWindow redirection. The plan was sound — it would eliminate expose-event repaints, enable alpha-blended effects, and potentially leverage GPU acceleration.

Fifteen years later, it's unimplemented. The app_server still uses a single shared framebuffer. All Painter objects attach to the same memory address. Drawing coordinates are converted to screen space. Expose events trigger full repaints.

The reason isn't that the plan was wrong — it's that the team is small and other infrastructure was more urgent (package management, networking, launch_daemon, POSIX compliance). Compositing is an optimization that makes everything look and feel better, but the system works without it.

**Implication for pane:** Pane starts composited (smithay). This single fact means pane's visual experience will be better than Haiku's from day one on the display front. But pane should learn from Haiku's deferred compositing — if a feature is "nice to have" but the system works without it, it will get deferred indefinitely. Pane's rendering pipeline should be built compositing-first, not compositing-maybe-later.

### Input methods: functional but fragmented

Haiku has IME support through the input_server add-on mechanism. B_INPUT_METHOD_AWARE applications can receive B_INPUT_METHOD_EVENT messages. CJK input methods exist (BeCJK, Mozc). But the system has separate mechanisms for keyboard layout switching and IME switching, and users have requested a unified input framework.

**Implication for pane:** Wayland's text-input protocol (zwp_text_input_v3) provides the IME infrastructure. Pane needs to ensure its Interface Kit passes IME events correctly through the session-typed channels. This is a "get it right from the start" area — retrofitting IME support into a text rendering pipeline is painful.

---

## 5. The Compatibility/Innovation Tension

### Binary compatibility: a ball and chain

Haiku R1's defining constraint is binary compatibility with BeOS R5 on 32-bit x86. This means:

- Class sizes must be identical (no adding data members)
- Virtual function table layouts must be identical (no adding/removing/reordering virtuals)
- Function names must match (same C++ mangling)
- Reserved virtual slots from BeOS must be consumed carefully
- When reserved slots run out, external C symbols with mangled names must redirect to new implementations via the Perform() pattern

This is maintained through heroic technical effort: tracking reserved slots, using pointer indirection for new data members (converting `fUnused` array entries into pointers to extension structs), and implementing the Perform() dispatch pattern for new virtual methods.

The 64-bit version drops binary compatibility (no BeOS R5 applications exist for x86_64) but maintains API compatibility. This is healthier — the API contract without the ABI constraint.

The gcc2 requirement for binary compatibility is particularly painful. Haiku maintains a "gcc2 hybrid" build system where the core libraries are compiled with gcc 2.95 (for BeOS binary compatibility) while modern software uses gcc 13+. This dual-compiler build is a significant engineering burden.

**Implication for pane:** Pane has no legacy binaries to support. This is pure advantage. Pane can design its ABI from scratch, using modern Rust conventions, without reserving vtable slots or maintaining struct size invariants. The lesson from Haiku is that binary compatibility is enormously expensive to maintain and should only be pursued when there's a compelling catalog of existing software to run. Pane has no such catalog.

### The OSnews argument: "Haiku isn't a BeOS successor anymore"

Thom Holwerda's OSnews article crystallizes a tension that has been building for years. The argument:

1. The original vision was native-first: applications written for the BeOS API, exploiting its unique features
2. Haiku has pragmatically embraced ported software via Qt, GTK (through Xlibe/Wayland compatibility layers), and POSIX compliance
3. The vast majority of modern, maintained software on Haiku is ported Qt/GTK applications
4. WebPositive (native browser) can't compete with ported browsers
5. This creates a vicious cycle: users prefer ports, developers stop writing native apps, the platform's unique character erodes

The community response splits three ways:
- **Pragmatists**: without ported applications, alternative OSes become unusable research projects
- **Purists**: ports compromise the platform's identity; the native experience is what matters
- **Realists**: a small team can't maintain a native ecosystem while competing with Linux's software base

**Implication for pane:** This is the most important lesson from Haiku's experience. Pane faces the same tension but in a different structural position:

Pane runs on Linux. It doesn't need to port applications — Wayland applications run natively. The "application gap" problem is structurally different. Pane's challenge isn't "how do we run Firefox" (it already does, via XWayland or native Wayland) but "how do we make applications that exploit pane's unique capabilities" — the session-typed communication, the attribute infrastructure, the composition model.

Haiku's experience suggests that the unique platform capabilities will only matter if they're nearly free for application developers to adopt. If using pane's typed channels is harder than using dbus, developers will use dbus. If pane's attributes require special-casing, developers will use SQLite. The path to adoption is making the right thing the easy thing.

### Xlibe and compatibility layers: pragmatism over purity

Haiku's approach to running X11/GTK software is instructive. Rather than running an X11 server (like XWayland on Linux), waddlesplash built Xlibe — an Xlib compatibility layer implemented directly on top of Haiku's native API.

Xlibe translates X11 API calls into Haiku API calls without a server intermediary. This gives several advantages over a server approach:
- Native window property translation (X11 windows become Haiku windows with proper decorations)
- Potential drag-and-drop interop between X11 and native applications
- No additional server process or protocol overhead

A Wayland compatibility layer followed, using modified libwayland with socket logic replaced by Haiku's native messaging. GTK3 was ported via this Wayland path.

The key insight: Xlibe was chosen because X11's API is "a relatively stable target" with decades of documentation, while Wayland's protocol-based design makes standalone library implementation impractical — Wayland requires full server implementations.

**Implication for pane:** Pane IS a Wayland compositor, so it gets Wayland application compatibility natively. For X11 applications, XWayland provides compatibility. Pane doesn't need Haiku's cleverness here because it's already on the right side of the ecosystem boundary. But the principle — meet applications where they are, then gradually provide native alternatives that are better — applies.

---

## 6. The Registrar and System Services

### How the registrar works

The registrar (`src/servers/registrar/`) is Haiku's central system service, managing:

- **Application roster**: registration, activation, info queries, lifecycle monitoring
- **MIME database**: type installation/deletion, icon/handler association, sniffer rules
- **Message runners**: scheduled message delivery (timer events)
- **Clipboard**: named clipboard management, change watching
- **Disk device monitoring**: device enumeration, change notifications
- **Authentication**: user authentication management
- **Package watching**: tracking package activation/deactivation for MIME database updates

The registrar is a BServer (inheriting from BApplication, which inherits from BLooper). All communication happens via BMessages through the standard messaging infrastructure. The registrar protocol is documented in Haiku's internals docs — each operation has a defined request message format and reply format.

Key design pattern: applications register with `B_REG_ADD_APP` providing signature, executable reference, team ID, thread ID, and port. The registrar assigns a token and can reject duplicate instances of single-launch applications. This is centralized application lifecycle management built on the same messaging infrastructure that everything else uses.

The MIME manager watches filesystem directories for changes when packages are activated/deactivated, automatically updating the MIME database caches. This is the "infrastructure composes naturally" pattern — package management, filesystem monitoring, and type registration all work through the same message-passing channels.

**Implication for pane:** pane-roster serves the same role as Haiku's registrar but with session-typed channels instead of untyped BMessages. The registrar's protocol (documented message formats with specific fields) is essentially a manually-specified session type. Pane makes this explicit and compiler-checked.

### How the launch_daemon composes with the registrar

The launch_daemon and registrar work together through pre-registered ports. When the system boots:

1. launch_daemon starts as the first userland process
2. It creates communication ports for all known services before starting any of them
3. Services start in parallel, each finding its pre-registered port ready
4. Messages sent to a service before it's fully initialized queue in the port
5. The registrar starts and takes over application lifecycle management
6. When app_server creates a display session, launch_daemon forks a child with the user's UID for session services

This eliminates startup dependency management. No service needs to declare "I depend on the registrar" because the registrar's port exists before the registrar itself starts. Messages just queue.

**Implication for pane:** This pre-registration pattern maps directly to pane's session-typed architecture. pane-roster can pre-create typed channel endpoints for all known services. Components connect to their channel endpoints immediately. The session type system guarantees that the protocol will be followed regardless of startup ordering.

---

## 7. The Ecosystem Challenge

### The application gap

Haiku's most painful ongoing challenge is the application gap. The native application ecosystem is small. The most-used software is ported:

- **Web browsers**: WebPositive (native, WebKit-based) is functional but struggles with JavaScript-heavy sites. Users use Falkon (Qt/WebEngine), and a GSoC 2024 project is migrating WebPositive from WebKitLegacy to WebKit2.
- **Office**: LibreOffice (ported)
- **Development**: Qt Creator, various text editors
- **Media**: MediaPlayer (native), ffmpeg-backed

HaikuPorts (the ports collection) has become the primary source of applications. Most months, HaikuPorts receives more commits than the OS itself. This is both a sign of platform stability (porters can work without hitting OS bugs) and a sign of ecosystem dependence (the platform's usable software comes from elsewhere).

The HaikuPorter build tool is strict by design — it catches undeclared dependencies, incorrect linking, and packaging errors. This produces high-quality packages but raises the barrier for casual porting efforts.

### Qt and GTK: making the ecosystem question structural

Qt was ported to Haiku in 2009 with a native backend. Qt's internal abstraction makes this feasible — Qt's platform abstraction layer is well-defined and Haiku is "just another platform." Qt applications on Haiku look and work well.

GTK was harder. GTK's internals are less abstracted. Rather than writing a native Haiku backend (which would require deep GTK surgery), the team pursued the Xlibe approach: emulate X11 directly on top of Haiku's API. Then the Wayland compatibility layer provided another path.

The result: Qt and GTK applications run on Haiku, but they don't use Haiku-native features (BFS attributes, replicants, the native file panel). They look like Linux applications transplanted onto Haiku's desktop. They work, but they don't compose with the platform.

**Implication for pane:** Pane faces a different version of this problem. Wayland applications will run on pane immediately — that's the point of being a Wayland compositor. But they won't use pane's unique capabilities (session-typed communication, typed file attributes, the pane composition model). The question is whether pane's unique capabilities can be exposed through Wayland protocol extensions, or through opt-in libraries that applications can adopt incrementally, rather than requiring a completely separate toolkit.

### WebPositive: the browser problem

The state of web browsing defines the usability of any alternative OS in 2026. WebPositive uses WebKit — specifically, it's stuck on WebKitLegacy (an older internal architecture). A GSoC 2024 project migrated to WebKit2, enabling per-tab process isolation, better crash resilience, and access to newer WebKit APIs (including ad-blocking).

But the fundamental problem is resource asymmetry. Browser engine development requires hundreds of engineers working continuously. Haiku has maybe a dozen active core developers. Keeping up with web standards, security patches, and performance optimizations is structurally impossible for a project this size.

Users work around this by using ported browsers (Falkon, eventually Firefox/Iceweasel through GTK/Wayland compatibility). But this means the most-used application on any modern desktop is a ported application that doesn't use the platform's unique features.

**Implication for pane:** Pane runs standard Linux browsers. Firefox, Chromium, and others work via XWayland or native Wayland. This is not a problem pane needs to solve. The lesson from Haiku is: never try to build your own browser engine. Use the ones that exist.

---

## 8. Architectural Lessons: What Proved Prescient, What Proved Naive

### Prescient

**The threading model.** Per-component message loops with actor-style isolation. BeOS was right about this in 1995 and it's still right in 2026. Modern systems (Erlang/OTP, Akka, Swift actors) have converged on the same pattern. The fact that Haiku's reimplementation of this model is essentially unchanged after 25 years is the strongest possible validation.

**BFS attributes and queries.** The idea that files should carry rich typed metadata and that the filesystem should be queryable like a database was prescient. Modern systems approximate this with Spotlight (macOS), GNOME Tracker, or application-level databases, but none integrate it as deeply as BFS. The email-as-files-with-attributes pattern remains the cleanest email UX design that's ever been implemented.

**Infrastructure-first design.** The kit hierarchy — where the messaging layer is the foundation, everything builds on top of it, and BMessage is the universal data format — produced a system where new functionality composed naturally with existing functionality. The registrar can monitor packages because packages are filesystem events and filesystem events are messages. The launch_daemon can manage services because services communicate through ports and ports carry messages. Nothing needed special integration because everything was already integrated through the messaging layer.

**The per-window server thread.** Having a dedicated server-side thread per window (ServerWindow) enables true parallelism in rendering. View clipping calculations happen independently per window, scaling naturally with CPU count. This was designed for dual-processor BeBoxes in 1996 and scales to 64-core machines today.

**The Media Kit's producer/consumer graph.** Node-based media processing with shared-memory buffer passing was ahead of the industry in 1997. PipeWire (2020s) essentially rediscovered the same architecture for Linux.

### Naive or insufficient

**Single-user security model.** BeOS was designed for a world where computers had one user and weren't networked. Haiku inherits this — you run as root, there's no privilege separation, no application sandboxing, no capability system. The community acknowledges this is a blocker for enterprise or security-sensitive use. Plans for R2 include basic multi-user support, but the single-user assumption is deeply embedded in the architecture.

**No package management.** Be, Inc. shipped a commercial OS without package management. Users installed software by dragging folders. This was charming in 1998 and untenable by 2005. Haiku spent years building what Be should have built — and their solution (packagefs) is arguably more innovative than anything on Linux.

**No layout management.** Manual pixel positioning of GUI elements was a product of the era (1996) but should have been recognized as a problem earlier. The Layout API took years to design, implement, and migrate existing applications to. Starting from scratch, there's no excuse for not having layout management from day one.

**Networking.** BeOS's net_server was primitive even by 1998 standards. The networking stack was the weakest subsystem and required a complete replacement in Haiku. The modular, kernel-level stack that replaced it is solid but consumed years of development effort.

**No compositing.** BeOS's rendering model (shared framebuffer, painter's algorithm for overlapping windows) was standard for 1996. But the absence of compositing means every window overlap or movement triggers full repaints. This is visible performance overhead that modern compositing eliminates. The fact that Haiku still hasn't implemented compositing in 2026 shows how hard it is to retrofit after the fact.

### What Haiku developers would do differently

Based on forum discussions and project direction threads:

1. **Not maintaining gcc2 compatibility.** The dual-compiler build system for BeOS binary compatibility is enormously costly. If starting over, they'd target API compatibility only (which the 64-bit version already does).

2. **Compositing from the start.** The shared-framebuffer model should have been replaced early, when the code was still young enough to restructure.

3. **Multi-user from the start.** Retrofitting privilege separation and user isolation into a single-user system is harder than designing it in from the beginning.

4. **Clearer R1/R2 boundary.** The project struggled with scope — how much to add beyond BeOS R5 compatibility before calling it R1. The ticket count for R1 went from 2,183 in 2015 to 600 in 2024, but new issues were created nearly as fast as old ones were resolved, suggesting systemic prioritization challenges.

---

## 9. Haiku Innovations (Not From Original BeOS) That Pane Should Consider

### PackageFS's immutable-state model

The idea that the system volume is a read-only projection of activated packages, with atomic state transitions and boot-time rollback, is genuinely innovative. Pane won't implement its own package management, but the principle — configuration as declarative state, with rollback — applies to pane's own settings and component management.

### The pre-registered port pattern

Launch_daemon's approach of creating communication endpoints before services start, allowing messages to queue, eliminates startup dependency management as a concern. This maps directly to pane's session-typed channels and is a better solution than systemd-style dependency graphs or socket activation.

### Stack and Tile

Haiku's Stack and Tile feature (present in the source at `src/servers/app/stackandtile/`) allows windows to be grouped by stacking (tabs) or tiling (side-by-side with shared edges). It's integrated into the app_server and enabled by default since Alpha 3.

The implementation uses SATGroup, SATWindow, and separate Stacking/Tiling modules. When windows are dragged near each other, they snap into groups. Groups can be stacked (appearing as tabs on a single window frame) or tiled (sharing edges so they resize together).

The unsolved problem: group persistence. Stack and Tile groups don't survive across reboots because there's no API for storing and restoring application window state. A new state management API has been proposed but not implemented.

**Implication for pane:** Window grouping (tiling and tabbing) should be built into pane's window management from the start, not added as a feature later. The persistence problem (saving and restoring window arrangements) is worth solving early — it requires a session management protocol between the compositor and applications.

### Xlibe's approach to compatibility

Rather than running a separate server process, implementing the foreign API directly on top of the native API. This gives better integration (native window decorations, potential drag-and-drop interop) than a server-based approach. For pane, this principle might apply to future compatibility needs — if pane ever needs to support a non-Wayland protocol, implementing it as a library translation layer rather than a separate server is the better approach.

### FreeBSD driver compatibility layer

The strategy of importing an entire driver ecosystem through a compatibility shim rather than writing drivers from scratch. Pane doesn't need this (it's on Linux), but the principle — leverage existing ecosystems through thin translation layers rather than reimplementing — applies broadly.

---

## 10. What Pane Learns from Haiku's Journey

### Things pane avoids by being on Linux

1. **Kernel development.** Haiku has spent 25 years on kernel work: scheduler, memory management, SMP, device drivers. Pane uses Linux's kernel. This alone saves decades of effort.

2. **Hardware drivers.** Despite the FreeBSD compatibility layer, Haiku's hardware support is limited. Pane gets Linux's driver ecosystem.

3. **Network stack.** Haiku spent years building a modern network stack. Pane uses Linux's.

4. **Filesystem implementation.** BFS reimplementation was a major effort. Pane uses existing Linux filesystems plus xattr for metadata.

5. **POSIX compliance.** Haiku wrestles with POSIX compliance continuously. Linux is the reference implementation.

6. **Browser engine.** WebPositive's perpetual catch-up race doesn't exist for pane. Standard browsers run natively.

7. **Compositing.** Fifteen years of deferred compositing. Pane starts composited via smithay/Wayland.

### Things pane should replicate from Haiku's experience

1. **Message passing as the universal substrate.** BMessage is everywhere in Haiku. Pane's session-typed channels should be equally pervasive. If there's ever a case where pane components communicate through shared memory, global variables, or ad-hoc IPC, the design has gone wrong.

2. **The kit hierarchy.** Coherent domain groupings with clean dependency ordering. Not a flat collection of libraries, but a layered ecology where each kit builds on the ones below it.

3. **Infrastructure first.** Haiku's Beta 1 came 17 years after the project started, because the team built infrastructure before features. Pane's first milestone should be the messaging infrastructure working correctly, not the first pretty window.

4. **The pre-registered port pattern.** Create communication endpoints before the services that use them exist. Let messages queue. Eliminate startup ordering as a concern.

5. **The registrar pattern.** A central service that manages application lifecycle, type associations, clipboard, and system coordination — built on the same messaging infrastructure as everything else.

6. **The Translation Kit pattern.** Format conversion as a system service, not a per-application responsibility. Applications work with interchange formats; translators handle the rest.

### Things pane should learn from Haiku's mistakes

1. **Don't defer compositing.** Build the rendering pipeline compositing-first. Retrofitting compositing after building an entire rendering system without it is a decade-long project that may never happen.

2. **Don't defer layout management.** Haiku's Layout API came years after the Interface Kit. Every application written before it existed had to be manually migrated. Build layout management into the Interface Kit from the beginning.

3. **Don't defer multi-user.** Haiku's single-user assumption is embedded throughout the system. Pane runs on a multi-user OS (Linux), so it inherits multi-user support, but pane should ensure its own abstractions (settings, sessions, per-user state) are multi-user-aware from the start.

4. **Don't build your own browser.** Use the ones that exist.

5. **Make the right thing the easy thing.** Replicants failed because they required explicit opt-in. BFS attributes are underused because ported software ignores them. Pane's unique capabilities need to be the path of least resistance, not an extra step. If using pane's typed channels requires more code than using dbus, developers will use dbus.

6. **Have a clear scope.** Haiku's R1 milestone has been shrinking for a decade but never reaching zero. Define what "done" looks like before starting, and resist scope creep.

### Things that need updating for 2026

1. **Security model.** BeOS's "no security, single user, trust everything" model is unacceptable in 2026. Pane doesn't need full sandboxing in v1, but it should have a capability model in the architecture from the start, even if enforcement comes later.

2. **Accessibility.** BeOS had no accessibility features. Haiku's accessibility support is minimal. Pane should engage with the AT-SPI protocol and accessibility tree from early in the Interface Kit design.

3. **Theming and appearance.** BeOS's fixed appearance was part of its charm but modern desktop users expect theming. Pane should have a clean separation between rendering logic and appearance data.

4. **Modern display requirements.** HiDPI, fractional scaling, variable refresh rate, multiple monitors with different scales and orientations. All of these are table stakes in 2026 and all are poorly handled by Haiku. Pane gets many of these from Wayland but needs to handle them correctly in its own rendering layer.

5. **Input methods.** CJK input, voice input, accessibility input methods. These need to be integrated into the text input pipeline from the start, not added as an afterthought.

---

## 11. How Haiku's Experience Informs Pane's Architecture

### The fundamental strategic difference

Haiku is rebuilding BeOS faithfully, including the parts that weren't good. Pane is taking BeOS's good ideas and building them on modern infrastructure. This changes everything:

- Haiku maintains binary compatibility with 1998 software. Pane has no legacy.
- Haiku builds its own kernel. Pane uses Linux.
- Haiku builds its own display server. Pane builds on Wayland/smithay.
- Haiku builds its own network stack. Pane uses Linux's.
- Haiku retrofits modern features onto a 1990s architecture. Pane builds a 2026 architecture informed by 1990s ideas.

The result: pane can focus entirely on the desktop experience layer — the messaging infrastructure, the kit hierarchy, the rendering pipeline, the composition model — without spending years on the infrastructure that Linux already provides.

### What survives the transplant from Haiku to pane

1. **The actor model concurrency discipline** — messages in, messages out, no shared mutable state across components. Pane strengthens this with session types.

2. **The centralized rendering server** — the app_server pattern where rendering is a service, drawing commands flow from client to server, and the server owns the display. Pane implements this through Wayland but the conceptual separation is the same.

3. **The registrar pattern** — centralized application lifecycle, type management, and system coordination as a message-driven service.

4. **The kit hierarchy** — domain-coherent groupings with clean layering and BMessage (or its pane equivalent) as the universal data format.

5. **Infrastructure-first design** — the messaging layer is the foundation, everything else builds on top.

### What doesn't survive

1. **Binary compatibility constraints** — no vtable slot reservation, no struct size invariants, no gcc2.
2. **The shared-framebuffer rendering model** — pane starts composited.
3. **The single-user assumption** — pane inherits Linux's multi-user model.
4. **Manual pixel positioning** — pane's Interface Kit has layout management from day one.
5. **The BMessage type system** — untyped `uint32` message codes are replaced by session types that the compiler checks.

Haiku's 25-year journey is the closest existing precedent to what pane is attempting. Their successes validate pane's core design choices (message passing, kit hierarchy, infrastructure-first). Their struggles — compositing, hardware, browsers, the native/ported software tension — are exactly the problems that pane avoids by building on Linux. And their innovations — packagefs, launch_daemon's pre-registered ports, Stack and Tile — offer specific design ideas that pane can adopt and improve upon.
