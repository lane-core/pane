# pane-app Kit API Review — Final

**Date:** 2026-03-27
**Reviewer:** Be Systems Engineer (consultant)
**Scope:** All files in `crates/pane-app/src/`, `tests/hello_pane.rs`
**Previous review:** 2026-03-26 (this file, now overwritten)

---

## Fix Verification

Three fixes were requested from the previous review (P4/P5, P6, P3). Here's what landed.

### Fix 1: PaneProxy as back-channel (was P4 + P5)

**Status: Applied. One compile error.**

The design intent is correct and well-executed:

- `PaneProxy` exists in `proxy.rs` as a `Clone`-able handle wrapping
  `(PaneId, mpsc::Sender<ClientToComp>)`. Fields are `pub(crate)`. Good.
- `Handler` trait methods all take `&PaneProxy` as first parameter after `&mut self`.
  Every method — `ready`, `resized`, `focused`, `blurred`, `key`, `mouse`,
  `command_activated`, `command_dismissed`, `command_executed`, `completion_request`,
  `close_requested`, `disconnected`, `unhandled` — gets it. Consistent. Correct.
- `run()` closure signature is `FnMut(&PaneProxy, PaneEvent) -> Result<bool>`. Proxy
  comes first, which is the right convention — it's the context, not the payload.
- `looper.rs` constructs one `PaneProxy` per loop and passes it by reference to every
  dispatch. The proxy lives on the stack of `run_closure`/`run_handler`. Clean.
- `Pane::proxy()` method exists (pane.rs:81-83) for pre-run access. Good — lets you
  clone the proxy and hand it to a background thread before entering the loop.
- The test (`hello_pane.rs:43`) uses `|_proxy, event|`. Correct.

The `BMessenger` analogy is apt. BMessenger was a lightweight proxy holding a `port_id`
and `team_id` — just enough to `SendMessage()` without owning the target. PaneProxy holds
a PaneId and a channel sender. Same idea, same ownership discipline.

**Bug (C1): `PaneProxy::new` doesn't exist.** `pane.rs` calls `PaneProxy::new(id, comp_tx.clone())`
on lines 82, 87, and 105. But `proxy.rs` defines no `new` method. The struct has `pub(crate)`
fields, so crate-internal code could construct via struct literal, but the call sites use
a named constructor that doesn't exist. This won't compile.

One-line fix — add to `proxy.rs`:

```rust
pub(crate) fn new(id: PaneId, sender: mpsc::Sender<ClientToComp>) -> Self {
    PaneProxy { id, sender }
}
```

### Fix 2: VecDeque::pop_front for pending_creates (was P6)

**Status: Applied correctly.**

- `app.rs:3` imports `VecDeque`.
- Line 27 declares `pending_creates: Arc<Mutex<VecDeque<...>>>`.
- Line 48 initializes with `VecDeque::new()`.
- Line 66 uses `pop_front()`.
- Line 115 uses `push_back()`.

FIFO ordering preserved. The old `Vec::pop()` was LIFO, which would have misrouted
PaneCreated responses when two panes were being created concurrently. Good fix.

### Fix 3: Explicit create_pane(Tag) + create_component_pane() (was P3)

**Status: Applied correctly.**

`app.rs:97-109` provides two distinct methods:

```rust
pub fn create_pane(&self, tag: Tag) -> Result<Pane>
pub fn create_component_pane(&self) -> Result<Pane>
```

Both delegate to `create_pane_inner(wire_tag: Option<CreatePaneTag>)`. No `impl Into<Option<Tag>>`.
`create_component_pane` is a better name than `create_pane_default` — it communicates *what
the pane is*, not just that it lacks configuration.

The doc comment on `create_component_pane` — "Component panes have no title or command
surface — they are building blocks meant to be interacted with through their parent
pane's tag or their own content area" — is exactly the right level of explanation.

---

## Overall API Assessment

### What's working well

**1. The ownership model is clean and the mapping to Be is precise.**

