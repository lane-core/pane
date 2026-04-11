---
type: reference
status: current
sources: [.claude/agent-memory/be-systems-engineer/reference_be_naming_philosophy]
created: 2026-04-10
last_updated: 2026-04-10
importance: normal
keywords: [be_naming, naming_philosophy, getter, setter, verb_object, predicate, past_participle, B_prefix]
related: [reference/haiku/_hub, policy/beapi_naming_policy]
agents: [be-systems-engineer, pane-architect]
---

# Be API naming philosophy

Be's naming patterns, verified against Haiku headers and Be
Newsletter archives.

## Patterns

- **Bare getter, Set-prefixed setter:** `Name()` / `SetName()`,
  not `GetName()`. Exception: `GetInfo()`,
  `GetSupportedSuites()` — when filling output parameters.
- **Verb-Object for mutators:** `AddHandler()`, `RemoveHandler()`,
  `PostMessage()`
- **Count + At for indexed collections:** `CountHandlers()` +
  `HandlerAt(index)` — consistent across all kits (Window, View,
  Looper)
- **Is-predicates:** `IsEmpty()`, `IsLocked()`, `IsLaunching()`
- **Past-participle notification hooks:** `FrameResized()`,
  `WindowActivated()`, `WorkspacesChanged()`
- **Imperative commands:** `Quit()`, `Show()`, `Hide()`, `Zoom()`
- **`B_` prefix on constants from `AppDefs.h`:** four-char codes,
  namespace disambiguation (`B_QUIT_REQUESTED = '_QRQ'`)

## Source files

`headers/os/app/Looper.h`, `Handler.h`, `Application.h`,
`Message.h`, `AppDefs.h`, `interface/Window.h`, `interface/View.h`

## Newsletter context

- **Issue 2-15** (Roy West): UI text style guide (title
  capitalization, ellipsis on panel-opening actions)
- **Issue 2-16** (Ming Low): DR9 type changes (`long → int32`,
  `status_t`, const correctness)
- **Issue 1-1** (Ringewald): design philosophy — "elegant"
  inherited from Mac heritage, applied to API aesthetics

No formal written API naming style guide found in newsletters —
consistency was maintained by small team review.

## Application to pane

See `policy/beapi_naming_policy` for the three-tier rule (faithful
adaptation → justified divergence → Rust idiom).
