# Test Plan: pane-app API informed by Haiku's Application Kit Tests

Based on systematic study of Haiku's test suite at
`~/src/haiku/src/tests/kits/app/` and pane's current tests across
`pane-app`, `pane-session`, and `pane-proto`.

---

## 1. Mapping Haiku concepts to pane types

| Haiku | pane | Notes |
|-------|------|-------|
| BApplication | App | Singleton connection to compositor |
| BLooper | looper (module) | Per-pane event loop |
| BHandler | Handler (trait) | Structured event dispatch |
| BMessage | PaneEvent / CompToClient / ClientToComp | Split into domain types |
| BMessenger | PaneHandle | Cloneable send-only handle |
| BMessageFilter | Filter (trait) / FilterChain | Consume/pass chain |
| BMessageQueue | mpsc::Receiver | Implicit in channel |
| BPropertyInfo | PropertyDecl / RouteTable / ScriptQuery | Scripting stubs |

---

## 2. What Haiku tests and what pane covers

### 2.1 BLooper tests (Haiku: `blooper/`)

**Haiku test files and coverage:**

| File | Tests | What it covers |
|------|-------|----------------|
| `AddHandlerTest.cpp` | 2 | Null handler, unlocked looper |
| `RemoveHandlerTest.cpp` | 5 | Null, non-member, unlocked, with-filters |
| `CountHandlersTest.cpp` | 2 | Zero handlers, add/remove counting |
| `HandlerAtTest.cpp` | 2 | Index lookup |
| `IndexOfTest.cpp` | 2 | Reverse lookup |
| `RunTest.cpp` | 2 | Double-run, thread identity |
| `QuitTest.cpp` | 2 | Unlocked quit, message-based quit |
| `IsMessageWaitingTest.cpp` | 4 | Locked/unlocked x empty/filled |
| `AddCommonFilterTest.cpp` | 4 | Null, unlocked, locked, shared-filter |
| `RemoveCommonFilterTest.cpp` | ? | Mirror of add |
| `SetCommonFilterListTest.cpp` | ? | Bulk filter replacement |
| `LooperForThreadTest.cpp` | 2 | Lookup by thread ID |
| `LooperSizeTest.cpp` | ? | Port buffer sizing |
| `PerformTest.cpp` | 1 | Unknown perform_code returns error |

**pane's current coverage (in `tests/looper.rs`):**

- closure_receives_key_and_exits
- closure_handles_close
- closure_handles_channel_close_as_disconnect
- closure_ignores_wrong_pane_id
- filter_consumes_event
- handler_dispatches_to_correct_methods

**Assessment:** pane tests the happy path of the looper (receive, filter, dispatch) but doesn't test edge cases around lifecycle, error conditions, or filter chain management.

### 2.2 BHandler tests (Haiku: `bhandler/`)

**Haiku test files and coverage:**

| File | Tests | What it covers |
|------|-------|----------------|
| `BHandlerTester.cpp` | 16 | Constructor (null/valid/archive), Archive, Instantiate, SetName, Perform, FilterList |
| `NextHandlerTest.cpp` | 2 | Default (null), after add-to-looper |
| `SetNextHandlerTest.cpp` | 12 | Chain navigation: same looper, different loopers, locked/unlocked |
| `AddFilterTest.cpp` | 4 | Null, no-looper, unlocked-looper, locked-looper |
| `RemoveFilterTest.cpp` | 7 | Null, with-looper, without-looper, not-owned |
| `SetFilterListTest.cpp` | 5 | Null, no-looper, locked/unlocked, replace-with-null |
| `LockLooperTest.cpp` | 3 | No looper, initially unlocked, initially locked |
| `LockLooperWithTimeoutTest.cpp` | ? | Timed lock variants |
| `UnlockLooperTest.cpp` | ? | Unlock semantics |
| `IsWatchedTest.cpp` | 5 | No watchers, add/remove, send-notice, remove-nonexistent |
| `HandlerLooperTest.cpp` | ? | Handler's owning looper |

**pane's current coverage:** The Handler trait is tested only via `handler_dispatches_to_correct_methods` in `looper.rs`. No tests exist for:
- Handler default method behavior
- Filter add/remove/replace on a running vs. pre-run Pane
- Handler with empty event stream
- Handler error propagation

### 2.3 BMessenger tests (Haiku: `bmessenger/`)

**Haiku test files and coverage:**

