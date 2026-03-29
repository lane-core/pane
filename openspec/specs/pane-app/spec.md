# pane-app Kit Specification

The pane-app kit is the developer's primary interface for building pane-native applications. It is analogous to BeOS's Application Kit -- the foundation on which everything else is built. BApplication, BLooper, BHandler, BMessenger: these four classes were the spine of BeOS. Their Rust equivalents in pane-app serve the same structural role.

The standard this API must meet was stated by Benoit Schillings in Be Newsletter #1-2: "common things are easy to implement and the programming model is CLEAR. You don't need to know hundreds of details to get simple things working."

This spec defines the API in terms of actual Rust type signatures. It is a component spec, not a research document.

---

## Hello Pane

This is the litmus test. A minimal pane-native application that connects to the compositor, creates a pane with a tag line, handles input events, and exits cleanly.

```rust
use pane_app::{App, Tag, cmd, BuiltIn};

fn main() -> pane_app::Result<()> {
    let app = App::connect("com.example.hello")?;

    let pane = app.create_pane(
        Tag::new("Hello").commands(vec![
            cmd("close", "Close this pane")
                .shortcut("Alt+W")
                .built_in(BuiltIn::Close),
        ]),
    )?;

    pane.run(|event| match event {
        pane_app::PaneEvent::Key(key) if key.is_escape() => Ok(false),
        pane_app::PaneEvent::Close => Ok(false),
        _ => Ok(true),
    })
}
```

Fourteen lines. The tag is declared as a title ("Hello") with one command (close). At rest, the pane shows a tab labeled "Hello" with a close widget and a `:` indicator. Hitting the activation key opens the command surface; typing "close" or browsing the empty-query list shows the close command with its Alt+W shortcut.

What happens underneath:

1. `App::connect` opens a unix socket to pane-comp, runs the session-typed handshake (ClientHello → ServerHello → Capabilities → Branch), registers with pane-roster, and stores the active-phase transport.
2. `app.create_pane` sends a CreatePane request over the active-phase protocol, receives a PaneId and an initial Resize, spawns a per-pane looper thread with its own sub-session, and returns a Pane handle. The `Tag` is sent to the compositor, which renders the tab chrome.
3. `pane.run` enters the pane's message loop on the per-pane thread, dispatching CompToClient messages to the closure. Returning `Ok(false)` sends RequestClose and exits the loop.
4. When all panes are closed, `App` sends a graceful disconnect and drops the connection.

A pane with no commands: `app.create_pane(Tag::new("Status"))`. A component pane with no tag line at all: `app.create_pane(None)`.

The developer does not need to know any of this. The 14 lines above are the complete, working application.

---

## 1. The Application Type

### What it is

`App` is the entry point for every pane-native process. One per process. It connects to pane-comp, handshakes with session types, registers with pane-roster, and provides the factory for creating panes.

In BeOS, BApplication was a BLooper -- it inherited the message loop and thread. In pane, `App` is *not* a looper. The app object owns the connection to the compositor and the roster registration, but message loops live on per-pane threads. This is a deliberate departure: BApplication's looper was a source of confusion (what messages go to the app looper vs. the window looper?), and the common case was to override `BApplication::MessageReceived` only to dispatch to a window anyway.

### Type definition

```rust
/// The application entry point. One per process.
///
/// Connects to pane-comp, registers with pane-roster, and provides
/// the factory for creating panes.
///
/// # Lifecycle
/// 1. `App::connect(signature)` -- handshake with compositor
/// 2. `app.create_pane(tag)` -- create panes (each gets its own thread)
/// 3. Panes run their message loops
/// 4. When the last pane closes, or `app.quit()` is called, the app
///    sends a graceful disconnect and drops its connection.
///
/// # Thread safety
/// `App` is `Send` but not `Sync`. It is created on the main thread
/// and can be moved to another thread, but it must not be shared.
/// Pane handles obtained from `create_pane` are independently `Send`.
pub struct App {
    /// Active-phase writer to the compositor.
    conn: Connection,
    /// Application signature (MIME-style identifier).
    signature: String,
    /// Roster registration handle. Dropped on disconnect.
    roster: Option<RosterRegistration>,
    /// Tracks live panes. When this reaches zero and no new panes
    /// are being created, the app can exit.
    pane_count: Arc<AtomicUsize>,
}
```

### Construction

```rust
impl App {
    /// Connect to the compositor and register with the roster.
    ///
    /// `signature` is a reverse-DNS identifier (e.g., "com.example.editor").
    /// This is used for:
    /// - Launch semantics (single-launch apps are identified by signature)
    /// - Roster registration
    /// - Routing rule matching
    ///
    /// The connection handshake is session-typed. If the compositor rejects
    /// the connection (version mismatch, signature conflict), this returns
    /// an error with a reason.
    ///
    /// # Errors
    /// - `ConnectError::NotRunning` -- compositor socket not found
    /// - `ConnectError::Rejected(reason)` -- handshake rejected
    /// - `ConnectError::Io(e)` -- transport-level failure
    pub fn connect(signature: &str) -> Result<Self, ConnectError> { .. }

    /// Connect with explicit launch semantics.
    ///
    /// Launch semantics control what happens when a second instance of
    /// this signature tries to launch:
    /// - `SingleLaunch`: the launch message is delivered to the existing
    ///   instance. The new process exits.
    /// - `ExclusiveLaunch`: only one instance may exist. The new process
    ///   gets an error.
    /// - `MultipleLaunch`: no restriction (default).
    ///
    /// These map directly to BeOS's B_SINGLE_LAUNCH / B_EXCLUSIVE_LAUNCH /
    /// B_MULTIPLE_LAUNCH semantics via pane-roster.
    pub fn connect_with(
        signature: &str,
        launch: LaunchMode,
    ) -> Result<Self, ConnectError> { .. }
}

/// What happens when a second instance of this signature launches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    /// Deliver the launch message to the existing instance.
    SingleLaunch,
    /// Reject the second launch.
    ExclusiveLaunch,
    /// Allow multiple instances (default).
    MultipleLaunch,
}
```

### The handshake (hidden from the developer)

Inside `App::connect`, the kit runs the session-typed handshake:

```
Client                              Compositor
  |-- ClientHello { sig, version } ------>|
  |<------------ ServerHello { caps } ----|
  |-- ClientCaps { requested } ---------->|
  |                                       |
  |     (compositor decides)              |
  |                                       |
  |<-- [Accept] -- Accepted { resolved }--| -> active phase
  |<-- [Reject] -- Rejected { reason } ---| -> error
```

