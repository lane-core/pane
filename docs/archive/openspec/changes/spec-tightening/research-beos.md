# BeOS/Haiku Research — Tasks 1.1–1.8

Research for pane spec-tightening. Primary sources: Haiku API documentation (haiku-os.org/docs/api/), the Be Book (haiku-os.org/legacy-docs/bebook/), Be Newsletters, "Practical File System Design" (Giampaolo, 1998), OSnews articles on BeOS architecture, Haiku developer documentation.

---

## 1.1 Haiku API Kits — Architecture Overview

### The kit structure

The BeOS/Haiku API is organized into "kits" — coherent groupings of classes that each address a domain. The kits are not libraries in the Unix sense (collections of unrelated functions); they are cohesive subsystems with internal design consistency and well-defined relationships to each other. The major kits:

**Application Kit** — the foundation. Provides the messaging infrastructure (BMessage, BLooper, BHandler, BMessenger), the application lifecycle (BApplication, BRoster), and the system interaction surface (BClipboard, BNotification). Every GUI application starts here. The kit's design centers on one premise: everything communicates via asynchronous messages dispatched through per-thread message loops.

**Interface Kit** — the GUI. BWindow, BView, BControl, and the Layout API. The critical architectural fact: BWindow inherits from BLooper, so every window has its own thread running its own message loop. The Interface Kit does not exist independently of the Application Kit — it is built on top of the messaging infrastructure. Event handling, drawing, and user interaction are all message-driven.

**Storage Kit** — the filesystem abstraction. BNode, BFile, BDirectory for data access; BQuery for attribute-based search; BNodeInfo and BMimeType for the MIME type system; NodeMonitor for change notification. The Storage Kit treats the filesystem not as a dumb byte store but as a structured, queryable, typed data layer.

**Support Kit** — foundational utilities. BArchivable/BArchiver/BUnarchiver for object serialization into BMessages; BLocker/BAutolock for thread-safe locking; BFlattenable for byte-stream serialization; BDataIO/BPositionIO for stream abstraction. The Support Kit provides the serialization and threading primitives that other kits depend on.

**Translation Kit** — format conversion as system service. BTranslatorRoster discovers and mediates between translator add-ons. Applications never need to know about specific file formats — they work with a common interchange format (B_TRANSLATOR_BITMAP for images, B_STYLED_TEXT_FORMAT for text), and translators handle conversion to/from specific formats. Extensibility by dropping add-ons into a directory.

**Other kits:** Mail Kit (email protocol handling and attribute-based message storage), Media Kit (unified streaming media with producer/consumer topology), Network Kit, Locale Kit, Device Kit, Game Kit.

### Why this structure matters

The kits form a layered dependency graph, not a flat collection:

```
         Interface Kit
              |
        Application Kit
         /          \
   Support Kit    Storage Kit
```

The Application Kit's messaging infrastructure is the spine. The Interface Kit builds on it (windows are loopers). The Storage Kit provides the data layer. The Support Kit provides serialization and threading primitives used everywhere. The Translation Kit bridges Storage Kit data (files) with Interface Kit rendering (bitmaps) through Application Kit messaging (BMessage-based configuration).

This is not accidental. The kits were designed as a coherent ecology, not as independent modules bolted together. The BMessage type is the lingua franca: it carries UI events, IPC, serialized objects, query results, translator configurations, and clipboard data. A single data type, used everywhere, for everything.

---

## 1.2 BMessage/BLooper/BHandler — The Threading Model

### The architecture

Three classes form the concurrency core:

**BMessage** is the protocol unit. It carries a `what` field (uint32 identifying the message type) and zero or more typed name-value pairs. Data is stored with explicit type codes (`B_FLOAT_TYPE`, `B_INT32_TYPE`, `B_STRING_TYPE`, etc.) — every name is associated with exactly one type, preventing type confusion at the protocol level. Multiple values can share the same name (array semantics). Messages can be flattened to byte streams (for persistence, network transmission, or cross-process delivery) and unflattened back.

Beyond payload, BMessage carries delivery metadata: whether the sender expects a reply (`IsSourceWaiting()`), whether the source is remote (`IsSourceRemote()`), a return address for replies (`ReturnAddress()`), drag-and-drop context (`WasDropped()`, `DropPoint()`). The `SendReply()` methods support both asynchronous and synchronous request-response patterns with configurable timeouts.

**BLooper** is a thread with a message queue. When `Run()` is called, it spawns a thread that continuously receives messages from a port, passes them through filters, and dispatches them to handlers. The message loop:

1. Retrieve message from port (blocks until available)
2. Apply common filters (BMessageFilter objects)
3. Select handler: explicitly targeted handler → preferred handler → the looper itself
4. Call `DispatchMessage()`, which can be overridden by subclasses for interception
5. The handler's `MessageReceived()` processes the message

Locking: BLooper uses recursive semaphore-based locking. `Lock()` calls stack within a single thread (no self-deadlock), and every `Lock()` must be paired with an `Unlock()`. The lock protects the looper's internal state — you must hold it to modify handler chains, filters, or other looper data.

**BHandler** processes messages. It implements `MessageReceived()`, which examines the `what` field and acts accordingly. Handlers form chains within a looper — if a handler doesn't recognize a message, it calls the base class implementation, which passes it to the next handler in the chain (set via `SetNextHandler()`). This is a chain-of-responsibility pattern: each handler has self-contained logic for the messages it understands, and unknown messages propagate.

BHandler also supports an observer pattern: `SendNotices()` emits state-change notifications to subscribed observers, enabling decoupled state synchronization without direct messaging.

### The critical design decision: one thread per window

BWindow inherits from BLooper. Every window runs in its own thread with its own message loop. BApplication also inherits from BLooper, running the application's main message loop in a separate thread. An application with three windows has four threads: one for the application, one per window.

Additionally, each window has a _second_ thread running inside the application server (app_server), handling graphics updates. So the rendering thread and the event-handling thread are separate. Drawing never blocks on event processing and vice versa.

The scheduler was designed around this. The BeOS microkernel scheduled tasks preemptively with very short time slices — three-thousandths of a second. All tasks were allowed use of a processor for only this brief window before preemption. The scheduler favored responsiveness: real-time and UI threads got priority. Media threads had dedicated priority levels. The system was designed for SMP, with unrelated threads naturally distributing across processors.

### Why pervasive multithreading produced stability rather than chaos

