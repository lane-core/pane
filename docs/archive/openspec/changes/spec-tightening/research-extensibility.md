# Extensibility Models and Plugin Ecosystems — Research

Research for pane spec-tightening. Covers extensibility mechanisms across historical and modern systems, focusing on the design problem itself: what makes extension work, what breaks, and what the structural requirements are for a "vast ecosystem over safe OS surface."

Sources:

- Mastering Emacs, "Why Emacs Has Buffers": <https://www.masteringemacs.org/article/why-emacs-has-buffers>
- Nullprogram, "The Limits of Emacs Advice": <https://nullprogram.com/blog/2013/01/22/>
- EmacsWiki, Advice: <https://www.emacswiki.org/emacs/Advice>
- Emacs nadvice.el source: <https://github.com/emacs-mirror/emacs/blob/master/lisp/emacs-lisp/nadvice.el>
- Emacs mode hooks: <https://emacsdocs.org/docs/elisp/Mode-Hooks>
- MELPA: <https://melpa.org/>
- The Be Book, Translation Kit: <https://www.haiku-os.org/legacy-docs/bebook/TranslatorAddOns.html>
- The Be Book, Input Server: <https://www.haiku-os.org/legacy-docs/bebook/TheInputServer_Introduction.html>
- Haiku API, BArchivable: <https://www.haiku-os.org/docs/api/classBArchivable.html>
- Pike, "The Plumber": <https://doc.cat-v.org/plan_9/4th_edition/papers/plumb>
- Acme man page (plan9port): <https://9fans.github.io/plan9port/man/man4/acme.html>
- Alex Karle, "Using the Plan 9 Plumber to Turn Acme into a Git GUI": <https://alexkarle.com/blog/plan9-acme-git-gui.html>
- Apple, NSServices: <https://developer.apple.com/documentation/bundleresources/information-property-list/nsservices>
- NeXTSTEP AppKit Installing New Services: <https://wiki.preterhuman.net/NeXTSTEP_AppKit_Installing_New_Services>
- Robservatory, "The Useful Yet Useless Services Menu": <https://robservatory.com/the-useful-yet-useless-services-menu/>
- VS Code Extension Host: <https://code.visualstudio.com/api/advanced-topics/extension-host>
- VS Code Extension System (DeepWiki): <https://deepwiki.com/microsoft/vscode/3-product-configuration-and-policy>
- Zellij WASM plugin system: <https://zellij.dev/news/new-plugin-system/>
- Zellij plugin system (DeepWiki): <https://deepwiki.com/zellij-org/zellij/4-cli-and-commands>
- Extism framework: <https://github.com/extism/extism>
- Neovim extension system (DeepWiki): <https://deepwiki.com/neovim/neovim/4-extension-and-plugin-system>
- NixOS module system deep dive: <https://nix.dev/tutorials/module-system/deep-dive.html>
- Chrome extension permissions architecture: <https://voicewriter.io/blog/the-architecture-of-chrome-extension-permissions-a-deep-dive>
- MDN, Content Scripts: <https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Content_scripts>
- Stinnett, "Extensibility via Capabilities and Effects": <https://convolv.es/blog/2024/11/01/capabilities-effects/>

---

## 1. Emacs: The Maximalist Model

### How Emacs extension works

Emacs is a Lisp machine wearing an editor costume. The C core provides a bytecode interpreter, a display engine, buffer management, and OS-level I/O. Everything else — every editing command, every mode, every UI element — is Elisp running inside that interpreter. The distinction between "the editor" and "extensions" is essentially nonexistent: `forward-char` is an Elisp function, and so is a user's custom keybinding. Extension happens by writing more Lisp into a system that is already entirely Lisp.

**Buffers as universal abstraction.** The buffer is Emacs's fundamental data structure. A buffer is not a file view — it is a mutable text region with associated metadata (point, mark, text properties, overlays, local variables, modes). Files, network streams, process output, shell sessions, compilation results, REPL interactions, help text, and ephemeral scratch data all live in buffers. The critical property: the buffer you see on screen *is* the data structure Elisp operates on. There is no separate model/view split. When Elisp inserts text, the display updates. When you type, Elisp functions execute. This identity between representation and interaction is what makes Emacs composable — any function that works on text works on any buffer, regardless of what the buffer "is."

**Modes (major and minor).** Every buffer has exactly one major mode that defines its primary behavior: what keymap is active, what syntax highlighting rules apply, what indentation function runs. `python-mode`, `org-mode`, `dired-mode` — each is a function that sets up buffer-local state. Minor modes are orthogonal behaviors that stack: `flycheck-mode` (syntax checking), `company-mode` (completion), `visual-line-mode` (soft wrapping). A buffer can have any number of active minor modes. Modes compose by layering keymaps and hook functions. The `define-derived-mode` macro creates a new major mode by inheriting from a parent, running the parent's setup, then applying the child's customizations.

**Hooks.** The primary extension point. A hook is a list of functions called at a specific event. `after-save-hook` runs after saving. `python-mode-hook` runs when entering Python mode. Users extend behavior by adding functions to hooks with `add-hook`. Mode hooks chain: a derived mode runs its parent's hooks first (coordinated by `run-mode-hooks` and `delay-mode-hooks` to prevent premature execution in derived mode hierarchies). Hooks are dynamic — any code can add or remove hook functions at any time. There is no static declaration of what hooks exist; by convention major modes provide `modename-mode-hook`, but any variable whose name ends in `-hook` is a hook.

**Advice.** The mechanism for modifying existing functions without replacing them. The modern `nadvice.el` system (replacing the older `advice.el`) lets you wrap any function with `:before`, `:after`, `:around`, or `:filter-args` behavior using `advice-add`. This is monkey-patching with a structured API — you can intercept any function call, inspect arguments, modify return values, or replace the function entirely. It is explicitly documented as a last resort ("check whether there are any hooks or options to achieve what you want to do before using advice"). The old `defadvice` system was more complex and less composable; `nadvice.el` simplified it to function composition.

**Keymaps.** Hierarchical key-to-command bindings. Each mode defines a keymap. Minor mode maps take priority over major mode maps, which take priority over the global map. Prefix keys create sub-keymaps. Users can rebind anything. Keymaps are first-class values — they can be passed around, composed, and modified programmatically.

### What makes the Emacs ecosystem thrive

**The universal abstraction.** Buffers mean that any package that operates on text composes with any other package that operates on text. An email client (mu4e), a git interface (magit), a file manager (dired), a web browser (eww), a music player (emms) — they all live in buffers. The cursor works. Search works. Kill/yank works. Keyboard macros work. Bookmarks work. Any command that operates on text operates in these contexts. This is the deepest reason for Emacs's ecosystem: packages compose for free because they share a universal substrate.