The session type for the client side:

```rust
/// The client-side handshake protocol.
///
/// After acceptance, `finish()` yields the transport for the active phase.
pub type PaneHandshake = Send<ClientHello,
    Recv<ServerHello,
        Send<ClientCaps,
            Branch<
                Recv<Accepted, End>,    // accepted -> finish -> active phase
                Recv<Rejected, End>,    // rejected -> ConnectError
            >>>>;
```

The developer never writes this type, never calls `send()` or `recv()` on a `Chan`, never matches on `BranchResult`. The kit does it internally. This is the BApplication::_ConnectToServer() pattern -- in Haiku, the connection to app_server involved AS_CREATE_APP messages, port creation, and server memory allocation. None of that was visible to the developer. They wrote `BApplication("application/x-vnd.My-App")` and it worked.

### Lifecycle

```rust
impl App {
    /// Quit the application. Sends RequestClose to all panes,
    /// waits for confirmation, then disconnects.
    ///
    /// If called from a pane's event handler, the close propagates
    /// after the handler returns.
    pub fn quit(&self) { .. }

    /// Block until all panes have closed.
    ///
    /// This is the typical main-thread pattern:
    /// ```
    /// let app = App::connect("com.example.foo")?;
    /// let pane = app.create_pane(tag)?;
    /// pane.spawn(); // start the looper on its own thread
    /// app.wait();   // block until all panes close
    /// ```
    pub fn wait(&self) { .. }

    /// The application signature.
    pub fn signature(&self) -> &str { .. }

    /// Whether this application is the first instance of its signature.
    /// Useful for single-launch apps that need to know if they are the
    /// primary or a secondary launch.
    pub fn is_primary(&self) -> bool { .. }
}
```

### Why App is not a Looper

In BeOS, BApplication inherited BLooper. This meant the app object had its own thread and message queue. Messages that weren't targeted at a specific window went to the app looper. This was useful for:

- Application-wide messages (B_ABOUT_REQUESTED, B_ARGV_RECEIVED, B_REFS_RECEIVED)
- Single-launch re-delivery (new instance's args arrive as messages to the existing app)
- System-wide scripting (the app is the root of the specifier hierarchy)

In pane, these responsibilities are handled differently:

| BeOS mechanism | Pane equivalent |
|---|---|
| B_ABOUT_REQUESTED | Routing rule: "about" action dispatches to the app's about handler |
| B_ARGV_RECEIVED | `App::connect_with` handles re-delivery through pane-roster |
| B_REFS_RECEIVED | Routing rule: file type dispatch to the appropriate pane |
| Scripting root | The filesystem at `/pane/` is the root of the scripting hierarchy |
| App-wide messages | Per-pane handlers or a shared state Arc |

The app object doesn't need its own message loop because per-pane loopers handle all the message processing, and inter-pane coordination goes through shared state (Arc<Mutex<T>>) or the compositor. This is simpler and eliminates the "where does this message go?" confusion that BeOS developers regularly encountered.

---

## 2. The Pane Type

### What it is

`Pane` is the per-pane handle. Each pane is a sub-session with the compositor, running on its own thread. This is the BWindow equivalent -- BWindow inherited BLooper, giving every window its own thread and message queue. Pane does the same, but the looper is internal rather than inherited.

### Type definition

```rust
/// A handle to a single pane (window/panel).
///
/// Each pane has:
/// - Its own looper thread for message dispatch
/// - A sub-session connection to the compositor
/// - A tag line (compositor-rendered chrome)
/// - A body (client-rendered content area)
///
/// The pane's looper thread processes compositor messages (resize, focus,
/// input events) and dispatches them to the handler. Heavy work should
/// be spawned on separate threads -- the looper must stay responsive.
///
/// George Hoffman (Be Newsletter #2-36): "Keeping a window locked or its
/// thread occupied for long periods of time (i.e. over half a second or
/// so) is Not Good."
pub struct Pane {
    /// Pane identity assigned by the compositor.
    id: PaneId,
    /// Initial geometry from PaneCreated.
    geometry: PaneGeometry,
    /// Receiver for the unified LooperMessage channel.
    receiver: Receiver<LooperMessage>,
    /// Sender to the compositor (for RequestClose after exit).
    comp_tx: Sender<ClientToComp>,
    /// Sender to this pane's looper (cloned into PaneHandle).
    looper_tx: Sender<LooperMessage>,
    /// Shared reference to the app's pane count.
    pane_count: Arc<AtomicUsize>,
    /// Signal for app.wait() when all panes exit.
    done_signal: Arc<(Mutex<()>, Condvar)>,
    /// Filter chain applied before handler dispatch.
    filters: FilterChain,
}
```

### Creation

```rust
impl App {
    /// Create a new pane with the given tag line.
    ///
    /// The pane is created but not yet running -- call `run()` to enter
    /// the message loop on the current thread, or `spawn()` to run it
    /// on a new thread.
    ///
    /// This sends a CreatePane request to the compositor and blocks until
    /// the compositor responds with the pane's id and initial geometry.
    /// The compositor also creates its server-side pane thread at this point.
    ///
    /// # Errors
    /// - `PaneError::Disconnected` -- compositor connection lost
    /// - `PaneError::Refused(reason)` -- compositor refused the pane
    pub fn create_pane(&self, tag: Tag) -> Result<Pane> { .. }

    /// Create a component pane (no tag line).
    pub fn create_component_pane(&self) -> Result<Pane> { .. }
}
```

### Running the message loop

Two patterns, matching BeOS's `BWindow::Show()` (background) and `BApplication::Run()` (foreground):

```rust
impl Pane {
    /// Run the pane's message loop on the current thread.
    ///
    /// The closure receives events and returns `Ok(true)` to continue
    /// or `Ok(false)` to exit the loop. An error in the closure
    /// exits the loop and propagates the error.
    ///
    /// This is the simple case -- one pane, running on the main thread.
    /// For multi-pane applications, use `spawn()`.
    pub fn run<F>(self, handler: F) -> Result<()>
    where
        F: FnMut(PaneEvent) -> Result<bool>,
    { .. }

    /// Run the pane's message loop with a stateful handler.
    ///
    /// For applications that need mutable state across events.
    /// The handler trait provides typed dispatch.
    pub fn run_with<H: Handler>(self, handler: H) -> Result<()> { .. }

