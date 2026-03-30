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
| `BMessageRunner` | `TimerToken` (receipt from `send_periodic`) | Rule 2: method on host, not standalone type. |
| `property_info` | `Attribute` | Modernized name; aligns with attrs/, AttrValue, BFS xattrs. |
| `filter_result` | `FilterAction` | More descriptive enum name. |

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
| `AddCommonFilter()` | `add_filter()` | One filter level only. |
| `AddShortcut()` | `add_shortcut()` | Faithful |

## Messenger Methods

| Be | pane | Rationale |
|----|------|-----------|
| `SendMessage()` | `send_message()` | Faithful |
| `SetTitle()` | `set_title()` | Faithful |
| `SetPulseRate()` | `set_pulse_rate()` | Faithful (on Messenger due to Rust ownership) |
| `ResizeTo()` | `resize_to()` | Faithful |
| `SetSizeLimits()` | `set_size_limits()` | Faithful |
| `Hide()`/`Show()` | `set_hidden(bool)` | Single method — Rust-idiomatic. |

## Structural

| Be | pane | Rationale |
|----|------|-----------|
| `MessageReceived` switch | Typed Handler methods + exhaustive match | Compiler catches missing variants. |
| `Lock()`/`Unlock()` | `&mut self` on Handler | Borrow checker replaces locking. |
| Dynamic `BMessage` fields | Typed `Message` enum + filesystem scripting | Two roles separated. |
| Deep view traversal | Pane boundary principle — declared Attributes only | Stable scripting contracts. |
| Unbounded kernel port | Bounded sync_channel(256) | Backpressure. |
| `be_app` global | `App` is a held value | No globals. |
| No crash monitoring | `Messenger::monitor()` + `Message::PaneExited` | Erlang-style. |