The common expectation is that more threads means more race conditions, more deadlocks, more unpredictable behavior. BeOS contradicted this. The explanation is not that threading is inherently stable, but that BeOS's _specific discipline_ made it stable:

**1. Message passing eliminated shared mutable state.** Cross-looper communication happened exclusively through asynchronous BMessage posting. Loopers didn't share data — they exchanged messages. Each looper processed messages sequentially within its own thread. This is the actor model in practice: each looper is an actor with a mailbox, processing one message at a time.

**2. Self-contained operational semantics at every boundary.** Each handler implemented `MessageReceived()` with logic that depended only on the message contents and the handler's own state. A handler didn't need to know about the global state of the system — only about the messages it received and the side effects it produced. This is the "operational semantics of the OS implementing routines for handling operations in the abstract" that the designer described.

**3. The protocol WAS the coordination mechanism.** There was no global coordinator saying "handler A runs before handler B" or "window 1 gets priority over window 2." Instead, each component followed the message protocol: receive a message, process it, optionally send replies or new messages. Stability was emergent from the protocol discipline, not imposed top-down.

**4. Isolation contained failures.** A window thread hanging on a long computation didn't freeze other windows. A misbehaving handler in one looper didn't corrupt another looper's state. The one-thread-per-window model meant that failure domains were small and isolated. The user could close a frozen window without affecting the rest of the system.

**5. The scheduler cooperated with the architecture.** Because the system had many fine-grained threads (not a few monolithic processes), the scheduler could make fine-grained decisions about which threads to run. UI threads got short time slices but high priority. Compute threads got lower priority. The system could stay responsive even under heavy load because the scheduler had enough thread granularity to keep the right things running.

### What this means

The stability wasn't despite the complexity of pervasive multithreading — it was _because_ the threading model forced a discipline that happened to eliminate the common sources of instability. Message passing prevented shared state corruption. Per-handler operational semantics prevented global state entanglement. The protocol replaced the global coordinator. The scheduler had enough granularity to maintain responsiveness.

The counterargument — and it was real — was that porting existing single-threaded applications was difficult. Developers had to restructure around message passing. Large applications like Steinberg's Nuendo required adding synchronization layers. And BeOS could lose messages when a thread's message queue filled faster than it could process them. But for applications designed natively for the model, the result was remarkable stability and responsiveness.

### How BMessage enabled loose coupling

BMessage is not an RPC mechanism. There's no function pointer, no method call, no tight binding between sender and receiver. The sender constructs a BMessage with a `what` code and typed data, addresses it via BMessenger (which identifies a target by team ID + looper + handler token), and posts it. The sender doesn't know or care:

- What the receiver will do with the message
- Whether the receiver is in the same process or a different one
- Whether the receiver is local or remote
- What handlers are registered in the target looper
- Whether the message will be filtered, transformed, or discarded

The receiver, conversely, doesn't know or care who sent the message. It examines `what` and the data fields and acts accordingly. The `what` field is a convention, not a binding — any handler can process any message type.

This loose coupling is what made BeOS applications composable. Drag-and-drop was just sending a BMessage with drag data to whatever view the mouse was over. Clipboard was just posting a BMessage to the clipboard looper. Scripting was just sending BMessages with scripting commands. The same mechanism served all inter-component communication.

---

## 1.3 Translation Kit — Plugin Model

### Architecture

The Translation Kit implements format conversion as a system-wide service. Three components:

**Translator add-ons** are shared libraries that live in `~/config/add-ons/Translators/` (user) or `/boot/beos/system/add-ons/Translators/` (system). Each add-on exports:

Required functions:

- `Identify()` — examine a data stream and determine if the translator can handle it. Must read as little data as possible ("do not read more data than you need to make an educated guess" — critical for network streams where reading means downloading).
- `Translate()` — convert data from the input stream to the output stream in the specified format.

Required data:

- `translatorName` — short identifier for menus
- `translatorInfo` — description with authorship
- `translatorVersion` — version number (MM.mm encoding, e.g., 314 = v3.14)
- `inputFormats[]` and `outputFormats[]` — arrays of `translation_format` structs declaring supported conversions

Optional functions:

- `MakeConfig()` — create a BView for user-configurable translator settings (e.g., JPEG quality slider)
- `GetConfigMessage()` — serialize current settings into a BMessage for programmatic access

Each `translation_format` struct contains:

- Type code (e.g., `B_JPEG_FORMAT`, `B_PNG_FORMAT`)
- Group (e.g., `B_TRANSLATOR_BITMAP`, `B_TRANSLATOR_TEXT`)
- Quality rating (0.0–1.0): how well the translator handles this format
- Capability rating (0.0–1.0): how reliably the conversion works
- MIME type string
- Human-readable name

### Quality-based selection

When multiple translators can handle the same input, BTranslatorRoster selects by multiplying quality and capability ratings. The highest combined score wins. This means translators self-declare their competence, and the system uses these declarations for routing without any central registry of "preferred translators."

Quality and capability for the _output_ format are not considered during identification — only input analysis matters for translator selection. This separates "can I understand this data?" from "can I produce that format?"

### The common interchange format

The genius of the design: translators don't convert between arbitrary format pairs. They convert between a _specific format_ and a _common interchange format_. For images, this is `B_TRANSLATOR_BITMAP` — a simple 32-byte header followed by raw pixel data (bounds, row bytes, color space, size). For text, it's `B_STYLED_TEXT_FORMAT`.

This means:

- A JPEG translator converts JPEG ↔ B_TRANSLATOR_BITMAP
- A PNG translator converts PNG ↔ B_TRANSLATOR_BITMAP
- A TIFF translator converts TIFF ↔ B_TRANSLATOR_BITMAP

An application that knows how to render B*TRANSLATOR_BITMAP can display \_any* image format, because the Translation Kit mediates the conversion. Adding a new format means adding one translator — not modifying every application.

The number of translators needed is linear (one per format), not quadratic (one per format pair). This is the key scalability insight.

### How "any app could read any format" worked in practice

An application that wanted to open an image file:

1. Called `BTranslatorRoster::Identify()` with the file data
2. The roster tried each installed translator's `Identify()` function
3. The translator with the highest quality×capability score was selected
4. The application called `BTranslatorRoster::Translate()` with the selected translator
5. The translator converted the data to B_TRANSLATOR_BITMAP
6. The application rendered the bitmap