| File | Tests | What it covers |
|------|-------|----------------|
| `BMessengerTester.cpp` | 19 | Construction: null, handler-only, handler+looper, handler-wrong-looper, copy, by-signature, by-team |
| `SendMessageTester.cpp` | 1 (but exhaustive) | Uninitialized, local/remote x preferred/specific, timeouts, blocking, reply |
| `TargetTester.cpp` | 10 | IsTargetLocal, Target() for all target flavors |
| `LockTargetTester.cpp` | 7 | Uninitialized, local unlocked, local locked, blocked-by-other-thread, remote |
| `LockTargetWithTimeoutTester.cpp` | ? | Timed lock variants |
| `MessengerAssignmentTester.cpp` | 3 | operator= from uninitialized, preferred, specific |
| `MessengerComparissonTester.cpp` | 3 | ==, !=, < for all combinations |
| `ForwardMessageTest.cpp` | ? | Message forwarding |

**pane's current coverage:** PaneHandle has zero dedicated tests. It's used implicitly in looper tests but:
- No test that PaneHandle::set_title actually sends the right ClientToComp
- No test for disconnected PaneHandle (send after channel close)
- No test for PaneHandle equality / cloning behavior
- No test for PaneHandle Debug impl

### 2.4 BMessage tests (Haiku: `bmessage/`)

**Haiku test files and coverage:**

| File | Tests | What it covers |
|------|-------|----------------|
| `MessageConstructTest.cpp` | 3 | Default, what-init, copy |
| `MessageOpAssignTest.cpp` | 1 | operator= |
| `MessageDestructTest.cpp` | ? | Destruction cleans up data |
| `MessageItemTest.h` | 12 per type | Add/Find/Replace/Has/FindData for each type (Int32, String, Bool, etc.) + null-name, flatten/unflatten |
| `MessageEasyFindTest.cpp` | ? | Shorthand find methods |
| `MessageSpeedTest.cpp` | ~60 | Performance: create/lookup/read/flatten/unflatten x count x type |

**pane's current coverage (`pane_event.rs`, `roundtrip.rs`):**
- PaneEvent::from_comp for each variant (6 tests)
- proptest roundtrip for all wire types (11 tests)
- FKey validation, is_escape checks

**Assessment:** Protocol types are well-tested for serialization. PaneEvent construction is tested. What's missing is the equivalent of Haiku's "item manipulation" tests — but pane events are enums, not dynamic bags, so this gap is mostly structural. The real gap is in event construction edge cases.

### 2.5 BApplication tests (Haiku: `bapplication/`)

**Haiku test files and coverage:**

| File | Tests | What it covers |
|------|-------|----------------|
| `BApplicationTester.cpp` | 5 | Signature validation: null, invalid MIME, wrong supertype, mismatch, valid |
| `AppRunTester.cpp` | 22 | Launch modes: MULTIPLE/SINGLE/EXCLUSIVE x ARGV_ONLY, same/different signatures |
| `AppQuitTester.cpp` | 4 | Quit not-running, quit from looper thread, quit from other thread, quit without lock |
| `AppQuitRequestedTester.cpp` | ? | QuitRequested semantics |

**pane's current coverage (`hello_pane.rs`):**
- One integration test: connect, create pane, inject close, verify lifecycle

**Assessment:** The hello_pane test is good as a smoke test, but Haiku tests App construction failures, double-run, quit from various contexts, and lifecycle ordering exhaustively. pane has none of that.

### 2.6 BPropertyInfo tests (Haiku: `bpropertyinfo/`)

**Haiku coverage:** Construction, FindMatch (wildcard/specific commands x specifiers), Flatten/Unflatten.

**pane's current coverage (`stubs.rs`):** RouteTable::route returns NoMatch, PropertyDecl and ScriptQuery are constructible. These are just type-existence tests.

### 2.7 BMessageQueue tests (Haiku: `bmessagequeue/`)

**Haiku coverage:** AddMessage (null, valid), ConcurrencyTest1 (3-thread add/remove/next), ConcurrencyTest2, FindMessage, CountMessages.

**pane equivalent:** The mpsc channel is the queue. pane-session has calloop_integration tests covering receive-from-channel and crash detection. No concurrent-access tests for the channel itself (mpsc handles this).

---

## 3. Gaps: what Haiku tests that pane doesn't

### Priority 1: Would catch real bugs today

