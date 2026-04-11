---
type: policy
status: current
supersedes: [pane/beapi_translation_rules, auto-memory/reference_beapi_translation_rules]
sources: [pane/beapi_translation_rules, auto-memory/reference_beapi_translation_rules]
verified_against: [docs/architecture.md@2026-04-05, docs/naming-conventions.md]
created: 2026-04-05
last_updated: 2026-04-10
importance: high
keywords: [beapi, translation_rules, vertical_integration, namespace, dynamic_static, virtual_overrides, lifecycle, error_handling, observers]
related: [policy/beapi_naming_policy, reference/haiku/beapi_divergences, reference/haiku/_hub]
agents: [be-systems-engineer, pane-architect]
---

# BeAPI → pane Translation Rules

Apply in order. Given a Be concept, these rules determine the
pane equivalent.

## Rule 0: Vertical integration

If it exists because Be controlled kernel / graphics server
directly, solve the underlying problem with pane's tools (Wayland,
Linux VFS, s6).

- `BView::Draw()` → content is bytes sent to compositor
- `BWindow::ConvertToScreen()` → Wayland doesn't expose global coords
- `BLooper::Lock() / Unlock()` → Rust's `&mut self` eliminates this

## Rule 1: Namespace

Drop `B` prefix. Crate path replaces it.

- `BMessage` → `pane_app::Message`
- `BMessenger` → `pane_app::Messenger`

## Rule 2: Configure-and-attach → method on host

If Be had a standalone type you construct then pass to an owner,
make it a method on the owner.

- `new BMessageFilter(...)` → `pane.add_filter(impl Filter)`
- `BWindow::AddShortcut(key, mod, msg)` → `pane.add_shortcut(combo, cmd, args)`
- `new BMessageRunner(target, msg, interval)` → `messenger.set_pulse_rate(interval)`

**Exception:** If the type has significant runtime behavior, it
survives as a trait (`Filter`).

## Rule 3: Dynamic → static (protocol) + filesystem (scripting)

- Protocol messages → typed enum variants (`Message::Key(KeyEvent)`)
- Scripting / inspection → filesystem at `/pane/<id>/attrs/`
- `BMessage::FindString("title")` → `cat /pane/1/attrs/title`

## Rule 4: Virtual overrides → trait with defaults

- `BHandler::MessageReceived(BMessage*)` → split into per-variant
  Handler methods
- All methods have defaults
- Compiler catches missing variants via `Handles<P>` exhaustive
  match

## Rule 5: Lifecycle — consumed by run()

- `new BWindow(...)` long-lived → `Pane` consumed by `run()`
- Communication during run → `Messenger` (Clone + Send)
- `PaneBuilder` consumed by `run_with` — setup is a distinct
  phase from the event loop

## Rule 6: Error handling

- `status_t` / `InitCheck()` → `Result<T, E>`, fail at construction
- Three error channels (Protocol, Control, Crash) replace flat
  `status_t`. See `docs/architecture.md` "Error channels" table.

## Rule 7: Observers

- **Death watching:** `Handler::pane_exited(pane, reason)`. How to
  register interest is deferred.
- **Conversation-level failure:** Dispatch entries resolve to
  `on_failed` when peer drops `ReplyPort`.
- **Property changes:** pane-fs at `/pane/<id>/attrs/` (deferred).

## Rule 8: System services

Separate crates / services, not globals. The compositor IS the
registry (no separate registrar process).

## Naming

**Full guide:** `docs/naming-conventions.md`

CamelCase → snake_case. Message variants match handler methods:
`Message::CloseRequested` ↔ `Handler::close_requested()`.

Method patterns (adapted from Be, convergent with Rust):

- **Getters:** bare name — `name()`, `id()`
- **Setters:** `set_` prefix — `set_content()`, `set_pulse_rate()`
- **Predicates:** `is_` prefix — `is_locked()`, `is_hidden()`
- **Mutating ops:** verb + object — `add_handler()`, `remove_handler()`
- **Notification hooks:** past-participle — `activated()`, `resized()`, `close_requested()`
- **Commands:** imperative — `quit()`, `show()`, `hide()`
- **Builders:** bare names per Rust convention — `Tag::new("Title").command(cmd(...))`
