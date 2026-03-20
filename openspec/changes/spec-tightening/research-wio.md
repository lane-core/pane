# Wio Research — Rio on Wayland

Research for pane spec-tightening. Primary sources: wio source code (<https://git.sr.ht/~sircmpwn/wio>), Drew DeVault's blog post "Announcing wio" (2019-05-01), wio mailing list and issue tracker on sr.ht, wio commit history, "In praise of Plan 9" (DeVault, 2022-11-12).

Sources:

- Repository: <https://git.sr.ht/~sircmpwn/wio>
- Announcement: <https://drewdevault.com/2019/05/01/Announcing-wio.html>
- Mailing list: <https://lists.sr.ht/~sircmpwn/wio>
- Bug tracker: <https://todo.sr.ht/~sircmpwn/wio>
- Plan 9 post: <https://drewdevault.com/2022/11/12/In-praise-of-Plan-9.html>

---

## What wio is

Wio is an experimental Wayland compositor by Drew DeVault that reproduces the look and feel of Plan 9's rio window manager. It was written in C against wlroots (the same library that powers sway) and announced in May 2019. DeVault describes it as "a Wayland compositor based on wlroots which has a similar look and feel to Plan 9's Rio desktop."

The project is incomplete and has been essentially dormant since 2020. The last meaningful feature work (border-drag resizing, negative-coordinate fixes) was contributed by community members; DeVault's own last commit was removing an obsolete wlroots interface. The README explicitly acknowledges the project is missing "a Rio-esque FUSE filesystem and Rio's built-in command line."

### What it implements from rio

**The visual model:** Gray background (`#777777`), windows with colored borders (teal `#50A1AD` when active, light cyan `#9CE9E9` when inactive), a right-click menu with green color scheme (`#3D7D42` selected, `#EBFFEC` unselected). The border width is hardcoded at 5 pixels. These are the authentic rio colors.

**The interaction model:** Right-click on the background opens a menu with five items: New, Resize, Move, Delete, Hide. These are operated with the mouse in the classic rio pattern:

- **New:** Click to set first corner, drag to set second corner, release to create window in that rectangle.
- **Resize:** Click a window to select it, then click-drag a new rectangle to define its new geometry.
- **Move:** Click a window, click where to place it.
- **Delete:** Click a window to close it.
- **Hide:** Listed in the menu but not implemented.

The state machine for this is explicit in the code — an enum with 15 states (`INPUT_STATE_NONE` through `INPUT_STATE_HIDE_SELECT`) and a large switch statement in `handle_button_internal()` that dispatches transitions. Each state corresponds to a step in a multi-click interaction sequence. This is a direct translation of rio's modal mouse interaction into imperative C.

**Border-drag resizing:** A community contributor (Leon Plickat) added rio-style border dragging, where grabbing a window's border and pulling resizes it. A later patch series by Leonid Bobrov ("Behave more like Rio", 11 patches) refined this to handle corner combinations and diagonal resizing.

### What it doesn't implement from rio

**The filesystem interface.** Rio is a 9P file server. Wio is not a file server of any kind. Issue #2 on the tracker is "Implement 9P filesystem with rio-esque API" — filed by DeVault himself at project creation, never implemented, no discussion beyond the title.

**Per-process namespaces.** Rio's entire purpose is that each window's child process sees a private `/dev/cons`, `/dev/mouse`, `/dev/draw`. Wio doesn't do this. Each window runs an independent Wayland client (cage + terminal). The programs inside don't get a synthetic device namespace; they get a regular Wayland session.

**The built-in text editor/command line.** Rio has an integrated editable text area in each window that doubles as a command line and scrollback. Wio delegates this entirely to the terminal emulator (alacritty by default).

**Window hiding/showing.** Issue #1, also filed at creation, never implemented.

**Recursive composition.** In rio, rio can run inside rio because it exports the same interface it consumes. Wio achieves a limited version of this through cage nesting (see below), but it's an architectural consequence rather than a fundamental property.

---

## Architecture

Wio is roughly 800 lines of C across five source files plus four headers:

| File       | Size    | Role                                                            |
| ---------- | ------- | --------------------------------------------------------------- |
| `main.c`   | 8.3 KB  | Server init, menu texture generation, CLI parsing               |
| `input.c`  | 17.4 KB | All mouse/keyboard handling, the state machine, window spawning |
| `output.c` | 13.8 KB | Frame rendering, border drawing, menu rendering                 |
| `view.c`   | 5.8 KB  | XDG surface lifecycle, focus, hit testing                       |
| `layers.c` | 9.3 KB  | wlr-layer-shell support (for waybar, swaybg, etc.)              |

### wlroots primitives used

- `wlr_backend` — hardware abstraction (DRM/KMS or nested Wayland)
- `wlr_renderer` — software rendering (rect fills, texture blits)
- `wlr_output` + `wlr_output_layout` — multi-monitor management
- `wlr_xdg_shell` — client window lifecycle (the only shell supported)
- `wlr_layer_shell_v1` — layer surfaces for panels, backgrounds, overlays
- `wlr_cursor` + `wlr_xcursor_manager` — cursor management and theming
- `wlr_seat` — input device multiplexing (keyboard focus, pointer focus)
- `wlr_data_device_manager` — clipboard
- Various "free" protocols: screencopy, gamma control, export-dmabuf

The compositor follows the standard wlroots pattern: create display, create backend, wire up listeners for new outputs/inputs/surfaces, enter event loop. There is no custom Wayland protocol — wio is purely a compositor that manages standard XDG shell clients.

### The cage-per-window design

This is wio's most architecturally interesting decision and also its most problematic one.

When you create a new window (select "New" from the menu, draw a rectangle), wio doesn't just spawn a terminal. It spawns `cage -- alacritty` — that is, a terminal running inside cage, which is itself a minimal kiosk Wayland compositor. Cage connects to wio as a Wayland client, occupies the rectangle you drew, and runs alacritty fullscreen within that rectangle.

The spawning code in `new_view()` is a double-fork pattern:

```c
if (snprintf(cmd, sizeof(cmd), "%s -- %s",
        server->cage, server->term) >= (int)sizeof(cmd)) {
    fprintf(stderr, "New view command truncated\n");
    return;
}
pid_t pid, child;
if ((pid = fork()) == 0) {
    setsid();
    // ... signal cleanup ...
    if ((child = fork()) == 0) {
        execl("/bin/sh", "/bin/sh", "-c", cmd, (void *)NULL);
        _exit(0);
    }
    // write child PID back through pipe
    _exit(0);
}
```

The child PID is tracked in a `wio_new_view` struct with the target geometry. When the cage process connects as a Wayland client and maps its surface, wio matches it to the pending `new_view` by process ID and places it in the drawn rectangle.

DeVault's stated motivation: this achieves the rio property where "each window taking over its parent's window, rather than spawning a new window." From inside cage, you can launch graphical Wayland applications and they appear within that window's rectangle rather than as new top-level windows. He called this "interesting use-cases which aren't possible at all on X11" and attributed it to "Wayland's fundamentally different and conservative design."

**The cost:**

- Memory: Issue #22 reports wio consuming 564MB of RAM. The reporter found cage alone consuming 686MB. Each window is a full compositor process with its own wlroots instance, renderer, and protocol state. This is not viable for a system with many windows.
- Latency: Every input event and every rendered frame goes through an extra compositor hop. There's no data on how much this costs, but it's not free.
- Complexity: wio has to match spawned cage processes to pending window geometries by PID. The code is ~50 lines of fork/pipe/waitpid machinery for what rio accomplishes with `mount -b $'8½serv' /dev; exec rc`.

### Rendering

The renderer is straightforward software rendering through wlroots. The `output_frame()` callback:

1. Clear to gray background
2. Render bottom/background layer surfaces
3. For each view (back to front): render its four border rectangles, then render its surface
4. If in an interactive state (new/resize/move), render a selection/preview border overlay
5. Render top layer surfaces
6. If menu is open, render menu
7. Render overlay layer surfaces
8. Render software cursor
9. Commit frame

Border rendering is four `wlr_render_rect()` calls per window — top, right, bottom, left rectangles. Color selection: red for active selection, teal for focused window, light cyan for unfocused. No anti-aliasing, no shadows, no decorations beyond the flat border. This matches rio's aesthetic.

Menu rendering uses pre-generated Cairo textures for the text items ("New", "Resize", etc.) in two variants: black-on-white (inactive) and white-on-black (active/hovered). Hit testing is done per-frame during render by checking cursor position against each menu item's bounding box.

---

## The rio-to-Wayland translation problem

The fundamental issue wio faces is that rio and Wayland solve the same problem (letting multiple programs share a display) through incompatible mechanisms.

### What rio does

Rio is a 9P file server that multiplexes device files. Each window's child process gets a private namespace where `/dev/cons`, `/dev/mouse`, `/dev/draw` are served by rio. The child process doesn't know it's in a window — it thinks it's talking to the kernel's display driver. Rio can run inside rio because it exports the same interface it imports. The mechanism is per-process namespace manipulation: `mount`, `bind`, and the kernel's namespace fork on `rfork(RFNAMEG)`.

The interface between rio and its clients is _untyped byte streams over file descriptors_. The draw protocol is 23 message types. The mouse protocol is 10-byte messages. The text protocol is ASCII read/write. Everything goes through the filesystem namespace, and the namespace is per-process.

### What Wayland does

Wayland is a typed object protocol over Unix domain sockets. Clients create objects (surfaces, buffers, seats) by sending requests; the compositor sends events back on those objects. The protocol is versioned, extensible through protocol extensions (XML-defined interfaces), and strongly typed. There is no filesystem interface. There is no per-process namespace.

A Wayland client knows it's a Wayland client. It links against libwayland-client, creates a `wl_display`, binds to globals, and speaks the protocol. "Transparent" multiplexing in the rio sense is not possible because the client is aware of the display protocol at the library level.

### How wio bridges the gap

It doesn't, really. Wio translates rio's _visual behavior_ (the menu, the borders, the draw-a-rectangle-to-create-a-window interaction) but not rio's _architectural mechanism_ (filesystem multiplexing, per-process namespaces, transparent composition).

The cage-per-window approach is the closest wio gets to rio's model. In rio, a window contains a private instance of the display interface. In wio, a window contains cage, which is a private instance of a Wayland compositor. The analogy holds superficially: programs launched inside a wio window appear within that window's rectangle, just as programs launched inside a rio window draw within that window's rectangle.

But the analogy breaks down at every structural level:

1. **Resource cost:** In rio, creating a window costs a `fork()`, a `mount()`, and some bookkeeping in the rio process. Creating a wio window costs a cage process with its own wlroots instance — hundreds of megabytes of overhead per window.

2. **Transparency:** In rio, the child process doesn't need to know about rio. In wio, the child process needs to be a Wayland client. The terminal emulator (alacritty) needs to understand Wayland protocols, handle surface lifecycle, deal with buffer management. There is no illusion of talking to hardware.

3. **Recursive depth:** Rio inside rio is free — same interface, same cost. Cage inside cage inside wio is an exponential resource explosion.

4. **Filesystem interface:** Rio exports `/dev/wsys/*` for programmatic window manipulation from outside. Wio has no equivalent. Issue #2 proposed a FUSE filesystem but noted `/dev/text` (rio's text content file) would be "challenging" — because Wayland clients own their rendering buffers, and there's no protocol for extracting semantic text content from an arbitrary surface.

5. **Namespace isolation:** Rio's per-process namespaces mean each window has a clean, isolated view of the device tree. Wio's cage instances provide process isolation but not namespace-based device virtualization. A program in a wio window can't `open("/dev/cons")` — there is no `/dev/cons`.

DeVault acknowledged this gap. The README lists the missing pieces: "a Rio-esque FUSE filesystem" and "running things in their own namespace." The announcement blog post mentions FUSE as a potential path but notes the challenges. These features were never implemented.

### What this tells us about the translation problem

The rio model depends on three Plan 9 properties that Linux/Wayland doesn't have:

1. **Everything is a file.** Rio's interface is just files. The display is a file. The mouse is a file. Text I/O is a file. This means any tool that can read/write files can interact with the window system — `cat`, `echo`, shell scripts, remote machines over 9P.

2. **Per-process namespaces are cheap.** Plan 9's `rfork(RFNAMEG)` creates a copy-on-write namespace for a process. Mounting rio onto `/dev` in a child's namespace costs almost nothing. Linux has mount namespaces (`CLONE_NEWNS`), but they're heavier (they're designed for containers, not per-window isolation) and they don't compose with Wayland's socket-based protocol.