#### P1-1. PaneHandle disconnection (cf. BMessenger uninitialized/invalid tests)
**Haiku reference:** `BMessengerTester::BMessenger1-3` — uninitialized messenger returns false/BAD_VALUE.
`SendMessageTester::TestUninitialized` — send to invalid messenger returns BAD_PORT_ID.

**pane gap:** No test sends through a PaneHandle whose channel receiver has been dropped.

**Proposed test:** `test_pane_handle_send_after_disconnect`
- File: `crates/pane-app/tests/proxy.rs` (new file)
- Create PaneHandle with an mpsc::channel, drop the receiver, call set_title
- Assert returns `Err(PaneError::Disconnected)`

#### P1-2. PaneHandle send correctness (cf. BMessenger::Target tests)
**Haiku reference:** `TargetTester::TargetTest1-5` — verify Target() returns correct handler/looper.

**pane gap:** No test verifies that PaneHandle methods produce the correct ClientToComp variants.

**Proposed tests (all in `crates/pane-app/tests/proxy.rs`):**
- `test_pane_handle_set_title_sends_correct_message` — call set_title, recv from channel, assert ClientToComp::SetTitle with correct PaneId and title
- `test_pane_handle_set_vocabulary_sends_correct_message` — same for vocabulary
- `test_pane_handle_set_content_sends_correct_message` — same for content
- `test_pane_handle_set_completions_sends_correct_message` — same for completions
- `test_pane_handle_clone_sends_to_same_channel` — clone a handle, send from both, verify both arrive on same receiver

#### P1-3. Filter chain edge cases (cf. BHandler AddFilter/RemoveFilter/SetFilterList)
**Haiku reference:** `AddFilterTest` — null filter, filter without looper, filter with unlocked looper, filter with locked looper.
`RemoveFilterTest` — null, not-owned filter, various looper states.
`SetFilterListTest` — null list, replace list, replace with null.
`AddCommonFilterTest` — filter already owned by another looper.

**pane gap:** Only one filter test (`filter_consumes_event`). No tests for:
- Empty filter chain (all events pass through)
- Multiple filters in chain order
- Filter that transforms an event (via FilterAction::Pass with modified event)
- Filter that consumes all events (looper should just drain)

**Proposed tests (in `crates/pane-app/tests/looper.rs`):**
- `test_empty_filter_chain_passes_all` — no filters, all events reach handler
- `test_filter_chain_ordering` — two filters, first transforms, second consumes transformed; verify ordering
- `test_filter_consume_all_then_disconnect` — filter consumes everything, channel closes, verify graceful exit
- `test_multiple_filters_partial_consume` — 3 filters, middle one consumes Key events only, verify Focus/Close still pass

#### P1-4. Handler default methods (cf. BHandler construction tests)
**Haiku reference:** `BHandlerTester::BHandler1-5` — construction with null/valid/archive.
`BHandlerTester::FilterList1` — default handler has no filters.

**pane gap:** No test that a Handler's default methods actually return the correct values (Ok(true) for most, Ok(false) for close_requested and disconnected).

**Proposed tests (in `crates/pane-app/tests/handler.rs`, new file):**
- `test_handler_default_close_returns_false` — struct implementing Handler with no overrides, send Close, verify loop exits
- `test_handler_default_key_returns_true` — send key, verify loop continues (needs second event to terminate)
- `test_handler_default_disconnect_returns_false` — drop sender, verify loop exits
- `test_handler_override_close_to_continue` — override close_requested to return Ok(true), verify loop continues past Close

#### P1-5. Looper lifecycle: channel close during processing (cf. BLooper quit tests)
**Haiku reference:** `QuitTest::QuitTest1` — quit while unlocked. `QuitTest::QuitTest2` — quit via message.
`RunTest::RunTest1` — double Run() detection.

**pane gap:** `closure_handles_channel_close_as_disconnect` exists but doesn't test:
- Channel closed mid-event (between filter and dispatch)
- Multiple events queued, channel closes after last one
- Handler returns error during processing

**Proposed tests (in `crates/pane-app/tests/looper.rs`):**
- `test_looper_handler_error_propagates` — handler returns Err, verify run_closure returns that Err
- `test_looper_processes_all_queued_before_disconnect` — queue 3 events then drop sender, verify all 3 are processed before Disconnected
- `test_looper_error_stops_processing` — queue 3 events, handler errors on 2nd, verify 3rd not processed

### Priority 2: Design contract enforcement