    // For multi-pane applications, spawn each on its own thread:
    //
    //   let p1 = app.create_pane(tag1)?;
    //   let p2 = app.create_pane(tag2)?;
    //   thread::spawn(move || p1.run(handler1));
    //   thread::spawn(move || p2.run(handler2));
    //   app.wait(); // block until both close
}

/// A lightweight, cloneable handle for sending messages to the compositor
/// and posting events to the pane's own looper. The BMessenger equivalent.
///
/// Use inside event handlers and spawned threads to update pane state
/// without owning the Pane itself. Clone freely — each clone sends to
/// the same channels.
#[derive(Clone)]
pub struct PaneHandle { .. }

impl PaneHandle {
    /// The pane's compositor-assigned ID.
    pub fn id(&self) -> PaneId { .. }

    // --- Compositor-bound messages ---

    /// Update the pane's title.
    pub fn set_title(&self, title: PaneTitle) -> Result<()> { .. }

    /// Update the pane's command vocabulary.
    pub fn set_vocabulary(&self, vocabulary: CommandVocabulary) -> Result<()> { .. }

    /// Update the pane's body content.
    pub fn set_content(&self, content: &[u8]) -> Result<()> { .. }

    /// Respond to a completion request.
    pub fn set_completions(&self, token: u64, completions: Vec<Completion>) -> Result<()> { .. }

    // --- Self-delivery (BLooper::PostMessage) ---

    /// Post an event to this pane's own looper.
    ///
    /// The BLooper::PostMessage equivalent. Worker threads (network,
    /// computation) post results back to the event loop for sequential
    /// processing. The event goes through the same filter chain and
    /// handler dispatch as compositor events.
    pub fn post_event(&self, event: PaneEvent) -> Result<()> { .. }

    /// Post an event after a delay (BMessageRunner single-shot).
    pub fn post_delayed(&self, event: PaneEvent, delay: Duration) -> Result<()> { .. }

    /// Post an event repeatedly at an interval (BMessageRunner periodic).
    ///
    /// Returns a TimerToken for cancellation. First delivery after
    /// one interval. The timer stops when cancelled or when the pane exits.
    pub fn post_periodic(&self, event: PaneEvent, interval: Duration) -> Result<TimerToken> { .. }
}

/// Token for cancelling a periodic timer. Call cancel() explicitly.
pub struct TimerToken { .. }

impl TimerToken {
    pub fn cancel(&self) { .. }
}
```

### Pane operations

```rust
impl Pane {
    /// Get the pane's compositor-assigned ID.
    pub fn id(&self) -> PaneId { .. }

    /// Get the current geometry.
    pub fn geometry(&self) -> PaneGeometry { .. }

    /// Get a PaneHandle for this pane.
    pub fn proxy(&self) -> PaneHandle { .. }

    /// Add a filter to the pane's filter chain.
    pub fn add_filter(&mut self, filter: impl Filter) { .. }
}
```

---

## 3. Events

### The PaneEvent enum

This is the CompToClient enum, but named for the developer. BeOS's BWindow::DispatchMessage handled system messages (B_WINDOW_RESIZED, B_KEY_DOWN, B_MOUSE_DOWN, B_QUIT_REQUESTED) and dispatched them to handler methods. Pane collapses this into a flat enum that the handler matches on.

```rust
/// Events delivered to a pane's message loop by the compositor.
///
/// The looper thread receives these in order. Each event must be
/// handled promptly -- the looper thread must stay responsive.
///
/// Rust's exhaustive match guarantees every variant is handled.
/// This is the session type guarantee transferred to the active phase:
/// the type system enforces completeness of handling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaneEvent {
    // -- Geometry --

    /// The pane was resized. Includes new dimensions and scale factor.
    /// Respond by re-rendering content to fit the new size.
    Resize(PaneGeometry),

    // -- Focus --

    /// This pane gained focus.
    Focus,
    /// This pane lost focus.
    Blur,

    // -- Input --

    /// A keyboard event directed at this pane.
    Key(KeyEvent),
    /// A mouse event directed at this pane.
    Mouse(MouseEvent),

    // -- Tag line --

    /// A tag action was executed by the user (clicked, B2-executed, etc.).
    /// The action is identified by its label + command.
    TagAction(TagAction),
    /// A route action was triggered from the tag line.
    /// The kit has already evaluated routing rules; this carries the result.
    TagRoute(RouteResult),

    // -- Lifecycle --

    /// The compositor is closing this pane (user action or quit sequence).
    /// Return `Ok(false)` from the handler to acknowledge.
    Close,
    /// The compositor connection was lost (crash or restart).
    /// The kit is attempting reconnection. The pane should preserve its
    /// state and wait. If reconnection succeeds, a Reconnected event follows.
    Disconnected,
    /// The compositor connection was restored after a disconnect.
    /// The pane has been re-registered. A Resize follows with current geometry.
    Reconnected,

    // -- Scripting --

    /// A scripting query addressed to this pane.
    /// See section 7 (Scripting Integration).
    ScriptQuery(ScriptQuery),
}
```

### Why a flat enum, not handler methods

BeOS used virtual method dispatch: `BWindow::MessageReceived(BMessage*)` switched on `message->what`. BHandler subclasses overrode `MessageReceived` and called the base class for unrecognized messages. This was flexible but had two problems:

1. **Non-exhaustive.** Forgetting to handle a message code compiled fine. The base class silently swallowed it. Bugs from unhandled message types were common and silent.

2. **The type was erased.** BMessage carried typed data, but the `what` code was a uint32. The developer had to know which fields to extract and what types they were. Getting it wrong was a runtime error.

Pane's `PaneEvent` enum solves both. Rust's exhaustive match forces the developer to handle every variant (or explicitly `_ => Ok(true)` to pass). Each variant carries its own typed payload -- no field extraction, no type confusion.

The tradeoff: PaneEvent is not extensible without protocol version negotiation. New variants require a new protocol version. This is deliberate -- protocol evolution is a versioned, explicit process, not a silent addition of new `what` codes that old handlers silently drop.

---

## 4. The Handler Trait

For applications that need more structure than a closure, the `Handler` trait provides typed dispatch. This is the BHandler equivalent -- but instead of chain-of-responsibility on BMessage `what` codes, it's typed method dispatch on enum variants.

```rust
/// Handles events for a single pane.
///
/// Default implementations return `Ok(true)` (continue processing).
/// Override the methods you care about. This is the BHandler pattern:
/// each handler has self-contained logic for the events it understands.
///
/// Unlike BHandler, there is no chain-of-responsibility. Unhandled events
/// are not passed to a "next handler" -- they are handled by the default
/// implementation, which does nothing. Chaining was useful in BeOS for
/// scripting protocol delegation (ResolveSpecifier walked the handler chain).
/// In pane, scripting goes through the filesystem, so handler chaining
/// is not needed for its original purpose.
pub trait Handler {
    /// Called once before the message loop starts.
    /// Analogous to BApplication::ReadyToRun() / BWindow::Show().
    fn ready(&mut self, _pane: &Pane) -> Result<()> {
        Ok(())
    }