| pane-app | BeOS | Role |
|----------|------|------|
| `App` | `BApplication` | Connection owner, pane factory |
| `Pane` | `BWindow` | Handle, consumed by event loop entry |
| `PaneProxy` | `BMessenger` | Cloneable send-capability |
| `looper::run_*` | `BLooper::task_looper()` | Per-pane message dispatch |
| `Handler` | `BHandler` | Override what you understand |
| `FilterChain` | `BMessageFilter` chain | Ordered intercept/transform |
| `PaneEvent` | `BMessage::what` | Typed, exhaustive, compiler-checked |

App owns the connection and dispatcher. Pane owns its receiver channel and takes `self`
on `run()`/`run_with()`. PaneProxy is a clone-and-send capability. The thread-per-pane
model with FIFO message dispatch is exactly the BLooper discipline: concurrency *between*
panes, sequential processing *within* a pane.

The move to consuming `self` on `run()` is *stronger* than BLooper's `Lock()`/`Unlock()`
convention. You literally cannot mutate pane state behind the event loop's back. The proxy
is the only escape hatch, and it only lets you send messages — which is the whole point.

**2. Dual-mode handler design is right.**

Closures for hello-pane. Handler trait for real applications. Both get `&PaneProxy`.
`dispatch_to_handler` in `looper.rs` is the translation layer — exhaustive match on
PaneEvent, dispatches to the right Handler method. Clean and compiler-verified.

Handler defaults are sensible: everything continues except `close_requested` (accepts)
and `disconnected` (exits). This matches BHandler's philosophy.

**3. Tag builder is pleasant API craft.**

`Tag::new("Editor").commands(vec![...])` reads well. The `cmd()` free function with
terminal methods (`.client()`, `.built_in()`, `.route()`) enforces that every command
has an action. You can't create a half-built Command — the builder's type system prevents it.
`CommandBuilder` is an incomplete type; you *must* call a terminal to get a `Command`.

**4. FilterChain improves on BMessageFilter.**

Ordered chain, can transform or consume events, runs before the handler. `FilterAction`
carries the (possibly modified) event in `Pass`, which is cleaner than Be's approach of
mutating through a pointer. One less thing to get wrong.

**5. PaneProxy's method surface is exactly right.**

`set_title`, `set_vocabulary`, `set_content`, `set_completions`, `id()`. These are the
operations a handler needs, and nothing more. The internal `send()` helper deduplicates
the error mapping. The `Debug` impl omits the sender (which has no useful debug output).
Small details, but they add up.

### Issues to fix

**Critical:**

**C1. PaneProxy::new doesn't exist — code won't compile.**

Severity: critical. Covered above. One-line fix.

**Moderate:**

**M1. lib.rs Quick Start example is stale.**

Line 22 shows `pane.run(|event| match event {` — the old single-argument closure.
The actual signature is `FnMut(&PaneProxy, PaneEvent) -> Result<bool>`. Should be:

```rust
pane.run(|_proxy, event| match event {
    pane_app::PaneEvent::Key(key) if key.is_escape() => Ok(false),
    pane_app::PaneEvent::Close => Ok(false),
    _ => Ok(true),
})
```

Stale doc examples are worse than no doc examples — they actively mislead.

**M2. Pane duplicates PaneProxy methods.**

`Pane` has `set_title()`, `set_vocabulary()`, and `set_content()` (lines 116-141).
`PaneProxy` has identical methods doing the same thing through the same channel. Once
`Pane::proxy()` exists (line 81), the methods on Pane itself are redundant.

Worse: the Pane methods can't be called after `run()` consumes self, which is *exactly
when you'd want to update title/content*. Pre-run, you configure through `Tag`. Post-run,
you use the proxy in the handler. There's no moment when `pane.set_title()` is the right
call over `proxy.set_title()`.

Recommendation: remove `set_title`, `set_vocabulary`, `set_content` from `Pane`. Let
the proxy own the send-side surface. This also eliminates duplicated `.map_err(|_|
PaneError::Disconnected)?` patterns across two types.

**M3. App::wait() is a spin loop.**

`app.rs:146-149` polls `pane_count` every 50ms. Works but crude. At Be we'd have used
a semaphore or port. Here, a condvar or a watch-channel that fires on pane_count reaching
zero would be appropriate. Not blocking for the demo phase, but don't ship this.

**M4. MockCompositor::close_first_pane_after takes `delay: Duration` but ignores it.**

