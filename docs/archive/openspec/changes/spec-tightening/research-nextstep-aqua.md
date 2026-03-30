# NeXTSTEP and Early Mac OS X Aqua Research — Tasks 3.1–3.15

Research for pane spec-tightening. Primary sources: NeXTSTEP 3.3 developer documentation (nextop.de/NeXTstep_3.3_Developer_Documentation/), Apple archived developer documentation (developer.apple.com/library/archive/), NSHipster IPC overview, Grokipedia entries for NeXTSTEP/Aqua/Quartz Compositor/Interface Builder/Display PostScript, Computer History Museum blog on NeXTSTEP and OOP, 512 Pixels Aqua history, UX Planet Aqua analysis, Apple newsroom archives, Aesthetics Wiki on Frutiger Aero, GNUstep documentation, modelessdesign.com HIG history.

Sources:

- NeXTSTEP developer docs: <https://www.nextop.de/NeXTstep_3.3_Developer_Documentation/>
- Services menu architecture: <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/SysServices/Articles/using.html>
- NeXTSTEP AppKit Services: <https://wiki.preterhuman.net/NeXTSTEP_AppKit_Installing_New_Services>
- NSHipster IPC: <https://nshipster.com/inter-process-communication/>
- Distributed Objects: <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/DistrObjects/Concepts/connections.html>
- Interface Builder: <https://grokipedia.com/page/Interface_Builder>
- Display PostScript: <https://grokipedia.com/page/Display_PostScript>
- Aqua UI: <https://grokipedia.com/page/Aqua_(user_interface)>
- Quartz Compositor: <https://grokipedia.com/page/Quartz_Compositor>
- NeXTSTEP overview: <https://grokipedia.com/page/NeXTSTEP>
- CHM NeXTSTEP article: <https://computerhistory.org/blog/the-deep-history-of-your-apps-steve-jobs-nextstep-and-early-object-oriented-programming/>
- OPENSTEP secrets: <https://www.3fingeredsalute.com/untold-secrets-openstep/>
- 512 Pixels Aqua: <https://512pixels.net/2014/04/aqua-past-future/>
- UX Planet Aqua: <https://uxplanet.org/apple-aqua-exploring-the-legacy-of-macos-x-user-interface-3a11eb9b7dba>
- NeXTSTEP tech review: <https://www.paullynch.org/NeXTSTEP/NeXTSTEP.TechReview.html>
- HIG history: <https://modelessdesign.com/backdrop/401>
- Frutiger Aero: <https://aesthetics.fandom.com/wiki/Frutiger_Aero>
- Accessibility: <https://developer.apple.com/library/archive/documentation/Accessibility/Conceptual/AccessibilityMacOSX/>
- Cocoa/Carbon: <https://en.wikipedia.org/wiki/Cocoa_(API)> and <https://en.wikipedia.org/wiki/Carbon_(API)>
- Exposé: <https://www.osnews.com/story/4939/mac-os-x-103-panther-review/>
- Miller columns: <https://en.wikipedia.org/wiki/Miller_columns>
- Dock: <https://en.wikipedia.org/wiki/Dock_(macOS)>

---

## 3.1 NeXTSTEP Services Menu

### Architecture

The Services menu was NeXTSTEP's system for cross-application composition. Every application included a "Services" submenu. Any application could register as a service provider, and any application could invoke those services on its current selection. The concept was a GUI-level Unix pipe: selected data flows out of one application, through a service, and the result flows back.

The mechanism rested on the pasteboard — NeXTSTEP's system clipboard abstraction. When a user invoked a service:

1. The requesting app wrote its current selection to a pasteboard, declaring the data type (NXAsciiPboardType, NXRTFPboardType, NXTypedFilenamePboardType, etc.).
2. The system routed the pasteboard to the service provider.
3. The service provider read the data, performed its operation, and optionally wrote a result back to the pasteboard.
4. The requesting app read the result and replaced the selection (or took other action).

Service providers declared their capabilities through a `services` file in the application bundle (or in `~/Library/Services/`, `/LocalLibrary/Services/`). The declaration specified:

- **Send type**: what pasteboard data type the service accepts as input
- **Return type**: what it produces as output
- **Port or executable**: how to reach the service
- **Menu item name**: what appears in the Services submenu

Example declaration from NeXTSTEP:

```
Filter:
Port: NXUNIXSTDIO
Send Type: NXTypedFilenamePboardType:mine
Return Type: NXAsciiPboardType
Executable: mine-ascii
```

Discovery was dynamic. The Services menu rebuilt itself from the installed service descriptions. `NXUpdateDynamicServices()` could trigger re-registration programmatically. The `make_services` utility rebuilt the cache for debugging.

### What made it powerful

Three properties distinguished Services from ad hoc integration:

**Universality.** Any text selected in any application could be acted on by any service. The user didn't need to know which application provided the service. A dictionary lookup, a URL opener, a text transformer, a mail sender — all lived in the same menu, available everywhere. This was the first system-wide mechanism for "do something with this selection" that operated across arbitrary applications.

**Type-driven matching.** Services were matched against the current selection's pasteboard type. If you had text selected, text services appeared. If you had a file selected, file services appeared. The menu dynamically filtered to show only applicable services. This made the menu context-sensitive without any per-application configuration.

**Composability.** Services could chain. A service that accepted text and returned text was a text filter. Filters could be composed: select text → encrypt → base64-encode → copy. Each step was independent. The pasteboard was the universal interchange format — the same role that pipes play in Unix, but operating on typed, structured data rather than byte streams.

### On Mac OS X

Apple carried Services into Mac OS X with the Cocoa API. The technical machinery became more sophisticated: apps registered send/return types via `[NSApp registerServicesMenuSendTypes:returnTypes:]`, validation happened through the responder chain (`validRequestorForSendType:returnType:`), data transfer used `writeSelectionToPasteboard:types:` and `readSelectionFromPasteboard:`, and programmatic invocation was available via `NSPerformService()`.

But Services never reached its potential on Mac OS X. The reasons are instructive:

**Discoverability.** The Services menu was buried in the application menu. Users who didn't already know about it had no reason to look. Before Snow Leopard, all services appeared regardless of applicability (most greyed out), creating a cluttered, confusing menu. After Snow Leopard, services were filtered to context — better, but now users couldn't even see what was _possible_ in other contexts.

**Carbon apps didn't participate.** Carbon applications (the majority of Mac software in the early years) didn't support the Cocoa pasteboard protocol. The two-world problem — Cocoa vs Carbon — meant Services only worked between Cocoa apps. This killed the universality that made NeXTSTEP Services compelling.

**No composition UI.** There was no visual way to chain services or see the pipeline. Each invocation was a one-shot: select → invoke → done. The Unix-pipe analogy was implicit but invisible.

**Intermittent failures.** The Services menu was notorious for getting stuck in a "building..." state, failing to discover services, or losing items. The dynamic registration mechanism was fragile in practice.

The lesson: cross-application composition is one of the most powerful ideas in desktop design, but it requires universality (all apps participate), discoverability (users can find what's possible), and composition visibility (the pipeline is tangible). NeXTSTEP had the first. Mac OS X lost it. Neither had the third.

---

## 3.2 Objective-C Message Passing and Late Binding

### The bet

NeXT chose Objective-C — Brad Cox's Smalltalk-inspired extension of C — as the system programming language. Where C++ (BeOS's choice) resolved method calls at compile time through vtables, Objective-C resolved them at runtime through message dispatch. Every method call was a message send: `[object doSomething]` compiled to `objc_msgSend(object, @selector(doSomething))`.