    /// The pane was resized.
    fn resized(&mut self, _pane: &Pane, _geometry: PaneGeometry) -> Result<bool> {
        Ok(true)
    }

    /// The pane gained focus.
    fn focused(&mut self, _pane: &Pane) -> Result<bool> {
        Ok(true)
    }

    /// The pane lost focus.
    fn blurred(&mut self, _pane: &Pane) -> Result<bool> {
        Ok(true)
    }

    /// A key event.
    fn key(&mut self, _pane: &Pane, _event: &KeyEvent) -> Result<bool> {
        Ok(true)
    }

    /// A mouse event.
    fn mouse(&mut self, _pane: &Pane, _event: &MouseEvent) -> Result<bool> {
        Ok(true)
    }

    /// A tag action was executed.
    fn tag_action(&mut self, _pane: &Pane, _action: &TagAction) -> Result<bool> {
        Ok(true)
    }

    /// A route result arrived.
    fn tag_route(&mut self, _pane: &Pane, _result: &RouteResult) -> Result<bool> {
        Ok(true)
    }

    /// The pane is being closed.
    fn close_requested(&mut self, _pane: &Pane) -> Result<bool> {
        Ok(false) // default: accept the close
    }

    /// The compositor connection was lost.
    fn disconnected(&mut self, _pane: &Pane) -> Result<bool> {
        Ok(true) // default: wait for reconnection
    }

    /// The compositor connection was restored.
    fn reconnected(&mut self, _pane: &Pane) -> Result<bool> {
        Ok(true)
    }

    /// A scripting query arrived.
    fn script_query(&mut self, _pane: &Pane, _query: &ScriptQuery) -> Result<bool> {
        Ok(true)
    }

    /// Catch-all for future event variants.
    /// Called only if no typed method matched (forward compatibility).
    fn unhandled(&mut self, _pane: &Pane, _event: &PaneEvent) -> Result<bool> {
        Ok(true)
    }
}
```

### Example: a handler with state

```rust
struct Editor {
    buffer: String,
    cursor: usize,
    dirty: bool,
}

impl Handler for Editor {
    fn ready(&mut self, pane: &Pane) -> pane_app::Result<()> {
        pane.set_content(self.buffer.as_bytes())?;
        Ok(())
    }

    fn key(&mut self, pane: &Pane, event: &KeyEvent) -> pane_app::Result<bool> {
        if event.state != KeyState::Press {
            return Ok(true);
        }
        match &event.key {
            Key::Char(c) => {
                self.buffer.insert(self.cursor, *c);
                self.cursor += c.len_utf8();
                self.dirty = true;
                pane.set_content(self.buffer.as_bytes())?;
            }
            Key::Named(NamedKey::Backspace) if self.cursor > 0 => {
                self.cursor -= 1;
                self.buffer.remove(self.cursor);
                self.dirty = true;
                pane.set_content(self.buffer.as_bytes())?;
            }
            _ => {}
        }
        Ok(true)
    }

    fn close_requested(&mut self, _pane: &Pane) -> pane_app::Result<bool> {
        if self.dirty {
            // In a real editor: prompt to save.
            // For now: discard and close.
        }
        Ok(false) // accept close
    }
}
```

### Why not chain-of-responsibility

BHandler's `SetNextHandler()` created a chain where unrecognized messages propagated from handler to handler. This was powerful for two things:

1. **Scripting delegation.** `ResolveSpecifier()` walked the handler chain to find which handler owned a given property. "the title of window 1" resolved through the window's handler chain until a handler claimed "title."

2. **Cross-cutting concerns.** BMessageFilter could intercept messages before they reached any handler. Filters were per-handler or per-looper (common filters).

In pane:

- Scripting delegation goes through the filesystem hierarchy (`/pane/<id>/attrs/title`), not through a handler chain. The pane-fs FUSE layer handles specifier resolution. This is strictly more powerful than BHandler chaining because it works across process boundaries and is accessible from any language.

- Cross-cutting concerns (logging, metrics, input preprocessing) are better served by filter functions registered on the looper. See section 5 (Filters).

The chain-of-responsibility pattern is not needed when you have exhaustive enum matching (the type system ensures completeness) and filesystem-based scripting (the query resolution doesn't traverse in-process handler chains).

---

## 5. Message Filters

BLooper had two levels of message filtering: per-handler filters and "common" looper-wide filters. BMessageFilter was a small subclassable object that could inspect, modify, or consume messages before they reached their handler. William Adams (Be Newsletter #2-36): "You didn't have to sub-class the BLooper or BHandler classes. You did have to sub-class BMessageFilter, but in a growing system, sub-classing a nice small object that is unlikely to change is probably easier than sub-classing a highly active object like BWindow or BApplication."

Pane preserves the concept but uses a trait instead of subclassing. The trait approach gives both the ergonomics of closures (implement only what you need) and the extensibility of named types.

```rust
/// A message filter that intercepts events before the handler sees them.
///
/// Filters run in registration order. A consumed event skips all
/// remaining filters and the handler.
pub trait Filter: Send + 'static {
    /// Process an event. Return Pass(event) to continue dispatch
    /// (possibly with a modified event), or Consume to swallow it.
    fn filter(&mut self, event: PaneEvent) -> FilterAction;

    /// Whether this filter is interested in this event type.
    /// Returns true by default (filter sees everything). Override
    /// to skip events your filter doesn't care about — a key-remap
    /// filter can return false for mouse events.
    fn wants(&self, _event: &PaneEvent) -> bool { true }
}

pub enum FilterAction {
    /// Pass the event through (possibly modified).
    Pass(PaneEvent),
    /// Consume the event — the handler never sees it.
    Consume,
}

