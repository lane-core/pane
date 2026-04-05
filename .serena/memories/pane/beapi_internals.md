# BeOS/Haiku Internals — Design Knowledge for Pane

Extracted from Haiku source analysis. Covers internal mechanisms
that inform pane's architecture. Complements `beapi_divergences`
(which tracks the public API mapping) and `beapi_translation_rules`
(which covers naming/pattern rules).

Source reference: `reference/haiku-src/`

---

## app_server Wire Protocol

The link between BApplication and app_server is a buffered binary
protocol over kernel ports — NOT BMessage-based.

- `LinkSender` buffers multiple messages before flushing.
  `StartMessage(code)` → `Attach(data)` → `EndMessage()` → `Flush()`.
  Multiple messages can be batched before a single Flush().
- `LinkReceiver` reads with `GetNextMessage(code)`, then pulls
  typed fields with `Read<Type>()`. No self-describing format —
  both sides agree on field order.
- `ServerLink` = `LinkSender` + `LinkReceiver`. `FlushWithReply()`
  flushes then blocks for a response (sync call path).
- `ServerProtocol.h` defines ~370 opcodes. Each opcode implies
  a specific sequence of Attach calls.
- Batching was critical for performance: async calls are batched;
  sync calls force flush + round-trip (much slower).

**pane mapping:** pane-session's postcard-framed wire protocol is
the equivalent. The wire protocol should NOT be self-describing —
compact binary where both sides agree on the schema (postcard +
Rust types). The batching insight maps to calloop's ability to
coalesce multiple messages in a single wakeup.

## BLooper Lock Contention

BLooper (and BWindow) had a benaphore-based lock. Any thread
accessing the looper's handlers or state had to Lock() first.

Problems:
- **Deadlock:** Lock ordering bugs were the most common BeOS app
  bug. Be Commandment #2 ("Thou shalt not lock the same objects
  in differing orders") was a convention, not enforced.
- **Priority inversion:** Window thread blocks on lock held by
  low-priority worker thread.
- **Aliasing after delete:** A deleted looper could be replaced at
  the same address. Pointer-based locks became unreliable.
  BMessenger-based locking was safer (checked identity).
- **Lock scope confusion:** Holding locks too long blocked the
  window thread, causing app_server to discard update messages.

**pane mapping:** `&mut self` during dispatch, enforced by the
borrow checker. No locks needed — handler state only accessed
from the looper thread. Cross-thread communication goes through
`Messenger::send_message()` (calloop channel). All four problems
eliminated by construction.

## BMessage Internals and Reply Mechanism

BMessage internals (from MessagePrivate.h):
- `message_header`: what code, flags, target token, reply chain
  (reply_port, reply_target, reply_team), body info
- `field_header`: type_code, count, data_size, hash collision chain
- Fields stored in flat buffer with 5-bucket hash table for name
  lookup. Names are strings; values typed by type_code.
- Flags: REPLY_REQUIRED, REPLY_DONE, IS_REPLY, WAS_DELIVERED

Reply mechanism:
- Every BMessage carries reply_port/reply_target/reply_team
- Delivery code sets these to point back to sender's looper
- SendReply() uses them; REPLY_REQUIRED enforces reply obligation
- SendMessage(msg, &reply) creates temp port, sets REPLY_REQUIRED,
  sends, blocks on reply port

**pane mapping:** Message trait carries data only (no reply state).
ReplyPort is a separate obligation handle consumed by .reply().
Reply address is in ReplyPort, not the message. This makes
Message Clone-safe — BMessage could never be cloned because it
carried mutable reply state.

## BClipboard — Centralized Lock-Based Model

- Data lived in the registrar (not the copying app)
- Lock() → Clear() → add data → Commit() → Unlock()
- System-wide lock (one holder at a time)
- Data was a BMessage (multiple representations simultaneously)
- StartWatching()/StopWatching() for change notifications
- Server-side: 111 + 225 LOC (registrar/Clipboard.cpp)

Simpler than X11 because:
- Push-based (data uploaded at copy time, persists after source exits)
- X11 is pull-based (paste requests data from source, which must
  still be running)
- No per-request MIME negotiation (all representations stored at
  copy time)
- One clipboard, not three (X11 PRIMARY/SECONDARY/CLIPBOARD)

**pane mapping:** ClipboardWriteLock typestate = Lock→Clear→Write
→Commit→Unlock. The typestate handle is consumed by commit,
making "forgot to commit" impossible. Wayland's clipboard is
pull-based like X11; pane's protocol-level clipboard abstraction
hides this from the developer.

## BRoster and Service Discovery

- Every BApplication registered with registrar at startup
  (B_REG_ADD_APP → B_REG_COMPLETE_REGISTRATION)
- Signature-based: MIME strings (application/x-vnd.Be-MAIL)
- Launch(signature), GetAppInfo(), GetAppList(), StartWatching()
- TRoster: 2068 LOC, largest file in registrar

Problems:
- String-based identity, manually assigned, no collision prevention
- Registrar was SPOF
- Complex launch logic (pre-registration, exclusive launch, etc.)
- Haiku added launch_daemon alongside registrar — two overlapping
  service managers

**pane mapping:** ServiceId (UUID + reverse-DNS name). UUID
deterministic (derived from name). No separate registrar — the
compositor IS the registry. DeclareInterest during session setup.
The functoriality_principle memory explains why ServiceId has
structure from day one.

## Error Handling — status_t and InitCheck()

BeOS patterns:
- `status_t` returns (int32). B_OK for success, negative for error.
- `InitCheck()` for objects that can't fail in constructors.
  Construct → check → use. Easy to forget the check.

What worked: universal, consistent, well-documented.
What didn't: InitCheck easy to forget, two-phase construction
(object exists in invalid state), flat error codes (no type safety,
no context), B_ERROR tells you nothing about what or why.

**pane mapping:** Result<T, E> replaces status_t/InitCheck. Three
error channels (Protocol, Control, Crash) are a principled
taxonomy BeOS never had. BeOS used status_t for everything,
conflating operation failure, peer crash, and resource loss.

## Two Kinds of Threading

BeOS threads:
1. **System-managed:** Every BLooper/BWindow spawned a thread for
   its message loop. Cost: ~20.75 KB per looper thread, ~56 KB
   per window (two threads — client + server side).
2. **Developer-spawned:** Worker threads for computation/IO.
   Communicated with loopers via BMessenger::SendMessage().

Contract: looper threads stay responsive (never block). Workers
do heavy lifting and post results back.

**pane mapping:** calloop collapses system-managed threads to one
event loop per pane. Same responsiveness guarantee without per-
looper thread overhead. Contract unchanged: don't block the event
loop. Worker threads still the developer's responsibility,
communicate via Messenger::send_message() (calloop channel).
You don't need N threads for N-way responsiveness — you need N
event sources feeding one loop.