**MELPA and distribution.** MELPA (Milkypostman's Emacs Lisp Package Archive) provides a package repository with automated builds from Git repos. Users install packages with `M-x package-install`. The barrier to publishing is low: submit a recipe (a pointer to a Git repo with a naming convention and autoload cookies). As of 2025, MELPA hosts ~5,500 packages. GNU ELPA is the official repository requiring FSF copyright assignment; MELPA is the community repository with no such requirement, which is why it's larger.

**Everything is inspectable and modifiable at runtime.** You can `describe-function` any function, jump to its source, modify it, and `eval-defun` the modification — all without restarting. Variables, keymaps, hooks, faces, modes — everything is live and introspectable. This collapses the development loop: the system you're extending is the system you're running.

**The "Emacs as OS" metaphor.** Emacs provides: a process manager (start, stop, communicate with subprocesses), a filesystem interface (dired, tramp for remote files), a window manager (frames, windows, buffer display rules), a network stack (url.el, eww), an init system (init.el, `emacs-startup-hook`), a package manager (package.el), IPC (emacsclient), and a scripting language that controls all of it. The metaphor is not hyperbole — Emacs is a userspace environment that mediates between the user and the OS, with the buffer as the universal abstraction where the OS's console was.

### The costs

**No type safety.** Elisp is dynamically typed. A hook function that expects a string argument will silently break if passed a list. Advice that wraps a function with the wrong arity produces errors only at call time. Package updates that change function signatures break dependent packages silently. There is no compile-time check that modes, hooks, and advice compose correctly. The `cl-defstruct` and `eieio` systems add optional structure, but the core extension mechanisms (hooks, advice, keymaps) are untyped.

**Namespace collisions.** Elisp has a single global namespace. Packages prefix their symbols (`magit-`, `org-`, `helm-`), but this is convention, not enforcement. Nothing prevents two packages from defining the same function name. Buffer-local variables are global symbols with buffer-local bindings — collisions require careful naming discipline. The `require`/`provide` system tracks loaded features but doesn't scope names.

**Monkey-patching and version fragility.** Advice is open-heart surgery: wrapping a function means your code depends on the exact behavior and argument structure of the wrapped function. When Emacs core or a dependency changes the function, your advice breaks. This is why Emacs upgrades regularly break packages — the extension mechanism is *inherently* coupled to implementation details. The byte-compilation limit makes it worse: some primitive functions are compiled to bytecode instructions, and advice on those functions silently fails in byte-compiled code because the function call is compiled away (the `narrow-to-region` problem documented by Wellons).

**Performance chaos.** Elisp is single-threaded (there are cooperative threads but no preemptive concurrency). Every hook function, every piece of advice, every mode setup runs in the same thread. A slow hook blocks the entire editor. Packages that add themselves to `post-command-hook` (which runs after every keystroke) can make typing laggy. There is no isolation: a poorly-written package degrades the entire environment. Native compilation (Emacs 28+) helps with CPU cost but doesn't address the concurrency model.

**Debugging is archaeology.** When something breaks, the question is always "which of the 150 packages I have installed added advice to the function that just threw an error?" `debug-on-error` gives a stack trace, but the stack includes all the advice wrappers, hook dispatchers, and mode setups, making the actual cause hard to find. `emacs-init-time` and `profiler-start` help, but the fundamental problem is that extension happens through global mutation of shared state.

### Could you get the power without the chaos?

The key insight: Emacs's power comes from the universal abstraction (buffers) and the runtime modifiability. The chaos comes from untyped extension points, global namespace, implicit coupling through advice, and single-threaded execution. The question is whether you can keep the former while addressing the latter.

What "Emacs as an OS, but with statically typed abstractions" means:
- The buffer's role (a universal substrate for heterogeneous content) would be played by pane's typed protocol — the pane as universal UI object, with a session-typed interface that any extension can speak.
- Modes would become pane modes — wrappers around a base client library with domain-specific semantics, where the type system enforces interface compatibility.
- Hooks would become typed event subscriptions with statically known event types.
- Advice (monkey-patching) would be replaced by compositional middleware — interceptors that wrap protocol interactions with the type system ensuring the wrapper is compatible with the wrapped.
- The single global namespace would be replaced by isolated processes communicating over typed channels.

---

## 2. BeOS Extensibility: Translation Kit and Replicants

### Translation Kit: the model of filesystem-based extension

The Translation Kit is the cleanest example of "drop a file, gain a capability" in desktop OS history. The architecture:

**The protocol.** Every translator add-on is a shared library exporting five symbols: `translatorName` (short name), `translatorInfo` (description), `translatorVersion` (version), `Identify()` (recognition function), and `Translate()` (conversion function). Optionally, `inputFormats` and `outputFormats` arrays declare supported format conversions. Optionally, `GetConfigMessage()` and `MakeConfig()` enable user-configurable parameters.

**Identify/Translate.** `Identify()` receives a `BPositionIO` stream (a seekable byte stream) and determines whether the add-on can handle the data. It fills an `outInfo` struct with quality and capability ratings, then returns `B_OK` or `B_NO_TRANSLATOR`. `Translate()` performs the actual conversion: reads from `inSource`, converts to the requested `outType`, writes to `outDestination`. Both functions are stateless — no persistent state between calls.

**Quality ratings and the roster.** Each translator self-reports quality (how well it handles the format) and capability (how much of the format spec it covers) as floats. The `BTranslatorRoster` uses these ratings to select the best translator when multiple translators claim to handle the same format. This is competitive dispatch: if two translators both handle PNG, the one with higher quality wins. The ratings are honor-system — there is no verification.

**Common interchange format and O(n) scaling.** The critical architectural move: the Translation Kit defines standard intermediate formats (`B_TRANSLATOR_BITMAP` for images, `B_TRANSLATOR_TEXT` for text). Every image translator must be able to translate to/from the standard bitmap format. This means adding a new format requires one translator (format ↔ bitmap), not N translators (format ↔ every other format). N formats require N translators, not N(N-1)/2 converters. The common interchange format is what makes the ecosystem scale linearly.

**Discovery and loading.** Translators are installed in `B_USER_ADDONS_DIRECTORY/Translators/` (or system equivalents). The `BTranslatorRoster::Default()` singleton scans these directories and loads all shared libraries it finds. When `inputFormats`/`outputFormats` arrays are exported, the roster can skip the `Identify()` call for format combinations it already knows the translator can't handle, further reducing overhead. Adding a translator is literally copying a .so file. Removing it is deleting it.

### Replicants: cross-process embedding via BArchivable

**BArchivable protocol.** Any C++ object that inherits from `BArchivable` can serialize itself into a `BMessage` via `Archive()` and be reconstructed via a static `Instantiate()` that takes a `BMessage*`. The `deep` parameter controls whether child objects are also archived. `AllArchived()` and `AllUnarchived()` hooks handle cross-references in complex object hierarchies. The global `instantiate_object()` function can reconstruct any `BArchivable` from a `BMessage` without knowing the concrete type at compile time — it reads the class name from the message and dlsyms the `Instantiate` function from the appropriate library.

**Replicants.** A replicant is a BView that can be "torn off" from one application and embedded in another application's window (typically the Desktop shelf, but any BShelf). The host application loads the replicant's shared library, instantiates the BView from a BMessage archive, and parents it into its own window. The replicant's code runs in the host's address space — it has access to the host's BLooper message loop, and it draws into the host's BWindow.

**What it cost: C++ ABI fragility.** Because replicants load shared libraries into the host process, they depend on binary compatibility between the replicant's library and the host. C++ has no stable ABI — different compilers, different compiler versions, or different standard library implementations produce incompatible binaries. In practice this meant: replicants only worked reliably when built with the same compiler and against the same system libraries as the host. On BeOS (single vendor, single compiler), this was manageable. On Haiku (open source, multiple toolchain versions), it remains a source of breakage. The lesson: in-process code loading without a stable ABI boundary is inherently fragile. Every system that has tried cross-process C++ object embedding has hit this wall.

### Add-ons broadly: the common pattern

**Input Server add-ons.** The Input Server supports three add-on types loaded from well-known subdirectories under `input_server/`: `devices/` (BInputServerDevice — event generators, corresponding to hardware drivers), `filters/` (BInputServerFilter — event processors that inspect and modify events in a pipeline), and `methods/` (BInputServerMethod — character input methods for complex scripts like CJK). The Input Server loads all add-ons at boot. Events flow through a pipeline: devices generate → filters process/modify → app_server receives. Each add-on type has a C++ base class with virtual hook functions to override. Installing an add-on means dropping a shared library in the right directory (though the Input Server required a restart to pick up new add-ons — no live discovery).

**Screen savers, Tracker add-ons, media add-ons.** The pattern repeats: well-known directory, shared library, C++ base class with virtual functions, system-managed lifecycle. The Media Kit's add-on system used the same pattern for audio/video codecs and I/O. Tracker add-ons extended the file manager's context menu.

### How BMessage/BLooper made plugins safer

The messaging architecture provided a natural safety boundary. Every BLooper (and therefore every BWindow) has its own thread running its own message loop. Add-ons that communicate via BMessage rather than direct function calls get automatic thread safety — the message is queued and dispatched by the receiving thread. This doesn't prevent add-ons from corrupting shared state within a process, but it does mean that well-behaved add-ons interact through the same asynchronous message-passing discipline that the rest of the system uses. The message is the interface — you can inspect it, log it, filter it, forward it. BMessageFilter (attached to any BHandler) can intercept and modify messages before they're dispatched, providing a composable interception mechanism at the message level rather than the function level.

The contrast with Emacs is instructive: Emacs's hooks and advice operate at the function-call level within a single thread. BeOS's BMessage/BLooper operates at the message level across threads. The message-level approach is inherently more isolated (the add-on can't reach into the host's call stack) but less flexible (you can only intercept message delivery, not arbitrary function calls).

---

## 3. Plan 9 Extensibility: Filesystem as Extension Surface

### Acme: extension without an API

Acme's extension model is the purest example of filesystem-as-API. Acme is a 9P file server. Every window exposes files:

```
/mnt/acme/
  index          # list of windows
  new/
    ctl           # write to create a window
  1/
    ctl           # write commands (name, get, put, del, dump, ...)
    addr          # set/read address
    body          # read/write body text
    data          # read/write at current address
    tag           # read/write tag line
    event         # read events (mouse, keyboard, menu)
    xdata         # like data but full rune contents
    errors        # error output for this window
```

External programs extend acme by reading and writing these files. A spell checker reads `body`, runs aspell, writes corrections to `data`. A git interface reads `tag` to get the filename, runs git commands, writes results to a new window's `body`. A language server reads `event` to detect saves, runs the formatter, writes back to `body`. These are regular programs in any language — Go, shell, Python, C — that interact with acme by opening files and doing I/O. There is no acme SDK, no acme API, no dynamic linking, no shared address space.

The `event` file is the hook mechanism. When a user B2-clicks text, an event message appears on the window's `event` file. A program reading `event` can handle the event (execute the text as a command in a domain-specific way) or write it back to `event` to pass it to acme's default handler. This is the equivalent of Emacs hooks, but over a file descriptor instead of a function pointer. Any process that can read a file can be an event handler.

**Trade-offs.** The filesystem interface is language-agnostic and process-isolated, which are significant advantages over in-process extension. But it has costs: file I/O is slower than function calls (not dramatically — 9P is fast on local sockets, but the system call overhead adds up for high-frequency operations like cursor movement). The interface is untyped — `ctl` commands are text strings, and errors are discovered at write time. There's no discoverability — you need to read the man page to know what commands `ctl` accepts. And the filesystem interface is a fixed surface: acme exposes what acme chooses to expose. You can't add a new file to a window's directory from outside.

### The plumber: routing rules as text files

The plumber is a pattern-matching message router. Programs send messages (text + attributes); the plumber matches them against rules and delivers them to named ports. Rules are text files:

```
# open .go files in acme
type is text
data matches '([a-zA-Z¡-￿0-9_\-./]+)\.(go)'
data set $0
plumb to edit
```

Each rule specifies matching conditions (type, data pattern, attributes) and a delivery action (port name, data transformations). Rules are read from `$HOME/lib/plumbing` at startup and can be reloaded. Adding behavior means adding text to a rules file. The mechanism is declarative — you specify what to match and where to send, not how to process.

Plumber rules compose through ordering: rules are tried in sequence, first match wins. Rules can enrich messages (set attributes, transform data) before delivery. The plumber doesn't know what will handle the message at the destination — it just routes to a port. Applications listen on ports.

**The filesystem extension pattern.** Adding a plumber rule is adding text to a file. Adding an acme extension is writing a program that reads/writes files. In both cases, the extension surface is the filesystem — the same interface the system uses internally (acme uses its own 9P filesystem; the plumber uses the same message protocol). This is "the system is extended through the same interfaces it uses internally" in its purest form.

### Filesystem-as-extension vs API-based extension

The Plan 9 model trades richness for isolation. An API-based extension (Emacs advice, VS Code extension host) can do anything the host can do, with full access to internal state. A filesystem-based extension can only do what the exposed files permit. This is both the strength (extensions can't corrupt the host, can't cause type errors in the host's code, can't introduce memory safety issues) and the weakness (extensions are limited to the granularity of the exposed interface).

The key insight: the filesystem model works when the exposed interface is rich enough. Acme's ~10 files per window are sufficient for an enormous range of extensions because text editing is fundamentally about reading and writing text, and the filesystem exposes exactly that. A compositor's extension surface would need to expose different primitives: layout operations, visual state, event streams, content types.

---

## 4. NeXTSTEP Services and Bundles

### The Services menu: inter-application operations

NeXTSTEP Services is the earliest implementation of cross-application operations mediated by content type. The concept: any application can register operations it can perform on certain content types, and any application can invoke those operations on content the user has selected.

**How it works.** An application declares services in its `Info.plist` (originally the `.service` bundle description file) with keys: `NSMenuItem` (the menu item text), `NSMessage` (the Objective-C selector to invoke), `NSPortName` (the application's name for Mach port communication), `NSSendTypes` (UTIs/pasteboard types the service accepts), `NSReturnTypes` (UTIs it returns), and `NSRequiredContext` (minimum text selection, etc.). The system scans all installed applications at login and builds the Services menu from their declarations.

When the user selects text and chooses a service, the system: (1) puts the selected data on a shared pasteboard, (2) launches the service application if not running, (3) sends it the `NSMessage` selector, (4) the service reads from the pasteboard, processes, writes the result back, (5) the calling application reads the result and replaces the selection. The pasteboard is the interchange format — like BMessage for BeOS, it's typed data that both sides can read.

**The bundle model.** NeXTSTEP pioneered the modern bundle: a directory with a defined structure (executable, resources, Info.plist) that the system treats as a single unit. `.app` bundles are applications. `.framework` bundles are shared libraries with headers and resources. `.service` bundles contain service descriptions. `.plugin` bundles contain loadable code. The insight: a plugin is a directory, not a compiled binary. The directory contains everything needed — code, resources, metadata — in a self-describing format. You install by copying the directory. You uninstall by deleting it.

### Why Services failed to reach potential on macOS

NeXTSTEP had ~12 applications, all written by the same team, all participating in Services. When Mac OS X launched with thousands of third-party applications, the Services menu broke in several ways:

**Uncontrolled proliferation.** Applications install services without user consent or notification. One user documented 123 services after a fresh system setup — 58 active, the rest disabled. The menu becomes an unmanageable list.

**No permission model.** Any installed application can register any service. There's no review, no approval, no capability restriction. Applications can even silently claim keyboard shortcuts that conflict with user bindings.

**Poor discoverability.** Services are buried in a submenu (Application → Services) that most users never discover. Context menus show a subset, but even the subset can be overwhelming. Managing which services are active requires navigating to System Preferences → Keyboard → Shortcuts → Services — a path almost no user finds organically.

**No composition.** Services are atomic operations: one app processes one selection. You can't chain services (pipe the output of one into the input of another). You can't filter the Services menu contextually (beyond basic type matching). The pasteboard round-trip means services can only operate on copyable data.

**The real failure.** The concept is sound: content-type-based cross-application operations, declaratively registered, discoverable at the system level. The execution failed because (a) there was no quality control or permission system for registrations, (b) the UI (a flat menu) doesn't scale, (c) applications have no incentive to implement Services when they can just be self-contained, and (d) the one-shot pasteboard protocol is too limited for complex operations.

The lesson for pane: the Services concept maps to pane-roster's service registry + pane-route's pattern matching. The corrective is: typed registration with capability declarations (not an open dump of menu items), compositional routing (chain operations, don't round-trip through a pasteboard), and UI through routing (text-as-action, not menu browsing).

---

## 5. Modern Extensibility Models

### VS Code: extension host architecture

VS Code is the dominant example of a thriving extension ecosystem (50,000+ extensions in the marketplace). The architecture that enables this:

**Process isolation.** Extensions run in a separate Node.js process called the extension host. The editor (renderer process) communicates with the extension host over a typed RPC protocol. A misbehaving extension cannot crash the editor — at worst, it crashes the extension host, which can be restarted. There are three host types: `LocalProcess` (Node.js child process for desktop Electron), `LocalWebWorker` (web worker for browser), `Remote` (TCP socket for remote development).

**Typed RPC protocol.** All communication is defined in `extHost.protocol.ts` with typed interfaces: `MainThreadXxxShape` (methods the extension host can call on the renderer) and `ExtHostXxxShape` (methods the renderer can call on the extension host). Every method has a typed signature. This means the boundary between editor and extension is fully specified — you can't call an untyped method, you can't pass the wrong argument type, and the compiler enforces the protocol.

**Scoped API surface.** Each extension receives a uniquely scoped `vscode` API object, created by a factory function. Extensions cannot access each other's state or capabilities. The API is organized into namespaces (`vscode.commands`, `vscode.workspace`, `vscode.window`, `vscode.languages`) backed by ExtHost service implementations. The stable API surface is declared in `vscode.d.ts` — the TypeScript type definitions are the contract.

**Activation events.** Extensions declare when they should be activated: `onLanguage:typescript`, `onCommand:my.command`, `workspaceContains:**/.git`, `onStartupFinished`. This is lazy loading — extensions that don't match the current context don't run at all. The system also infers activation events from manifest contributions (registering a command implicitly triggers activation when that command is invoked).

**Proposed API governance.** New APIs are proposed in `.d.ts` files. Extensions must explicitly declare `enabledApiProposals` and pass runtime checks. Proposals graduate to stable by moving into `vscode.d.ts`. This prevents premature API exposure while allowing experimentation.

**What makes the ecosystem huge.** (1) The barrier to entry is low: write a TypeScript/JavaScript package, declare a manifest. (2) The API surface is large and well-typed, covering editor features, language support, debugging, testing, SCM, and terminal. (3) Process isolation means extensions can't degrade the editor. (4) The marketplace provides discovery, ratings, and one-click install. (5) Monthly release cadence with new APIs means the extension surface keeps growing. (6) TypeScript provides both developer ergonomics and API safety.

**What it costs.** The RPC boundary means every interaction involves serialization. Extensions can't access the DOM (no direct UI manipulation beyond the provided API). The Node.js runtime is heavy. Complex extensions (like language servers) end up running additional child processes (the Language Server Protocol itself runs in a separate process from the extension host). The ecosystem is strongly centralized around Microsoft's marketplace and API.

### Browser extensions: sandboxed content manipulation

Browser extensions demonstrate the most battle-tested permission and sandboxing model:

**Component isolation.** An extension has multiple components with different privilege levels: content scripts (can access webpage DOM, cannot access Chrome APIs), background service workers (can access Chrome APIs, cannot access DOM), and popup pages (API access, no DOM access). Communication between components is exclusively through string-based message passing — no shared objects.

**Manifest-declared permissions.** All capabilities are declared in `manifest.json`: host permissions (which websites the extension can access), API permissions (which Chrome APIs it can call), and content script injection patterns. Users see a permission prompt at install time. The manifest is the contract — the browser enforces it at runtime.

**Sandboxing model.** Content scripts run in an isolated world — they share the page's DOM but have a separate JavaScript execution context. They cannot access the page's JavaScript variables or the extension's background scripts directly. The Content Security Policy (CSP) restricts what code can execute. Manifest V3 bans `eval()` entirely; code requiring dynamic evaluation must be isolated in sandbox pages with no API access.

**The lesson.** The browser model works because: (1) the permission boundary is enforced by the runtime, not by convention, (2) capabilities are declared statically and approved by the user, (3) the sandboxing architecture prevents extensions from escalating privileges, and (4) message passing between components prevents shared-state bugs. The cost is rigidity — extensions can only do what the API permits, and the API must anticipate every extension category.

### Nix: declarative system configuration as extensibility

The NixOS module system represents a fundamentally different approach: extensibility through declarative specification rather than imperative plugin code.

**Module structure.** A NixOS module is a function returning three things: `imports` (paths to other modules), `options` (declarations via `mkOption` with types), and `config` (values assigned to options). Modules compose by merging — multiple modules can define values for the same option, and the type system's merge function determines how they combine. `types.lines` concatenates strings. `types.attrsOf` deep-merges attribute sets. `types.enum` rejects conflicts.

**Lazy fixpoint evaluation.** The module system evaluates all modules in a shared context using a fixpoint. The `config` argument in any module provides access to the fully-merged configuration — including values set by other modules. There is no execution order. There is no plugin lifecycle. The final system state is determined by the *set* of all module definitions, not their application sequence. This eliminates an entire class of bugs: no "this plugin must load before that plugin," no "the hook ran before the state was initialized."

**What this means for extensibility.** The Nix model shows that "declarative specification of behavior modification" (the pane design vision) can be a real thing, not just a slogan. A NixOS system is fully described by its module tree. Adding capability means adding a module file. Removing it means removing the file. The system rebuilds from the specification. This is the Translation Kit pattern elevated to the entire system: drop a file → gain behavior, remove a file → lose behavior, and the type system prevents invalid compositions.

**The cost.** The Nix expression language is its own thing — steep learning curve, non-obvious evaluation model, error messages that require understanding lazy evaluation to debug. The module system works because NixOS controls the entire evaluation. Applying this model to a running system (hot-reloading modules without restart) is a harder problem that NixOS solves by rebuilding and switching rather than patching in place.

### Lua embedding: small language as extension surface

Lua is the canonical example of a small, embeddable language used as an extension layer. 30,000 lines of C, 1.5MB uncompressed. It appears in:

**Neovim.** Embeds LuaJIT as its primary extension language alongside legacy Vimscript. Lua plugins run in-process with direct access to the `vim.api` (auto-generated, versioned C functions). No serialization overhead, synchronous access to editor state. Remote plugins communicate over msgpack-RPC for process isolation at the cost of latency. The module loading system respects Neovim's runtime path: `lua/` for on-demand modules, `plugin/` for auto-loaded, `ftplugin/` for filetype-specific. The architecture is layered: in-process Lua for performance-critical integrations, remote plugins for isolation and external library access.

**Redis.** Uses Lua for server-side scripting via EVAL. Lua scripts execute atomically on the server, manipulating data without network round-trips. The embedding is minimal — the script runs in a sandbox with access to Redis commands and basic Lua.

**Nginx (OpenResty).** The `ngx_http_lua_module` embeds Lua into the request processing pipeline. Lua handlers run within nginx's event loop, enabling dynamic request routing, upstream selection, and response generation without the overhead of a separate process.

**The pattern.** Lua succeeds as an extension language because: (1) it's small enough to embed anywhere without bloat, (2) the C API is simple (a stack-based VM), (3) LuaJIT provides near-native performance, (4) the language is simple enough that occasional users can write extensions without deep expertise. The cost: Lua is dynamically typed, so extension boundaries are enforced by convention. In-process execution means a Lua error can crash the host (Neovim handles this with pcall, Redis with script timeout limits).

**Neovim's specific lessons.** The dual-track architecture (in-process Lua + remote plugins over RPC) is a pragmatic trade-off: use the fast path for tight integrations, the isolated path for heavy or risky operations. The auto-generated, versioned API (`nvim_*` functions) provides forward compatibility without breaking existing plugins. The diagnostic framework uses namespaced identifiers to prevent conflicts between providers — a type-light version of the namespace isolation that Emacs lacks.

### WASM plugins: sandboxed extensibility

Zellij (terminal multiplexer) and Extism (general-purpose WASM plugin framework) represent the current state of WASM-based extension:

**Zellij's architecture.** Plugins run in WASM sandboxes using the wasmi interpreter (migrated from wasmtime in v0.44.0 for a lighter runtime). Each plugin gets isolated memory, a WASI filesystem (with mounted directories: `/host` for the working directory, `/data` for plugin state, `/cache` for shared cache), and configurable resource limits. Communication is via Protocol Buffers: the host serializes events to the plugin's stdin, the plugin serializes commands back. The plugin thread uses a pinned executor (4-16 threads) where each plugin instance is pinned to a specific thread by ID hash, preventing concurrent access.

**Zellij's permission model.** Plugins must request capabilities: `ReadApplicationState`, `ChangeApplicationState`, `OpenFiles`, `RunCommands`, `WebAccess`, etc. Users grant permissions via prompts; grants persist in SQLite. Events can be cached during permission requests and applied once granted. This is the browser extension model adapted for a terminal multiplexer.

**Zellij's rendering.** Plugins write UTF-8 + ANSI escape codes to stdout. The host reads stdout and renders plugin output in its designated pane. This is elegant: the extension surface is "produce terminal output," which is the same interface the rest of the terminal uses.

**Extism.** A general framework for adding WASM plugin support to any application. Host applications define functions that plugins can call. Plugins are compiled WASM blobs that can be distributed as single files. The runtime handles sandboxing, memory management, and the host↔plugin calling convention. Supports writing plugins in Rust, Go, C, JavaScript, Python, and others — any language that compiles to WASM.

**The WASM trade-off.** Sandboxing is real — WASM's linear memory model prevents plugins from accessing host memory. But: WASM's capability model is coarse (you either have access to a host function or you don't), performance overhead is significant compared to native code (wasmi is an interpreter, wasmtime JIT-compiles but is heavier), and the developer experience is still rough (debugging WASM plugins is harder than debugging native code). The promise is "any language, fully sandboxed"; the reality is "Rust works well, other languages have friction, and you pay for the sandbox."

---

## 6. The Design Problem Itself

### What makes plugin ecosystems thrive vs die

Looking across all the systems studied, the thriving ecosystems share structural properties:

**Universal substrate.** Emacs has buffers. VS Code has the editor model + typed API. Browser extensions have the DOM + Chrome APIs. BeOS had BMessage. Plan 9 had the filesystem. The substrate must be rich enough that diverse extensions can compose with it, and stable enough that extensions don't break when the system evolves. Pane's substrate is the pane protocol + session types + the filesystem.

**Low barrier to entry.** MELPA: push to a Git repo, submit a recipe. VS Code: write a TypeScript package, publish to marketplace. Browser extensions: write a manifest + JavaScript. BeOS translators: compile a .so, copy to a directory. Plan 9 acme: write a program that reads/writes files. The deployment mechanism must be trivial. "Drop a file" is the gold standard.

**Composition for free.** Emacs packages compose because they all operate on buffers. VS Code extensions compose because they all register providers through the same API. BeOS translators compose because they all speak the common interchange format. When extensions compose without knowing about each other, the ecosystem grows combinatorially. When extensions must explicitly integrate, the ecosystem grows linearly.

**Safety proportional to trust.** Browser extensions: full sandboxing, declared permissions, user consent. VS Code: process isolation, typed API, no DOM access. BeOS translators: in-process but stateless (no persistent state between calls). Emacs: no isolation at all, trust everything. The more isolation, the more the ecosystem can scale without quality control — untrusted code can participate safely. The less isolation, the more the ecosystem depends on community norms.

**What kills ecosystems.** (1) The API changes too fast (extensions break on every update). (2) The deployment mechanism is too complex (nobody bothers publishing). (3) The substrate is too narrow (extensions can't do what users need). (4) There's no discovery mechanism (users can't find extensions). (5) There's no quality signal (users can't distinguish good from bad). (6) The extension model is too coupled to implementation details (advice on byte-compiled primitives). NeXTSTEP Services died because of (4), (5), and a too-narrow substrate (3). Many Linux desktop extension systems die because of (1) and (2).

### "Safe parts of the OS surface" — what is a safe extension boundary?

A safe extension boundary has four properties:

1. **Type safety.** The extension can't send the system a message it doesn't understand. Session types enforce this: if the protocol says "send a CellRegion," you can't send a string. The extension's interaction with the system is governed by a typed contract.

2. **Memory safety.** The extension can't corrupt the system's memory. Process isolation achieves this automatically (separate address spaces). In-process extension achieves it through language-level guarantees (Rust's ownership system for native code, WASM's linear memory for sandboxed code).

3. **Capability restriction.** The extension can only access what it's been granted access to. The browser extension model (declared permissions, user consent) is the proven approach. The Plan 9 model (you can only read/write the files you can see) is the elegant approach. Both work.

4. **Composability.** The extension boundary must allow extensions to compose with each other and with the system without explicit coordination. This requires a shared substrate (a common data model, a common protocol) that all extensions speak.

The unsafe boundary is: in-process code loading with unrestricted access to the host's internal state (Emacs advice, BeOS replicants with C++ ABI coupling, any system where a plugin can call arbitrary internal functions). These give maximum power at the cost of fragility.

The "safe parts of the OS surface" in pane's design are: the pane protocol (session-typed, compiler-verified), the filesystem interface (well-known directories, xattrs, config files), the routing system (pattern-matching rules as text files), and the service registry (typed capability declarations). These are all public interfaces that the system itself uses. Extensions that operate on these interfaces compose with the system for the same reason the system's own components compose: the protocol governs the interaction.

### Declarative vs imperative extension

The spectrum:

| Approach | Example | Power | Safety | Composability |
|---|---|---|---|---|
| Pure data | Plumber rules, config files | Low — can only configure | High — data can't crash | High — data merges |
| Declarative spec | NixOS modules, CSS | Medium — specifies what, not how | High — evaluated, not executed | High — merge semantics defined |
| Scripting | Lua in Neovim, Elisp | High — full language | Low — can do anything | Medium — depends on discipline |
| Typed API | VS Code extensions | High within the API | Medium — process-isolated but powerful | Medium — API defines composition points |
| Native code | BeOS add-ons, Emacs C modules | Maximum | Lowest — in-process, ABI-coupled | Low — deep coupling |

Pane's vision of "declarative specification with good UX abstractions for modification" places it in the declarative-spec-to-typed-API range. Routing rules are pure data (like plumber rules). Pane modes are typed API (like VS Code extensions, but with session types instead of TypeScript interfaces). Translators are somewhere between (a shared library with a typed protocol, like BeOS but with Rust's safety guarantees instead of C++ ABI fragility).

### "Same interfaces internally and externally" — what this means for extension

This is the Plan 9 principle: the filesystem that acme uses to manage its own windows is the same filesystem that external programs use to extend acme. The protocol that pane-comp uses to talk to pane-route is the same protocol that a user's custom extension would use to talk to pane-route.

The implications:

1. **No special-case extension API.** There is no "plugin SDK" separate from the system's own interfaces. A plugin is a program that speaks the pane protocol, registers with pane-roster, watches directories with pane-notify, reads/writes xattrs with pane-store. The same tools a developer uses to build pane's own servers are the tools an extension author uses.

2. **Extensions are interchangeable with system components.** If the routing system is just a server that speaks a protocol, a user could replace it with their own routing server that speaks the same protocol. This is the Unix pipe philosophy: components are replaceable as long as they honor the interface contract.

3. **Testing is the same.** The test infrastructure for pane's own servers works for extension servers. Property-based tests over the protocol work for any component that speaks the protocol.

4. **The extension surface is the protocol surface.** The set of things an extension can do is exactly the set of things the protocol permits. If the protocol is well-designed, the extension surface is well-designed. If the protocol is too narrow, extensions are too limited. Protocol design *is* extension design.

### Static types as guardrails for extensibility

The contrast between Emacs (no types at extension boundaries) and what pane aims for (session types at every boundary):

**Emacs.** A hook function can have any signature. A piece of advice can wrap any function. Mode setup can mutate any buffer-local variable. The only validation is at runtime: if the types mismatch, you get an error at the point of the mismatch. This enables maximum flexibility but makes composition unreliable.

**Session types.** A pane extension that speaks the wrong protocol step gets a compile-time error, not a runtime crash. The session type specifies the entire conversation: what you send, what you receive, in what order, with what choices. The compiler verifies both sides. An extension that tries to send a `CellRegion` when the protocol expects a `TagLine` won't compile. An extension that tries to receive when it should be sending won't compile. The protocol advances with the conversation — the type tracks where you are in the interaction.

**What this gives you.** (1) Extensions can be verified against the protocol without running them. (2) Protocol evolution is explicit — changing a session type is a type-level change that forces all implementations to update. (3) Extensions compose safely because the type system ensures they speak compatible protocol halves. (4) Debugging is easier because protocol violations are caught at compile time, not discovered in production.

**What this costs.** (1) The session type must be designed before extensions can be written — the type is the bottleneck. (2) Protocol evolution requires managing backward compatibility at the type level (versioned session types, negotiation). (3) The type system can't express all extension constraints (a session type says "you must send a CellRegion" but not "the CellRegion must be within the viewport bounds"). (4) Extensions in languages that can't express session types (shell scripts, Python) must use a runtime bridge that enforces the protocol dynamically — the safety guarantee degrades to runtime checking for these extensions.

### The spectrum: data plugins to code plugins to full apps

Pane's extension model spans a spectrum of complexity:

**Pure data (no code).** Plumber routing rules are text files. Config values are files in well-known directories. xattrs on files are metadata. These are the simplest extensions: add a file, gain behavior. No compilation, no ABI, no runtime. The system reads the data and acts on it.

**Structured data (schema but no code).** A translator metadata declaration (xattrs specifying content types and capabilities). A service registration (content_type_pattern, operation_name, description). These carry enough structure that the system can validate them but don't contain executable logic.

**Typed code (compiled library).** A translator binary (a shared library with the Identify/Translate protocol). An input method add-on. These are compiled Rust code that implements a typed interface. They run in-process or as separate processes depending on the extension category. The session type or trait interface ensures compatibility at compile time.

**Full server (separate process).** A pane mode is a full pane client — a separate process speaking the pane protocol, with its own event loop, its own state, its own lifecycle. A "git pane" or "mail pane" is a thin server wrapping shared infrastructure (pane-shell-lib for terminal emulation, pane-app for lifecycle management) with domain-specific semantics. These are full applications that happen to participate in the pane ecosystem.

The key insight: all four levels use the same interfaces. A routing rule uses the filesystem interface. A translator uses the filesystem for discovery and the pane protocol for negotiation. A pane mode uses the pane protocol for rendering and the filesystem for configuration. The interfaces are uniform; the complexity varies.

---

## 7. Synthesis: What This Teaches About Extensibility as a Design Problem

### The contrast: Emacs chaos vs statically typed abstractions

Emacs demonstrates that a universal abstraction (buffers) + unrestricted extensibility (hooks, advice, global namespace) produces a thriving but chaotic ecosystem. The chaos is not accidental — it's the direct consequence of the extension model. Hooks are untyped function lists. Advice is untyped function wrapping. The namespace is flat and global. Extensions compose by accident (they happen to work on the same buffer) or by convention (they agree on naming prefixes), never by construction.

"Statically typed abstractions" for pane means: the universal abstraction (pane + protocol) is typed end-to-end. Modes are not "a function that mutates global state" but "a server that speaks a typed protocol with verified composition." Event subscriptions are not "a list of functions called in arbitrary order" but "a set of typed handlers registered for specific message types, dispatched through the event loop." Extension composition is not "I hope these two packages don't both advice `save-buffer`" but "these two extensions speak compatible protocol halves, verified by the compiler."

The goal is to keep Emacs's universality (everything is a pane, the way everything is a buffer) while replacing Emacs's extension mechanism (global mutation) with one that provides the same power through typed composition.

### What "declarative specification of behavior modification" means concretely

Drawing from the research:

1. **Routing rules (Plan 9 plumber model).** Behavior modification for "what happens when you click text" is a declarative rule: match pattern → route to handler. Adding a rule is dropping a file. The rule is data, not code. The system reads it and acts.

2. **Configuration as files (BeOS/Plan 9 model).** Behavior modification for "how does this server behave" is a config file in a well-known directory. Change the font by writing to a file. Change the color scheme by writing to files. No restart, no reload command — the system watches and responds.

3. **Service registration (NeXTSTEP Services model, corrected).** Behavior modification for "what operations are available" is a typed declaration: (content_type_pattern, operation_name, description). The declaration is metadata, not code. The system indexes it and offers it contextually.

4. **Pane modes (Emacs major modes model, typed).** Behavior modification for "how does this pane behave" is a server that wraps shared infrastructure with domain-specific semantics. The mode is code, but its interface is declarative: it speaks the pane protocol, exposes a tag line, provides a filesystem, and registers services. The session type declares what it does. The behavior is specified by the protocol, not by arbitrary code execution.

The "good UX abstractions for modifying the declarative specification" means: you can edit routing rules by editing text (they are text). You can change config by writing files (through any tool — a file manager, a shell, a text editor, the FUSE interface). You can install a pane mode by copying a file. You can browse available services through the roster. The modification UX is the same as the usage UX: text, files, the pane protocol.

### What a "vast ecosystem over safe OS surface" requires

From the research, the structural requirements:

1. **A universal substrate rich enough for diverse extensions.** The pane + protocol model must cover: rendering (cell grid, surfaces), interaction (events, routing), data (attributes, files), lifecycle (roster, init), and configuration (filesystem). If any of these is missing or too narrow, an entire category of extensions is impossible.

2. **Drop-a-file deployment.** Translators, routing rules, config values, input methods — all discovered by scanning directories. pane-notify watches. No registration step, no build step, no activation step beyond placing the file.

3. **Typed boundaries that prevent composition failures.** Session types for protocol-speaking extensions. Type-checked traits for in-process extensions (translators). File format validation for data extensions (routing rules). The boundary is where the system validates the extension's contract.

4. **Process isolation by default.** Pane modes are separate processes. The compositor doesn't load extension code. The router doesn't load extension code. Extensions that need in-process execution (translators, input methods) operate through narrow, typed interfaces.

5. **Composition without coordination.** Extensions that speak the protocol compose with extensions that speak the protocol. A routing rule doesn't need to know about the translator that identifies the file type. The translator doesn't need to know about the pane mode that displays the result. Each component operates on its part of the pipeline.

6. **Discovery and quality signals.** The roster knows what services exist. The translator roster knows what formats are supported. Routing rules are inspectable text files. The system must be self-documenting: you can discover what extensions are installed and what they do through the same interfaces you use to interact with the system.

### How the reference systems inform a unified philosophy

**From BeOS Translation Kit:** The pattern of "drop a shared library, system gains a capability" with a common interchange format that prevents O(n²) scaling. The quality-rating system for competitive dispatch. The stateless Identify/Translate protocol that keeps translators simple. **Applied to pane:** translators in `~/.config/pane/translators/` with a Rust trait interface instead of C++ symbols, session types for the identify/translate conversation, and the pane protocol's message model as the common interchange format.

**From Plan 9 filesystem interface:** The principle that the extension surface is the same surface the system uses internally. The filesystem as the universal discovery and configuration mechanism. The plumber's declarative rules as the extension model for behavior routing. **Applied to pane:** the pane-fs FUSE interface exposes what the system uses; routing rules are text files; config is files in directories; plugin discovery is directory scanning.

**From NeXTSTEP Services:** The concept of cross-application operations mediated by content type, declaratively registered. The bundle model (a directory is a plugin). **Applied to pane:** service registration in pane-roster, type-based routing in pane-route, but with the correctives Services lacked — typed declarations, compositional routing instead of one-shot pasteboard, and discoverability through the protocol rather than a flat menu.

**From Emacs:** The universal abstraction (pane = buffer) that makes all extensions compose. The mode system as domain-specific behavior layered over shared infrastructure. The hook system as event-driven extension. **Applied to pane:** pane modes wrapping pane-shell-lib, typed event subscriptions through the protocol, but with static types preventing the composition failures that hooks and advice produce in Emacs.

**From VS Code:** Process isolation, typed API boundaries, activation events for lazy loading, proposed API governance for safe evolution. **Applied to pane:** extensions as separate processes speaking typed protocols, session type evolution as the analogue of API versioning, capability declarations for what an extension can do.

**From Nix:** Declarative specification with type-safe merging, fixpoint evaluation, modules as composable units. **Applied to pane:** the filesystem-as-config model where the system state is the set of all files in well-known directories, with the type system (xattr schemas, session types, trait interfaces) preventing invalid compositions.

**From WASM (Zellij):** Sandboxed execution with explicit permissions, Protocol Buffer communication, event-driven activation, rendered output through the host's display. **Applied to pane:** the same architecture applies if pane adds WASM support for untrusted extensions — sandboxed execution, permission prompts, event subscriptions, rendering through the cell grid. The pane protocol's typed messages serve the same role as Zellij's Protocol Buffers.

The unified philosophy: **the extension surface is the protocol surface, the deployment mechanism is the filesystem, and the type system is the safety guarantee.** Extensions at every level of the spectrum (data → code → server) use the same interfaces, discovered the same way (directory scanning), validated the same way (type checking at the boundary), and compose the same way (through the protocol, not through shared state).

---

## Extensibility in the Context of Pane's Egalitarian Orientation

The research above surveys how extensibility has been achieved across historical and modern systems — what structural properties make ecosystems thrive, what makes them die, and what the design trade-offs are at every point on the spectrum from pure data to full applications. This section connects those findings to the specific philosophical commitments articulated in pane's foundations document, which frame extensibility not as a feature category but as the fundamental nature of the system.

### 1. The source-based distribution philosophy

The foundations document makes an observation about the trajectory of Linux distributions that bears directly on extension design: source-based distributions (Arch, Gentoo) succeeded because "the lack of opinionation was their uniform design principle." They were initially derided as impractical exercises for bored hackers. They are now among the most popular and culturally definitive Linux distributions. The features critics dismissed — full user control over package builds, rolling releases, explicit configuration — turned out to be exactly what their users loved most. The communities united by the personal curation a customized system enables form the most distinctive quality of the Linux ecosystem as a whole.

This is not background context. It is a design constraint. Pane's extension model must feel like a natural extension of what Linux users already do — not a new paradigm to learn. Customizing pane should feel like customizing your Linux system, because it IS customizing your Linux system. Routing rules are text files in directories. Configuration is files in well-known paths. Translators are compiled libraries dropped into place. Pane modes are programs that speak a protocol. None of these mechanisms require learning a pane-specific workflow that has no analogue in the user's existing practice. A user who knows how to write a shell script and put it in `~/.local/bin/` already understands the deployment model. A user who knows how to edit dotfiles already understands the configuration model. A user who knows how to install packages from source already understands the translator model.

The congruence is pivotal. The foundations document is explicit: "The ability of users to curate their experience on pane should feel like a natural extension of the kinds of things they do on their Linux systems all the time." Extension that requires users to abandon their existing mental models — to learn a bespoke plugin framework, a new configuration language, a new packaging system — violates this principle no matter how technically elegant it is. The extension surface must be legible to someone who already knows Linux, because the extension surface IS Linux, with typed protocols providing safety guarantees that the underlying system lacks.

### 2. pane.nvim and pane.el as integration strategy

The foundations spec envisions stock plugins for existing cult-favorite tools — `pane.nvim`, `pane.el`, and equivalents for helix and other editors — as the primary way to demonstrate pane's capabilities to the users who matter most. This is not merely an adoption strategy. It is a design principle with architectural consequences.

Users already love vim, emacs, and helix. They are infamously, sometimes absurdly loyal to their specific choices. Showcasing pane's infrastructure in settings they are already familiar with does several things at once. It builds trust: users who are historically hostile to endeavors that attempt to port features from mainstream operating systems while smuggling in the same downsides see that pane readily accommodates their own opinionated choices. It demonstrates composability: if pane's protocols compose ergonomically with tools the user already knows, the user has evidence that pane's architecture is real, not marketing. And it provides a forcing function on pane's own design: if pane's interfaces cannot be naturally surfaced through a neovim plugin, if pane's routing cannot be invoked from an emacs buffer, then pane's interfaces are not composable enough and the design needs revision.

The deeper point: the extension model and the integration model are the same thing. A `pane.nvim` plugin that lets you manage routing rules from a neovim buffer is exercising the same filesystem interface and the same protocol that a standalone pane mode uses. A `pane.el` package that exposes pane attributes as buffer-local variables is using the same attribute store that the compositor queries. The stock plugins are not wrappers around pane — they are pane clients, indistinguishable in kind from any other participant in the system. This is the "same interfaces internally and externally" principle applied to the editor ecosystem: the tools users already live in become first-class pane participants, not through special-case integration code, but through the same protocol surface that everything else uses.

### 3. Agents as extension contributors

The foundations document describes agents as system inhabitants who build customizations on behalf of users. A user says "I want shell output lines matching this pattern routed to a scratchpad." The agent writes the routing rule, drops it in the directory, the system gains the behavior. The user didn't write code. The agent didn't modify pane's internals. It produced a small, declarative artifact and placed it on the same extension surface that human developers use.

This is the critical design requirement: the extension surface must be the same for agents and humans. An agent that produces a routing rule produces a text file with the same syntax a human would write. An agent that builds a translator produces a Rust library implementing the same trait interface a human developer would implement. An agent that configures a pane mode drops configuration files in the same directories a human would use. There is no "agent API" separate from the human API. The filesystem is the API. The protocol is the API. Agents are participants, not a special class of extension author with their own privileged surface.

Over time, a user's collection of agent-built customizations becomes a personal configuration — shareable, versionable, composable. This is the emacs and neovim plugin ecosystem dynamic, transposed: agents contribute alongside humans, producing artifacts that live on the same filesystem, governed by the same typed interfaces, discoverable through the same mechanisms. The difference is that the barrier to contribution drops to natural language. A user who cannot write Rust or Elisp can still describe desired behaviors, and the agent produces the declarative specifications — routing rules, translators, pane modes — that realize them. The specifications are inspectable, editable, and removable. The user retains full control because the artifacts are files, not opaque state mutations.

### 4. The democratic orientation

The foundations document states the principle directly: "The best user experiences are not generated by imposing a particular view of what computing should be, but by providing a powerful and flexible foundation that users can build on to invent their own experiences." The stock UX is "just one presentation of a variety of possible experiences enabled by the underlying system architecture."

This has a specific implication for extensibility that distinguishes pane from every system surveyed in the research above. In VS Code, extension is a feature: there is the editor, and there are extensions to the editor, and the extension API is a boundary between the two. In Emacs, extension is more fundamental — the system is Elisp all the way down — but there is still a distinction between "Emacs" and "packages." In pane, the infrastructure does not presume what users want to do. Extension is not a power-user feature bolted onto the side of a finished experience. It is the fundamental nature of the system. The base experience and the customized experience use the same mechanisms. A user running stock pane with no modifications is using the same routing infrastructure, the same filesystem interfaces, the same protocol surface that a user with hundreds of custom routing rules and a dozen agent-built pane modes is using. The stock configuration is just one set of files in the well-known directories. Customization is changing those files, adding new ones, or removing ones you don't want.

The infrastructure-first design principle connects directly: "When the system provides infrastructure rather than finished experiences, users can compose their own experiences from that infrastructure. The developer doesn't get the final word." The extension model is the mechanism by which this promise is kept. If extension were difficult, or required special knowledge, or operated through different interfaces than the system itself uses, the democratic orientation would be rhetoric rather than reality. The extension surface must be democratically accessible — not in the sense of dumbed down, but in the sense of using the same tools and interfaces the system itself uses, so that understanding the system and extending the system are the same activity.

### 5. "Freedom is not too difficult"

The foundations document articulates pane's core thesis against a specific historical backdrop: "All of these considerations are made to give us the best chance to prove the viability of a new paradigm in operating system design allowing us to break with the misconception that freedom is too difficult to bother with."

This is the claim that pane's extension model must vindicate. The mainstream trajectory of computing — from the open experimentation of the early personal computer era through the increasingly locked-down platforms of the 2010s and 2020s — was driven by the premise that user freedom and usability are opposed. That customization is dangerous. That users who modify their systems will break them. That the responsible thing is to restrict what users can do, for their own protection. The result was platforms that are polished on first contact and hostile to anyone who wants to understand or modify what they're using.

Pane's extension model is the direct rebuttal. Freedom and usability are not opposed — they are the same thing, if the infrastructure is right. Typed protocols mean that extensions that violate the system's contracts don't compile, rather than crashing at runtime — so users can experiment without fear of corruption. Filesystem-based deployment means that adding and removing extensions is as simple and reversible as adding and removing files — so customization is not a one-way door. Process isolation means that a misbehaving extension cannot degrade the rest of the system — so the cost of experimentation is bounded. Declarative specifications mean that the artifacts of customization are human-readable text, not opaque binary state — so users can understand, share, and reason about their configurations.

The extension model proves the thesis by construction: here is a system where you can modify anything, where the modification mechanisms are the same as the system's own mechanisms, where the type system prevents you from breaking things in ways that are hard to recover from, and where the result is a system that is more capable, more personal, and more yours than any locked-down platform could be. The infrastructure makes freedom safe. The safety makes freedom accessible. The accessibility makes freedom the default rather than the exception.