The application never needed to know that the file was JPEG or PNG or TIFF. It just asked "what is this?" and "give me a bitmap." When a user installed a new translator (by dropping a shared library into the Translators directory), every application immediately gained the ability to read that format.

Similarly, a "Save As" dialog could enumerate all installed translators and offer every supported output format. The application wrote B_TRANSLATOR_BITMAP; the selected translator converted to the target format.

### Design principles

1. **Discovery by convention:** Translators are found by scanning well-known directories. No registration, no manifest, no database. Drop a file, it works.

2. **Quality as routing:** Self-declared quality ratings enable automatic selection without central authority. Multiple translators for the same format coexist peacefully — the best one wins.

3. **Stream-oriented:** Translators operate on BPositionIO streams, not files. This means they work on memory buffers, network streams, clipboard data, or any other seekable byte source.

4. **Configuration as BMessage:** Translator settings are serialized into BMessages, making them storable, transmissible, and introspectable using the same machinery as everything else in the system.

---

## 1.4 BFS Attributes and BQuery — Metadata as First Class

### Extended attributes on BFS

BFS (the Be File System) was designed by Dominic Giampaolo and Cyril Meurillon over ten months starting September 1996. It was a 64-bit journaled filesystem built from scratch for BeOS, but its defining feature was the attribute system.

Every file on BFS can carry an arbitrary number of extended attributes — name-value pairs stored as metadata alongside the file. Unlike Unix extended attributes (which are opaque byte blobs), BFS attributes are **typed**: each attribute has an explicit type code (B_STRING_TYPE, B_INT32_TYPE, B_FLOAT_TYPE, B_DOUBLE_TYPE, etc.). The filesystem knows the type, not just the bytes.

The BNode class provides the API:

- `WriteAttr(name, type_code, offset, buffer, length)` — write typed data
- `ReadAttr(name, type_code, offset, buffer, length)` — read typed data
- `GetAttrInfo(name, attr_info*)` — get type and size
- `GetNextAttrName(buffer)` — iterate all attributes on a node
- `RemoveAttr(name)` — delete an attribute
- `Lock()`/`Unlock()` — exclusive access for transactional consistency

### Attribute indexing

BFS can index attributes. An index is a filesystem-global B+ tree keyed on a specific attribute name. Once indexed, an attribute's values are efficiently searchable across all files on the volume.

Three attributes are indexed by default (free on every file):

- `name` — the filename
- `size` — 64-bit data size
- `last_modified` — modification timestamp (seconds since epoch)

Additional indices are created with `fs_create_index()` (or `mkindex` from the shell). Indices only track attributes written _after_ index creation — existing attributes are not retroactively indexed. Indexed attribute values are limited to 255 bytes.

Only string and numeric types are indexable. The B+ tree implementation is the same one used for directory entries, reused for attribute indices.

### BQuery — filesystem as database

BQuery executes searches against indexed attributes. The lifecycle:

1. **Set volume:** `SetVolume()` — queries target a specific BFS volume
2. **Set predicate:** `SetPredicate("MAIL:status == New")` — a logical expression comparing attributes to values
3. **Fetch:** `Fetch()` — executes the query asynchronously
4. **Read results:** `GetNextEntry()`/`GetNextRef()`/`GetNextDirents()` — iterate matching files

The predicate language supports:

- Comparison: `=`, `<`, `>`, `<=`, `>=`, `!=`
- Logic: `||`, `&&`, `!`, parentheses
- Wildcards: `*` (prefix/suffix only)
- Quoting: single quotes for values with spaces
- Date parsing: natural language dates ("yesterday", "last Friday", "9 days before yesterday")

Alternatively, predicates can be built programmatically using Reverse Polish Notation: `PushAttr("MAIL:from")`, `PushString("rob@plan9.bell-labs.com")`, `PushOp(B_EQ)`.

**Every query must include at least one indexed attribute.** This is not a limitation — it's an optimization constraint. The query engine uses the index to narrow the search before evaluating the full predicate.

### Live queries

Static queries execute once and return a fixed result set. Live queries continue monitoring after the initial fetch, delivering update messages when files enter or leave the result set.

To make a query live: call `SetTarget(BMessenger)` before `Fetch()`. The BMessenger identifies the BHandler/BLooper that will receive update messages.

Live query updates are delivered as BMessages with `what = B_QUERY_UPDATE`:

**B_ENTRY_CREATED** (a file now matches the predicate):

- Fields: `opcode`, `name` (string), `directory` (int64), `device` (int32), `node` (int64)
- The application can construct an `entry_ref` and a `node_ref` from these fields

**B_ENTRY_REMOVED** (a file no longer matches):

- Fields: `opcode`, `directory`, `device`, `node`
- Notably lacks the `name` field — the application must track names itself from previous CREATED messages

The update messages follow the same format as node monitor messages (same opcodes, same fields) — only the `what` field differs. This means existing node monitoring code can handle query updates with minimal adaptation.

Updates can start arriving immediately after `Fetch()`, even before all static results have been retrieved via `GetNext*()`. Applications must synchronize iteration with message processing or accept that "entry dropped out" messages may arrive for entries not yet seen.

### Node monitoring — the reactive layer

The node monitoring system provides general-purpose filesystem change notification. Applications register via `watch_node()` with a node reference, flags, and a target BMessenger. Monitored events:

- `B_WATCH_NAME`: entry renamed or moved → `B_ENTRY_MOVED` message
- `B_WATCH_STAT`: metadata changed → `B_STAT_CHANGED` message
- `B_WATCH_ATTR`: attributes changed → `B_ATTR_CHANGED` message
- `B_WATCH_DIRECTORY`: entries created/removed in a directory → `B_ENTRY_CREATED`/`B_ENTRY_REMOVED`
- `B_WATCH_MOUNT`: devices mounted/unmounted → `B_DEVICE_MOUNTED`/`B_DEVICE_UNMOUNTED`

The kernel constructs and delivers notification BMessages directly, without going through the Application Kit's normal messaging infrastructure — it uses port/token pairs to address the target BHandler/BLooper directly.

Live queries use the same underlying mechanism. When a file's attributes change, the kernel evaluates the change against all registered live queries and sends appropriate B_ENTRY_CREATED or B_ENTRY_REMOVED messages to query targets. The notification is synchronous in the context of the triggering thread — the filesystem operation doesn't return until all listeners have been notified.

Haiku consolidates duplicate watches: calling `watch_node()` multiple times on the same node from the same target uses only one monitor slot. BeOS would send one message per `watch_node()` call.

