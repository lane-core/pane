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
    pty: Pty,
    screen: ScreenBuffer,
    clipboard: ClipboardHandle,
}

impl Handler for Shell {
    fn ready(&mut self, proxy: &Messenger) -> Result<Flow> {
        proxy.set_pulse_rate(Duration::from_millis(16))?; // 60fps
        Ok(Flow::Continue)
    }
    fn close_requested(&mut self, proxy: &Messenger) -> Result<Flow> {
        self.pty.signal(SIGHUP)?;
        Ok(Flow::Stop)
    }
    fn request_received(&mut self, proxy: &Messenger, service: ServiceId, msg: Box<dyn Any + Send>, reply: ReplyPort) -> Result<Flow> {
        // A script writes to /pane/<id>/text — inject into PTY
        if let Some(text) = msg.downcast_ref::<String>() {
            self.pty.write_all(text.as_bytes())?;
        }
        drop(reply);
        Ok(Flow::Continue)
    }
}

impl DisplayHandler for Shell {
    fn key(&mut self, _proxy: &Messenger, event: KeyEvent) -> Result<Flow> {
        self.pty.write_all(&event.to_bytes())?;
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
- **request_received with ServiceId**: scripts write text to the
  shell through the ad-hoc request path. The ServiceId tells the
  shell what kind of injection this is.
- **pane-fs namespace**: `/pane/<id>/text` is a blocking-read file
  (Plan 9 pattern). `cat /pane/<id>/text` streams terminal output.
  `echo "ls" > /pane/<id>/cons` sends input. No special IPC —
  the filesystem IS the scripting interface.

**Why headless matters:** A CI system runs `pane-shell` headless on
a build server. The shell pane exists in the namespace, runs
commands, produces output accessible at `/pane/<id>/text`. No
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
- **pane-fs namespace**: each monitored service is a pane. The
  unified namespace shows them at `/pane/monitor-web/`,
  `/pane/monitor-db/`, etc. `cat /pane/monitor-db/content` returns
  the current health status as bytes. Alerting scripts are just
  `while read status < /pane/monitor-db/event; do ...`.
- **Session suspension**: the dashboard pane suspends when the user
  closes it. The monitoring panes keep running headless. When the
  user reopens the dashboard, it resumes the suspended session —
  the monitoring panes were never affected.
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
- **Undo integration**: `RecordingOptic` wraps the editor's
  property optics. When the user accepts a completion, the
  insertion is recorded with old/new values. Undo reverses it.
  `CoalescingPolicy` groups rapid keystrokes into single undo
  steps.
- **Scripting via optics**: `/pane/<id>/attr/cursor` returns the
  cursor position. `/pane/<id>/attr/selection` returns the
  selected text. External tools (linters, formatters) read and
  write through the namespace. `DynOptic` handles the
  serialization at the boundary; the editor's internal optics are
  monomorphic.

---

## 4. File manager as query engine

BeOS's Tracker was a file manager that doubled as a query UI
because the filesystem *was* the database. pane recovers this
through pane-fs.

**Architecture exercised:**
- **pane-fs as query system**: navigating to
  `/pane/query/kind=image&modified>2026-03-01` returns a synthetic
  directory listing of files matching the query. The file manager
  doesn't implement queries — it reads directories. The query
  engine is pane-fs.
- **Routing**: double-clicking a file routes the content to a
  handler. The routing table matches content type to application
  signature. `RouteResult::MultiMatch` presents a chooser to the
  user. The quality rating determines the default.
- **Clipboard with locality**: copying a file path from the local
  file manager and pasting into a remote terminal session — the
  clipboard entry has `Locality::Any`, so the remote instance's
  namespace mount sees it. Copying a password from a local
  password manager uses `Sensitivity::Secret { ttl: 30s }` with
  `Locality::Local` — auto-cleared after 30 seconds, invisible
  to remote mounts.
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

- **Session suspension**: the user closes their laptop. The
  compositor Connection suspends. The chat Connection stays active
  (different server, independent suspension). Messages accumulate.
  When the laptop opens, the compositor Connection resumes, the
  handler receives `ready()`, and renders the accumulated messages.

---

## 6. Build system dashboard (RoutingHandler)

A build tool that exposes build status through the namespace.
No display — it's a headless daemon that other panes query.

**Architecture exercised:**
- **RoutingHandler**: the build daemon implements RoutingHandler
  to serve namespace queries:

```rust
impl Handler for BuildDaemon {
    fn ready(&mut self, proxy: &Messenger) -> Result<Flow> {
        proxy.set_content(b"idle")?;
        Ok(Flow::Continue)
    }
}

impl RoutingHandler for BuildDaemon {
    fn route_query(&mut self, proxy: &Messenger, query: RouteQuery, reply: ReplyPort) -> Result<Flow> {
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

- **Namespace as API**: `/pane/build/attr/status` → "building".
  `/pane/build/attr/targets` → "kernel\nlibc\ninit".
  `echo "build kernel" > /pane/build/cmd` starts a build.
  Shell scripts automate builds through the filesystem.
- **Messenger::monitor**: the editor pane monitors the build pane.
  When the build finishes (pane_exited or state change), the editor
  updates its diagnostics. Push-based, not polling.
- **Headless remote**: the build daemon runs on a powerful remote
  machine. The developer's local editor connects to it via
  `App::connect_service()`. Same protocol, same routing, different
  machine. The developer types `build kernel` in their local
  command surface; the command routes to the remote build daemon
  through pane-fs.

---

## 7. The guide (AI agent as system inhabitant)

The introduction describes a guide agent that teaches new users by
demonstrating the system. The guide is a pane.

**Architecture exercised:**
- **Agent as user**: the guide runs under its own unix account.
  Its `.plan` file governs what it can access (Landlock sandbox,
  network namespace restrictions). PeerAuth::Kernel identifies it
  by uid, not by self-reported string.
- **Scripting via optics**: the guide reads and writes other panes'
  properties to demonstrate features. It writes to
  `/pane/editor/attr/theme` to show theming. It reads
  `/pane/shell/attr/cursor` to point out where the user is. The
  optic discipline (GetPut, PutGet) guarantees that reading after
  writing returns the written value — the guide's demonstrations
  are reliable, not racy.
- **Clipboard for teaching**: the guide copies example commands to
  the clipboard with `Sensitivity::Normal, Locality::Local` — the
  user pastes them into a shell. The guide doesn't need display
  access to do this; clipboard is an independent service
  Connection.
- **pane-fs for self-description**: `cat /pane/guide/text` returns
  what the guide is currently saying. `cat /pane/guide/attr/topic`
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
involves reading or writing `/pane/<id>/...` paths. Shell scripts,
external tools, and AI agents all use the same filesystem
interface. No SDK needed for basic automation — `cat` and `echo`
are sufficient.

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