#### P2-1. App connection failures (cf. BApplication signature validation)
**Haiku reference:** `BApplicationTester::BApplication1-3` — null sig, invalid MIME, wrong supertype all produce errors.

**pane gap:** No test for App::connect with invalid signature, or when compositor is unreachable.

**Proposed tests (in `crates/pane-app/tests/app.rs`, new file):**
- `test_app_connect_test_creates_valid_connection` — verify App::connect_test returns Ok
- `test_app_create_pane_returns_valid_pane` — verify created pane has non-zero ID and reasonable geometry
- `test_app_create_multiple_panes` — create 2 panes, verify different IDs
- `test_app_drop_signals_all_panes` — (may need MockCompositor support) drop App, verify panes get notification

#### P2-2. PaneEvent construction completeness (cf. BMessage item tests)
**Haiku reference:** `MessageItemTest::MessageItemTest1-12` — exhaustive add/find/replace/flatten for every type.

**pane gap:** `pane_event.rs` tests 6 of the CompToClient variants. Missing:
- Focus, Blur
- Mouse events
- CommandActivated, CommandDismissed
- CompletionRequest

**Proposed tests (in `crates/pane-app/tests/pane_event.rs`):**
- `test_from_comp_focus` — CompToClient::Focus with matching pane
- `test_from_comp_blur` — CompToClient::Blur with matching pane
- `test_from_comp_mouse` — CompToClient::Mouse with matching pane
- `test_from_comp_command_activated` — CompToClient::CommandActivated
- `test_from_comp_command_dismissed` — CompToClient::CommandDismissed
- `test_from_comp_completion_request` — CompToClient::CompletionRequest
- `test_from_comp_all_variants_wrong_pane` — every variant with wrong pane_id returns None

#### P2-3. PaneHandle comparison/identity (cf. BMessenger comparison tests)
**Haiku reference:** `MessengerComparissonTester::ComparissonTest1-3` — two uninit equal, init vs uninit not equal, same target equal, different targets not equal.

**pane gap:** PaneHandle doesn't implement Eq/PartialEq. Should it? If yes, test it. If no, at least test that .id() matches.

**Proposed tests (in `crates/pane-app/tests/proxy.rs`):**
- `test_pane_handle_id_matches_construction` — create with known PaneId, verify .id() returns it
- `test_pane_handle_clone_has_same_id` — clone, verify both have same id
- `test_pane_handle_debug_includes_id` — format!("{:?}", handle) contains the pane ID

#### P2-4. Tag/Command builder edge cases (cf. BPropertyInfo construction/FindMatch)
**Haiku reference:** `PropertyConstructionTest`, `PropertyFindMatchTest` — construct with empty properties, wildcard commands/specifiers, then verify FindMatch behavior.

**pane gap:** `tag_builder.rs` tests the happy paths. Missing:
- Empty command list
- Command with no shortcut
- Command with no action (should this be possible?)
- Very long title/description strings
- Unicode in titles

**Proposed tests (in `crates/pane-app/tests/tag_builder.rs`):**
- `test_tag_empty_commands` — Tag::new("X").commands(vec![]), verify groups empty
- `test_cmd_no_shortcut` — cmd without shortcut, verify None
- `test_cmd_no_action_default` — cmd("x","y") without calling client/built_in/route, verify action
- `test_tag_unicode_title` — Tag::new("日本語テスト"), verify roundtrip
- `test_tag_empty_string_title` — Tag::new(""), verify it works

### Priority 3: Stress and concurrency

#### P3-1. Concurrent PaneHandle sends (cf. BMessageQueue ConcurrencyTest1)
**Haiku reference:** `ConcurrencyTest1` — 3 threads: one adding 5000 messages, one NextMessage() 100 times, one adding + removing. Verifies count and no lost messages.

**pane gap:** No concurrent test for PaneHandle. The mpsc channel handles this at the Rust level, but testing it confirms the contract.

**Proposed test (in `crates/pane-app/tests/proxy.rs`):**
- `test_pane_handle_concurrent_sends` — spawn 4 threads each sending 100 messages via cloned PaneHandle, verify receiver gets exactly 400 messages

#### P3-2. Looper under load (cf. BLooper IsMessageWaiting with port buffer)
**Haiku reference:** `IsMessageWaitingTest` — tests interaction between port buffer and message queue under various lock states.

**pane gap:** No test that the looper handles a burst of messages correctly.