### How metadata-as-first-class changed the UX

The traditional approach: metadata lives in application databases. Your email client has a database of headers. Your music player has a database of tags. Your contacts app has a database of fields. Each application maintains its own index, its own search, its own data model.

The BFS approach: metadata lives on the files themselves, as typed attributes. The filesystem indexes them. Any application can read any file's attributes. Queries work across all files regardless of which application created them.

Concrete effects:

1. **No import/export.** When mail_daemon downloaded an email, it wrote the file and its attributes. There was no "import into mail client" step. The file existed; the attributes existed; any application could see them immediately.

2. **Cross-application search.** A Tracker query for `MAIL:from == "rob"` found all emails from Rob. The same query mechanism that found files by name could find emails by sender, songs by artist, contacts by city.

3. **Shell scriptability.** From the command line: `catattr MAIL:subject /boot/home/mail/inbox/*` to list all email subjects. `addattr -t string META:city "Cambridge" ~/contacts/rob` to tag a contact. `query "META:city == Cambridge"` to find all contacts in Cambridge. The `listattr`, `catattr`, `addattr`, `rmattr` tools made attributes a first-class part of the shell workflow. The `query` command made the database accessible from scripts.

4. **Universal Tracker columns.** Tracker (the file manager) could display any attribute as a column in any directory listing. Email directories showed From, Subject, Date, Status columns. Music directories showed Artist, Album, Track columns. Contact directories showed Name, Company, Phone columns. All from the same Tracker code — it just read different attributes per file type.

5. **User-defined metadata.** Users could add custom attributes to any file and create custom indices. A photographer could add `PHOTO:location` attributes, index them, and find all photos from a location with a query.

---

## 1.5 Replicant/BArchivable System

### How it works

A replicant is a BView that can serialize itself, be transmitted across process boundaries, and reconstruct itself inside a host application. Four components:

**BArchivable** (Support Kit): provides the `Archive()` method (serialize object state into a BMessage) and the static `Instantiate()` method (reconstruct an object from a BMessage). The archive BMessage includes a `class` field identifying the object's class and an `add_on` field specifying the application or add-on that contains the class's code.

**BDragger**: a small handle view (the "dragger") attached to a replicable BView. When the user grabs the dragger, it triggers `Archive()` on the associated BView, wrapping the serialized state in a BMessage with `what = B_ARCHIVED_OBJECT`.

**BShelf**: a view that accepts dropped replicants. When it receives a B_ARCHIVED_OBJECT message, it:

1. Reads the `add_on` field to identify the originating application
2. Loads the application or add-on as a shared library (via the image loading system)
3. Finds the `Instantiate()` function for the archived class
4. Calls `Instantiate()` to reconstruct the BView from the archived BMessage
5. Adds the reconstructed view as a child of the shelf's container view

**The code loading**: this is the critical mechanism. When a replicant is instantiated in a host application, the host loads the replicant's originating binary (or a dedicated add-on library) into its own address space. The replicant's code _runs inside the host process_. This is not cross-process rendering — it's cross-process code injection via shared library loading.

### The protocol

Creating a replicant requires four things:

1. The BView must have a BDragger attached (as child, parent, or sibling)
2. `Archive()` must serialize all member variables into a BMessage, including an "add_on" field with the app's signature
3. A constructor accepting a single BMessage argument must exist (for deserialization)
4. A static `Instantiate()` method must reconstruct the view from a BMessage

Shelves validate incoming replicants: they compare the shelf's name against the dropped message's `shelf_type` field (if present). A replicant can be picky about which shelves it lives on. The shelf's `CanAcceptReplicantView()` hook provides additional validation.

Shelves can save and restore their state to files, persisting replicants across sessions. On restart, the shelf reloads each replicant by loading its add-on and calling `Instantiate()`.

### What it enabled

Replicants were conceptually powerful:

- A clock widget could live on the Desktop, in the Deskbar, or inside any application with a shelf
- An email notifier could embed itself in any host
- A stock ticker, weather display, or system monitor could be a replicant
- The Deskbar tray was implemented as a shelf — system tray icons were replicants
- The pattern generalized to any BView that could archive and instantiate itself

Replicants were, in principle, a cross-process component embedding system — an alternative to OLE/ActiveX/OpenDoc that used BMessage serialization instead of COM interfaces.

### What it cost

**1. In-process code execution.** The replicant's code runs in the host's address space. A buggy replicant can crash the host. There is no process isolation — a replicant that corrupts memory corrupts the host. This is fundamentally different from, say, X11 embedding (where the embedded application runs in its own process).

**2. Static variables don't work across boundaries.** If the replicant's code uses global or static variables, those variables in the host's loaded copy are independent of the originating application's copy. State that depended on process-global data would break.

**3. C++ fragility.** The system depends on name mangling, vtable layout, and ABI compatibility between the replicant's binary and the host's runtime. Different compiler versions, different compiler flags, or different library versions could cause silent corruption.

**4. Underutilization.** Despite the potential, replicants were barely used. The developer community was small, and Be marketed replicants as toys ("little Desktop toys") rather than as serious infrastructure. As one developer observed: "Almost no one ever used them. No great mountains of applications ever came out for them." The framing as entertainment undermined adoption.

**5. No protocol-level safety.** There was no type checking on the replicant's behavior once loaded into the host. The host trusted the replicant's code completely. No sandboxing, no capability restriction, no protocol enforcement.

### The lesson

The replicant system demonstrates both the power and the danger of architectural composition. The power: if serialization is universal (everything archives into BMessages) and code loading is standardized, cross-process component embedding falls out naturally. The danger: without process isolation and protocol-level safety guarantees, the composition is fragile.

