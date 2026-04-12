---
type: reference
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [haiku, test_suite, port_candidates, app_kit, blooper, bmessenger, bhandler, bmessage, concurrency]
related: [reference/haiku/_hub, reference/haiku/beapi_divergences, architecture/app, architecture/looper]
agents: [be-systems-engineer, pane-architect]
sources: [haiku/src/tests/kits/app/]
verified_against: [~/src/haiku/src/tests/kits/app/ 2026-04-11]
---

# Haiku Application Kit Test Suite Audit

Full audit of ~/src/haiku/src/tests/kits/app/ for pane port
candidates. Every test cited with file:line. Priority: HIGH
(directly tests something pane has), MEDIUM (pattern pane uses
differently), LOW (Be-specific behavior pane doesn't have).

## 1. Test Inventory

### BLooper Tests (blooper/)

**RunTest.cpp**
- RunTest1 (line 26): Run() called twice → debugger msg. MEDIUM — pane uses run_with consuming builder, double-run is impossible by construction.
- RunTest2 (line 42): Run() returns thread_id matching Thread(). LOW — pane looper is not a named thread.

**QuitTest.cpp**
- QuitTest1 (line 28): Quit() while unlocked → error msg. MEDIUM — pane uses Flow::Stop, no lock.
- QuitTest2 (line 37): Send B_QUIT_REQUESTED via BMessenger → orderly quit. HIGH — maps to LifecycleMessage::CloseRequested.

**AddHandlerTest.cpp**
- AddHandlerTest1 (line 36): AddHandler(NULL) → no crash, count==1. LOW — pane has no dynamic handler add.
- AddHandlerTest2 (line 52): AddHandler while unlocked → debugger. LOW — no lock model in pane.

**RemoveHandlerTest.cpp**
- RemoveHandler1-5 (lines 30-101): NULL, wrong looper, unlocked, with filters. LOW — no dynamic handler removal in pane.

**AddCommonFilterTest.cpp**
- AddCommonFilterTest1-4 (lines 29-90): NULL filter, unlocked, valid, double-ownership. MEDIUM — pane has MessageFilter<M> but different ownership model.

**IsMessageWaitingTest.cpp**
- IsMessageWaiting1-5 (lines 31-167): Queue empty/filled, locked/unlocked, port vs queue. MEDIUM — pane equivalent would be checking calloop channel.

**LooperForThreadTest.cpp**
- LooperForThreadTest1-2 (lines 25-41): Valid/invalid thread lookup. LOW — no global looper registry in pane.

**SetCommonFilterListTest.cpp**
- Tests 1-4 (lines 29-98): NULL, unlocked, valid, double-ownership. MEDIUM — filter ownership semantics.

**LooperSizeTest.cpp, CountHandlersTest.cpp, HandlerAtTest.cpp, IndexOfTest.cpp**: LOW — internal collection management pane doesn't have.

**PerformTest.cpp**: LOW — BPerformable protocol not in pane.

### BHandler Tests (bhandler/)

**IsWatchedTest.cpp**
- IsWatched1 (line 25): No watchers → false. HIGH — maps to pane Watch/Unwatch/PaneExited.
- IsWatched2 (line 37): Add then remove watcher → true then false. HIGH — watch table lifecycle.
- IsWatched3 (line 52): Add, send notice, remove → correct. HIGH — notice delivery during watch.
- IsWatched4 (line 69): Remove non-existent watcher → false. HIGH — idempotent unwatch.
- IsWatched5 (line 82): SendNotices without watchers → no crash. HIGH — empty watch table.

**AddFilterTest.cpp**
- AddFilter1-4 (lines 31-87): NULL, no looper, unlocked looper, locked looper. MEDIUM — pane has filters but different lifecycle.

**LockLooperTest.cpp**
- LockLooper1-4 (lines 27-88): No looper, unlocked, locked, cross-thread. LOW — no lock model in pane.

**HandlerLooperTest.cpp, NextHandlerTest.cpp, SetNextHandlerTest.cpp**: LOW — handler chain not in pane.

**RemoveFilterTest.cpp, SetFilterListTest.cpp**: MEDIUM — filter management lifecycle.

### BMessenger Tests (bmessenger/)

**SendMessageTester.cpp**
- TestUninitialized (line 152): Send to uninitialized → B_BAD_PORT_ID. HIGH — maps to Messenger validity.
- TestInitialized (line 214): All 5 SendMessage overloads with local/remote targets, timeouts, delivery/reply checking. HIGH — core messaging.
- SendMessageTest1 (line 382): Full matrix of target kinds × send flavors. HIGH — comprehensive messaging.

**TargetTester.cpp**
- IsTargetLocalTest1-5 (lines 36-101): Uninitialized, local preferred, local specific, remote preferred, remote specific. MEDIUM — pane doesn't distinguish local/remote at Messenger level.
- TargetTest1-5 (lines 107-177): Target() with various messenger states. MEDIUM — pane uses Address not handler pointers.

**MessengerAssignmentTester.cpp**
- AssignmentTest1-3 (lines 67-153): Copy uninitialized, local preferred, local specific. MEDIUM — pane Messenger is Clone, assignment is trivial.

**MessengerComparissonTester.cpp**: MEDIUM — equality semantics.

**LockTargetTester.cpp, LockTargetWithTimeoutTester.cpp**: LOW — lock model.

**ForwardMessageTest.cpp**: HIGH — message forwarding through chain of loopers with reply.

### BMessage Tests (bmessage/)

**MessageConstructTest.cpp**
- Test1-3 (lines 28-63): Default, what-init, copy. LOW — pane Message is a trait, not a struct.

**MessageSpeedTest.cpp**
- Create/Lookup/Read/Flatten/Unflatten at 5/50/500/5000 scale. MEDIUM — benchmarking pattern is valuable for pane-proto CBOR serialization.

**MessageDestructTest.cpp, MessageOpAssignTest.cpp, MessageEasyFindTest.cpp**: LOW — BMessage field API not in pane.

**MessageInt32ItemTest.h and similar type-specific tests**: LOW — dynamic field API.

### BApplication Tests (bapplication/)

**AppRunTester.cpp**
- RunTest1-22 (lines 53-762): Exhaustive launch mode tests (MULTIPLE_LAUNCH, SINGLE_LAUNCH, EXCLUSIVE_LAUNCH, ARGV_ONLY). Lifecycle ordering: ArgvReceived → ReadyToRun → QuitRequested. HIGH for lifecycle ordering, LOW for launch modes.

**AppQuitRequestedTester.cpp**
- QuitRequestedTest1 (line 52): Return false first time, true second → doesn't quit then quits. HIGH — close_requested() returning bool.

**AppQuitTester.cpp**
- QuitTest1-4 (lines 52-149): Not running, from looper thread, from other thread, unlocked. HIGH for quit-from-other-thread; MEDIUM for others.

### BMessageQueue Tests (bmessagequeue/)

**ConcurrencyTest1.cpp**
- Three-thread concurrent add/remove/next with count verification. HIGH — concurrent channel access pattern.

**ConcurrencyTest2.cpp**
- Five-thread test: lock holder + NextMessage/RemoveMessage/AddMessage/Lock all blocking. HIGH — demonstrates queue behavior under contention.

### BMessageRunner Tests (bmessagerunner/)

**SetIntervalTester.cpp**
- SetInterval1-7 (lines 65-309): Uninitialized, exhausted, mid-delivery interval change, zero interval, negative interval. HIGH — maps to set_pulse_rate + TimerToken.

**SetCountTester.cpp, GetInfoTester.cpp**: MEDIUM — count-based delivery not in pane.

### BPropertyInfo Tests (bpropertyinfo/)

**PropertyConstructionTest.cpp**: MEDIUM — PropertyInfo construction and comparison.
**PropertyFindMatchTest.cpp**: HIGH — FindMatch with command/specifier/property matching, wildcard handling.
**PropertyFlattenTest.cpp**: MEDIUM — serialization.

### Messaging Tests (messaging/)

**HandlerLooperMessageTest.cpp**: HIGH — targeted message delivery to specific handler in looper with multiple handlers.
**PortLinkTest.cpp**: HIGH — low-level port buffering, multi-message flush, oversized message rejection, WouldBlock check.

### Common Infrastructure (common/)

**AppRunner.cpp**: Test harness for launching separate BApplication processes and capturing output. Uses kernel ports for IPC. Pattern: launch → capture output → compare to expected strings.
**PipedAppRunner.cpp**: Variant using pipe-based output capture.
**CommonTestApp.cpp**: Base class for test applications.

## 2. Test Design Patterns

### Message ordering
Haiku tests ordering primarily through expected-output string comparison in AppRunTester (ReadyToRun before QuitRequested, ArgvReceived before ReadyToRun). BMessageRunner tests verify temporal ordering with timestamps. The messaging test HandlerLooperMessageTest verifies targeted delivery (message to specific handler vs looper's preferred handler).

### Handler lifecycle
AppRunTester verifies the full lifecycle: construction → Run() → ReadyToRun() → (dispatch loop) → QuitRequested() → Run() returns → destruction. AppQuitRequestedTester specifically tests the QuitRequested return value controlling quit behavior.

### Port/channel backpressure
SendMessageTester tests B_WOULD_BLOCK and B_TIMED_OUT returns when the target port is full. SMLooper deliberately blocks (port capacity is 1, posts MSG_BLOCK then snoozes) to create backpressure. PortLinkTest tests oversized message rejection and WouldBlock on empty port read.

### Death notification
IsWatchedTest tests the StartWatching/StopWatching/SendNotices lifecycle. Tests cover: no watchers, add/remove, send notice during watch, remove non-existent, notices to empty table.

### Concurrent access
ConcurrencyTest1: 3 threads (adder, remover-via-NextMessage, adder-with-periodic-RemoveMessage) + final count verification. ConcurrencyTest2: 5 threads testing all queue operations blocking on a held lock, then verifying mutual exclusion when released.

### Helper infrastructure
- **SMLooper/SMHandler**: BLooper subclass with configurable blocking (BlockUntil snoozes), reply delay, delivery/reply success tracking with timestamps.
- **SMTarget hierarchy**: Abstract target → LocalSMTarget (creates looper + optional handler) → RemoteSMTarget (launches separate process, communicates via kernel ports).
- **AppRunner**: Launches test apps as separate processes, captures output via kernel port reader thread, compares against expected output strings.
- **BThreadedTestCaller**: Multi-thread test runner — assigns named threads to member functions, coordinates execution.
- **testMessageClass**: BMessage subclass with static destructor counter for leak detection.
- **DEBUGGER_ESCAPE macro**: Catches debugger calls in test context (for tests that trigger intentional debugger messages).

## 3. Mapping Table (HIGH priority)

| Haiku test | What it tests | pane equivalent | pane file | Difficulty |
|---|---|---|---|---|
| IsWatched1-5 | Watch/Unwatch lifecycle | Watch/Unwatch/PaneExited on ProtocolServer | pane-session/src/server.rs | Easy |
| QuitTest2 | CloseRequested via messenger | LifecycleMessage::CloseRequested | pane-app/src/looper.rs | Easy |
| AppQuitRequestedTest1 | QuitRequested return value controls quit | close_requested() -> bool | pane-app/src/looper.rs | Easy |
| AppRunTest1-4 lifecycle | ReadyToRun/QuitRequested ordering | ready()/close_requested() ordering | pane-app/tests/ | Easy |
| SendMessageTest1 uninitialized | Send to invalid target | send_request to invalid ServiceHandle | pane-app/src/service_handle.rs | Medium |
| SendMessageTest1 timeout/backpressure | B_WOULD_BLOCK, B_TIMED_OUT | try_send_request → Backpressure | pane-app/src/service_handle.rs | Medium |
| SetInterval3-5 | Timer reset, interval change | set_pulse_rate / TimerToken | pane-app/src/timer.rs | Easy |
| ConcurrencyTest1 | 3-thread add/remove/count | Channel stress (calloop + mpsc) | pane-app/tests/stress.rs | Medium |
| ConcurrencyTest2 | 5-thread blocking queue ops | Backpressure stress | pane-app/tests/pub_sub_stress.rs | Medium |
| PropertyFindMatch | Specifier/command matching | PropertyInfo::find_match | pane-app (future) | Hard |
| ForwardMessageTest | Message forwarding chain | Inter-pane message forwarding | pane-app/tests/ | Medium |
| HandlerLooperMessage | Targeted delivery to handler | Service dispatch routing | pane-app/src/service_dispatch.rs | Easy |
| PortLinkTest | Buffer overflow, WouldBlock | Frame overflow, backpressure | pane-session/src/frame_writer.rs | Medium |

## 4. Tests Exposing Design Differences

### Lock model → Borrow checker
Haiku: LockLooper1-4, AddHandlerTest2, RemoveHandler3, AddCommonFilterTest2, SetCommonFilterListTest2 all test lock semantics. Pane: &mut self on Handler replaces locking entirely. These tests document a whole category that pane eliminates by construction.

### Dynamic handler management → Static service setup
Haiku: AddHandler, RemoveHandler, CountHandlers, HandlerAt, IndexOf test runtime handler add/remove. Pane: services are declared at PaneBuilder setup, frozen before run_with. No dynamic add/remove. Deliberate divergence: stability of dispatch contracts.

### Untyped message fields → Typed Message enum
Haiku: All MessageItemTest variants test dynamic Add/Find/Replace of typed fields in BMessage. Pane: Message is a typed enum; no dynamic fields. Serialization is CBOR via serde, not manual field management.

### Single-function send with timeout → Two-function send split
Haiku: SendMessageTester exhaustively tests SendMessage with delivery_timeout + reply_timeout. Pane: send_request (infallible) + try_send_request (returns Backpressure). No timeout parameter — timeouts are handled by CancelHandle + watchdog. This is the D1/D7 divergence.

### Global looper registry → No global state
Haiku: LooperForThread tests global thread→looper lookup. Pane: no global registries, no be_app.

### BMessageRunner count-based delivery → TimerToken pulse
Haiku: SetCount, SetInterval test count-limited and unlimited periodic messages via registrar. Pane: set_pulse_rate returns TimerToken, Drop cancels. No count limit — pulse until cancelled or pane exits.

### Message queue as separate object → calloop channel
Haiku: BMessageQueue is a lockable, introspectable queue (CountMessages, FindMessage, Lock/Unlock). Pane: calloop channel is opaque, no Lock, no FindMessage. ConcurrencyTest1/2 patterns translate to channel stress tests rather than queue API tests.

## 5. Recommended Port List (25 tests, prioritized)

### Tier 1: Direct ports (lifecycle + watch)

1. **IsWatched1** (bhandler/IsWatchedTest.cpp:25) → `test_no_watchers_not_watched` — empty watch table.
2. **IsWatched2** (bhandler/IsWatchedTest.cpp:37) → `test_watch_unwatch_lifecycle` — add/remove watcher.
3. **IsWatched3** (bhandler/IsWatchedTest.cpp:52) → `test_watch_notice_unwatch` — notice delivery during active watch.
4. **IsWatched4** (bhandler/IsWatchedTest.cpp:69) → `test_unwatch_nonexistent_idempotent` — remove non-existent watcher.
5. **IsWatched5** (bhandler/IsWatchedTest.cpp:82) → `test_notice_empty_watch_table` — notify with no watchers.
6. **AppQuitRequestedTest1** (bapplication/AppQuitRequestedTester.cpp:52) → `test_close_requested_false_then_true` — close_requested return value.
7. **AppRunTest1 lifecycle** (bapplication/AppRunTester.cpp:53) → `test_lifecycle_ordering_ready_before_quit` — ready() before close_requested().

### Tier 2: Messaging semantics

8. **SendMessage uninitialized** (bmessenger/SendMessageTester.cpp:152) → `test_send_to_invalid_handle` — error on invalid target.
9. **SendMessage B_WOULD_BLOCK** (bmessenger/SendMessageTester.cpp:244-250) → `test_try_send_backpressure` — try_send_request returns Backpressure.
10. **SendMessage B_TIMED_OUT** (bmessenger/SendMessageTester.cpp:253-258) → `test_send_delivery_timeout` — timeout semantics (adapted to CancelHandle).
11. **QuitTest2** (blooper/QuitTest.cpp:37) → `test_close_via_messenger` — LifecycleMessage::CloseRequested via Messenger.
12. **ForwardMessageTest** (bmessenger/ForwardMessageTest.cpp:1-55) → `test_message_forward_chain` — inter-pane forwarding with reply.

### Tier 3: Timer/pulse

13. **SetInterval3** (bmessagerunner/SetIntervalTester.cpp:117) → `test_pulse_rate_change_mid_delivery` — change pulse rate with pending delivery.
14. **SetInterval4** (bmessagerunner/SetIntervalTester.cpp:154) → `test_pulse_interval_reset` — timer reset on interval change.
15. **SetInterval5** (bmessagerunner/SetIntervalTester.cpp:192) → `test_unlimited_pulse_interval_change` — unlimited pulse with interval change.
16. **SetInterval1** (bmessagerunner/SetIntervalTester.cpp:65) → `test_pulse_on_uninitialized` — error on bad state.

### Tier 4: Concurrency/stress

17. **ConcurrencyTest1** (bmessagequeue/ConcurrencyTest1.cpp:55) → `test_concurrent_send_receive_count` — 3-thread channel stress with count verification.
18. **ConcurrencyTest2** (bmessagequeue/ConcurrencyTest2.cpp:43) → `test_concurrent_ops_under_contention` — 5-thread blocking operations.
19. **PortLinkTest oversized** (messaging/PortLinkTest.cpp:42-47) → `test_oversized_frame_rejected` — frame exceeding max size.
20. **PortLinkTest WouldBlock** (messaging/PortLinkTest.cpp:99-103) → `test_empty_channel_would_block` — non-blocking read on empty.

### Tier 5: Targeted dispatch + scripting

21. **HandlerLooperMessageTest** (messaging/HandlerLooperMessageTest.cpp:24-39) → `test_targeted_service_dispatch` — message to specific service handler.
22. **PropertyFindMatch** (bpropertyinfo/PropertyFindMatchTest.cpp:47) → `test_property_info_find_match` — specifier/command matching for scripting.
23. **PropertyConstruction** (bpropertyinfo/PropertyConstructionTest.cpp:164) → `test_property_info_construction` — PropertyInfo creation/comparison.

### Tier 6: Filter semantics

24. **AddCommonFilterTest3** (blooper/AddCommonFilterTest.cpp:62) → `test_filter_add_valid` — successful filter installation.
25. **AddCommonFilterTest4** (blooper/AddCommonFilterTest.cpp:79) → `test_filter_single_ownership` — filter can only belong to one pane.

### Adaptation notes

All ports require: (a) translating BLooper/BHandler to pane's Handler+run_with, (b) replacing Lock/Unlock with &amp;mut self, (c) replacing BMessage dynamic fields with typed enum variants, (d) replacing kernel ports with calloop channels, (e) replacing BMessenger(handler, looper) with Messenger from PaneBuilder.

The IsWatched tests (Tier 1) are the most direct ports — the Watch/Unwatch/PaneExited mechanism on ProtocolServer maps closely to StartWatching/StopWatching/SendNotices.

The SendMessage tests (Tier 2) require the most adaptation because pane splits Be's one-function-with-timeout into two functions (send_request/try_send_request) and replaces timeout-based blocking with CancelHandle + obligation handles.