impl Pane {
    /// Add a filter that runs before the handler sees events.
    pub fn add_filter(&mut self, filter: impl Filter) { .. }
}
```

The `wants()` method is a performance optimization. Most filters only care about one event type — a key remapper doesn't need to see mouse events, a rate limiter doesn't need to see focus changes. When `wants()` returns false, the filter is skipped for that event.

### Example: input logging filter

```rust
struct LogFilter;

impl Filter for LogFilter {
    fn filter(&mut self, event: PaneEvent) -> FilterAction {
        if let PaneEvent::Key(ref key) = event {
            tracing::debug!(?key, "input");
        }
        FilterAction::Pass(event)
    }
}

pane.add_filter(LogFilter);
```

### Example: key remapping filter

```rust
struct RemapFilter;

impl Filter for RemapFilter {
    fn wants(&self, event: &PaneEvent) -> bool {
        matches!(event, PaneEvent::Key(_))
    }

    fn filter(&mut self, mut event: PaneEvent) -> FilterAction {
        if let PaneEvent::Key(ref mut key) = event {
            // Remap Ctrl+H to Backspace
            if key.modifiers.contains(Modifiers::CTRL) && key.key == Key::Char('h') {
                key.key = Key::Named(NamedKey::Backspace);
                key.modifiers.remove(Modifiers::CTRL);
        }
    }
    FilterAction::Pass
}));
```

---

## 6. The Looper (Internal)

The looper is the per-pane message loop. It is not a public type -- developers interact with it through `Pane::run`, `Pane::run_with`, the `Handler` trait, and filters. But its design is the heart of the kit.

### How it works

When `Pane::run` or `Pane::run_with` is called, the kit creates a looper on the current thread. The looper:

1. Reads from a unified `LooperMessage` channel that carries both compositor messages (`FromComp(CompToClient)`) and self-delivered events (`Posted(PaneEvent)`). The dispatcher thread routes compositor messages to per-pane channels; worker threads and timers post directly via `PaneHandle::post_event()`.

2. Synthesizes `PaneEvent::Ready(geometry)` as the first event (before entering the loop).

3. Enters the message loop with drain-and-coalesce:
   ```
   loop {
       let msg = recv()?;                    // block on unified channel
       let batch = drain_and_coalesce(msg);  // drain remaining, coalesce Resize/MouseMove
       for event in batch {
           let event = apply_filters(event); // run filter chain (wants + filter)
           if event is consumed { continue }
           let keep_going = handler(event)?; // dispatch to handler
           if !keep_going { break }
       }
   }
   ```

4. On exit: if the handler chose to exit (not compositor-initiated close), sends `RequestClose` to the compositor. Decrements the pane count. If count reaches zero, signals the app's `wait()` condvar.

The coalescing rules: Resize events keep only the last geometry. MouseMove events keep only the last position. All other events are delivered in order. This is the BWindow::DispatchMessage optimization — intermediate resize geometries during a drag are meaningless if they haven't been drawn.

### Threading model

```
     App (main thread)
      |
      |-- create_pane() --> [compositor assigns PaneId]
      |
      |-- pane.run()  (blocks calling thread)
      |       looper reads from LooperMessage channel
      |       dispatches to handler/closure via PaneHandle
      |
      |   (or spawn a thread for each pane:)
      |   thread::spawn(move || pane.run(...))
      |
      |-- app.wait() blocks until pane_count == 0
```

Each pane thread is independent. A slow handler in pane A does not block pane B's event processing. This is the BeOS guarantee: "The idea behind the window thread is that there will always be a thread ready to react to a message from another window, or user input, or an app_server update message" (George Hoffman, Be Newsletter #2-36).

### What runs on the looper thread

- Event deserialization
- Filter evaluation
- Handler dispatch
- Title/vocabulary updates (`set_title`, `set_vocabulary` via PaneHandle)
- Content updates (`set_content` via PaneHandle)

### What must NOT run on the looper thread

- Network I/O
- File system operations that might block (use a worker thread)
- Computation lasting more than ~500ms
- Blocking waits on other panes

The same discipline as BeOS: the window thread must stay responsive. The kit does not enforce this at compile time (Rust cannot express "this closure runs in O(n) time"), but the documentation and examples reinforce it. Hoffman: "If a window thread becomes unresponsive, and the user continues to provide input... its message queue will fill up."

### Batching

Following BeOS's Interface Kit pattern, the looper batches outgoing messages. Content updates and attribute writes accumulate in a send buffer and are flushed in chunks. This amortizes the per-message overhead (serialization, syscall, compositor-side deserialization).

Synchronous operations (request-response patterns) force a flush before sending the request, exactly as BeOS's synchronous app_server calls did. Hoffman: "A synchronous call requires that this cache be flushed."

```rust
impl Pane {
    /// Force a flush of the send buffer.
    ///
    /// Normally the kit auto-flushes after each event handler invocation.
    /// Call this explicitly only when you need to ensure the compositor
    /// has received all pending updates (e.g., before a synchronous query).
    pub fn flush(&self) -> Result<()> { .. }
}
```

---

## 7. Connection Management

### Transparent reconnection

If the compositor restarts (crash, upgrade), the kit handles reconnection automatically. The developer sees a `Disconnected` event, then a `Reconnected` event after the connection is restored.

During the disconnection window:

- Outgoing messages queue in the kit's send buffer (bounded -- see backpressure below)
- The kit attempts reconnection on a backoff schedule (100ms, 200ms, 400ms, ... up to 5s)
- On reconnection, the kit re-runs the handshake and re-registers all panes
- Queued messages are flushed after reconnection
- A `Reconnected` event is delivered to each pane, followed by a `Resize` with current geometry

### Backpressure

The send buffer has a bounded capacity (configurable, default 1MB). If the buffer fills during disconnection, further writes return `Err(PaneError::BufferFull)`. The developer can handle this by:

- Dropping low-priority updates
- Blocking until space is available (not recommended on the looper thread)
- Coalescing updates (replace the last pending content update instead of appending)

```rust
/// Connection configuration.
#[derive(Debug, Clone)]
pub struct ConnectConfig {
    /// Maximum send buffer size during disconnection (bytes).
    /// Default: 1MB.
    pub send_buffer_limit: usize,
    /// Maximum reconnection attempts before giving up.
    /// Default: 30 (approximately 1 minute with backoff).
    pub max_reconnect_attempts: u32,
    /// Initial reconnection delay.
    /// Default: 100ms.
    pub initial_reconnect_delay: Duration,
}
```

### Why transparent reconnection matters

In BeOS, if app_server crashed, every application died. The client/server protocol did not have reconnection semantics. This was acceptable in BeOS because app_server was stable, but it is not acceptable for a modern system where the compositor might be restarted for an upgrade, or might crash due to a GPU driver bug.

The kit absorbs the reconnection complexity. The developer writes their handler as if the connection is permanent. The Disconnected/Reconnected events are informational -- the handler can show a brief indicator, but it doesn't need to manage the reconnection itself.

This is the same principle as BMessenger-based locking (Be Newsletter #3-33, Pavel Cisler): "a messenger-based lock has a more elaborate locking check and handles an aliasing issue like this completely." The kit provides identity-safe reconnection, not just socket reconnection -- after reconnect, pane IDs may change, and the kit remaps them transparently.

---

## 8. Routing Integration

### What routing is

Routing is how pane applications discover and dispatch to each other. When a user executes text in a tag line, or when an application wants to hand off content to another handler, routing rules determine what happens.

In BeOS, this was the MIME type system + BRoster + Translation Kit, working together:

- MIME types identified content kind
- BRoster knew which applications handled which types
- BTranslatorRoster mediated format conversion
- Quality ratings resolved ambiguity when multiple handlers matched

Pane unifies this under routing rules -- pattern-matched dispatch that subsumes file type association, command dispatch, and content transformation.

### Rule format

Routing rules are files. One file per rule, stored in well-known directories:

- `/etc/pane/route/rules/` -- system rules (installed by packages)
- `~/.config/pane/route/rules/` -- user rules (override system rules)

The kit watches these directories via pane-notify for live updates. Drop a file, gain a behavior.

Each rule file is TOML:

```toml
# /etc/pane/route/rules/open-image.toml
#
# Route image content to the image viewer.

