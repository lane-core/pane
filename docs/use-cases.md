# Use Cases

How the architecture serves real applications on pane Linux.
Each scenario identifies which architectural components it exercises
and why the design decisions matter for that case.

---

## 1. Terminal emulator (pane-shell)

The first real application. A pane that hosts a pseudo-terminal,
parses VT escape sequences, and renders a text grid.

**Handler structure:**

```rust
struct Shell {
    pty: PtySource,       // calloop source wrapping the PTY fd
    screen: ScreenBuffer,
    clipboard: ClipboardHandle,
}

impl Handler for Shell {
    fn ready(&mut self, proxy: &Messenger) -> Result<Flow> {
        // No pulse timer — the shell is entirely event-driven.
        // PTY output arrives via PtySource (a calloop fd source),
        // not on a fixed timer. The looper wakes only when there
        // is actual data to process.
        Ok(Flow::Continue)
    }
    fn close_requested(&mut self, proxy: &Messenger) -> Result<Flow> {
        self.pty.signal(SIGHUP)?;
        Ok(Flow::Stop)
    }
    fn request_received(&mut self, proxy: &Messenger, service: ServiceId, msg: Box<dyn Any + Send>, reply: ReplyPort) -> Result<Flow> {
        // A script writes to /pane/1/ctl — inject into PTY.
        // Check ServiceId before downcasting (the convention from
        // architecture.md §Protocol and Dispatch).
        if service == service_id!("com.pane.shell.input") {
            if let Some(text) = msg.downcast_ref::<String>() {
                self.pty.write_all(text.as_bytes())?;
            }
        }
        drop(reply);
        Ok(Flow::Continue)
    }
}

impl DisplayHandler for Shell {
    fn key(&mut self, _proxy: &Messenger, event: KeyEvent) -> Result<Flow> {
        // Non-blocking write to the PTY's input buffer.
        // If the child process isn't reading (buffer full),
        // PtySource buffers the remainder and flushes on
        // the next calloop writability event. Direct write_all
        // would block here, violating I2.
        self.pty.enqueue_input(&event.to_bytes());
        Ok(Flow::Continue)
    }
}

#[pane::protocol_handler(Clipboard)]
impl Shell {
    fn lock_granted(&mut self, _proxy: &Messenger, lock: ClipboardWriteLock) -> Result<Flow> {
        lock.commit(self.screen.selection_bytes(), ClipboardMetadata {
            content_type: "text/plain".into(),
            sensitivity: Sensitivity::Normal,
            locality: Locality::Any,
        })?;
        Ok(Flow::Continue)
    }
    // ...
}
```

**Architecture exercised:**
- **Handler + DisplayHandler split**: the shell works headless
  (Handler only — PTY I/O, scripting) or with display (adds key
  input, visual rendering). A headless shell is a remote command
  executor.
- **Clipboard via Handles\<Clipboard\>**: copy/paste through the
  typed service protocol, not through the Message enum.