`mock.rs:52` accepts `delay` but never reads it. The 200ms delay is hardcoded in the
injection path. Dead parameters erode trust. Either plumb the delay or remove the parameter.

**M5. Dispatcher mutex poisoning.**

`app.rs:65-66` uses `.lock().unwrap()` on `pending_creates`, line 75 on `pane_channels`.
If a handler panics (it's user code on another thread), the mutex could be poisoned
and the dispatcher crashes. Options: `parking_lot::Mutex` (no poisoning), or `.lock()
.unwrap_or_else(|e| e.into_inner())` to recover. Document the invariant either way.

**Minor:**

**m1. extract_pane_id returns Option\<u64\> but is exhaustive.**

All 12 `CompToClient` variants are listed. The function never returns `None`. The
`Option` return type is misleading. If the enum grows a variant without a pane field,
the match will fail to compile (which is correct — forces a decision). Change the
return type to `u64`.

**m2. PaneEvent::from_comp does redundant PaneId filtering.**

The dispatcher already routes by PaneId. The per-event `if *p == pane` check is
belt-and-suspenders. Either remove it and document the routing guarantee, or keep it
and document it as defense-in-depth. Don't leave it unexplained.

**m3. Handler requires Send + 'static but closures don't.**

`Handler: Send + 'static` (handler.rs:24). `run()`'s closure has no such bound. Both
currently run on the calling thread — no `thread::spawn` in either path. The bounds
on Handler are forward-looking (for when panes spawn their own threads), which is fine,
but the asymmetry should be documented. Or add the same bounds to the closure for
consistency.

**m4. `connection.rs` and `mock.rs` are pub modules.**

`connection::Connection`, `connection::MockConnection`, `connection::test_pair()` are
all accessible as `pane_app::connection::*`. These are internal plumbing and test
infrastructure. Should be `pub(crate)` or gated behind a feature flag.

---

## Previous review issues — disposition

| P# | Issue | Status |
|----|-------|--------|
| P1 | Test is noisy | **Open.** Test still has ~20 eprintln lines. Not blocking. |
| P2 | connect vs connect_test naming | **Open.** Still the same. Fine for now. |
| P3 | `impl Into<Option<Tag>>` too clever | **Fixed.** Two explicit methods. |
| P4 | Handler can't send messages | **Fixed.** PaneProxy passed to all methods. |
| P5 | Closure can't send messages | **Fixed.** Closure takes `&PaneProxy`. |
| P6 | LIFO pending_creates | **Fixed.** VecDeque with pop_front. |
| P7 | Spin loop in wait() | **Open.** Still 50ms polling (M3 above). |
| P9 | No inter-pane messaging | **Partially addressed.** PaneProxy solves handler-to-own-pane. Cross-pane remains future work. |
| P10 | "Command" overloaded | **Open.** Not blocking. |
| P11 | Stubs re-exported as public API | **Open.** routing/scripting still public (m4 touches this). |
| P12 | looper should be pub(crate) | **Open.** Still pub. |
| P13 | Dual SessionError paths | **Open.** Not blocking. |
| P14 | No source() on errors | **Open.** Not blocking. |

---

## Verdict

The three critical fixes from the first review are applied. The PaneProxy design is
correct — it's the right translation of BMessenger. The VecDeque fix is clean. The
split into `create_pane`/`create_component_pane` is the right call.

One compile error (C1: missing PaneProxy::new) needs fixing before anything else.
After that, the moderate issues are quality-of-life improvements that can be batched:
remove the duplicated methods on Pane, fix the stale doc example, address the dead
mock parameter.

The API is ready for continued development against. The bones are right. The ownership
model is sound. The mapping from Be concepts to Rust types is precise where it should
be precise and deliberately different where BeOS got it wrong (App-not-a-looper,
exhaustive enums over what-codes, consuming self over Lock/Unlock).

**Priority order for next pass:**

1. Fix PaneProxy::new compile error (C1) — one line
2. Fix lib.rs Quick Start (M1) — one line
3. Remove duplicated set_* from Pane (M2) — deletion
4. Add Send + 'static to closure or document the asymmetry (m3)
5. Make connection/mock pub(crate) (m4)