[match]
# Content type pattern (glob).
content_type = "image/*"
# Optional: action name pattern.
action = "open"

[target]
# Application signature to receive the dispatch.
signature = "com.pane.image-viewer"
# Launch mode: "launch" (new instance), "deliver" (existing instance), "auto" (respect app's launch mode)
mode = "auto"

[quality]
# Self-declared quality rating (0.0 - 1.0).
# When multiple rules match, the highest quality wins.
# This is the Translation Kit pattern.
rating = 0.8
```

Rule fields:

| Field | Required | Description |
|---|---|---|
| `match.content_type` | yes | Glob pattern over MIME types |
| `match.action` | no | Action name pattern (default: any) |
| `match.text` | no | Regex over the routed text content |
| `target.signature` | yes | Application signature to dispatch to |
| `target.mode` | no | Launch behavior (default: "auto") |
| `quality.rating` | yes | 0.0-1.0, used for multi-match disambiguation |
| `transform.extract` | no | Regex with capture groups, applied to text before dispatch |

### Evaluation

When a route action is triggered (tag line execution, programmatic dispatch):

1. **Identify content type.** The kit examines the content being routed. For text from the tag line, the content type is `text/plain`. For file references, the MIME type from pane-store attributes. For explicit dispatch, the caller specifies the type.

2. **Match rules.** The kit evaluates all loaded rules against the content type and action. Rules are evaluated in parallel -- there is no ordering dependency between rules.

3. **Resolve ambiguity.** If multiple rules match, the highest `quality.rating` wins. If tied, user rules override system rules. If still tied, the kit presents a disambiguation UI (like BeOS's "Open With" menu -- a floating pane listing options).

4. **Transform.** If the winning rule has a `transform.extract` field, the regex is applied and captured groups replace the content before dispatch.

5. **Dispatch.** The kit queries pane-roster for the target signature and delivers the content. The delivery mechanism depends on `target.mode`:
   - `launch`: pane-roster launches a new instance with the content as an argument
   - `deliver`: pane-roster delivers a route message to the existing instance
   - `auto`: respects the target app's LaunchMode declaration

### Programmatic routing

```rust
impl Pane {
    /// Trigger a route action programmatically.
    ///
    /// Equivalent to the user executing text in the tag line.
    /// The kit evaluates routing rules, resolves the target, and dispatches.
    ///
    /// Returns the route result (which handler was selected, or an error
    /// if no rules matched).
    pub fn route(&self, content: &str, content_type: &str) -> Result<RouteResult> { .. }
}

/// The result of routing evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RouteResult {
    /// Successfully dispatched to a handler.
    Dispatched {
        target_signature: String,
        target_pane: Option<PaneId>,
    },
    /// No matching rules. The content was not routed.
    NoMatch,
    /// Multiple matches with equal quality. The disambiguation UI
    /// was shown (or the caller should handle this).
    Ambiguous(Vec<RouteCandidate>),
}

/// A candidate from route evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteCandidate {
    pub signature: String,
    pub quality: f32,
    pub rule_path: String,
}
```

### Registering as a route handler

Applications register their handled content types with pane-roster at startup. This is the supply side of routing -- the rules are the demand side.

```rust
impl App {
    /// Register content type handlers with pane-roster.
    ///
    /// This declares what content types this application can handle.
    /// Other applications' routing rules can target this signature.
    ///
    /// Analogous to BeOS's application file info (app_flags, supported types)
    /// stored as BFS attributes on the executable.
    pub fn register_types(&self, types: &[ContentHandler]) -> Result<()> { .. }
}

/// A content type this application handles.
#[derive(Debug, Clone)]
pub struct ContentHandler {
    /// MIME type pattern (e.g., "text/plain", "image/*").
    pub content_type: String,
    /// What action this handler performs (e.g., "open", "edit", "convert").
    pub action: String,
    /// Self-declared quality (0.0 - 1.0).
    pub quality: f32,
}
```

---

## 9. Scripting Integration

### The spirit of BeOS scripting

BeOS's scripting protocol was one of its most important features: every application was automatable through the same messaging system it used internally. `hey` from the command line could query or modify any running application's state. The protocol was based on property specifiers (like an address path), resolved through the handler chain via `ResolveSpecifier`.

Pane's scripting goes through the filesystem. The pane-fs FUSE mount at `/pane/` exposes every pane's state as files and directories. This is strictly more powerful than BeOS's scripting protocol:

- **Any language.** BeOS scripting required constructing BMessages. Pane scripting works with `cat`, `echo`, `jq`, or any tool that reads and writes files.
- **Cross-process.** No need for a BMessenger targeting a specific team and handler. The filesystem is the namespace.
- **Discoverable.** `ls /pane/3/attrs/` shows all queryable properties. No equivalent of `GetSupportedSuites` needed -- the filesystem *is* the suite listing.

### How the kit participates

When pane-fs receives a read or write on a pane's filesystem node, it sends a protocol message to the compositor, which routes it to the appropriate pane's looper. The looper delivers it as a `PaneEvent::ScriptQuery`.

```rust
/// A scripting query delivered via the filesystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptQuery {
    /// What property is being accessed.
    pub property: String,
    /// The operation.
    pub op: ScriptOp,
    /// A response channel. The handler must reply.
    pub reply_to: ScriptReplyToken,
}

