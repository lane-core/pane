# BeAPI Divergences

Each entry: what Be called it, what pane calls it, why.
Default policy: use Be name (snake_case). Deviations are exceptions.

## Type Names

| Be | pane | Rationale |
|----|------|-----------|
| `BApplication` | `App` | Widespread contemporary convention (gtk, winit). |
| `BWindow` | `Pane` | Architectural: pane is the universal object, not a window. |
| `BMessageFilter` | `MessageFilter` | Faithful (dropped B prefix) |
| `BMessage` | `Message` | Faithful |
| `BMessenger` | `Messenger` | Faithful |
| `BHandler` | `Handler` | Faithful |
| `BMenuBar`/`BMenuItem` | `Tag`/`CommandBuilder` | Architectural: command surface, not menu bar. |
| `BMessageRunner` | `TimerToken` (receipt from `send_periodic_fn`) | Rule 2: method on host, not standalone type. Cancel-on-drop matches BMessageRunner's cancel-on-destruct. |
| `property_info` | `PropertyInfo` | Faithful adaptation of Be's property_info tables. Carries operations, specifier forms, value type. Replaced earlier `Attribute` stub. |
| `BHandler::ResolveSpecifier` + `GetSupportedSuites` | `ScriptableHandler` trait | Separate companion trait to Handler (not supertrait). Be had these on BHandler because every handler participated in the scripting chain; pane has one handler per pane, chain walk is inter-process. |
| (none) | `CompletionReplyPort` | Novel: typed ownership handle for completion responses. Consumed by `.reply()`, Drop sends empty list. No Be equivalent (Be's completion was synchronous within app_server). |
| (none) | `ScriptReply` | Novel: newtype over ReplyPort for scripting response schema enforcement. |
| `filter_result` | `FilterAction` | More descriptive enum name. |
| `ReplyPort` | `ReplyPort` | Novel: Be had no explicit reply handle (reply was via `BMessage::SendReply`). |

## Handler Methods

| Be | pane | Rationale |
|----|------|-----------|
| `QuitRequested()` | `close_requested()` | Unified vocabulary with Message::CloseRequested. |
| `Pulse()` | `pulse()` | Faithful |
| `FrameResized()` | `resized()` | Wayland has no position; deferred. |
| `WindowActivated(bool)` | `activated()`/`deactivated()` | Split — better Rust API. |
| `KeyDown()`/`KeyUp()` | `key(event)` | Collapsed — Rust tagged unions. |
| `MouseDown()`/`MouseUp()`/`MouseMoved()` | `mouse(event)` | Collapsed — same reason. |
| `MessageReceived()` | `fallback()` | Different role in typed-dispatch model. |
| `MessageReceived()` (app-defined) | `app_message()` | Narrower scope: only app-defined payloads, not universal catch-all. Variant is `Message::AppMessage`. |
| (none — implicit via `IsSourceWaiting`) | `request_received()` | Novel: explicit request-reply hook with `ReplyPort`. |
| `AddCommonFilter()` | `add_filter()` | One filter level only. |
| `AddShortcut()` | `add_shortcut()` | Faithful |

## Messenger Methods

| Be | pane | Rationale |
|----|------|-----------|
| `SendMessage()` | `send_message()` | Faithful |
| `SendMessage(msg, &reply)` (sync) | `send_and_wait()` | Tier 3: Rust has no overloading. Name describes what happens to the caller's thread. |
| (none — async with reply handler) | `send_request()` | Novel: returns token, reply arrives as `Message::Reply`. |
| `PostMessage()` (app-defined) | `post_app_message()` | Faithful to Be's `PostMessage`. |
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
| `MessageReceived` switch | Typed Handler methods + exhaustive match | Compiler catches missing variants. |
| `Lock()`/`Unlock()` | `&mut self` on Handler | Borrow checker replaces locking. |
| Dynamic `BMessage` fields | Typed `Message` enum + filesystem scripting | Two roles separated. |
| Deep view traversal | Pane boundary principle — declared Attributes only | Stable scripting contracts. |
| Unbounded kernel port | Unbounded calloop channel | Be's kernel ports were unbounded; pane briefly used bounded sync_channel(256) for backpressure but reverted to unbounded with the calloop migration — real backpressure happens at the compositor level, not the looper channel. |
| `be_app` global | `App` is a held value | No globals. |
| No crash monitoring | `Messenger::monitor()` + `Message::PaneExited` | Erlang-style. |
| C++ overloading for SendMessage | Distinct method names (`send_message`, `send_and_wait`, `send_request`) | Tier 3: Rust doesn't overload. |