The runtime maintained per-class dispatch tables mapping selectors (method names) to implementations (function pointers). When an object received a message:

1. Look up the selector in the object's class dispatch table.
2. If found, call the implementation.
3. If not found, walk up the superclass chain.
4. If still not found, invoke the forwarding machinery (`forwardInvocation:`).

This was fundamentally different from C++. In C++, calling a nonexistent method is a compile-time error. In Objective-C, it's a runtime event that can be intercepted, redirected, or handled dynamically.

### What it enabled

**Interface Builder.** The NIB file mechanism (see 3.3) depended entirely on dynamic dispatch. Interface Builder stored serialized object graphs — actual object instances, frozen to disk, with their connections (outlets and actions) recorded as selector names. At load time, the runtime reconstituted the objects, looked up the selectors, and wired the connections. This worked because the runtime could resolve `@selector(buttonPressed:)` against whatever object occupied the "target" slot, without compile-time knowledge of the target's class. Static dispatch (C++) cannot do this — you'd need code generation or explicit registration.

**Services.** The Services menu depended on the responder chain — messages sent up a chain of objects until one handled them. `validRequestorForSendType:returnType:` traveled the responder chain dynamically. The system didn't know at compile time which object would handle the service request.

**Distributed Objects.** Transparent proxying (see 3.7) required that any message sent to a proxy be captured and forwarded. Objective-C's `NSProxy` class overrode `forwardInvocation:` to serialize the message and send it across a connection. This is impossible in a language where method dispatch is resolved at compile time.

**Plugin loading.** Bundles containing compiled Objective-C code could be loaded at runtime, and their classes immediately participated in message dispatch. No registration step, no factory pattern, no interface definitions beyond the protocol.

### What it cost

**Performance.** Every message send went through `objc_msgSend`, which involved a hash lookup plus a function call. For the 1990s hardware NeXT targeted (25–33 MHz Motorola 68040), this overhead was noticeable. BeOS's C++ vtable dispatch was a simple indirect call — one pointer dereference. The per-message cost difference was roughly 5–10x for trivial methods.

**No static guarantees.** A misspelled selector was a runtime crash, not a compile-time error. The dynamism that enabled Interface Builder and Distributed Objects also meant entire categories of bugs were invisible until the code ran.

**Fragile base class problem.** Changes to a superclass's instance variables could break subclasses compiled against the old layout. (This was fixed much later with Objective-C 2.0's non-fragile ABI.)

### The trade-off in context

The comparison with BeOS is illuminating. BeOS chose C++ for performance: tight message loops, real-time audio, BMessage dispatch all benefited from compile-time method resolution. But BeOS paid for it in flexibility: Interface Builder couldn't exist in C++ (you need code generation or a scripting bridge), the BArchivable/replicant system required explicit registration for every archivable class, and plugin loading needed explicit factory functions.

NeXT's bet was that developer productivity and system integration mattered more than per-message performance. The dynamic runtime was the _reason_ NeXTSTEP felt integrated — the same mechanism that let Interface Builder wire connections let the Services menu discover capabilities let Distributed Objects proxy messages. One mechanism, three system-level features. That's the payoff of choosing a flexible substrate.

---

## 3.3 Interface Builder and the NIB/XIB Model

### Origins

Jean-Marie Hullot created the prototype — called "SOS Interface" — in 1986 at INRIA (Rocquencourt, near Paris), using the Ceyx language on Le_Lisp. The tool combined direct graphic manipulation with simple programming. When Denison Bollay demonstrated it to Steve Jobs, Jobs immediately recognized its value. Hullot joined NeXT in 1986, and by 1988 Interface Builder shipped as part of NeXTSTEP 0.8.

It was the first commercial application that allowed interface elements — buttons, menus, windows — to be placed visually using a mouse and connected to code without writing interface construction code.

### The core idea: UI as data, not code

Most GUI frameworks (then and now) construct interfaces by executing code: `new Button(x, y, w, h); button.setTitle("OK"); button.setTarget(this);`. Interface Builder took a different approach: the developer visually arranged real objects in a WYSIWYG editor, and the tool serialized the entire object graph to disk as a NIB file ("NeXT Interface Builder").

A NIB file was not a description of how to build an interface. It _was_ the interface — "freeze-dried" objects, archived through Objective-C's `NSCoding` protocol, ready to be reconstituted at runtime. When an application loaded a NIB, the runtime:

1. Deserialized the archived objects (windows, buttons, text fields, etc.)
2. Resolved connections: outlets (object references) and actions (message targets)
3. Inserted the reconstituted objects into the live application

No code generation. No intermediate format. The objects in the running application were the same objects the developer arranged in Interface Builder, reconstituted from their archived state.

### Outlets, actions, connections

**Outlets** were instance variables (later properties) marked with `IBOutlet` — weak references from code to interface objects. When the NIB loaded, the runtime set these references to the reconstituted objects. A controller with `@IBOutlet NSTextField *nameField` would find `nameField` pointing to the actual text field the developer placed in Interface Builder.

**Actions** were methods marked with `IBAction` — the target-action pattern. A button configured with target = controller, action = `buttonPressed:` would, when clicked, send `[controller buttonPressed:button]`. The connection was stored in the NIB as a selector name and resolved at runtime through Objective-C's dynamic dispatch. This is why Interface Builder required Objective-C: the selector-to-implementation binding happened at runtime, not compile time.

**The First Responder** was a placeholder representing "whoever is currently first in the responder chain." Actions targeted at the First Responder would travel up the chain until an object handled them — enabling menu items that worked on whatever was currently focused, without any compile-time knowledge of what that would be.

### What this enabled

**Separation of design from logic.** The interface could be redesigned — controls moved, resized, replaced — without changing any code. The code referred to outlets and actions; the NIB provided the concrete bindings.

**Rapid iteration.** Jobs claimed NeXTSTEP reduced UI development time from 90% of the total to 10%. Interface Builder was the primary mechanism. Small teams could build in weeks what previously took months.

**Consistency.** Because all applications used the same AppKit objects serialized through the same mechanism, they shared visual consistency by default. The system's look-and-feel was embodied in the objects, not in each application's rendering code.

**The MVC encoding.** Interface Builder didn't just enable MVC — it essentially _required_ it. The NIB was the View. The code was the Controller (and Model). They met at outlets and actions. This wasn't a pattern developers could choose to follow; it was the architecture the tool imposed.

### Evolution

NIBs were binary archives — compact for loading but opaque to diffing. XIBs (introduced with Interface Builder 3 / Xcode 3) were XML representations of the same object graph — functionally identical, but flat files suitable for version control. The build process compiled XIBs to NIBs for runtime use.

Apple eventually merged Interface Builder into Xcode (2011, Xcode 4), eliminating the separate application. And in 2019, SwiftUI offered a declarative alternative — but Interface Builder remains in use for UIKit and AppKit development, and the NIB mechanism still underpins much of the Apple ecosystem.

### Relation to pane