/// Scripting operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScriptOp {
    /// Read the property value.
    Get,
    /// Set the property value.
    Set(Vec<u8>),
    /// Execute the property as a command.
    Execute(Vec<u8>),
}

/// Token for replying to a scripting query.
///
/// Must be used exactly once -- the filesystem read is blocking
/// on this reply. Dropping without replying sends an error.
#[must_use = "scripting queries must be answered"]
pub struct ScriptReplyToken { .. }

impl ScriptReplyToken {
    /// Reply with a value.
    pub fn reply(self, data: &[u8]) -> Result<()> { .. }

    /// Reply with an error.
    pub fn error(self, message: &str) -> Result<()> { .. }
}
```

### Declaring scriptable properties

Applications declare what properties they expose. This is the `GetSupportedSuites` equivalent -- but declarative rather than imperative.

```rust
impl Pane {
    /// Declare scriptable properties for this pane.
    ///
    /// These become visible under `/pane/<id>/attrs/` as files.
    /// When pane-fs receives a read/write on one of these files, the
    /// kit delivers it as a ScriptQuery event.
    ///
    /// Properties are declared once at pane creation time. The declaration
    /// is sent to the compositor, which informs pane-fs.
    pub fn declare_properties(&self, props: &[PropertyDecl]) -> Result<()> { .. }
}

/// A property declaration for scripting.
#[derive(Debug, Clone)]
pub struct PropertyDecl {
    /// Property name (becomes a filename under attrs/).
    pub name: String,
    /// Whether this property is readable.
    pub readable: bool,
    /// Whether this property is writable.
    pub writable: bool,
    /// Human-readable description.
    pub description: String,
}
```

### The `hey` equivalent

On pane, the `hey` equivalent is standard Unix tools:

```sh
# BeOS:  hey StyledEdit get Title of Window 0
# Pane:
cat /pane/3/attrs/title

# BeOS:  hey StyledEdit set Title of Window 0 to "New Title"
# Pane:
echo "New Title" > /pane/3/attrs/title

# BeOS:  hey StyledEdit count Window
# Pane:
ls /pane/ | wc -l

# BeOS:  hey StyledEdit getsuites of Window 0
# Pane:
ls /pane/3/attrs/
```

No special tool. No special protocol. Just files. This is Plan 9's gift to the Be model: the namespace *is* the scripting interface.

---

## 10. The Active Phase Protocol

### Wire types

The active phase uses typed enum messages over the same unix socket that completed the handshake. The session type's job ended at `End`; the active phase uses serde enums with exhaustive matching.

The canonical definitions are in `crates/pane-proto/src/protocol.rs`. Key variants:

**ClientToComp:** CreatePane, RequestClose, SetTitle, SetVocabulary, SetContent, CompletionResponse

**CompToClient:** PaneCreated, Resize, Focus, Blur, Key, Mouse, Close, CloseAck, CommandActivated, CommandDismissed, CommandExecuted, CompletionRequest

Every CompToClient variant carries a PaneId for demultiplexing. Input events (Key, Mouse) carry `timestamp: Option<u64>` in microseconds.

### Multiplexing

A single connection carries messages for all of an application's panes. Each message carries a `PaneId` field that identifies which pane it targets. The kit demultiplexes incoming messages to the appropriate pane's looper thread via per-pane channels.

```
    [unix socket]
         |
    Dispatcher thread (reads from socket, demuxes by PaneId)
         |
    +----+----+----+
    |    |    |    |
   P1   P2   P3   P4   (per-pane looper threads)
```

This matches the BeOS pattern: one connection (port pair) between the application and app_server, with messages tagged by ServerWindow token. The Haiku source shows this clearly -- BApplication has a single `fServerLink` (the PortLink to app_server), and messages carry window tokens for routing to the correct ServerWindow thread.

The dispatcher thread is internal to the kit. Developers do not interact with it.

---

## 11. Error Types

```rust
/// Top-level kit error.
#[derive(Debug)]
pub enum Error {
    /// Connection to compositor failed.
    Connect(ConnectError),
    /// Pane operation failed.
    Pane(PaneError),
    /// Routing evaluation failed.
    Route(RouteError),
    /// I/O error.
    Io(std::io::Error),
}

/// Errors from App::connect.
#[derive(Debug)]
pub enum ConnectError {
    /// The compositor socket was not found.
    NotRunning,
    /// The handshake was rejected.
    Rejected { reason: String },
    /// Protocol version mismatch.
    VersionMismatch { ours: u32, theirs: u32 },
    /// I/O error during connection.
    Io(std::io::Error),
}

/// Errors from pane operations.
#[derive(Debug)]
pub enum PaneError {
    /// The compositor connection was lost.
    Disconnected,
    /// The compositor refused the operation.
    Refused { reason: String },
    /// The send buffer is full (during disconnection).
    BufferFull,
    /// I/O error.
    Io(std::io::Error),
}

/// Errors from routing.
#[derive(Debug)]
pub enum RouteError {
    /// No rules matched.
    NoMatch,
    /// The target application is not running and could not be launched.
    TargetUnavailable { signature: String },
    /// The roster is unreachable.
    RosterDown,
}

