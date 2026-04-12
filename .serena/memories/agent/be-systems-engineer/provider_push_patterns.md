---
type: reference
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [pub_sub, provider_push, WatchingService, BMessenger, ServerWindow, notification, fan_out]
sources: [haiku/src/servers/registrar/WatchingService.cpp, haiku/src/servers/registrar/Watcher.cpp, haiku/src/servers/registrar/TRoster.cpp, haiku/src/servers/app/ServerWindow.cpp, haiku/src/servers/app/ServerWindow.h, haiku/src/kits/app/Handler.cpp, haiku/src/kits/app/Roster.cpp, haiku/src/servers/registrar/MessageDeliverer.cpp]
verified_against: [haiku source 2026-04-11]
related: [decision/messenger_addressing, reference/haiku/internals, reference/haiku/appserver_concurrency]
agents: [be-systems-engineer]
---

# BeOS/Haiku provider-push patterns — how servers sent messages to clients

Three distinct mechanisms verified in Haiku source, each for a different
relationship type.

## 1. WatchingService (registrar → watchers)

The generic server→client notification fanout pattern.

**Architecture:**
- `WatchingService` is a mixin class that any server-side component can embed.
- Stores `map<BMessenger, Watcher*> fWatchers` — keyed by BMessenger (client target).
- `Watcher` wraps a `BMessenger fTarget` and has virtual `SendMessage()`.
- `EventMaskWatcher` extends Watcher with a uint32 event mask (bitfield of event categories).
- `WatcherFilter` is a predicate: `Filter(Watcher*, BMessage*)` decides whether a watcher
  gets a specific notification.
- `EventMaskWatcherFilter` filters by event mask bitwise AND.

**Registration flow (BRoster::StartWatching):**
1. Client calls `BRoster::StartWatching(BMessenger target, uint32 eventMask)`
2. Client sends `B_REG_START_WATCHING` message to registrar with messenger + mask
3. Registrar's `TRoster::HandleStartWatching` creates `EventMaskWatcher(target, events)`
4. Adds to `fWatchingService` (replaces any existing watcher for same BMessenger)

**Notification flow:**
1. Internal event fires (e.g., `_AppAdded(info)`)
2. TRoster calls `fWatchingService.NotifyWatchers(&message, &filter)`
3. NotifyWatchers iterates `fWatchers`, applying filter, calling `watcher->SendMessage()`
4. Watcher::SendMessage uses `MessageDeliverer::Default()->DeliverMessage(message, fTarget)`
5. MessageDeliverer is a singleton with retry logic, per-port queuing, background delivery thread

**Stale watcher cleanup (critical):**
- NotifyWatchers checks `watcher->Target().IsValid()` after failed send
- Invalid targets collected into `staleWatchers` list
- Removed after iteration (not during — avoids iterator invalidation)
- Known bug in source: "TODO: If a watcher is invalid, but the filter never selects it,
  it will not be removed" (WatchingService.cpp:213)

**Key insight:** The server stores BMessengers (addressing capability) for each
registered watcher. It does NOT store per-watcher connection state or session handles.
BMessenger was a lightweight value type (port_id + handler_token + team_id) that could
address any BHandler across the system. The watcher pattern is "give me your address, I'll
push messages to it."

## 2. BHandler StartWatching/SendNotices (peer-to-peer observer)

Local in-process or in-looper observer pattern.

- `BHandler` maintains `ObserverList* fObserverList` (lazy-allocated)
- `ObserverList` has two maps:
  - `map<uint32, vector<const BHandler*>> fHandlerMap` (same-looper, raw pointer)
  - `map<uint32, vector<BMessenger>> fMessengerMap` (cross-looper, via messenger)
- `StartWatching(BMessenger target, uint32 what)` sends a message to the observed
  handler, which adds the BMessenger to its observer list
- `SendNotices(uint32 what, BMessage* notice)` iterates all matching observers,
  sends B_OBSERVER_NOTICE_CHANGE via BMessenger
- Stale cleanup: `_SendNotices` erases invalid BMessengers during iteration

**Key insight:** The BMessenger form works cross-process. The BHandler* form only works
within the same BLooper. Both end up using BMessenger for actual delivery (the BHandler*
entries are validated into BMessengers before sending).

## 3. ServerWindow (app_server → client)

NOT BMessenger-based. Uses the direct port link protocol.

- ServerWindow stores `fClientLooperPort` (port_id of client's BWindow looper)
- ServerWindow stores `fClientToken` (handler token within client looper)
- `SendMessageToClient(BMessage* msg, int32 target)`:
  Uses `BMessage::Private::SendMessage(fClientLooperPort, fClientTeam, target, ...)`
  — direct kernel port write, bypassing BMessenger overhead
- `fLink` (BPrivate::PortLink) used for the binary Link protocol (StartMessage/Attach/Flush)
  — the high-performance path for draw commands and sync operations
- ServerWindow stores `fFocusMessenger` and `fHandlerMessenger` — BMessenger references
  back to the client, used for focus/handler targeting

**Two channels to the client:**
1. BMessage channel via `SendMessageToClient` — for high-level events (B_QUIT_REQUESTED,
   B_MINIMIZE, B_ZOOM, B_SCREEN_CHANGED, font/decorator changes)
2. Link protocol via `fLink` — for low-level draw responses (AS_SET_LOOK reply,
   AS_IS_FRONT_WINDOW reply, etc.)

**Key insight:** app_server obtained the client port during window creation handshake.
ServerWindow constructor takes `port_id clientPort, port_id looperPort` as parameters.
These were passed during the AS_CREATE_WINDOW protocol exchange. The server didn't need
to "discover" the client — it received addressing info as part of the connection setup.

## Summary for pane translation

All three patterns share a common structure:
1. **Client provides addressing info to server during registration**
2. **Server stores that info** (BMessenger, port_id, etc.)
3. **Server pushes messages using stored address**
4. **Server detects and cleans up dead clients**

The difference is the addressing mechanism (BMessenger for app-level, port_id for perf-critical),
the registration trigger (explicit StartWatching, window creation handshake), and
the cleanup strategy (validity check, process death notification).

For pane: the provider needs a way to address each subscriber individually after
InterestAccepted. The session_id is already a per-subscriber identifier. The question
is whether the provider gets a write channel (ServiceHandle-equivalent) per subscriber,
or whether the framework provides a fan-out primitive.