The BArchivable/BShelf architecture is an existence proof that components CAN compose across application boundaries using a shared serialization format. But the specific mechanism (loading code into the host's address space) was the wrong one — it traded safety for simplicity. The idea of "archive state, transmit it, reconstruct elsewhere" is sound; the implementation via in-process code injection is the problem.

---

## 1.6 BRoster and launch_daemon

### BRoster — the application registry

BRoster is the runtime application directory. It provides:

**Application tracking:**

- `IsRunning(signature)` / `IsRunning(entry_ref)` — check if an app is active
- `GetAppInfo(signature, app_info*)` — get team ID, thread, port, flags, entry_ref
- `GetAppList(team_id_list*)` — enumerate all running apps
- `GetAppList(signature, team_id_list*)` — enumerate instances of a specific app

**Application launching:**

- `Launch(mime_type, ...)` — launch the preferred handler for a MIME type
- `Launch(entry_ref, ...)` — launch a specific executable
- Launch variants accept initial messages, message lists, or command-line arguments
- The system respects app flags: `B_SINGLE_LAUNCH` (one instance per signature), `B_EXCLUSIVE_LAUNCH` (one instance systemwide), `B_MULTIPLE_LAUNCH` (unlimited instances)
- Launching a single-launch app that's already running returns `B_ALREADY_RUNNING` and delivers the launch message to the existing instance

**Application discovery:**

- `FindApp(mime_type, entry_ref*)` — locate the handler for a MIME type
- `FindApp(entry_ref, entry_ref*)` — locate the handler for a file
- Discovery searches: file's preferred app → MIME type's preferred app → supporting apps → supertype handlers

**Application monitoring:**

- `StartWatching(BMessenger, uint32 events)` — subscribe to roster events
- Events: `B_SOME_APP_LAUNCHED`, `B_SOME_APP_QUIT`, `B_SOME_APP_ACTIVATED`
- Event messages include: signature, team ID, thread ID, flags, entry_ref

**Recent information:**

- Recent documents (filtered by type or app)
- Recent folders
- Recent applications

The design separates _application identity_ (signature like "application/x-vnd.Be-MAIL") from _execution context_ (team ID). This enables the single-launch/exclusive-launch semantics: the roster knows whether "launch this app" means "start a new instance" or "activate the existing one."

### launch_daemon — the evolution

BeOS's original boot process was shell-script-based. Haiku's launch_daemon replaces this with a structured, event-driven service manager (inspired by Apple's launchd and Linux's systemd, but adapted to Haiku's architecture).

**Core concepts:**

- **Job**: a one-time application launch
- **Service**: a permanently running background application, automatically restarted if it quits or crashes
- **Target**: a logical grouping of jobs/services launched together on some occasion

**Configuration format:** driver_settings syntax (a lightweight INI-like format native to Haiku), parsed into BMessages internally. Configuration lives in:

- `/system/data/launch/` — system packages
- `/system/settings/launch/` — user customization
- `/config/data/launch/` and `/config/settings/launch/` — per-user startup

**The parallel boot innovation:** The launch_daemon's key architectural insight is pre-creating communication ports before starting any services. Port-based communication queues are established first; services can have messages queued for them before they even start. This means "no system servers need to have any dependencies defined; they are all started in parallel on boot."

How this works: the launch_daemon creates a port for each service (the port that other services will use to send it BMessages). When service A wants to talk to service B during boot, it sends a message to B's port — which already exists, even if B hasn't started yet. The message queues until B starts and begins reading from its port. This eliminates the need for explicit dependency ordering.

**Event-driven startup:**

- Demand loading: services start only when needed (when something sends to their port)
- Event registration: services like mount_server register events with launch_daemon, triggering notifications when conditions are met
- Conditions: safemode detection, read-only medium, file existence checks
- Boolean composition: `and`, `or`, `not` for complex launch conditions

**User session integration:** When app_server creates a display session, launch_daemon initiates the login target. The default auto_login process forks a child adopting the user's ID, which scans user-specific launch configurations.

### The evolution from BRoster to launch_daemon

BRoster was BeOS's runtime application directory — it tracked what was running and could launch new apps, but it didn't manage the system boot process or service lifecycle.

launch_daemon adds service lifecycle management:

- Automatic restart on crash (services, not jobs)
- Boot-time startup orchestration without dependency graphs
- Event-driven and on-demand activation
- Multi-threaded parallel boot

Together, BRoster + launch_daemon provide a complete application lifecycle: launch_daemon handles startup, restart, and event-driven activation; BRoster provides runtime discovery, monitoring, and the identity system (signatures, single-launch semantics).

---

## 1.7 Tracker + MIME + BQuery/BFS + mail_daemon — The Email Composition

This is the canonical example of infrastructure composition in BeOS. No single component implements an email client. Instead, general-purpose infrastructure composes into an email UX.

### The components

**mail_daemon**: a background process responsible for POP/SMTP communication with mail servers. It does NOT display email. It does NOT manage folders. It does NOT implement search. It does exactly two things:

1. Periodically fetches messages from the mail server (POP3)
2. Sends queued outgoing messages to the mail server (SMTP)

When mail_daemon fetches a message, it:

1. Saves the raw message as an individual file in `~/mail/`
2. Parses the headers
3. Writes each header as a typed BFS attribute on the file:
   - `MAIL:subject` (B_STRING_TYPE) — the subject line
   - `MAIL:from` (B_STRING_TYPE) — sender
   - `MAIL:to` (B_STRING_TYPE) — recipient(s)
   - `MAIL:cc` (B_STRING_TYPE) — CC recipients
   - `MAIL:when` (B_TIME_TYPE) — date/time
   - `MAIL:status` (B_STRING_TYPE) — "New", "Read", "Replied", "Sent", "Error"
   - `MAIL:flags` (B_INT32_TYPE) — additional state
   - `MAIL:priority` (B_STRING_TYPE) — priority level
   - `MAIL:thread` (B_STRING_TYPE) — threading information
   - `MAIL:account` (B_STRING_TYPE) — which account received it
4. Sets the MIME type to `text/x-email`

These attributes are indexed in BFS. The filesystem now functions as a structured email database without any email application being involved.

**Tracker** (the file manager): Tracker knows nothing about email. It displays files, shows attribute columns based on MIME type, and lets users open files with the appropriate handler. When Tracker displays a directory containing email files:

- The MIME type `text/x-email` tells Tracker which attributes to show as columns
- Tracker displays From, Subject, Date, Status as table columns — the same way it would display Size and Modified for regular files
- Clicking a column header sorts by that attribute
- Right-clicking allows adding/removing attribute columns
- Double-clicking an email file opens it with the registered handler (BeMail)

**BQuery + live queries**: Tracker can save queries as "virtual folders." A query for `(MAIL:status == "New") && (MAIL:account == "work")` acts as a live inbox for the work account. Because the query is live:

- When mail_daemon downloads a new message and writes its attributes, the live query automatically includes it
- When the user reads a message and BeMail updates `MAIL:status` to "Read", the live query automatically removes it
- The "inbox" is not a folder — it's a running query against the filesystem

**BeMail** (the mail reader/composer): a straightforward application that opens email files, displays their contents, and provides reply/compose/forward functionality. When composing, it writes a file with the message content and appropriate MAIL: attributes. mail_daemon picks up files with `MAIL:status == "Pending"` and sends them.

### How it composed

The user experience:

1. **Setup:** Configure mail_daemon with server credentials. Configure checking interval. Done.

2. **Inbox:** A saved query for `MAIL:status == "New"`. Opens in Tracker as a window with columns: From, Subject, Date. This is not a mail-client-specific inbox view — it's Tracker's standard file list with mail-specific columns.

3. **Notification:** The Deskbar's mail icon uses queries internally. The "# new messages" submenu "is populated by a query for email with the status 'New'."

4. **Reading:** Double-click a message in Tracker. BeMail opens. The file is just a file. Reading it marks `MAIL:status = "Read"`. The live query removes it from the "New" inbox.

5. **Searching:** Any Tracker query. `MAIL:from == "rob" && MAIL:when > "last week"`. The same query mechanism that finds files by name finds emails by sender and date. No separate "mail search" feature needed.

6. **Filing:** Drag an email file to a folder. Standard file management. No "move to folder" mail feature — just drag-and-drop in Tracker.

7. **Composing:** Open BeMail, write, send. BeMail creates a file with attributes. mail_daemon sends it. The file stays with `MAIL:status = "Sent"` — searchable, archivable, same as any other file.

8. **Custom "folders":** Save a query for `MAIL:from contains "@company.com"`. That's your "company" folder. Save a query for `MAIL:subject contains "[project-x]"`. That's your project mailing list folder. These "folders" are live — they automatically include new matching messages.

### The critical insight

No single component implemented "email." The components were:

- **mail_daemon**: POP/SMTP transport + file writing + attribute setting
- **BFS**: attribute storage + indexing + query evaluation
- **Tracker**: file display + attribute columns + query persistence
- **BeMail**: message viewing + composition + status updating
- **live queries**: dynamic result sets delivered via BMessage notifications

Each component was general-purpose:

- mail_daemon didn't need to know about Tracker
- Tracker didn't need to know about email
- BFS didn't need to know about either
- BQuery wasn't email-specific
- Live queries weren't email-specific

The email UX emerged from the infrastructure. And the same infrastructure worked for other domains. The IM Kit demonstrated this: instant messaging contacts were People files with `IM:connections` attributes. Contact presence was tracked via attribute changes. Live queries showed "all family members online." The same BFS + Tracker + BQuery + live query infrastructure that made email work also made instant messaging work — different attributes, same mechanism.

A BeOS developer demonstrated the generality even further by building an HTML contact database generator as a shell script: query for People files matching criteria, loop through results, extract attributes with `catattr`, interpolate into HTML templates. The filesystem-as-database was scriptable from day one.

### What made it possible

1. **Typed attributes on files, indexed by the filesystem.** Without this, there's no queryable email database — just opaque files.
2. **Live queries delivered as BMessages.** Without this, "inboxes" are static snapshots that go stale.
3. **Tracker displaying arbitrary attributes as columns.** Without this, email files look like any other files with no useful metadata visible.
4. **MIME type system connecting files to handlers and attribute schemas.** Without this, Tracker doesn't know which columns to show for email files.
5. **mail_daemon writing attributes, not managing a database.** Without this, the email data is locked inside a proprietary store.

Each of these was a general-purpose system feature, not built for email. The email experience was emergent.

---

## 1.8 Synthesis: How These Ideas Inform Pane's Design

### Principle 1: Building for the hardest case produces architecture better for every case

BeOS was engineered for symmetric multiprocessing when most personal computers had one CPU. This forced the design team to solve pervasive concurrency, message-based communication, fine-grained scheduling, and the elimination of shared mutable state. The result: an OS that was more stable and responsive than its contemporaries _on single-core machines_, because the discipline required for SMP was the same discipline that produced good architecture generally.

The mechanism: making concurrency pervasive forced every component to have self-contained operational semantics. Each BLooper ran its own thread; each BHandler processed its own messages; cross-component interaction happened only through BMessages. This eliminated the classes of bugs that plagued monolithic architectures: global state corruption, priority inversion from UI-blocking computations, cascading failures from shared data structures.

The principle isn't "SMP is magic." It's that the _constraint of designing for the hardest case eliminated accidental complexity that easier constraints would have permitted_. If you can share global state, you will. If you can block the UI thread, you will. If you can skip the protocol and call a function directly, you will. BeOS's threading model made all of these difficult or impossible, and the resulting codebase was better for it.

For pane, this principle manifests as: designing for typed protocol composition between independent servers forces each server to be self-contained and protocol-compliant. If pane-comp, pane-route, pane-roster, and pane-shell are separate processes communicating via session-typed protocols, they CANNOT share mutable state, CANNOT block each other, CANNOT skip the protocol. This is the same structural discipline that BeOS achieved through pervasive multithreading, realized through a different mechanism.

### Principle 2: Stability emerges from protocol, not from a global coordinator

BeOS had no master process orchestrating the system. There was no equivalent of systemd's PID 1 owning the process tree, or of a global event bus routing all messages. Instead, each component followed the BMessage protocol: receive messages, process them, send responses. The scheduler ran threads. The message system delivered messages. Stability was an emergent property of every component following the protocol.

This is a profound architectural insight. A global coordinator is a single point of failure and a bottleneck. It must understand every possible interaction between every component. It must handle every error condition. It becomes the most complex part of the system — and the most likely to fail,
which is tragic because it is also the most important component of the system.

A protocol-based architecture distributes this complexity. Each component handles its own error conditions. Each component defines its own response to each message type. The "coordination" is implicit in the protocol design: if A sends a request to B, B will respond according to the protocol. No third party needs to mediate.

The risk, as the designer noted, is that "you establish a protocol, ground rules, and then hope the resulting system is capable of sustained operation in the face of emergent system complexity. This seems very risky, but the startling fact was that it worked!" BeOS demonstrated empirically that protocol-based coordination is viable for a complete operating system with a GUI, media stack, and filesystem.

For pane, this translates directly: pane's servers compose via protocols, not via a supervisor. pane-comp doesn't manage pane-route's lifecycle. pane-route doesn't coordinate with pane-roster through a shared database. Each server follows its protocol. The session types formalize the protocols. Stability emerges from protocol compliance.

### Principle 3: What BMessage/BLooper achieved, session types can now formalize

BeOS's BMessage/BLooper discipline achieved several properties by convention and engineering skill:

- Messages carried typed data (type codes on every field)
- Handlers processed messages they understood and passed on others (chain of responsibility)
- Loopers ran one message at a time per thread (no concurrent access to handler state)
- The protocol was implicit in the `what` codes (conventions about which messages to send when)

These properties were enforced by the API design and developer discipline, not by a formal system. A developer could violate the protocol (send unexpected messages, skip required responses, abandon conversations) and the system would fail at runtime.

Session types formalize exactly these properties:

- **Typed messages** → session type payloads with compile-time type checking
- **Protocol ordering** → the session type specifies the exact sequence of sends and receives
- **One-at-a-time processing** → linear channel usage ensures each endpoint processes one message at a time
- **Conversation structure** → the session type IS the protocol, not a runtime convention
- **Completeness** → the linear discipline prevents silently abandoning a session

What BeOS engineers achieved by careful convention, session types enforce at compile time. The `what` field becomes a branch in a session type enum. The convention "after connecting, send capabilities, then enter the active loop" becomes a session type `Recv<Capabilities, Recv<ActiveLoop>>`. A handler that processes B_QUIT_REQUESTED becomes a branch in a choice type.

This is the "more than two decades of theoretical development that eventually caught up to what they were doing by skill, sensibility, and intuition alone." Honda's session types (1993) were being developed simultaneously with BeOS (1991–2001), but the theoretical work didn't reach practical programming until much later. Now it has, and pane can leverage it.

### Principle 4: Infrastructure-first design enables emergent composition

The email case study is the strongest evidence. No one designed an "email system" for BeOS. They designed:

- A filesystem with typed, indexed attributes (general purpose)
- A query engine with live results (general purpose)
- A file manager that displays attributes as columns (general purpose)
- A MIME type system connecting files to handlers (general purpose)
- A mail daemon that writes files with attributes (email-specific, but minimal)

Email emerged from the composition of these general-purpose systems. And the same infrastructure immediately supported IM, contacts, music libraries, photo organization — any domain where data could be represented as files with typed attributes.

The contrast with the conventional approach is stark. A conventional email client implements its own database, its own search, its own display, its own file management. None of this infrastructure is reusable. A contacts app does the same thing independently. A music player does the same thing independently. Each application reinvents the same infrastructure for its own domain.

BeOS's approach invests heavily in general infrastructure and gets email, IM, contacts, music, photos, and every future domain as consequences. The conventional approach invests lightly in each domain and gets nothing reusable.

For pane, the implications are:

- **Filesystem as interface** (pane-fs) should expose typed metadata, not just file contents. If pane state is represented as files with attributes, then queries, scripts, and external tools can compose with pane the way Tracker composed with email.
- **The routing infrastructure** (pane-route) should be general-purpose, not email-specific or web-specific. If routing rules can match any content pattern and dispatch to any handler, then new content types get routing for free.
- **The notification system** (pane-notify) is pane's equivalent of BFS live queries + node monitoring. It must be general-purpose — watch any state change, deliver typed notifications to any subscriber.
- **Each server should do one thing well and expose its state as inspectable, queryable data.** Pane-roster is a directory, not a supervisor. Pane-route is a router, not an application framework. The email lesson: the less each component tries to do, the more the composition achieves.

### Where pane is continuous with BeOS

**Message-based composition.** Pane's servers communicate via typed messages over protocols, the same fundamental model as BMessage/BLooper. The wire format differs (postcard serialization over unix sockets vs. BMessage flattening over ports), but the architectural pattern is identical: self-contained servers exchanging typed messages.

**Plugin discovery by convention.** The Translation Kit's model (drop a shared library in a directory, the system finds it) maps to pane's directory-based plugin discovery. The difference: pane uses filesystem watch (pane-notify) for dynamic discovery rather than scanning at startup.

**MIME types and content routing.** BRoster's `FindApp()` and launch semantics (preferred handler for MIME type, single-launch/exclusive-launch) are directly relevant to pane-route and pane-roster. Pane-roster maintains service registrations; pane-route uses content matching to dispatch. The composition is similar, though pane separates routing (pane-route) from registry (pane-roster) where BeOS merged them (BRoster did both).

**Application lifecycle awareness.** BRoster's `StartWatching()` for app launches and quits maps to pane-roster's service monitoring. The launch_daemon's parallel boot via pre-created ports is instructive for pane's startup sequence.

### Where pane departs from BeOS — and why

**Session types replace convention.** BeOS's BMessage protocol was a convention: "send this `what` code with these fields, expect this response." A developer could violate it. Pane's session types make the protocol a compile-time artifact. This is not a "better BMessage" — it's a fundamentally different approach to protocol compliance. The trade-off: session types require more upfront design work and constrain the protocol to what the type system can express. The gain: protocol violations are caught at compile time, not discovered as runtime crashes.

**Process isolation instead of in-process embedding.** The replicant system's failure mode — loading foreign code into the host process — is a direct lesson. Pane's servers are separate processes communicating via protocols. If a pane-native application crashes, the compositor continues. If a widget misbehaves, it misbehaves in its own process. Session types ensure that the protocol boundary is well-defined; process separation ensures that violations don't corrupt the host.

This means pane cannot have replicant-style embedding (drag a widget from one app into another). But the replicant system's _intent_ — composable UI components across application boundaries — can be achieved through protocol-level composition. A pane that displays another server's content does so via protocol, not by loading the other server's code.

**Filesystem as secondary interface, not primary.** In BeOS, BFS attributes were the primary metadata layer — the canonical location for file metadata. In pane, the typed protocol is primary and pane-fs is a translation layer for scripting and interop. This reversal reflects the difference between a complete OS (where the filesystem is the natural infrastructure) and a desktop environment on Linux (where the filesystem is shared with other systems and cannot be assumed to support BeOS-style attributes and indices).

However, pane-fs should still expose rich metadata — not as BFS attributes (Linux's xattr system is too limited), but through the filesystem interface's structure. Per-pane directories with `attrs` files containing typed metadata serve the same role as BFS attributes: making state inspectable and scriptable.

**Quality-based translator routing is a useful pattern.** The Translation Kit's quality/capability self-rating for automatic translator selection is elegant and applicable to pane-route's multi-match routing. When multiple handlers match a content pattern, self-declared quality ratings could determine the default — without requiring a central authority to rank handlers.

**The "free attributes" pattern.** BFS's three always-indexed attributes (name, size, last_modified) ensured that basic queries always worked without explicit index creation. Pane-fs could adopt a similar pattern: certain pane state attributes (pane type, server identity, creation time) are always available, always queryable. More specialized attributes are available per-server but require knowing about them.

### The deepest lesson

BeOS demonstrated that when you design a system around the hardest constraints — pervasive concurrency, no shared state, protocol-based coordination — the resulting architecture is not just adequate for easy cases but _superior_ for all cases. The discipline required to make SMP work correctly was the same discipline that made the system stable, responsive, and composable.

Pane's bet is the same bet: that designing for typed protocol composition between independent servers — accepting the constraints of session types, process isolation, and protocol-first communication — will produce a desktop environment that is more reliable, more scriptable, and more composable than one built on shared state and monolithic processes.

What BeOS engineers achieved by skill and convention, session types now formalize. What BFS attributes achieved by filesystem integration, pane-fs achieves by typed protocol exposition. What the email composition achieved by infrastructure-first design, pane achieves by making the infrastructure (routing, notification, registry) general-purpose and composable.

The continuity is in the ideas: message-based composition, self-contained operational semantics, protocol-emergent stability, infrastructure-first design. The departure is in the mechanisms: session types instead of runtime conventions, process isolation instead of in-process embedding, protocol-first instead of filesystem-first. The departures are justified by two decades of experience with what worked and what didn't in the BMessage model, and by the availability of formal tools (session types, linear logic) that didn't exist when BeOS was designed.

---

## Sources

### Documentation

- [Haiku API Reference](https://www.haiku-os.org/docs/api/) — complete Haiku API documentation
- [Haiku Application Kit](https://www.haiku-os.org/docs/api/group__app.html) — BMessage, BLooper, BHandler, BRoster
- [Haiku Interface Kit](https://www.haiku-os.org/docs/api/group__interface.html) — BWindow, BView, BControl
- [Haiku Storage Kit](https://www.haiku-os.org/docs/api/group__storage.html) — BNode, BQuery, BFile
- [Haiku Support Kit](https://www.haiku-os.org/docs/api/group__support.html) — BArchivable, BLocker
- [Haiku Translation Kit](https://www.haiku-os.org/docs/api/group__translation.html) — BTranslatorRoster, BTranslator
- [BMessage Class Reference](https://www.haiku-os.org/docs/api/classBMessage.html)
- [BLooper Class Reference](https://www.haiku-os.org/docs/api/classBLooper.html)
- [BHandler Class Reference](https://www.haiku-os.org/docs/api/classBHandler.html)
- [BQuery Class Reference](https://www.haiku-os.org/docs/api/classBQuery.html)
- [BRoster Class Reference](https://www.haiku-os.org/docs/api/classBRoster.html)
- [BArchivable Class Reference](https://www.haiku-os.org/docs/api/classBArchivable.html)
- [BNode Class Reference](https://www.haiku-os.org/docs/api/classBNode.html)
- [BTranslatorRoster Class Reference](https://www.haiku-os.org/docs/api/classBTranslatorRoster.html)
- [Messaging Foundations](https://www.haiku-os.org/docs/api/app_messaging.html)
- [Introduction to the Launch Daemon](https://www.haiku-os.org/docs/api/launch_intro.html)

### The Be Book (Legacy Documentation)

- [BQuery Overview](https://www.haiku-os.org/legacy-docs/bebook/BQuery_Overview.html) — complete query system documentation
- [Mail Kit Overview](https://www.haiku-os.org/legacy-docs/bebook/TheMailKit_Overview_Introduction.html) — mail daemon architecture and attributes
- [Mail Daemon Overview](https://www.haiku-os.org/legacy-docs/bebook/TheMailDaemon_Overview.html) — daemon operation
- [BShelf Overview](https://www.haiku-os.org/legacy-docs/bebook/BShelf_Overview.html) — replicant shelf system
- [Translator Add-Ons](https://www.haiku-os.org/legacy-docs/bebook/TranslatorAddOns.html) — complete translator plugin protocol
- [File System Architecture](https://www.haiku-os.org/legacy-docs/bebook/TheStorageKit_Overview_FileSystemArchitecture.html)

### Developer Articles

- [Introducing the launch_daemon](https://www.haiku-os.org/blog/axeld/2015-07-17_introducing_launch_daemon/) — Axel Dörfler's design post
- [Node Monitoring](https://www.haiku-os.org/documents/dev/node_monitoring) — kernel-level notification architecture
- [Replicants: More Application Than an Application](https://www.haiku-os.org/documents/dev/replicants_more_application_than_an_application/) — replicant mechanism and critique
- [Managing Your Replicants](https://www.haiku-os.org/documents/dev/managing_your_replicants_xshelfinspector_and_xcontainer/)
- [Workshop: Managing Email](https://www.haiku-os.org/docs/userguide/eo/workshop-email.html) — email UX from user perspective

### Technical Articles

- [Making the Case for BeOS's Pervasive Multithreading](https://www.osnews.com/story/180/making-the-case-for-beoss-pervasive-multithreading/) — OSnews
- [A Programmer's Introduction to the Haiku OS](https://www.osnews.com/story/24945/) — OSnews, DarkWyrm
- [IM With File System Support](https://www.osnews.com/story/5666/im-with-file-system-support-putting-the-bfs-attributes-in-good-use/) — IM Kit demonstrating BFS attribute generality
- [Scripting with the Be File System](https://birdhouse.org/beos/byte/24-scripting_the_bfs/) — shell-level attribute and query usage
- [Node Monitoring in BeOS/OpenBeOS](https://www.osnews.com/story/3575/node-monitoring-in-beosopenbeos/) — OSnews

### Be Newsletters

- [Be Newsletter Vol. 3, Issue 26](https://www.haiku-os.org/legacy-docs/benewsletter/Issue3-26.html) — Translation Kit architecture by Jon Watte

### Books

- Giampaolo, Dominic. _Practical File System Design with the Be File System._ Morgan Kaufmann, 1998. [Author's PDF](http://www.nobius.org/dbg/practical-file-system-design.pdf) — the definitive BFS reference covering attribute indexing, B+ trees, queries, and journaling

### Wikipedia

- [Be File System](https://en.wikipedia.org/wiki/Be_File_System) — overview of BFS architecture
- [BeOS](https://en.wikipedia.org/wiki/BeOS) — OS history and architecture overview