/// Alias for Results using the kit error type.
pub type Result<T> = std::result::Result<T, Error>;
```

---

## 12. Thread Safety Summary

| Type | Send | Sync | Rationale |
|---|---|---|---|
| `App` | yes | no | Owns the connection. Can be moved but not shared. |
| `Pane` | yes | no | Consumed by `run()`/`run_with()`. |
| `PaneHandle` | yes | yes | Cloneable cross-thread handle. `post_event()` / `set_title()` are safe. |
| `Filter` | must be Send | -- | Moved into the looper thread. |
| `Handler` | must be Send | -- | Moved into the looper thread. |
| `TimerToken` | yes | yes | `cancel()` is atomic. |
| `ScriptReplyToken` | yes | no | Must be used on the looper thread, but can be moved. |

The threading model is identical to BeOS:
- Each pane's looper thread owns its data exclusively
- Cross-thread communication goes through message passing (PaneHandle::post_event, or the compositor protocol)
- Shared state between panes uses Arc<Mutex<T>> -- the developer's choice, not the kit's

Potrebic's commandments (Be Newsletter #1-4) still apply:
1. "Thou shalt not covet another thread's state or data without taking proper precautions" -- Arc<Mutex<T>> in Rust, enforced at compile time.
2. "Thou shalt not lock the same objects in differing orders" -- still the developer's responsibility, but session types on the protocol side prevent protocol-level deadlocks.

---

## 13. What the Kit Hides

The kit exists to hide complexity that the developer should not need to know about. Here is what is hidden and why:

| Hidden mechanism | What the developer sees instead |
|---|---|
| Session-typed handshake (Chan, Send, Recv, Branch) | `App::connect()` returns an App or an error |
| Transport unwrapping (finish -> into_stream -> SessionSource) | Handled inside the kit's connection setup |
| Calloop (compositor-side event loop) | Not visible to client developers at all |
| Message serialization (postcard, length-prefix framing) | Typed Rust enums and structs |
| Sub-session multiplexing (PaneId-tagged messages, dispatcher thread) | Each pane gets its own Handler callbacks |
| Reconnection protocol (re-handshake, pane re-registration) | Disconnected/Reconnected events |
| Routing rule file parsing and evaluation | `pane.route()` or automatic tag line dispatch |
| Roster communication | `App::connect` handles registration transparently |

This is the BApplication::_ConnectToServer principle. In Haiku's source, `_ConnectToServer` sends AS_CREATE_APP, creates port links, allocates shared memory areas, and negotiates with the registrar. The developer who writes `BApplication("application/x-vnd.My-App")` knows none of this. The kit is a library that converts a simple API into a complex protocol.

The pane-app kit does the same. The session types, the transport bridge, the calloop integration, the three-phase protocol -- these exist to provide guarantees. The kit exists to make those guarantees invisible.

---

## 14. Dependency Structure

```
pane-app
  |-- pane-proto (wire types, PaneEvent, ClientToComp/CompToClient)
  |-- pane-session (Chan, Transport, SessionSource -- used internally)
  |-- pane-notify (watches routing rule directories)
  |-- std (threads, channels, atomics)
```

pane-app depends on pane-session but does not re-export it. Developers writing pane-native applications never need to add pane-session to their Cargo.toml. The session types are an implementation detail of the kit, not part of its public API.

pane-proto is re-exported selectively — the event types and key/mouse types are part of the developer's API. The wire serialization functions are not. The old `TagLine`/`TagAction`/`TagCommand` types in pane-proto are superseded by the `Tag` builder and `Command`/`CommandVocabulary` types described in the revised tag line spec (`tagline-revised-spec.md`). pane-proto's tag module will be rewritten to carry the wire representation of the new types (`PaneTitle`, `CommandVocabulary`, `Command`, `Completion`).

---

## 15. Open Questions

### Content model

The `set_content(&[u8])` API is a placeholder. The actual content model depends on the pane type:

- **Text panes:** content is structured text (pane-text buffer, supporting structural regexps)
- **Widget panes:** content is a widget tree rendered by pane-ui
- **Raw panes:** content is a pixel buffer (wl_buffer)

The content API will specialize per pane type. The current byte-slice API is the minimum viable surface for Phase 3.

### Multi-pane shared state patterns

Applications with multiple panes need to share state (e.g., an editor with a file list pane and an editing pane). The kit provides no built-in mechanism beyond Arc<Mutex<T>> and PaneHandle::post(). Whether the kit should provide a higher-level coordination primitive (analogous to BApplication's looper serving as a central mailbox) is an open question. The current design says: use Rust's standard concurrency tools. If a common pattern emerges during early application development, the kit can add a coordination layer.

### Tag line editing ownership

The tag line is compositor-rendered but client-specified. When the user edits tag line text directly (typing into the tag area), who owns the editing state? Two options:

1. **Compositor owns editing.** Keystrokes in the tag area go to the compositor's tag editor. The compositor sends the final text to the client as a TagAction. Simple but limits client customization of tag behavior.

2. **Client owns editing.** Keystrokes in the tag area are forwarded to the client as Key events with a "tag focus" flag. The client processes them and sends updated tag content back. More flexible but requires round-trips for every keystroke.

The current design assumes option 1 (compositor owns tag editing) with a notification to the client when tag content changes. This matches acme's model, where Plan 9's rio handled tag rendering and editing, and the application responded to tag commands.

### Capability negotiation scope

The handshake's capability negotiation determines what protocol features are available during the active phase. The initial capability set is minimal:

- `CAP_TEXT_CONTENT` -- text-mode content updates
- `CAP_WIDGET_CONTENT` -- widget-mode content updates
- `CAP_DIRECT_SCANOUT` -- direct pixel buffer submission

Future capabilities extend this without changing the core protocol. The kit handles capability presence/absence internally -- developers check `app.has_capability(CAP_WIDGET_CONTENT)` rather than handling unknown message variants.

---

## Sources

- Haiku source: `headers/os/app/Application.h`, `headers/os/app/Looper.h`, `headers/os/app/Handler.h`
- Haiku source: `src/kits/app/Application.cpp` (\_InitData, \_ConnectToServer, Run)
- Haiku source: `src/kits/app/Looper.cpp` (task\_looper, Run, DispatchMessage)
- Be Newsletter #1-2: Schillings on API clarity and threading motivation
- Be Newsletter #1-4: Potrebic on threading commandments and synchronization
- Be Newsletter #2-36: Hoffman on window thread responsiveness and app_server call batching; Adams on BLooper/BMessage/BHandler/BMessageFilter
- Be Newsletter #3-33: Cisler on thread synchronization and BMessenger-based locking
- Be Newsletter #4-46: Raynaud-Richard on memory costs of BeOS objects
- Architecture spec sections 3, 4, 6, 7 (server decomposition, kit decomposition, threading, protocol design)
- pane-session crate source (`types.rs`, `calloop.rs`, `transport/`)
- pane-proto crate source (`message.rs`, `event.rs`, `tag.rs`)
- Ergonomics review (`review-pane-session-ergonomics.md`)
- Maty integration plan (`maty-integration-plan.md`)
- Final sweep recommendations (`final-sweep-recommendations.md`)