**Proposed test (in `crates/pane-app/tests/looper.rs`):**
- `test_looper_processes_burst` — queue 1000 Focus/Blur events, verify handler sees all of them in order
- `test_looper_burst_with_filter` — queue 1000 events, filter drops 50%, verify handler sees ~500

#### P3-3. Multiple panes running concurrently (cf. BApplication multi-instance Run tests)
**Haiku reference:** `AppRunTester::RunTest1-22` — launch app twice with various modes, verify lifecycle ordering.

**pane gap:** hello_pane tests one pane. No test runs two panes simultaneously.

**Proposed test (in `crates/pane-app/tests/hello_pane.rs`):**
- `test_two_panes_concurrent_lifecycle` — create two panes, run each on its own thread, inject Close to both, verify both exit cleanly

---

## 4. Proposed test file organization

```
crates/pane-app/tests/
  hello_pane.rs      -- integration (existing + P3-3)
  looper.rs          -- looper/closure/handler dispatch (existing + P1-3, P1-5, P3-2)
  handler.rs         -- Handler trait default behaviors (NEW, P1-4)
  proxy.rs           -- PaneHandle send/disconnect/clone (NEW, P1-1, P1-2, P2-3, P3-1)
  pane_event.rs      -- PaneEvent::from_comp coverage (existing + P2-2)
  tag_builder.rs     -- Tag/cmd builder edge cases (existing + P2-4)
  stubs.rs           -- scripting stubs (existing, extend later)
  app.rs             -- App lifecycle/connection (NEW, P2-1)
```

---

## 5. Test priority matrix

| ID | Test name | Crate | File | Why it matters |
|----|-----------|-------|------|---------------|
| P1-1 | `test_pane_handle_send_after_disconnect` | pane-app | proxy.rs | Prevents silent message loss |
| P1-2a | `test_pane_handle_set_title_sends_correct_message` | pane-app | proxy.rs | Verifies the primary API contract |
| P1-2b | `test_pane_handle_set_vocabulary_sends_correct_message` | pane-app | proxy.rs | Same |
| P1-2c | `test_pane_handle_set_content_sends_correct_message` | pane-app | proxy.rs | Same |
| P1-2d | `test_pane_handle_set_completions_sends_correct_message` | pane-app | proxy.rs | Same |
| P1-2e | `test_pane_handle_clone_sends_to_same_channel` | pane-app | proxy.rs | Verifies clone semantics |
| P1-3a | `test_empty_filter_chain_passes_all` | pane-app | looper.rs | Baseline filter correctness |
| P1-3b | `test_filter_chain_ordering` | pane-app | looper.rs | Filter composition contract |
| P1-3c | `test_filter_consume_all_then_disconnect` | pane-app | looper.rs | Graceful degradation |
| P1-3d | `test_multiple_filters_partial_consume` | pane-app | looper.rs | Selective filtering |
| P1-4a | `test_handler_default_close_returns_false` | pane-app | handler.rs | Default lifecycle correctness |
| P1-4b | `test_handler_default_key_returns_true` | pane-app | handler.rs | Default continue behavior |
| P1-4c | `test_handler_default_disconnect_returns_false` | pane-app | handler.rs | Disconnect lifecycle |
| P1-4d | `test_handler_override_close_to_continue` | pane-app | handler.rs | Override mechanism works |
| P1-5a | `test_looper_handler_error_propagates` | pane-app | looper.rs | Error handling contract |
| P1-5b | `test_looper_processes_all_queued_before_disconnect` | pane-app | looper.rs | Message ordering guarantee |
| P1-5c | `test_looper_error_stops_processing` | pane-app | looper.rs | Fail-fast semantics |
| P2-1a | `test_app_connect_test_creates_valid_connection` | pane-app | app.rs | Constructor contract |
| P2-1b | `test_app_create_pane_returns_valid_pane` | pane-app | app.rs | Factory contract |
| P2-1c | `test_app_create_multiple_panes` | pane-app | app.rs | Multi-pane support |
| P2-2a-f | `test_from_comp_{focus,blur,mouse,...}` | pane-app | pane_event.rs | Event coverage gaps |
| P2-2g | `test_from_comp_all_variants_wrong_pane` | pane-app | pane_event.rs | Pane ID filtering |
| P2-3a | `test_pane_handle_id_matches_construction` | pane-app | proxy.rs | Identity contract |
| P2-3b | `test_pane_handle_clone_has_same_id` | pane-app | proxy.rs | Clone semantics |
| P2-3c | `test_pane_handle_debug_includes_id` | pane-app | proxy.rs | Debuggability |
| P2-4a-e | `test_tag_{empty,unicode,no_shortcut,...}` | pane-app | tag_builder.rs | Builder robustness |
| P3-1 | `test_pane_handle_concurrent_sends` | pane-app | proxy.rs | Thread safety |
| P3-2a | `test_looper_processes_burst` | pane-app | looper.rs | Throughput |
| P3-2b | `test_looper_burst_with_filter` | pane-app | looper.rs | Filtered throughput |
| P3-3 | `test_two_panes_concurrent_lifecycle` | pane-app | hello_pane.rs | Multi-pane concurrency |

