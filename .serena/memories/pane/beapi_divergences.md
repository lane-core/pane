# BeAPI Divergences

Each entry: what Be called it, what pane calls it, why.
Default policy: use Be name (snake_case). Deviations are exceptions.

## Type Names

| Be | pane | Rationale |
|----|------|-----------|
| `BApplication` | `App` | Widespread contemporary convention (gtk, winit). |
| `BWindow` | `Pane` | Architectural: pane is the universal object, not a window. |
| `BMessageFilter` | `MessageFilter` | Faithful (dropped B prefix) |
| `BMessage` | `Message` | Faithful. Clone-safe value events only; obligations extracted to internal types. |
| `BMessenger` | `Messenger` | Faithful. Wraps scoped Handle + ServiceRouter (routes by capability, not server). |
| `BHandler` | `Handler` | Faithful. Lifecycle + messaging only (~11 methods). |
| `BWindow` (display methods) | `DisplayHandler` | Separate trait — display is an opt-in capability, not the base case. |
| `BMenuBar`/`BMenuItem` | `Tag`/`CommandBuilder` | Architectural: command surface, not menu bar. |
| `BMessageRunner` | `TimerToken` (receipt from `send_periodic_fn`) | Rule 2: method on host, not standalone type. Cancel-on-drop matches BMessageRunner's cancel-on-destruct. |
| `property_info` | `PropertyInfo` | Faithful adaptation. Carries operations, specifier forms, value type. |
| `BHandler::ResolveSpecifier` + `GetSupportedSuites` | `ScriptableHandler` trait | Separate companion trait to Handler. Be had these on BHandler because every handler participated in the scripting chain; pane has one handler per pane. |
| (none) | `Protocol` | Novel: typed service relationship linking ServiceId + Message type. |
| (none) | `Handles<P>` | Novel: per-protocol dispatch trait. Derive macro generates dispatch. |
| (none) | `Flow` | Novel: `Continue`/`Stop` replaces `bool` return. Clearer than true=continue. |
| (none) | `ServiceId` | Novel: UUID + reverse-DNS name for service identity. Replaces string-based signatures. |
| (none) | `CompletionReplyPort` | Novel: typed ownership handle for completion responses. Consumed by `.reply()`, Drop sends empty list. |
| (none) | `ScriptReply` | Novel: newtype over ReplyPort for scripting response schema enforcement. |
| (none) | `CancelHandle` | Novel: handle for cancelling outstanding requests. Drop = no-op (request completes normally), `.cancel()` = voluntary abort. |
| (none) | `Dispatch<H>` | Novel: per-request typed dispatch entries for request/reply. Replaces ghost state in handler. |
| `filter_result` | `FilterAction` | More descriptive. `Pass`/`Transform`/`Consume` (three-way, not two-way). |
| `ReplyPort` | `ReplyPort` | Novel: Be had no explicit reply handle (reply was via `BMessage::SendReply`). |
| `PaneId`, `PaneGeometry`, `PaneTitle` | `Id`, `Geometry`, `Title` | Crate path is the namespace — `pane_proto::Id`, not `PaneId`. |

## Handler Methods

| Be | pane | Rationale |
|----|------|-----------|
| `QuitRequested()` | `close_requested()` | Unified vocabulary with Message::CloseRequested. |
| `Pulse()` | `pulse()` | Faithful |
| `FrameResized()` | `resized()` | On DisplayHandler. Wayland has no position; deferred. |
| `WindowActivated(bool)` | `activated()`/`deactivated()` | On DisplayHandler. Split — better Rust API. |
| `KeyDown()`/`KeyUp()` | `key(event)` | On DisplayHandler. Collapsed — Rust tagged unions. |
| `MouseDown()`/`MouseUp()`/`MouseMoved()` | `mouse(event)` | On DisplayHandler. Collapsed — same reason. |
| (none — implicit via `IsSourceWaiting`) | `request_received()` | On Handler. Explicit request-reply hook with `ReplyPort`. |
| `AddCommonFilter()` | `add_filter()` | One filter level only. |
| `AddShortcut()` | `add_shortcut()` | Faithful |

## Messenger Methods

| Be | pane | Rationale |
|----|------|-----------|
| `SendMessage()` | `send_message()` | Faithful |
| `SendMessage(msg, &reply)` (sync) | `send_and_wait()` | Tier 3: Rust has no overloading. Name describes what happens to the caller's thread. |
| (none — async with typed callback) | `send_request()` | Novel: returns `CancelHandle`, reply routes to typed callback via Dispatch entry. No ghost state. |
| `PostMessage()` (app-defined) | `post_app_message()` | Faithful. `T: AppPayload` (Clone + Send + 'static) excludes obligation handles at compile time. |
| `SetTitle()` | `set_title()` | Faithful |
| `SetPulseRate()` | `set_pulse_rate()` | Faithful (on Messenger due to Rust ownership) |
| `ResizeTo()` | `resize_to()` | Faithful |
| `SetSizeLimits()` | `set_size_limits()` | Faithful |
| `Hide()`/`Show()` | `set_hidden(bool)` | Single method — Rust-idiomatic. |

## Filter Methods

| Be | pane | Rationale |
|----|------|-----------|
| Static criteria (`message_delivery`, `message_source`) | `matches(&self, event)` | Novel: runtime predicate replacing static enum criteria. Named `matches` (standard predicate vocabulary). |

## Builder Patterns (Tier 3 Exception)

Builder methods use bare names per Rust convention, not `set_`/`add_` prefixes:
`Tag::new("Title").short("T").command(cmd(...))`. Be didn't have builders (used multi-param constructors). Haiku's `BLayoutBuilder` kept `Set`/`Add` but that was C++. Documented as a tier 3 exception in naming conventions.

## Structural

| Be | pane | Rationale |
|----|------|-----------|
| `MessageReceived` switch | `Handles<P>` derive macro + exhaustive match | Compiler catches missing variants. Typed per-protocol dispatch. |
| `Lock()`/`Unlock()` | `&mut self` on Handler | Borrow checker replaces locking. |
| Dynamic `BMessage` fields | Typed `Message` enum + filesystem scripting | Two roles separated. |
| Deep view traversal | Pane boundary principle — declared Attributes only | Stable scripting contracts. |
| Unbounded kernel port | Unbounded calloop channel | Be's kernel ports were unbounded; pane matches. |
| `be_app` global | `App` is a held value | No globals. |
| No crash monitoring | `Messenger::monitor()` + `Message::PaneExited` | Erlang-style. |
| C++ overloading for SendMessage | Distinct method names (`send_message`, `send_and_wait`, `send_request`) | Tier 3: Rust doesn't overload. |
| Single handler with all methods | Handler (lifecycle) + DisplayHandler (display) + `Handles<P>` (services) | Display is a capability, not the default. Services are per-protocol. |
| `application/x-vnd.*` strings | `ServiceId` (UUID + reverse-DNS) | Deterministic UUID survives renames; reverse-DNS prevents collisions. |
| Monolithic message enum | Clone-safe `Message` + internal obligation types | EAct KP2: no channel endpoints in message values. |
| `reply_received` / `reply_failed` on Handler | Dispatch entries with typed callbacks | No ghost state. No `Box<dyn Any>` downcast at handler surface. |
