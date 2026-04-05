# BeAPI Divergences

Each entry: what Be called it, what pane calls it, why.
Default policy: use Be name (snake_case). Deviations are exceptions.

Verified against `docs/architecture.md` as of 2026-04-05.
Entries marked (deferred) are planned but not yet in the spec.

---

## Core Types

| Be | pane | Rationale |
|----|------|-----------|
| `BApplication` | `App` | Widespread convention (gtk, winit). |
| `BWindow` | `Pane` | Architectural: pane is the universal object, not a window. |
| `BHandler` | `Handler` | Faithful (dropped B prefix). Lifecycle methods only. |
| `BMessenger` | `Messenger` | Faithful. Wraps Handle + ServiceRouter. |
| `BMessage` | `Message` trait | Faithful. Clone-safe value events only; obligations extracted to separate callbacks. |
| `BMessageFilter` | `MessageFilter<M>` | Faithful. Typed per-protocol, not erased. |
| `BLooper` | (looper, internal) | calloop event loop. Not a public type — the looper is the dispatch mechanism inside run_with. |
| `BMessageRunner` | `TimerToken` | Receipt from `set_pulse_rate()`. Cancel-on-drop matches BMessageRunner's cancel-on-destruct. |
| `property_info` | `PropertyInfo` | Faithful. Returned by `Handler::supported_properties()`. |
| `filter_result` | `FilterAction<M>` | `Pass`/`Transform(M)`/`Consume` (three-way, not two-way). |
| `BMenuBar`/`BMenuItem` | `Tag`/`CommandBuilder` | Architectural: command surface, not menu bar. Commands declared at pane creation via Tag. |

## Novel Types (no Be ancestor)

| pane | Role |
|------|------|
| `Protocol` | Typed service relationship: ServiceId + Message type. |
| `Handles<P>` | Per-protocol dispatch trait. Macro generates exhaustive match. |
| `Flow` | `Continue`/`Stop`. Handler's lifecycle decision. |
| `ServiceId` | UUID + reverse-DNS name. Deterministic UUID prevents collisions. |
| `ServiceHandle<P>` | Live connection to a service. Drop → RevokeInterest. |
| `PaneBuilder<H>` | Setup phase. Generic over H for Handles<P> bounds. Consumed by run_with. |
| `Dispatch<H>` | Per-request typed dispatch entries. Replaces ghost state in handler. |
| `CancelHandle` | Sender's handle for outstanding request. Drop = no-op, .cancel() = voluntary abort. |
| `ReplyPort` | Obligation handle for replies. Consumed by .reply(), Drop → ReplyFailed. |
| `CompletionReplyPort` | (in architecture.md obligation list, details deferred) |
| `ClipboardWriteLock` | Obligation handle. Consumed by .commit(), Drop → Revert. |
| `CreateFuture` | Obligation handle. Drop → cancel pending creation. |
| `AppPayload` | Marker trait (Clone + Send + 'static). Prevents smuggling obligation handles via post_app_message. |

## Handler Methods

| Be | pane | Rationale |
|----|------|-----------|
| `QuitRequested()` | `close_requested()` | Unified vocabulary with LifecycleMessage::CloseRequested. |
| `Pulse()` | `pulse()` | Faithful. |
| (none) | `ready()` | Novel: always the first event delivered. |
| (none) | `disconnected()` | Novel: primary connection lost. |
| (none) | `pane_exited(pane, reason)` | Novel: death notification for monitored panes. |
| (none) | `quit_requested() -> bool` | Novel: pre-close query. |
| (none) | `supported_properties()` | Novel: scripting property declaration. |
| (none) | `request_received(service, msg, reply)` | Novel: explicit request-reply hook with ReplyPort. |

Display-specific methods (FrameResized, WindowActivated, KeyDown,
MouseDown, etc.) are on `Handles<Display>`, not Handler. The
protocol_handler macro generates named methods from Display's
Message enum. Specific display message variants are deferred
pending Display protocol design.

## Messenger Methods

| Be | pane | Rationale |
|----|------|-----------|
| `PostMessage()` (app-defined) | `post_app_message<T: AppPayload>()` | Faithful. AppPayload bound excludes obligation handles at compile time. |
| `SendMessage(msg, handler, &reply)` (sync) | `send_and_wait()` | Distinct name — Rust has no overloading. |
| (none — async with typed callback) | `send_request()` | Novel: returns CancelHandle, reply routes to typed callback via Dispatch entry. |
| (none) | `set_content(data)` | Novel: set the pane's semantic content. |
| (none) | `set_pulse_rate(duration) -> TimerToken` | Faithful concept, returns cancel handle. |
| (none) | `set_pointer_policy(policy)` | Novel. |

Additional display/window methods (set_title, resize_to,
set_size_limits, show/hide) are deferred pending Display protocol
and compositor design.

## Filter Methods

| Be | pane | Rationale |
|----|------|-----------|
| Static criteria (message_delivery, message_source) | `matches(&self, msg) -> bool` | Runtime predicate replaces static enum criteria. |
| `B_DISPATCH_MESSAGE` / `B_SKIP_MESSAGE` | `FilterAction::Pass` / `Transform(M)` / `Consume` | Three-way. Transform is novel — modify in-flight. |

## Structural

| Be | pane | Rationale |
|----|------|-----------|
| `MessageReceived` switch | `Handles<P>` macro + exhaustive match | Compiler catches missing variants. Typed per-protocol. |
| `Lock()`/`Unlock()` | `&mut self` on Handler | Borrow checker replaces locking. See `beapi_internals` for full rationale. |
| Dynamic `BMessage` fields | Typed `Message` enum + filesystem scripting | Two roles separated. Protocol messages are typed; external scripting uses pane-fs. |
| Single handler with all methods | `Handler` (lifecycle) + `Handles<P>` per protocol | Display is an opt-in capability, not the default. All protocols use the same mechanism. |
| `application/x-vnd.*` strings | `ServiceId` (UUID + reverse-DNS) | Deterministic UUID survives renames. See `beapi_internals` gap 5 for roster comparison. |
| Monolithic message enum | Clone-safe `Message` + separate obligation handles | Forced by Serialize bound. Obligation handles contain LooperSender — not serializable. |
| reply_received/reply_failed on Handler | Dispatch entries with typed callbacks | No ghost state. No Box<dyn Any> downcast at handler surface. |
| `be_app` global | `App` is a held value | No globals. |
| Unbounded kernel port | Unbounded calloop channel | Be's kernel ports were unbounded; pane matches. |
| Deep view traversal for scripting | Declared properties only (PropertyInfo) | Stable scripting contracts at pane boundary. |
| Per-window thread (BWindow) | Single-threaded calloop per pane | Same responsiveness, no lock contention. See `beapi_internals` gap 2+7. |

## Deferred (not yet in architecture.md)

- Display-specific Messenger methods (set_title, resize_to, etc.)
- Death monitoring API (pane_exited hook exists; how to register interest is unspecified)
- Scripting handler (request_received exists; specifier chain and suite negotiation deferred)
- Dynamic filter management (add_filter/remove_filter API shape)
- Dynamic shortcut management (commands declared via Tag at creation; runtime mutation deferred)