Pane's spec describes UI as data in a different sense: cell grids are content sent over a protocol, widget trees are declarative structures rendered by the compositor. The parallel isn't serialized object graphs (pane doesn't archive Rust objects to disk) but the principle that the interface description should be data that flows through the system, not code that executes in a specific context. Interface Builder proved this principle could produce systems that felt more integrated, more consistent, and more maintainable than the code-generation alternative.

---

## 3.4 Workspace Manager and App Lifecycle

### Workspace Manager

The Workspace Manager was NeXTSTEP's desktop shell — the equivalent of macOS Finder, Windows Explorer, or BeOS Tracker. It handled:

- **File navigation**: the File Viewer, using Miller columns (a hierarchical browser showing directory contents in adjacent columns, each column representing one level of the path). This was NeXTSTEP's innovation, later adopted by macOS Finder's "column view."
- **The Shelf**: a holding area for frequently used files and directories, displayed as icons above the browser columns. Drag a file to the shelf for quick access.
- **Application management**: launching, activating, and terminating applications. The Workspace Manager maintained the relationship between documents, file types, and applications.
- **The Inspector**: a context-sensitive panel with four modes — Attributes (ownership, size), Tools (which apps can open this file), Access (permissions), and Contents (file format-specific content preview). The Contents inspector was extensible: bundles in standard search paths could provide custom inspectors for specific file types.
- **Volume management**: mounting and unmounting.
- **Session management**: logout.

The Inspector system was plugin-based. Inspector bundles — stored in `~/Apps`, `/LocalApps`, `/NextApps`, or the application package itself — were loaded into the Workspace Manager's process space. They communicated through the `WMInspector` API: the system sent `new` to access the inspector and `revert:` when the selection changed. Inspectors queried the current selection via `selectionCount` and `selectionPathsInto:separator:`. Once loaded, an inspector couldn't be unloaded without restarting the Workspace Manager — a limitation, but one that kept the architecture simple.

### The .app bundle model

NeXTSTEP pioneered the application bundle: an application was a directory (with `.app` extension) that appeared as a single file to the user. Inside:

- The executable
- NIB files (serialized interfaces)
- Resources: images, sounds, localization tables
- The `services` file (for Services menu registration)
- Info.plist (application metadata)

This was a decisive break from Unix tradition (executables in `/usr/bin`, libraries in `/usr/lib`, configs in `/etc`) and from Windows tradition (executables, DLLs, and registry entries scattered across the system). The bundle was self-contained: drag it to install, drag it to the trash to remove. No installer, no uninstaller, no registry.

The bundle model also enabled:

- **Localization**: bundles could contain language-specific `.lproj` directories with localized NIBs and strings.
- **Versioning**: the bundle carried its own version metadata.
- **Code loading**: `NSBundle` could dynamically load code and resources from any bundle, enabling plugins that were themselves bundles.

### The Dock

The NeXTSTEP Dock was a vertical strip of application icons along the right edge of the screen. The Workspace Manager and the Recycler always occupied fixed positions. Running applications showed solid icons; non-running applications showed their icon with an ellipsis below.

The Dock served three functions simultaneously: launcher (click to start), task switcher (click to activate), and status indicator (running vs not). Up to 12 customizable icons.

This was not the macOS Dock. The NeXTSTEP Dock was simpler, smaller, and more restrained — it didn't bounce, animate, magnify, or accumulate minimized windows. It was a list of applications. The evolution to macOS's Dock added the genie effect, magnification, window minimization, Stacks, and indicator dots, gaining visual spectacle at the cost of the original's clarity.

### App lifecycle

Application lifecycle in NeXTSTEP centered on the Workspace Manager as mediator. Launching: the user double-clicked a document or an app icon; the Workspace Manager resolved the file type to an application (via the Tools Inspector settings), found the application bundle, and launched it. Activation: clicking a running app's Dock icon brought it forward. Document handling: the Workspace Manager maintained the mapping between file types and applications, and passed file-open events to the launched application.

The contrast with BeOS's BRoster is instructive. BRoster was a programmatic API: any application could query it, launch applications by signature, check who was running. The Workspace Manager was a GUI shell that happened to manage the app lifecycle. The programmatic interface was secondary (through the Workspace Manager's API, or through `NSWorkspace` in the AppKit). BeOS made app lifecycle a first-class API that any process could use; NeXTSTEP made it a responsibility of the desktop shell.

---

## 3.5 Display PostScript / PDF Rendering

### Display PostScript

NeXT licensed Display PostScript (DPS) from Adobe in 1986–1987, making it the exclusive graphics subsystem. DPS extended Adobe's PostScript page-description language to enable real-time screen rendering. The display was treated as a "soft printer" — the same PostScript operators that described printed pages now described screen content.