3. **The protocol is the same at every level.** The kernel's display driver, rio, and rio-inside-rio all speak the same `/dev/draw` protocol. Wayland has no equivalent: the protocol between compositor and client is fundamentally different from the protocol between compositor and hardware (DRM/KMS).

Without these properties, you can simulate rio's UX (the menu, the borders, the interaction patterns) but not its architecture (transparent multiplexing through the filesystem). Wio demonstrates exactly this boundary.

---

## What worked and what didn't

### What worked

**The visual translation.** Wio looks right. The gray background, the colored borders, the right-click menu — these are straightforward to implement in any compositor and they successfully evoke rio.

**The interaction model translation.** The multi-step mouse interactions (menu → select operation → draw rectangle → result) work on Wayland. The state machine approach is mechanical but effective. Community contributors refined it to handle edge cases (negative coordinates, corner-drag resizing, border occlusion) that rio handles naturally.

**Layer shell integration.** Because wio uses wlroots, it gets layer shell support for free, meaning waybar, swaybg, and similar tools work. DeVault notes this as a capability that's "easy thanks to wlroots, but difficult on Plan 9 without kernel hacking." This is a genuine advantage of the Wayland ecosystem — rio has no mechanism for persistent overlay panels because it predates that interaction pattern.

**Multihead and HiDPI.** Also free from wlroots. Rio on Plan 9 doesn't handle multiple monitors gracefully; it was designed for a single display.

