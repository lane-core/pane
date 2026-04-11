---
name: Be API naming philosophy and patterns
description: Be's naming conventions verified from Haiku headers — verb-object mutators, bare getters, Count+At collections, Is-predicates, past-participle hooks, Set/Get asymmetry
type: reference
---

Be's API naming patterns, verified against Haiku headers and Be Newsletter archives:

- **Bare getter, Set-prefixed setter:** `Name()` / `SetName()`, not `GetName()`. Exception: `GetInfo()`, `GetSupportedSuites()` — when filling output parameters.
- **Verb-Object for mutators:** `AddHandler()`, `RemoveHandler()`, `PostMessage()`
- **Count+At for indexed collections:** `CountHandlers()` + `HandlerAt(index)` — consistent across all kits (Window, View, Looper)
- **Is-predicates:** `IsEmpty()`, `IsLocked()`, `IsLaunching()`
- **Past-participle notification hooks:** `FrameResized()`, `WindowActivated()`, `WorkspacesChanged()`
- **Imperative commands:** `Quit()`, `Show()`, `Hide()`, `Zoom()`
- **B_ prefix on constants from AppDefs.h:** four-char codes, namespace disambiguation (`B_QUIT_REQUESTED = '_QRQ'`)

Key files: `headers/os/app/Looper.h`, `Handler.h`, `Application.h`, `Message.h`, `AppDefs.h`, `interface/Window.h`, `interface/View.h`

Newsletter Issue 2-15 (Roy West): UI text style guide (title capitalization, ellipsis on panel-opening actions). Issue 2-16 (Ming Low): DR9 type changes (long→int32, status_t, const correctness). Issue 1-1 (Ringewald): design philosophy — "elegant" inherited from Mac heritage, applied to API aesthetics.

No formal written API naming style guide found in newsletters — consistency was maintained by small team review.