**Resolution independence.** DPS used a coordinate system where 1 unit = 1/72 inch (matching PostScript's print standard). This user-space coordinate system was transformed to device-space (pixels) via the Current Transformation Matrix (CTM), an affine matrix supporting translation, scaling, and rotation. A 2-inch square drawn in DPS code would be 2 inches on any display, regardless of resolution. This was genuinely resolution-independent rendering — not in the later "retina" sense of providing 2x bitmaps, but in the mathematical sense of vector geometry mapped through a coordinate transform.

**Rendering pipeline.** PostScript code was parsed into intermediate display lists, transformed via the CTM, then rasterized using scan-conversion algorithms. Incremental rendering minimized overhead: only modified display regions were redrawn. DPS supported alpha blending for compositing, with the `Sover` operator (source-over-destination) implementing standard Porter-Duff compositing.

**Window compositing.** Each window had its own PostScript context — a private execution environment with its own graphics state. The window server maintained a compositing buffer assembling these contexts according to z-order. This was proto-compositing-window-management: each window rendered independently into its own context, and the server composed them. (Lane: im curious, is the situation similar to wayland?)

**The WYSIWYG payoff.** Because screen rendering used the same imaging model as print rendering, what you saw on screen matched what the printer would produce. For the target market (universities, publishers, enterprises), this was transformative. NeXT hardware shipped with Display PostScript optimized for the Motorola 68040 (25–33 MHz), with custom framebuffers supporting high-resolution grayscale and color displays.

**The cost.** DPS was a stack-based interpreter executing PostScript code for every drawing operation. This was expensive:

- ASCII PostScript commands required parsing on every redraw
- The interpreter consumed 3–4 MB of RAM independently (on systems where 8 MB was minimum for grayscale)
- Interactive animation was difficult without careful optimization
- No hardware acceleration — all rasterization was CPU-bound

Performance was a constant complaint. The system was "generally too slow for interactive applications" without careful optimization. Binary preprocessing (bypassing ASCII parsing) and display list caching mitigated but didn't eliminate the overhead.

### Transition to Quartz

When Apple built Mac OS X, they replaced DPS with Quartz — a PDF-based rendering engine. The motivations: avoid ongoing Adobe licensing fees, and address DPS's performance problems on modern hardware. PDF is a natural PostScript evolution: it provides the same imaging model (paths, fills, strokes, transforms, compositing) without the full PostScript interpreter. Quartz maintained DPS's core virtues — resolution independence, device-independent rendering, high-fidelity output — while eliminating the interpretive overhead.

The intellectual lineage is direct: DPS proved that a programmatic imaging model (rather than bitmap blitting) could serve as the foundation of a desktop display system. Quartz proved it could be done efficiently. The entire compositing window manager paradigm — windows as independently rendered surfaces, composed by a central server — originated in DPS's per-window PostScript contexts.

---

## 3.6 The Dock and Application Switching

### NeXTSTEP's Dock

The NeXTSTEP Dock was minimal by later standards: a vertical column of application icons (up to 12, plus the fixed Workspace Manager and Recycler), positioned at the right screen edge. Its design was pure function:

- **Launcher**: click an icon to launch the application (if not running).
- **Switcher**: click a running application's icon to bring it to the front.
- **Status**: running apps showed a solid icon; non-running apps displayed an ellipsis below the icon.

No animation. No magnification. No window minimization into the Dock. No Stacks. The Dock was a list.

The design reflected NeXTSTEP's broader philosophy: _show the information, don't decorate it_. The distinction between running and non-running was communicated by a small typographic cue (the ellipsis), not by a bouncing animation or a glowing dot. The Dock occupied minimal screen real estate. It was always present, always the same size, always in the same place.

### Mac OS X's Dock

Apple reinvented the Dock for Mac OS X. Position: bottom-center (movable). Contents: applications, files, folders, minimized windows. Visual treatment: reflective 3D shelf with perspective, icon magnification on hover, bounce animation on launch, genie effect for window minimization, indicator dots for running apps, Stacks (Leopard) for folder contents.

The Mac OS X Dock collapsed several functions that were previously separate:

- Application switching (NeXTSTEP Dock + Command-Tab)
- Window recovery (previously: click the app's windows in the Window menu)
- File access (previously: Workspace Manager shelf)
- Status (previously: process list in Workspace Manager)

This consolidation was both the Dock's strength and weakness. It became the single interaction point for application management — easy to learn, always visible. But it mixed categories (running apps, minimized windows, file folders) in one spatial strip, making it harder to parse at a glance than the NeXTSTEP Dock's pure-application list.

### What the evolution teaches

The NeXTSTEP Dock was _information-efficient_: maximum information per pixel, minimal decoration. The Mac OS X Dock was _emotionally engaging_: the genie effect, the bounce, the reflections created a sense of physicality and responsiveness. Both are valid design strategies, but they serve different needs.

For a power-user desktop prioritizing density and comprehension (pane's stated goals), the NeXTSTEP model is the stronger reference: show what's running, show what's not, don't animate what doesn't need animating. The genie effect is beautiful; it also takes half a second during which nothing useful happens. The bounce animation communicates "your app is launching"; it also makes the Dock jitter distractingly when multiple apps launch at boot.

---

## 3.7 Distributed Objects

### Architecture

Distributed Objects (DO) was NeXT's system for transparent inter-process communication. The core idea: an object in one process could be proxied into another process, and messages sent to the proxy would be transparently forwarded to the real object across the process boundary.

The components:

**NSConnection.** The communication channel. An NSConnection object in the server process "vended" a root object; client processes obtained a proxy to that object via `[NSConnection rootProxyForConnectionWithRegisteredName:host:]`. NSConnections operated in pairs — one per communicating process. Each pair maintained the bookkeeping for message forwarding, reply routing, and proxy lifecycle.

**NSDistantObject.** The proxy class (a concrete subclass of `NSProxy`). When a client obtained a reference to a remote object, it received an NSDistantObject instance. This proxy captured every message sent to it, serialized the message (selector + arguments), forwarded it through the NSConnection, waited for the result, and returned it to the caller. From the caller's perspective, the proxy _was_ the remote object.

**NSProxy.** The abstract superclass. `NSProxy` didn't inherit from `NSObject` — it was a separate root class, implementing just enough of the runtime protocol to receive messages and forward them. This was architecturally significant: proxies weren't "objects pretending to be other objects"; they were a distinct kind of entity whose purpose was message forwarding.

**NSProtocolChecker.** A security mechanism: a proxy-of-a-proxy that filtered messages, forwarding only those defined in a specific `@protocol`. This restricted what remote clients could do without modifying the vended object.

### Message forwarding in detail

When NSDistantObject received a message:

1. The runtime obtained the method signature (from a cached protocol, or by querying the remote object over the network).
2. The sender's NSConnection encoded the arguments using NSCoding serialization.
3. The encoded message was transmitted to the server's NSConnection.
4. The server decoded the message, invoked it on the real object, encoded the return value (and any exception).
5. The return value was transmitted back and decoded by the client.

Objects passed as arguments were handled in two ways:

- **By copy**: the object was serialized (`encodeWithCoder:`) and a new instance created at the far end.
- **By reference**: a new NSDistantObject proxy was created at the far end, and messages to it traveled back through the connection.

Method signatures could be annotated: `oneway` (no return value, no blocking), `bycopy` (force pass-by-copy), `byref` (force pass-by-reference).

### Trade-offs

**Elegance.** DO made IPC feel like local method calls. For NeXTSTEP's developer audience (building enterprise and academic applications), this dramatically reduced the conceptual overhead of multi-process architectures.

**Limitations.**

- Any message to a proxy could throw an exception (network failure, timeout, server crash). This violated the normal Objective-C convention where messages to valid objects don't throw. Callers had to treat every proxy interaction as potentially exceptional — but the API's whole point was that you _didn't_ think about the proxy as special.
- No encryption. Connections were plaintext.
- Marshaling primitive types required care (the serialization distinguished objects from scalars).
- Not extensible — no way to customize the transport layer.
- Performance overhead: every message required serialization, transmission, deserialization, execution, and reverse. For chatty interfaces, this was painful.

### Comparison with Plan 9 and BeOS

Plan 9's 9P made inter-process communication look like file operations — read, write, stat. The abstraction was the file, not the object. This was lower-level but more universal: any process speaking the file protocol could participate, regardless of language or runtime.

BeOS's BMessage/BMessenger system made IPC look like message passing — explicit, typed messages between identified recipients. Not transparent (you knew you were sending a message), but also not fragile (a send that fails returns an error code, not an exception from an apparently-local method call).

DO occupied a middle ground: higher-level than 9P or BMessage (you called methods, not sent messages), but more fragile (the transparency was a lie when the network failed). The lesson: transparent proxying is seductive but dishonest. The network is not the same as a local function call. Systems that make IPC _explicit but ergonomic_ (BMessage, 9P, pane's session-typed channels) tend to produce more robust designs than systems that hide the boundary.

---

## 3.8 The NeXT Design Philosophy

### What made it feel considered

NeXTSTEP's "everything fits together" quality came from a specific set of architectural decisions, not from aesthetic polish alone:

**One runtime, one mechanism.** Objective-C's dynamic dispatch was the substrate for Interface Builder connections, Services menu discovery, Distributed Objects proxying, responder chain traversal, and plugin loading. These weren't separate features bolted together; they were all instances of the same mechanism (runtime message dispatch) applied to different problems. When the mechanisms are shared, the behaviors are consistent.

**One imaging model.** Display PostScript rendered everything: application UIs, the desktop, printed output. There was no bitmap-level API fighting with a vector API. Every visual element on screen was the output of the same imaging pipeline. This produced visual consistency that went deeper than theming — the anti-aliasing, the font rendering, the compositing all behaved identically everywhere because they were the same code.

**One application model.** Every app was a bundle. Every app had a NIB. Every app used the AppKit. Every app got Services for free. The Workspace Manager could inspect any app's contents, open any app's documents, and display any app's icon. Developers couldn't opt out of the model — the tools didn't offer an alternative. This constraint produced consistency: applications that used the standard model (which was all of them) looked and behaved consistently.

**Opinionated defaults.** NeXTSTEP had one font (Helvetica for UI), one color scheme (monochrome, later restrained color), one menu system (left-side, tear-off), one scroll direction, one window control style. There was no theme engine. The aesthetic was the system's identity. Developers could customize within the system's vocabulary but couldn't replace it.

**Developer productivity as design philosophy.** Jobs's claim that NeXTSTEP reduced UI development from "90% of the time" to "10% of the time" was also a design claim: if building an application is fast and the tools enforce consistency, more applications will exist, and they'll be more consistent. The quality of the developer experience (Interface Builder, AppKit, dynamic loading) was inseparable from the quality of the user experience. (Lane: This insight is something key to keep in mind for pane)

### The specific innovations

The system introduced or popularized:

- The Dock (launcher + switcher)
- The Shelf (quick file access)
- Miller columns (hierarchical file browsing)
- Drag and drop (system-wide, not just within apps)
- Services menu (cross-app composition)
- Inspectors (context-sensitive property panels)
- 3D chiseled widgets (depth through lighting on controls)
- Large full-color icons
- Real-time scrolling and window dragging
- Window modification indicators (the document-edited dot)

Each of these was individually useful. Their power was cumulative: drag and drop worked _with_ Services worked _with_ the pasteboard worked _with_ the Inspector. The features composed because they shared the same substrate.

### Jobs's design intuition

Jobs described NeXTSTEP as aiming for a system where "the line of code that the developer could write the fastest, maintain the cheapest, and that never breaks for the user, is the line of code the developer never had to write." This was a design statement disguised as an efficiency claim: the best UI is one that emerges from the framework's defaults, not from per-application custom code. If the framework's defaults are good, the resulting applications will be good — without each developer independently solving the same problems.

The system's aesthetic was described as "clean, minimalist motifs" with "a distinctive, minimalist aesthetic" — monochrome displays with precise typography and restrained color. This was the opposite of the contemporary GUI fashion (Windows 3.1's busy, colorful interfaces). NeXTSTEP looked _serious_ — designed for people who would spend all day looking at it. The information density was high, the decoration was low, and every element had a functional purpose.

---

## 3.9 Aqua Visual Design

### The design language

Aqua was unveiled at Macworld San Francisco on January 5, 2000. Steve Jobs introduced it as the interface "you wanted to lick." The name evoked water; the design was themed to replicate it.

**Visual elements:**

- **Translucency.** Window elements, the menu bar, and floating panels used alpha blending for layered depth effects. Background content showed through foreground elements, communicating spatial relationships. The translucency was decorative (the pinstripe texture showed through title bars) but also functional (floating panels let you see what was beneath them).

- **Gel buttons.** The signature Aqua element: glossy, three-dimensional buttons that appeared to be made of colored gel or candy. Lit from above, with a highlight on the upper half and a shadow on the lower half. The "pulsing" default button (a blue gel button that gently pulsed to indicate it was the default action) became iconic.

- **Traffic light window buttons.** Three small circles at the top-left of every window: red (close), yellow (minimize), green (zoom/fullscreen). Color-coded for immediate recognition. The circles contained × – + symbols on hover, a micro-interaction detail.

- **Pinstripe texture.** Subtle vertical lines in window backgrounds, giving surfaces a textured, non-flat quality. Inspired by the textured plastics of contemporary Apple hardware (the slot-loading iMac, the Power Mac G4). Later criticized as "annoying" and "overwhelming," eventually removed.

- **Drop shadows.** Every window cast a soft shadow, establishing visual hierarchy. Shadows were rendered by the Quartz Compositor — the first mainstream compositing window manager to generate per-window shadows automatically.

- **Rounded corners.** Windows, buttons, text fields, and most interactive elements had rounded corners. Carried over from the original 1984 Macintosh, but now rendered at higher fidelity with anti-aliasing.

- **The Dock.** A reflective 3D shelf with icon magnification, bounce animations, and the genie effect for window minimization. The Dock was Aqua's most visible element — and its most controversial.

- **The genie effect.** Window minimization animated the window "sucking" into its Dock icon in a fluid, warping animation. Pure spectacle — but spectacle that communicated _where the window went_. Spatial awareness through animation.

- **Color palette.** Blue, white, and grey as the principal colors. Toolbars and sidebars grey/metallic, backgrounds white, buttons accented with bright blue. The palette was cooler and cleaner than BeOS's warm, saturated colors. An optional "Graphite" mode offered neutral grey alternatives for professionals who found the blue distracting.

### What Aqua was trying to communicate

The design was a deliberate contrast to the computing aesthetic of the late 1990s. Cordell Ratzlaff, the Aqua design lead, asked: "What's the opposite of a computer interface?" and came up with "candy, liquor, and liquids" — substances that are sensory, pleasurable, and organic. The design philosophy was that a computer interface should invite interaction rather than demand it, should feel like a crafted physical object rather than a utilitarian tool. (Lane: "a computer interface should invite interaction rather than demand it" this is a core design principle I share)

Jobs's "lickable" comment was not throwaway marketing. It expressed a specific design position: interfaces should have material qualities — texture, depth, translucency, weight — that make them feel real. Flat rectangles of solid color feel abstract. Gel buttons with highlights and shadows feel like objects you can press. The question is whether this material quality serves comprehension or merely entertains. (Lane: yes, this part)

### The principles

Apple's Human Interface Guidelines for Mac OS X (2001) articulated ten design principles inherited from the original 1986 Macintosh HIG, reaffirmed through the Aqua era:

1. **Metaphors** — use familiar concepts (desktop, folders, documents)
2. **Direct manipulation** — users act on objects, not through commands
3. **See-and-point** — visual interaction, not memorized commands
4. **Consistency** — same operations work the same way everywhere
5. **WYSIWYG** — what you see is what you get (inherited from DPS/Quartz)
6. **User control** — the user initiates actions, the system responds
7. **Feedback** — every action has a visible result
8. **Forgiveness** — undo, confirmation, non-destructive defaults
9. **Perceived stability** — the interface feels solid and predictable
10. **Aesthetic integrity** — visual design supports function, not just decoration

Aesthetic integrity was the principle Aqua most visibly tested. The gel buttons, the translucency, the genie effect — were these decoration masquerading as function, or genuine communication of interface affordances? The answer was: both. The gel buttons' three-dimensionality communicated "pressable" (affordance). The translucency communicated "this floats above that" (spatial relationship). The genie effect communicated "this window went there" (spatial awareness). But the pinstripe texture communicated nothing functional. The pulsing button was beautiful but consumed attention.

The early Aqua (10.0–10.2) was maximal — every surface had texture, every transition had animation, every element had depth. Over subsequent releases, Apple dialed back: pinstripes were removed (10.5), brushed metal was eliminated (10.5), translucency was reduced (10.2), and the overall trajectory was toward less decoration and more restraint. By Yosemite (10.10), Aqua had evolved into an essentially flat design with selective translucency — closer to NeXTSTEP's restraint than to 10.0's exuberance. (I _loved_ early Aqua and im super interested in it as a design reference point)

### Performance reality

Aqua 10.0 was brutally slow. The translucency effects, the window compositing, the animations — all running on hardware barely capable of supporting them. "The original version was terribly slow, making a perfectly speedy Mac feel like it was dipped in molasses." Deliberately slow animations showcased the effects but made the system painful to use. 10.1 was the first usable version. 10.2 (Jaguar) introduced Quartz Extreme (GPU-accelerated compositing), which made the visual effects actually performant.

This is a recurring pattern: visual ambition outrunning hardware capability. The design was right; the implementation needed hardware to catch up. Pane, targeting modern GPUs with Vulkan/OpenGL via smithay, won't face this constraint — but the lesson is: design the visual language for the hardware that exists, not the hardware you wish existed.

---

## 3.10 Quartz Compositor

### Architecture

Quartz Compositor was the display server and compositing window manager in Mac OS X — the first mainstream compositing window manager. It replaced the immediate-mode drawing model (where applications drew directly to the framebuffer) with a retained-mode compositing model (where applications drew to off-screen buffers and the compositor assembled them).

**Window rendering.** Each application rendered its window contents to a backing store — an off-screen bitmap. The compositor read these backing stores and assembled them into the final screen image, respecting z-order, transparency, and visual effects. The key abstraction: a window was not a region of the screen; it was a texture that the compositor placed.

**The window server.** A single system-wide process (WindowServer) managed:

- Window lifecycle: creation, resizing, moving, stacking
- Compositing: layering window textures according to z-order
- Effect application: drop shadows, transparency blending (Porter-Duff rules)
- Event routing: receiving input events, determining which window was hit, dispatching to the owning process
- Focus and layering rules

**Backing store management.** Dirty region tracking: only changed rectangular areas were recomposited. Idle windows could be compressed (3–4x storage efficiency) and decompressed incrementally for affected regions. This was essential for the early hardware.

**Event routing.** Input events (keystrokes, mouse clicks) entered via HID drivers. WindowServer annotated them with timestamps and context (which window, which position), then dispatched to the appropriate application's run loop, where they became NSEvent objects (Cocoa) or Event Manager calls (Carbon).

### GPU acceleration

**Quartz Extreme (10.2 Jaguar).** Offloaded compositing from CPU to GPU using OpenGL. Each window's backing store became an OpenGL texture. The GPU composed the textures with hardware-accelerated blending. Requirements: compatible GPU with at least 16 MB VRAM. This was the inflection point where Aqua's visual ambition became actually usable.

**Effect pipeline.** With GPU compositing:

- Drop shadows: window silhouettes rendered into separate buffers, offset, Gaussian-blurred, composited behind the window. No CPU involvement.
- Translucency: alpha blending between window textures, performed by the GPU.
- The genie effect: mesh deformation of the window texture, rendered as a GPU animation.
- Exposé (10.3): all window textures scaled and positioned by the GPU simultaneously.

### What compositing enabled

**Visual hierarchy through depth.** Before compositing, windows were opaque rectangles that obscured each other completely. With compositing, windows could be translucent, cast shadows, and visually communicate their stacking order. Drop shadows established "this window is above that one" without requiring the user to mentally track overlap.

**System-wide visual consistency.** The compositor rendered drop shadows, rounded corners, and window chrome. Applications didn't draw these — the system did. This meant every window, regardless of toolkit (Cocoa, Carbon, X11), had consistent chrome.

**Animation as feedback.** The genie effect, window scaling, fade transitions — all were compositor operations on window textures. Applications didn't need to implement these. The compositor could animate any window because it controlled the window's texture.

**Accessibility.** Because the compositor maintained a semantic model of all windows and their positions, assistive technology could query the compositor for window information. The accessibility API (introduced in 10.2) provided programmatic access to the window hierarchy, enabling screen readers to describe the visual layout. This would be impossible in a non-compositing system where applications drew directly to the framebuffer.

### Security

Sandboxing: applications could not directly access the framebuffer. All graphics and input operations routed through WindowServer. This provided isolation — one application couldn't read another's window contents — and consistent rendering behavior.

### The compositing paradigm's lesson

The compositor model separated _what_ an application renders from _how_ it appears on screen. The application wrote pixels to a buffer. The compositor decided where those pixels went, how they were blended, what effects were applied. This is the same separation pane-comp implements: pane-native clients write cell content; the compositor rasterizes, positions, and renders. The application controls content; the compositor controls presentation.

---

## 3.11 Cocoa vs Carbon

### Two worlds

When Apple built Mac OS X from NeXTSTEP, it faced a fundamental problem: NeXTSTEP's native API (renamed Cocoa) was excellent but unfamiliar to existing Mac developers. The millions of lines of Classic Mac OS code couldn't run natively on the new system.

**The original plan (Rhapsody).** Apple initially proposed two boxes:

- **Yellow Box**: NeXTSTEP's APIs (Foundation, AppKit) — the native development environment.
- **Blue Box**: a Classic Mac OS emulator running old Mac apps in a compatibility window.

This was rejected by the developer community. Microsoft, Adobe, and other major developers refused to rewrite their applications in Objective-C/Yellow Box. Running in the Blue Box ("the penalty box") was unacceptable — it looked different, performed worse, and couldn't access new OS features.

**The Carbon compromise.** Apple went through the Classic Mac OS API and removed everything incompatible with preemptive multitasking and memory protection — shared-memory globals, direct hardware access, cooperative multitasking assumptions. What remained was Carbon: a modernized subset of the Mac Toolbox that could run on both Classic Mac OS (8.1+) and Mac OS X. Carbon applications were native Mac OS X citizens — they got Aqua chrome, could use Quartz, ran in protected memory — but they were written in C/C++ using procedural APIs rather than Objective-C using object-oriented frameworks.

Under the hood, Apple built a shared substrate: **Core Foundation** (CF). Many OpenStep Foundation classes (NSString, NSArray, NSDictionary) were reimplemented in pure C as CFString, CFArray, CFDictionary. Cocoa's Foundation called CF; Carbon's APIs also called CF. This "toll-free bridging" meant NSString and CFString were the same object in memory — Cocoa and Carbon code could pass strings between each other without conversion.

### What was gained

- **Developer adoption.** The Mac survived. Without Carbon, Adobe, Microsoft, and hundreds of other developers would not have shipped Mac OS X software. Photoshop, Office, and the rest of the Mac software ecosystem came to Mac OS X via Carbon.
- **Gradual migration path.** Developers could port to Carbon (months of work), ship a Mac OS X native app, then incrementally adopt Cocoa features over time. Many eventually rewrote in Cocoa. Some never did.
- **Shared infrastructure.** Core Foundation, Quartz, Core Audio, Core Data — the underlying technology was shared. Carbon and Cocoa applications could coexist and interoperate.

### What was lost

- **Services didn't work across the boundary.** Cocoa Services depended on the Cocoa pasteboard and responder chain. Carbon apps used a different event model. Services only worked fully between Cocoa apps — killing the universality that made NeXTSTEP Services powerful.
- **Framework investment was split.** Apple had to maintain two API surfaces for years. New features (Cocoa Bindings, Core Data UI, resolution-independent UI) often shipped for Cocoa first or only.
- **The NeXTSTEP vision was diluted.** NeXTSTEP was a coherent system where one programming model, one runtime, one set of conventions governed everything. Mac OS X was two systems grafted together. The coherence that made NeXTSTEP feel "integrated" was partially lost.

### Long-term resolution

Carbon was deprecated over roughly two decades. The final blow was macOS Catalina (2019), which dropped 32-bit support, eliminating the last Carbon-only applications. By that point, Cocoa (and increasingly Swift/SwiftUI) was the only development path. The NeXTSTEP heritage won — but it took 20 years.

### The lesson

You cannot have system-wide integration if applications speak different protocols. NeXTSTEP's coherence came from universal participation in one programming model. Mac OS X's two-world problem (Cocoa vs Carbon) fragmented that coherence. Pane's architecture avoids this by design: the pane protocol is the only way to be a native pane client. Legacy Wayland apps get a wrapper, but they don't pretend to be pane-native. The boundary is explicit, not a source of subtle incompatibilities.

(Lane: The general problem that they are attempting to address still applies to us, we have to think about a way to have a nice and seamless-enough UX because we're departing from a linux ecosystem. My hope is that we can lean into the proliferation of nice TUI apps with good UX, so that if the user utilizes a majority textual interface, this will be a seamless experience and easy to display, not nearly as jarring as the experience of using apps written for different GUI toolkits. To mitigate the latter, we could have themes that resemble the widget appearance and help bring it to uniformity with our design sensibility)

---

## 3.12 Exposé

### The innovation

Exposé (Mac OS X 10.3 Panther, October 2003) was a spatial window management feature: press F9, and every open window shrank and rearranged itself so all windows were visible simultaneously on screen, without overlapping. Mouse over a window to see its title. Click to bring it forward.

Three modes:

- **All Windows (F9):** every window from every application, scaled to fit.
- **Application Windows (F10):** all windows of the current application, scaled to fit.
- **Show Desktop (F11):** all windows slide off-screen, revealing the desktop.

### Why it worked

**Spatial awareness.** Exposé used the entire screen. Windows were as large as they could be while still fitting without overlap. The zooming/sliding animation showed each window moving from its actual position to its Exposé position, maintaining the spatial relationship. You could see _where_ a window was, not just _that_ it existed. This is fundamentally different from Alt-Tab (a linear list with no spatial information) or the taskbar (a label with no visual content).

**Live windows.** Exposé didn't show static thumbnails. The windows were live — still updating in real time even while scaled. A video kept playing. A progress bar kept progressing. This maintained the temporal connection to the content.

**Visual recognition.** Because the windows were actual rendered content (not icons or labels), you could recognize a window by what it looked like, not just by its title. The visual cortex's pattern recognition is faster than reading text labels.

**Smooth animation.** The transition was animated — windows smoothly scaled and repositioned. This animation wasn't decoration; it was information. It showed you the mapping between "normal" window positions and "Exposé" positions, enabling you to track specific windows through the transition.

**Hardware acceleration.** Exposé was a compositor operation: the GPU scaled and repositioned window textures. It was fast because it operated on the same data the compositor already maintained. No new rendering was required — just transformation of existing textures.

### What was lost (Mission Control)

In 10.7 Lion, Apple merged Exposé with Spaces into Mission Control. The result was widely criticized. Mission Control grouped windows by application rather than showing all windows individually; it mixed virtual desktop thumbnails with window thumbnails; and the spatial awareness was degraded because windows of the same application overlapped.

The lesson is precise: Exposé worked because it preserved individual identity (every window was separate), spatial relationships (windows moved smoothly from their positions), and visual recognition (full window content visible). Mission Control broke all three by grouping, overlapping, and mixing metaphors.

### Relevance

Pane is a tiling window manager — Exposé's problem (too many overlapping windows to track) doesn't directly arise. But the principle applies: when showing users an overview of their workspace, preserve spatial relationships, maintain visual identity, and animate transitions that show the mapping between overview and detail. Tag-based visibility switching in pane could use similar principles: smooth transitions that show windows appearing/disappearing in their layout positions, rather than instant cuts.

(Lane: this is neat, just to be sure though: I want to support floating and tiled views, sometimes mixed depending on the app)

---

## 3.13 The Services Menu in Mac OS X

### Evolution from NeXTSTEP

Mac OS X inherited the Services menu from NeXTSTEP with enhanced technical machinery. The Cocoa implementation used:

- `registerServicesMenuSendTypes:returnTypes:` for type declaration
- `validRequestorForSendType:returnType:` for context-sensitive menu filtering (via responder chain)
- `writeSelectionToPasteboard:types:` and `readSelectionFromPasteboard:` for data transfer
- `NSPerformService()` for programmatic invocation

The architecture was sound. The pasteboard-based data transfer was flexible. The type matching was precise. The responder chain integration was elegant.

### Why it never reached its potential

**The two-world problem.** Carbon applications didn't participate in the Cocoa Services system. In the early Mac OS X years, most major applications were Carbon. Services only worked between Cocoa apps, which meant the feature was invisible to most users. (Lane: again, this is our biggest risk also)

**Discoverability.** The Services menu was buried: Application menu → Services. In a menu system where most users never ventured past File and Edit, this was a death sentence. No UI affordance indicated that Services existed or what was available.

**Pre-Snow Leopard clutter.** Before 10.6, the Services menu showed every registered service regardless of context, with most items greyed out. A menu of 30+ items, 28 of them disabled, teaches users to ignore the menu.

**Post-Snow Leopard invisibility.** After 10.6, services were filtered to show only applicable items. Better UX when you found the menu — but now you couldn't even discover what services _existed_ in other contexts. The problem shifted from "too much noise" to "not enough signal."

**No composition visibility.** Unix pipes are visible: `cat file | grep pattern | sort`. Services had no equivalent visibility. Each invocation was atomic: select → invoke → done. You couldn't see the pipeline, build a pipeline, or save a pipeline for reuse.

**Reliability issues.** The dynamic discovery mechanism was fragile. The menu would get stuck "building," fail to find services, or lose items. Restarting sometimes fixed it; sometimes didn't.

### What it would have needed to succeed

1. **Universal participation.** Every application, regardless of toolkit, must participate. The Carbon/Cocoa split killed this. (Lane: this is why I'm being thoughtful to building the system such that it is easy to wrap non-pane applications in such a way that they expose as much of their interfaces to pane's API/system services as is possible to extract)
2. **Prominent placement.** The right-click context menu (added later in Mac OS X) was better than the application menu. But even better would be inline affordance: a visual cue on selected text that says "actions are available."
3. **Composition UI.** The ability to chain services: select text → transform → route → act. Visible, saveable, repeatable. Automator (10.4) was Apple's attempt at this, but it was a separate application, not integrated into the selection flow.
4. **Pipeline visibility.** Show the user what's happening: "this text → this service → this result." The pipeline should be as visible as the content.

### Implications for pane

Pane's routing system (pane-route) occupies the same design space as Services, but with key differences. B3-click on any text sends it to the router, which pattern-matches against rules and service registrations. This is always the same gesture, always discoverable (any text can be B3-clicked), and the multi-match UI (a transient scratchpad listing options as B2-clickable text) makes available operations visible. The roster's service registry (`content_type_pattern, operation_name, description`) is the type-driven matching that Services had, but queryable by the router rather than buried in a menu.
What pane doesn't yet have (and should consider): composition visibility. When a B3-click on `parse.c:42` resolves to "open in editor at line 42," the pipeline is: text → router → rule match → editor. This pipeline is invisible. Making it visible — even as a brief flash of the rule that matched — would teach users how routing works and how to customize it.

(Lane: I'll bring this up later, but please dont be so insistent on taking the literal mouse button approach. most people are using a trackpad with one (visible) button, the design consideration is different, although we ought to be thoughtful for gestures and yes even how mouse buttons work, I feel like we would be overfitting right now to be so insistent on the detail that different mouse clicks do different things at X widget. that's a granular detail we dont need to think about at this juncture)

---

## 3.14 Accessibility Architecture

### Mac OS X's approach

Mac OS X introduced its accessibility architecture in 10.2 (Jaguar). The design:

**Client-server model.** Accessibility clients (screen readers, input devices, automation tools) queried applications through the accessibility API. Applications exposed their UI elements through a hierarchy: windows → views → controls, each annotated with roles (button, text field, slider), values, labels, and relationships.

**Standard controls for free.** AppKit controls (NSButton, NSTextField, etc.) implemented accessibility automatically. Developers got basic accessibility by using standard controls. Custom controls required explicit protocol adoption.

**Role-based semantics.** Every UI element exposed a role (what it is), a value (its current state), a label (what it's called), and actions (what can be done to it). Accessibility clients used this semantic model to describe the interface to users who couldn't see it. (Lane: this is similar to my "semantic interfaces" idea)

**Compositing enabled it.** Because the Quartz Compositor maintained a model of all windows and their positions, the accessibility framework could query the window hierarchy programmatically. In a non-compositing system (where applications drew directly to the framebuffer), the system had no knowledge of what was on screen. Compositing gave the system a model of the visual layout that accessibility could interrogate. (Lane: interesting, curious about the situation of wayland WRT to this)

**VoiceOver (10.4 Tiger).** Apple shipped a full screen reader built into the OS — the first major OS to include one without additional installation. VoiceOver traversed the accessibility hierarchy, reading element descriptions and enabling keyboard-driven navigation.

### Lessons

The compositing model doesn't just enable visual effects — it enables accessibility by giving the system a semantic model of what's on screen. Pane's compositor (pane-comp) maintains a layout tree of panes, each with a tag line, a body, and a protocol connection. This is richer than "a list of window textures" — it's a structured model that could expose pane identity, tag line content, and cell grid structure to accessibility clients. The widget content model (with semantic roles: buttons, labels, lists) is directly mappable to accessibility roles. Cell grid panes (terminal-like content) remain a challenge — there's no semantic structure beyond a grid of characters — but this is the same challenge terminals have always posed.

---

## 3.15 Synthesis: What NeXTSTEP and Early Aqua Teach About Desktop Interface Design

### What makes a desktop feel integrated vs assembled from parts

NeXTSTEP felt integrated because it was built on shared mechanisms. One runtime (Objective-C), one imaging model (Display PostScript), one application model (bundles + NIBs + AppKit), one inter-app mechanism (pasteboard + Services). These weren't policy choices — they were architectural decisions that made alternative approaches difficult. Integration emerged because there was no way to _not_ integrate.

Mac OS X felt assembled because it was. Cocoa and Carbon were two API worlds sharing an underlying substrate (Core Foundation, Quartz) but diverging at the application level. Services worked in Cocoa but not Carbon. The developer experience of writing a Cocoa app and a Carbon app were fundamentally different. The user could feel this: Cocoa apps had smoother, more consistent behaviors; Carbon apps felt like they were wearing an Aqua costume over a Classic Mac OS body.

The rule is: **integration is an architectural property, not a cosmetic one.** You can't paint integration onto a fragmented system. The components must share the same protocol, the same conventions, the same mechanisms — or the seams will show. Pane's design enforces this: one protocol (pane-proto), one rendering model (cell grid + surface compositing), one routing mechanism (pane-route), one service registry (pane-roster). Legacy Wayland apps get a wrapper, but they're explicitly not native pane clients — the boundary is architectural, not cosmetic.

### The relationship between visual refinement and usability

Aqua proved that visual refinement matters — but also that it has a cost, and the cost must be paid consciously.

**What refinement communicates:**

- Drop shadows communicate depth (which window is above which).
- Translucency communicates spatial relationship (this panel floats above that content).
- Rounded corners communicate interactivity (this element is approachable, interactive).
- Animation communicates causality (this window went there, this button was pressed).
- Color communicates state (active, inactive, default, warning).

Each of these is functional communication. The visual treatment carries information that helps the user understand the interface's structure and behavior.

**What refinement costs:**

- Performance. Aqua 10.0 was unusable because the visual effects exceeded the hardware's capacity. Every decorative effect has a rendering cost.
- Attention. The pulsing default button, the pinstripe texture, the genie effect — each demands a slice of the user's attention. When every element is visually rich, nothing stands out.
- Maintenance. Complex visual effects (gel buttons, translucency, shadows) require careful implementation across every control, every state, every resolution.

**The sweet spot.** NeXTSTEP was too austere for mass-market appeal but excellent for power users who spent all day in the system. Aqua 10.0 was too exuberant — visual richness overwhelming functional communication. By 10.2, Apple had found a better balance: shadows and translucency where they communicated depth, restrained color, reduced animation.

Pane's stated aesthetic — "BeOS's information density, Mac OS X Aqua 1.0's rendering refinement and warmth" — targets a specific point on this spectrum: dense like NeXTSTEP/BeOS, warm and refined like Aqua, but not exuberant. Matte bevels, not gel buttons. Selective translucency on floating elements, not universal. This is essentially the Aqua 10.2 balance point, with BeOS density substituted for Aqua's spaciousness.

### What the Services menu got right about cross-application composition, and why it didn't succeed

**What it got right:**

1. The abstraction: "selected content" → "operation" → "result." This is the fundamental composition pattern for desktop interaction.
2. Type-driven matching: the system knows what operations are available for the current content type.
3. Pasteboard as interchange: a universal, typed data exchange medium between applications.
4. Universality (on NeXTSTEP): every app participated because every app used the same framework.

**Why it didn't succeed on Mac OS X:**

1. Loss of universality (Carbon apps didn't participate).
2. Discoverability failure (buried in menus, no affordance).
3. No composition visibility (you couldn't see or build pipelines).
4. Reliability issues (the menu was buggy).

The core idea — system-wide, type-driven composition of operations on selected content — is one of the best ideas in desktop design. It failed on Mac OS X for implementation and ecosystem reasons, not because the idea was wrong.

### What Aqua's visual language teaches about depth, warmth, and information hierarchy

**Depth as hierarchy.** Aqua's most durable contribution is the use of depth (shadows, translucency, z-order) to communicate information hierarchy. The window in front casts a shadow on the window behind. The floating panel is translucent, showing the content beneath. The button appears to rise from the surface, inviting pressing. These are not decorations — they are spatial metaphors that the visual cortex interprets naturally.

**Warmth as welcome.** Aqua's blue/white/grey palette, combined with soft shadows and rounded corners, created an interface that felt welcoming rather than utilitarian. This was a deliberate contrast to the grey, angular interfaces of Windows and Unix desktops. "Warm" in this context means: soft lighting (not harsh), rounded shapes (not angular), saturated but not neon colors, and smooth transitions (not instant cuts). Warmth is about reducing the cognitive hostility of the interface.

**Information hierarchy through restraint.** The most important lesson from Aqua's evolution is that information hierarchy requires restraint. When everything has a shadow, nothing stands out. When everything is translucent, depth becomes noise. The effective visual hierarchy comes from contrast: some things are visually rich (the active window, the focused control, the default button), and other things are visually quiet (the background, the inactive controls, the chrome). Aqua 10.0 made everything rich; Aqua 10.2 started to learn restraint; by Yosemite, the lesson was fully absorbed.

For pane: depth through lighting on controls (matte gradients, 1px highlight/shadow edges), selective translucency on floating elements only, warm saturated palette for accents against a warm grey base, color as information (dirty, focus, error) not decoration. This is the distillation of what Aqua got right, filtered through the density and restraint of BeOS/NeXTSTEP.

### The Frutiger Aero intersection

Frutiger Aero — the retrospective name for the design aesthetic of roughly 2004–2013 — sits at the intersection of the ideas discussed here. Its defining qualities: glossy/transparent surfaces mimicking glass and water, gradients and shadows giving elements three-dimensional touchable quality, bright saturated colors (especially blues and greens), translucent overlays adding depth without overwhelming, and an overall feeling described as "warm, lush, and optimistic."

Frutiger Aero was the mainstream evolution of ideas that Aqua pioneered. Where Aqua 10.0 was maximal and exuberant, Frutiger Aero (as seen in Windows Vista/7's Aero, later macOS releases, and product design of the era) found a more sustainable balance: depth and warmth serving comprehension rather than spectacle. The aesthetic communicated "this technology is for humans" — not cold and utilitarian, not childish and decorated, but polished, inviting, and clear.

Pane's stated aesthetic — "what if Be Inc. had continued into the 2000s" — is asking what Frutiger Aero would look like if built by engineers who valued information density over visual spaciousness, matte finishes over glossy ones, and compositional power over simplified interaction. The answer: dense layouts, matte beveled controls, selective translucency, warm palette, typographic clarity, and every visual element carrying functional information. Not NeXTSTEP's austerity, not Aqua's exuberance, but the considered middle ground where visual refinement serves the work.