### What didn't work

**The cage-per-window architecture.** Conceptually elegant, practically expensive. 564MB+ of RAM for the compositor alone means this approach doesn't scale. The double-fork spawning is fragile (matching PID to window geometry through a pipe). The extra compositor hop adds latency. Nobody built on this idea.

**The FUSE filesystem.** Never implemented. This was supposed to be the key architectural contribution — a `/dev/wsys`-like filesystem for programmatic window access. The blog post acknowledges `/dev/text` as particularly challenging because Wayland clients own their render buffers with no semantic text layer.

**Community momentum.** The project attracted a burst of contributions in 2019-2020 (Leon Plickat's border-drag work, Leonid Bobrov's 11-patch "Behave more like Rio" series, a handful of bug fixes), then stalled. The mailing list shows patches for wlroots API churn (0.13 compatibility fixes) and SSL certificate issues, but no new feature work since 2020. Most tracker issues remain open.

**Escape from toy status.** Wio never became usable as a daily driver. Missing: window hiding, fullscreen support, damage tracking for battery efficiency, client-initiated resize, the FUSE filesystem, the built-in command line. These are listed in the announcement as future work and none were completed.

---

## Drew DeVault's writing about wio and rio

DeVault's 2019 announcement post frames wio primarily as a demonstration of Wayland's capabilities: "This has been something I wanted to demonstrate on Wayland for a very long time." The emphasis is on nested compositors as a Wayland-native pattern, not on faithfully reproducing Plan 9's architecture.