---

## 6. What Haiku tests that doesn't map to pane (by design)

These Haiku test categories exercise features that pane intentionally doesn't replicate:

- **Locking tests** (LockLooper, LockTarget, LockTargetWithTimeout): BeOS required manual lock/unlock of the looper before modifying handler state. pane's looper runs on a single thread with sequential dispatch — no locking needed. This is a genuine improvement: the locking was a major source of deadlocks in BeOS apps.

- **Handler chaining** (NextHandler, SetNextHandler): BeOS had a linked list of handlers within a looper, where unhandled messages bubbled up the chain. pane uses a flat dispatch model — one Handler per pane, or a closure. The filter chain provides pre-dispatch interception but not post-dispatch bubbling. If pane later adds handler delegation, the SetNextHandler tests would be the template.

- **Archiving/Instantiation** (BHandler Archive, Instantiate): BeOS serialized handlers into BMessages for drag-and-drop and persistence. pane doesn't have this — Handler is a trait, not a serializable object. If pane adds pane state persistence, this test category becomes relevant.

- **Node monitoring / watching** (IsWatched, StartWatching, StopWatching, SendNotices): BeOS handler observation protocol. pane doesn't have this yet. When it adds compositor-level event subscription (e.g. "notify me when any pane's title changes"), these tests are the template.

- **Remote targets** (BMessenger by signature/team, IsTargetLocal, remote send): BeOS could send messages to other applications by signature. pane's PaneHandle only targets the compositor. If pane adds inter-pane messaging, the BMessenger remote tests are the template.

- **Application launch modes** (SINGLE_LAUNCH, EXCLUSIVE_LAUNCH, MULTIPLE_LAUNCH): BeOS managed app uniqueness at the OS level. pane runs on unix — process management is external (systemd, s6). This is correctly not replicated.

---

## 7. Observations from the study

### What Haiku's tests teach about test design

1. **Test the invalid cases first.** Almost every Haiku test suite starts with null parameters, uninitialized objects, and unlocked loopers. This is where BeOS R5 had segfaults that Haiku fixed. pane should adopt this discipline: every public method needs a test for its failure mode.

2. **Test lifecycle ordering.** Haiku's AppRunTester verifies the exact sequence: ArgvReceived, ReadyToRun, QuitRequested. pane's `handler_dispatches_to_correct_methods` does this for handler methods, but the App-level lifecycle (connect -> create_pane -> run -> close -> drop) is only tested in the integration test.

3. **Test the cross-thread cases.** Haiku uses BThreadedTestCaller extensively — `LockTargetTester` runs two threads that contend for a lock. pane's Rust ownership model prevents many of these bugs, but the concurrent-PaneHandle-sends test (P3-1) is still valuable.

4. **Test with real timing.** Haiku's SendMessageTester tests delivery with actual delays and timeouts (10ms, 20ms, 40ms), verifying both success and timeout. pane should test that the looper actually processes events within bounded time, not just that it processes them eventually.

### What pane already does better than Haiku's test suite

1. **Property-based testing.** The `pane-proto/tests/roundtrip.rs` proptest suite is more thorough than Haiku's per-type item tests — it generates random instances and verifies serialization roundtrips for all types. Haiku's message tests are exhaustive but hand-written.

2. **Session type testing.** `pane-session` tests are genuinely novel — crash recovery, fragmented writes, calloop integration. Haiku had nothing equivalent because BeOS ports didn't have these failure modes.

3. **Mock compositor.** The MockCompositor pattern in `hello_pane.rs` is a clean integration test harness. Haiku's AppRunner/PipedAppRunner was more fragile (spawning real processes, parsing stdout).