- **Event-driven I/O**: the shell has no pulse timer. PTY output
  arrives via a calloop fd source that wakes the looper only when
  the PTY fd is readable. Zero CPU when idle. (Pulse timers are
  for periodic work like health checks — see #2.)
- **request_received with ServiceId**: scripts inject text through
  the ad-hoc request path. The handler checks ServiceId before
  downcasting — the convention established in the architecture spec.
- **pane-fs namespace** (see `docs/pane-fs.md`): `/pane/1/body`
  is the terminal's semantic content (command output as text).
  `/pane/1/ctl` accepts line commands (same as writing to a
  Plan 9 `cons` file). `/pane/1/event` is a blocking-read
  JSONL stream of terminal events. No special IPC — the
  filesystem IS the scripting interface.

**Why headless matters:** A CI system runs `pane-shell` headless on
a build server. The shell pane exists in the namespace, runs
commands, produces output accessible at `/pane/1/body`. No
display needed. The same binary, the same protocol, the same
Handler code.

---

## 2. System monitor agent (headless, multi-machine)

A monitoring daemon that creates one headless pane per monitored
service. Runs on a remote machine. A dashboard on the user's
desktop connects to it for display.

**Architecture exercised:**
- **Headless as base case**: the agent is Handler only. No
  DisplayHandler, no compositor dependency. It connects to a
  headless pane server on the monitoring machine.
- **Multi-server topology**: the dashboard on the user's machine
  connects to the remote monitoring server (TLS) and the local
  compositor (unix socket) — two Connections in one App, routed
  by ServiceRouter.
- **pane-fs namespace**: each monitored service is a pane,
  accessible by number under `/pane/`. The per-signature index
  (`/pane/by-sig/com.ops.monitor/`) lists all monitoring panes.
  `cat /pane/3/body` returns the current health status.
  Alerting scripts read the event stream:
  `while read line < /pane/3/event; do ...`.
- **Session suspension (Phase 3)**: the dashboard pane suspends
  when the user closes it. The monitoring panes keep running
  headless. When the user reopens the dashboard, it resumes the
  suspended session — the monitoring panes were never affected.
  (Suspension is a Phase 3 feature; in Phase 1, the dashboard
  disconnects and reconnects fresh.)
- **Dispatch on_failed for degraded state**: the dashboard
  periodically requests metrics from each monitored service via
  `send_request`. If a service is down, `on_failed` fires and the
  dashboard updates that pane's status to "unreachable" rather
  than crashing:

```rust
fn pulse(&mut self, proxy: &Messenger) -> Result<Flow> {
    for (id, messenger) in &self.services {
        let id = *id;
        proxy.send_request::<Self, Metrics>(
            messenger,
            MetricsQuery,
            move |dashboard, proxy, metrics| {
                dashboard.update_status(id, Status::Healthy(metrics));
                Ok(Flow::Continue)
            },
            move |dashboard, _proxy| {
                dashboard.update_status(id, Status::Unreachable);
                Ok(Flow::Continue)
            },
        )?;
    }
    Ok(Flow::Continue)
}
```

- **Host as contingent server**: the monitoring machine is a server
  the dashboard connects to. The user's local machine is also a
  server (the compositor). Neither has architectural privilege.

---

## 3. Text editor with AI completion

An editor that uses a local language model for code completion.
The model runs in a worker thread; completions arrive as
application-defined protocol messages.

**Architecture exercised:**
- **Application-defined Protocol**: the editor defines its own
  protocol for model results, dispatched through typed
  `Handles<ModelProtocol>`:

```rust
struct ModelProtocol;
impl Protocol for ModelProtocol {
    const SERVICE_ID: ServiceId = service_id!("com.pane.editor.model");
    type Message = ModelMessage;
}

enum ModelMessage {
    Completion { cursor: usize, text: String, confidence: f32 },
    DiagnosticReady { path: String, diagnostics: Vec<Diagnostic> },
    IndexingProgress { files_done: u32, files_total: u32 },
}

#[pane::protocol_handler(ModelProtocol)]
impl Editor {
    fn completion(&mut self, proxy: &Messenger, cursor: usize, text: String, confidence: f32) -> Result<Flow> {
        if cursor == self.cursor_position {
            self.show_inline_completion(&text, confidence);
        }
        Ok(Flow::Continue)
    }
    fn diagnostic_ready(&mut self, proxy: &Messenger, path: String, diagnostics: Vec<Diagnostic>) -> Result<Flow> {
        self.update_diagnostics(&path, diagnostics);
        Ok(Flow::Continue)
    }
    fn indexing_progress(&mut self, _proxy: &Messenger, done: u32, total: u32) -> Result<Flow> {
        self.status_bar.set_progress(done, total);
        Ok(Flow::Continue)
    }
}
```

- **Exhaustive matching**: if the model adds a new message variant
  (e.g., `RefactorSuggestion`), the attribute macro generates a
  match that fails to compile until the editor handles it. No
  silent message drops.
- **Filter chain** (ShortcutFilter): the editor registers keyboard
  shortcuts as a composable filter:

```rust
let mut shortcuts = ShortcutFilter::new();
shortcuts.add(KeyCombo::new(Key::Char('s'), Modifiers::CTRL), "save", "");
shortcuts.add(KeyCombo::new(Key::Char('z'), Modifiers::CTRL), "undo", "");
pane.add_filter(shortcuts);
```

  When the user presses Ctrl+S, the filter transforms
  `Message::Key(event)` → `Message::CommandExecuted { command: "save" }`
  before the handler sees it. The handler's `command_executed()`
  method fires; `key()` never sees the shortcut keystroke. Filters
  run in registration order — a later logging filter would see
  `CommandExecuted`, not the original `Key`.

- **CancelHandle for stale completions**: when the user types,
  the editor sends a completion request to the model. If the user
  keeps typing before the model responds, the previous request is
  stale:

```rust
fn key(&mut self, proxy: &Messenger, event: KeyEvent) -> Result<Flow> {
    self.buffer.insert(event.char);
    // Cancel any outstanding completion request.
    if let Some(handle) = self.pending_completion.take() {
        handle.cancel(); // Dispatch entry removed, no callback fires
    }
    // Send a new completion request.
    let handle = proxy.send_request::<Self, Completion>(
        &self.model_messenger,
        CompletionQuery { cursor: self.cursor, context: self.context() },
        |editor, proxy, completion| {
            editor.show_inline_completion(&completion);
            Ok(Flow::Continue)
        },
        |editor, _proxy| {
            // Model unavailable — degrade gracefully, no crash.
            editor.clear_completion_hint();
            Ok(Flow::Continue)
        },
    )?;
    self.pending_completion = Some(handle);
    Ok(Flow::Continue)
}
```

  This demonstrates three Dispatch features at once: `send_request`
  with typed callbacks, `CancelHandle` for voluntary abort, and
  `on_failed` for graceful degradation when the model is
  unavailable.
- **`post_app_message` vs application-defined Protocol**: the
  editor uses `ModelProtocol` (a full Protocol with exhaustive
  dispatch) for the model's structured message vocabulary. But the
  editor also spawns a one-off worker thread to auto-save:

```rust
let messenger = proxy.clone();
std::thread::spawn(move || {
    if save_to_disk(&path, &content).is_ok() {
        // Simple fire-and-forget — no Protocol needed.
        let _ = messenger.post_app_message("Auto-save complete".to_string());
    }
});
```

  `post_app_message<T: AppPayload>` is for one-off notifications
  that don't warrant a full Protocol definition. The looper
  delivers it to the handler via the app-message dispatch path
  (downcast from `AppPayload`). The rule: if the message
  vocabulary is known at compile time and has multiple variants,
  use a Protocol. If it's a single fire-and-forget event, use
  `post_app_message`.

- **Undo integration** (see `docs/optics-design-brief.md`):
  `RecordingOptic` wraps the editor's property optics, capturing
  old/new values on each `set()`. `CoalescingPolicy` groups rapid
  keystrokes into single undo steps. These are kit-level types
  from the optics subsystem, not core protocol concepts.
- **Scripting via pane-fs** (see `docs/pane-fs.md`):
  `/pane/2/attrs/cursor` returns the cursor position.
  `/pane/2/attrs/selection` returns the selected text.
  External tools (linters, formatters) read and write through
  the namespace. The `DynOptic` trait (see `docs/scripting-optics-design.md`)
  handles type-erased serialization at the boundary; the editor's
  internal optics are monomorphic.

---

## 4. File manager as query engine

BeOS's Tracker was a file manager that doubled as a query UI
because the filesystem *was* the database. pane recovers this
through pane-fs.

**Architecture exercised:**
- **pane-fs as query system** (see `docs/pane-fs.md` §Unified
  Namespace and `docs/distributed-pane.md` §3): every directory
  under `/pane/` is a computed view — a filter predicate over
  indexed pane state. The file manager navigates query directories
  the same way it navigates regular directories. The query engine
  is pane-fs; the file manager just reads directories.
- **Routing** (Phase 3 — `RoutingHandler`): double-clicking a file
  routes the content to a handler. The routing table matches
  content type to application signature. Multi-match presents a
  chooser to the user. Routing quality scoring (0.0–1.0, from
  Be's Translation Kit pattern) is a routing subsystem detail,
  not core protocol — see `docs/v1-subsystem-inventory.md` §Routing.
- **Clipboard with locality**: copying a file path from the local
  file manager and pasting into a remote terminal session — the
  clipboard entry has `Locality::Any`, so the remote instance's
  namespace mount sees it. A password manager would use
  `Sensitivity::Secret { ttl: 30s }` with `Locality::Local` —
  auto-cleared after 30 seconds, invisible to remote mounts.
  (The architecture spec defines `Sensitivity` and `Locality`
  as enum types; the `Secret { ttl }` and `Local` variants
  shown here are illustrative — the final variant set is
  determined during clipboard service implementation.)
- **Observer pattern via pane-notify**: the file manager watches
  the displayed directory. When a file is created or renamed,
  `pane-notify` delivers the event. The file manager updates its
  display. No polling, no custom IPC — filesystem notifications.

---

## 5. Chat application (multi-server, federation)

A messaging client that connects to a remote chat service and the
local compositor. Demonstrates the multi-Connection architecture.

**Architecture exercised:**
- **Multi-server topology**: the App holds two Connections:
  1. Local compositor (unix socket) — provides Display
  2. Remote chat service (TLS) — provides a custom `ChatProtocol`

  The ServiceRouter maps `com.pane.display` to Connection 1 and
  `com.example.chat` to Connection 2. The developer writes one
  set of handler code; the routing is invisible.
- **Per-Connection failure isolation**: if the chat server drops,
  `chat_service_lost()` fires but the display remains alive. The
  user sees "Reconnecting..." in the pane, not a crash. If the
  compositor crashes, the chat Connection is unaffected — messages
  keep arriving, the handler keeps processing them, the pane just
  can't display.
- **Cross-Connection causality via send_request**: receiving a
  "new message" event from the chat server and updating the
  display is a cross-Connection pattern. The handler's control
  flow establishes the causal order:

```rust
fn message_received(&mut self, proxy: &Messenger, msg: ChatMessage) -> Result<Flow> {
    self.messages.push(msg);
    proxy.set_content(self.render_messages())?; // goes to compositor Connection
    Ok(Flow::Continue)
}
```

  No cross-Connection ordering guarantee needed — the handler's
  sequential execution IS the ordering.

- **Session suspension (Phase 3)**: the user closes their laptop.
  The compositor Connection suspends. The chat Connection stays
  active (different server, independent suspension). Messages
  accumulate. When the laptop opens, the compositor Connection
  resumes, the handler receives `ready()`, and renders the
  accumulated messages. (In Phase 1, the compositor Connection
  disconnects; the chat Connection is unaffected either way.)

---

## 6. Build system dashboard (RoutingHandler — Phase 3)

A build tool that exposes build status through the namespace.
No display — it's a headless daemon that other panes query.

**Architecture exercised:**
- **RoutingHandler (Phase 3)**: the build daemon implements
  RoutingHandler to serve namespace queries. In Phase 1, the same
  functionality is achieved through `request_received` with
  ServiceId-based dispatch; RoutingHandler formalizes this as a
  trait in Phase 3.

```rust
impl Handler for BuildDaemon {
    fn ready(&mut self, proxy: &Messenger) -> Result<Flow> {
        proxy.set_content(b"idle")?;
        Ok(Flow::Continue)
    }
}

// Phase 3 — RoutingHandler formalizes namespace query handling.
impl RoutingHandler for BuildDaemon {
    fn route_query(&mut self, proxy: &Messenger, query: RouteQuery, reply: ReplyPort) -> Result<Flow> {
        // ReplyPort::reply takes impl Serialize + Send + 'static.
        // Strings serialize naturally via postcard.
        match query.path() {
            "status" => reply.reply(self.status.to_string()),
            "targets" => reply.reply(self.targets.join("\n")),
            "log" => reply.reply(self.current_log.clone()),
            _ => drop(reply), // ReplyFailed for unknown paths
        }
        Ok(Flow::Continue)
    }
    fn route_command(&mut self, proxy: &Messenger, cmd: &str, args: &str) -> Result<Flow> {
        if cmd == "build" {
            self.start_build(args)?;
        }
        Ok(Flow::Continue)
    }
}
```

- **Namespace as API** (see `docs/pane-fs.md`):
  `/pane/5/attrs/status` → "building".
  `/pane/5/attrs/targets` → "kernel\nlibc\ninit".
  `echo "build kernel" > /pane/5/ctl` starts a build.
  Shell scripts automate builds through the filesystem.
  The per-signature index at `/pane/by-sig/com.dev.build/`
  lists all build daemon panes.
- **Pane death notification**: the editor registers interest in
  the build pane's exit via `pane_exited()` on Handler. When the
  build daemon exits (crash or completion), the editor receives
  `PaneExited { pane, reason }` and updates its diagnostics.
  This is push-based via the ExitBroadcaster (architecture spec
  §Termination semantics), not polling.
- **Headless remote**: the build daemon runs on a powerful remote
  machine. The developer's local editor connects to it via
  `App::connect_service()`. Same protocol, same routing, different
  machine. The developer writes `build kernel` to
  `/pane/5/ctl`; the command routes to the remote build daemon
  through pane-fs.

---

## 7. The guide (AI agent as system inhabitant)

The introduction describes a guide agent that teaches new users by
demonstrating the system. The guide is a pane.

**Architecture exercised:**
- **Agent as user**: the guide runs under its own unix account.
  Its sandbox policy (Landlock rules, network namespace
  restrictions) governs what it can access. PeerAuth::Kernel
  identifies it by uid, not by self-reported string.
- **Scripting via pane-fs (Phase 3 — RoutingHandler enables
  cross-pane writes)**: the guide reads and writes other panes'
  properties to demonstrate features. It writes to
  `/pane/2/attrs/theme` to show theming. It reads
  `/pane/1/attrs/cursor` to point out where the user is. The
  optic discipline (GetPut, PutGet) guarantees that reading after
  writing returns the written value — the guide's demonstrations
  are reliable, not racy. (In Phase 1, the guide can read
  attributes but cross-pane writes require RoutingHandler.)
- **Clipboard for teaching**: the guide copies example commands to
  the clipboard with `Sensitivity::Normal, Locality::Local` — the
  user pastes them into a shell. The guide doesn't need display
  access to do this; clipboard is an independent service
  Connection.
- **pane-fs for self-description**: `cat /pane/7/body` returns
  what the guide is currently saying. `cat /pane/7/attrs/topic`
  returns what it's teaching. A curious user discovers this and
  learns the namespace by using it to inspect their teacher.

---

## Architectural themes across use cases

**Headless is not degraded.** The monitoring agent (#2), build
daemon (#6), and guide (#7) are headless by design, not by
limitation. They are full participants in the protocol, the
namespace, the scripting system. Display is something they could
opt into, not something they're missing.

**The namespace is the scripting interface.** Every use case
involves reading or writing pane-fs paths (`/pane/<n>/body`,
`/pane/<n>/attrs/...`, `/pane/<n>/ctl`, `/pane/<n>/event`).
Shell scripts, external tools, and AI agents all use the same
filesystem interface. No SDK needed for basic automation — `cat`
and `echo` are sufficient. See `docs/pane-fs.md` for the full
path convention.

**Multi-server is invisible to handlers.** The chat client (#5)
connects to two servers. The editor (#3) might connect to a
remote LSP server. The developer writes one set of handler methods;
ServiceRouter handles the Connection topology. Handler code is
identical for local and remote.

**Protocol extensibility via Handles\<P\>.** The editor's
ModelProtocol (#3), the chat's ChatProtocol (#5), and the
clipboard service (#1, #4) all use the same dispatch pattern.
New services are additive — they don't modify Handler, Message,
or any existing trait.

**Failure isolation preserves function.** Compositor crash doesn't
kill the chat (#5). Chat server drop doesn't kill the display.
Build server disconnect doesn't kill the editor. Each Connection
fails independently; the pane continues with its remaining
capabilities.

---

## Feature index

Quick reference: which use case demonstrates which feature.

| Feature | Demonstrated in |
|---|---|
| Handler + DisplayHandler split | Shell (#1), Monitor (#2) |
| Handles\<P\> for services | Shell (#1), Editor (#3), Chat (#5) |
| Application-defined Protocol | Editor (#3) |
| `post_app_message` (fire-and-forget) | Editor (#3) |
| Filter chain (ShortcutFilter) | Editor (#3) |
| Dispatch + `send_request` | Editor (#3), Monitor (#2), Chat (#5) |
| Dispatch `on_failed` (degraded state) | Monitor (#2), Editor (#3) |
| CancelHandle | Editor (#3) |
| Multi-server topology | Monitor (#2), Chat (#5), Build (#6) |
| Per-Connection failure isolation | Chat (#5) |
| pane-fs namespace | Shell (#1), Monitor (#2), File manager (#4), Build (#6), Guide (#7) |
| RoutingHandler (Phase 3) | Build (#6), File manager (#4) |
| Session suspension (Phase 3) | Monitor (#2), Chat (#5) |
| Optics / scripting | Editor (#3), Guide (#7) |
| Clipboard | Shell (#1), File manager (#4), Guide (#7) |
| pane-notify | File manager (#4) |
| Headless-first | Shell (#1), Monitor (#2), Build (#6), Guide (#7) |