His 2022 "In praise of Plan 9" post discusses rio's design approvingly — "Rio implements a /dev/draw-like interface in userspace, then mounts it in the filesystem namespace of its children" — but does not mention wio or reflect on his own attempt to translate rio's ideas. This suggests he sees wio as a demonstration rather than a serious architectural project.

The mailing list contains a thread titled "How close will wio stay to rio?" (Leon Plickat, ~2019) with a reply from DeVault, but the discussion content is not accessible from the archive index. The question itself is telling — there was genuine uncertainty about whether wio aimed to be a faithful rio clone or an rio-inspired compositor with its own direction.

DeVault's blog post on using cage for remote Wayland sessions (2019-04-23) provides context for the cage-per-window idea. Cage was originally designed as a kiosk compositor — run one app fullscreen, exit when it exits. Using it as a per-window container in wio was a creative repurposing, but cage wasn't designed to be instantiated hundreds of times.

---

## Relevance to pane

Pane is not trying to be rio. Pane draws from acme's philosophy (opinionated integration, text as interface, structured content) and BeOS's architecture (message-passing, typed data, kit-based API). But wio's experience with Plan 9 ideas on Wayland is directly informative.

### Lessons from wio

**1. Visual translation is easy; architectural translation is hard.** Wio proves you can make a Wayland compositor that looks and feels like rio. It also proves that rio's architectural properties (transparent filesystem multiplexing, per-process namespaces, recursive self-similarity) don't transfer. The surface UX is portable; the system architecture is not. Pane should internalize this distinction.

**2. The filesystem interface is the hard part.** Wio's biggest gap is the absence of a `/dev/wsys`-like filesystem. This is also the feature that would have mattered most — programmatic window manipulation from scripts and external tools. The challenge isn't implementing FUSE; it's that Wayland clients own their rendering buffers and there's no protocol for extracting semantic content. Pane's approach to external tool integration (the plumber, the message bus) needs to account for this: you can expose _compositor state_ as structured data, but you can't expose _client content_ without client cooperation.

**3. Nested compositors don't scale.** The cage-per-window approach is a dead end for general use. The resource cost is prohibitive and the architectural benefit (programs appear inside their parent window) can be achieved more cheaply through proper surface hierarchy management in the compositor itself. Wayland's subsurface protocol and wlr-foreign-toplevel-management provide some of this without the overhead.

**4. The state machine approach to modal interactions works.** Wio's explicit 15-state input state machine for menu → operation → target → result interactions is verbose but correct and debuggable. Pane's interaction model will be different (three-button mouse, text commands rather than menus), but any multi-step mouse interaction on Wayland will need a similar state machine. This is a mechanical translation problem, not a design problem.

**5. wlroots gives you the hard parts for free.** Multihead, HiDPI, layer shell, screencopy, gamma control, clipboard — wio gets all of these from wlroots without writing protocol code. Pane uses smithay instead of wlroots, but the lesson is the same: a compositor toolkit handles the protocol machinery, letting you focus on the window management model. The question is always what the toolkit _doesn't_ give you, and the answer is always "the interaction model and the integration architecture."

**6. Rio's specific UX patterns may not be the right ones to borrow.** Wio faithfully reproduces rio's right-click menu and draw-to-create interaction. These work, but they're designed for a system where every window is a terminal with rio's built-in text editing. On Wayland, where windows contain arbitrary GUI applications, the draw-to-create pattern is less natural (you're placing an opaque cage rectangle, not creating a conversational text context). Pane's acme-influenced model — where windows have tags, text is a command interface, and the plumber connects content to tools — is a better fit for Wayland's client model because it adds structure _above_ the Wayland protocol rather than trying to replace it.

### What wio confirms about pane's approach

Pane's decision to work _with_ Wayland's client model rather than against it is validated by wio's experience. Rio's power comes from replacing the display interface with a file server — you can't do that on Wayland without reimplementing the entire display stack, and if you did, no existing Wayland application would work with it. Better to accept Wayland's protocol model and build integration on top: structured tags for window identity, a plumber for inter-window navigation, a type system for content routing, message-passing for tool composition. These are acme's ideas more than rio's, and they compose with Wayland rather than fighting it.

Pane's use of smithay rather than wlroots means the specific wlroots APIs wio uses aren't directly relevant, but the _layer structure_ is: background → windows with borders → overlays → menus → cursor. Smithay provides equivalent primitives. The rendering model (clear, layer by layer, software cursor) is universal for Wayland compositors.

The one rio idea worth preserving is **programmatic access to the compositor's state**. Rio does this through the filesystem; wio never got there. Pane should provide this through its own mechanism (likely the message bus / structured IPC) rather than through FUSE. The point is not the filesystem — the point is that scripts and tools can inspect and manipulate windows without going through the GUI.
