# Naming Conventions

How to coin identifiers in pane's public API. Companion to
`kit-documentation-style.md` (how to *document* what you name).

---

## Principles

Pane's API sits on a spectrum between faithful BeOS adaptation and
idiomatic Rust. Three tiers, applied in order:

1. **Faithful adaptation** (default). Use the Be name, drop the `B`
   prefix, apply Rust case conventions. `BHandler` → `Handler`.
   `PostMessage` → `post_message`. This is the starting point for
   every identifier.

2. **Justified divergence**. When the pane concept is architecturally
   different from Be's, coin a new name. `BWindow` → `Pane` (because
   a pane is the universal surface, not a desktop window).
   `BApplication` → `App` (widespread contemporary convention).
   Every divergence requires an entry in the divergences tracker
   (`pane/beapi_divergences`).

3. **Rust idiom**. When Rust has an established convention that Be
   lacked — because the language lacked it — use the Rust way.
   Iterators instead of Count+At. `Result<T, E>` instead of
   `status_t`. `From`/`Into` instead of conversion functions.
   Builder patterns instead of multi-parameter constructors.

"Sounds better" is not a reason for tier 2. The Be API was
battle-tested. Faithful adoption enforces sane design and lets
developers who know BeOS find their footing immediately.

---

## Decision Tree

When naming a new type, method, or constant:

```
1. Does Be/Haiku have a name for this concept?
   ├─ YES → Use it (tier 1). Drop B prefix, apply Rust case.
   │        ├─ Is the pane concept architecturally different?
   │        │   └─ YES → Coin a new name (tier 2). Document in divergences.
   │        └─ Does Rust have a standard pattern that replaces it?
   │            └─ YES → Use the Rust pattern (tier 3).
   └─ NO → Is this genuinely novel?
           └─ Name descriptively. Prefer vocabulary already
              established in pane. No divergence entry needed
              (there's nothing to diverge from).
```

When in doubt, check the Haiku headers (`reference/haiku-book/`)
for what Be called it. Ask "what did Be call this?" before every
new identifier.

---

## Type Names

- **Drop the `B` prefix.** The crate path is the namespace:
  `pane_app::Message`, not `BMessage`. A prefix in Rust actively
  fights the module system.

- **CamelCase** per Rust convention. `MessageFilter`, not
  `message_filter` or `BMessageFilter`.

- **Traits named for the role**, not the capability grammar.
  `Handler`, `MessageFilter` — not `Handleable`, `Filterable`.
  Be named classes for what they *are*; traits should too.

- **Enums named for the domain.** `Message`, `FilterAction`,
  `ExitReason`. The enum name scopes its variants; no prefix
  needed on the variants themselves.

### Examples from the codebase

| Be | pane | Tier | Rationale |
|----|------|------|-----------|
| `BMessage` | `Message` | 1 | Faithful |
| `BHandler` | `Handler` | 1 | Faithful (trait, not class) |
| `BMessenger` | `Messenger` | 1 | Faithful |
| `BLooper` | (absorbed into `Pane`) | 3 | Rust ownership makes the looper an implementation detail |
| `BApplication` | `App` | 2 | Contemporary convention (gtk, winit) |
| `BWindow` | `Pane` | 2 | Architecturally different — universal surface, not window |
| `BMessageRunner` | `TimerToken` | 2 | Configure-and-attach → method on host (translation rule 2) |
| `filter_result` | `FilterAction` | 2 | More descriptive enum name |
| `property_info` | `PropertyInfo` | 1 | Faithful adaptation. Carries operations, specifier forms, value type (see optics-design-brief.md). |

---

## Method Names

Be's method naming was principled but never formally documented.
The patterns below are recovered from the Haiku headers and
confirmed by Be engineering culture. Most converge naturally
with Rust convention.

### Getters — bare name

The common case. Be used bare getters because you read state more
often than you write it. Rust agrees.

```rust
fn name(&self) -> &str;
fn id(&self) -> PaneId;
fn handler_count(&self) -> usize;
fn is_hidden(&self) -> bool;
```

Reserve `get_` for methods that perform complex extraction or fill
output parameters — not simple property access. If the getter
returns a reference or copy of a field, it should be bare.

### Setters — `set_` prefix

```rust
fn set_name(&mut self, name: &str);
fn set_pulse_rate(&mut self, rate: Duration);
fn set_hidden(&self, hidden: bool);
```

Be's `SetTitle()` / `Title()` pattern maps directly to
`set_title()` / `title()` in Rust. No translation friction.

### Predicates — `is_` prefix

```rust
fn is_locked(&self) -> bool;
fn is_hidden(&self) -> bool;
fn is_valid(&self) -> bool;
```

Be used `IsFoo()`. Rust uses `is_foo()`. Direct mapping.

### Mutating operations — verb + object

```rust
fn add_handler(&mut self, handler: Handler) -> HandlerId;
fn remove_handler(&mut self, id: HandlerId) -> Option<Handler>;
fn add_filter(&mut self, filter: impl MessageFilter);
fn add_shortcut(&mut self, combo: &str, command: impl Into<String>);
```

Be's `AddHandler()` / `RemoveHandler()` maps directly.

### Notification hooks — past participle

Called when something *already happened*. Name reflects the
completed event.

```rust
fn activated(&mut self, proxy: &Messenger) -> Result<bool>;
fn deactivated(&mut self, proxy: &Messenger) -> Result<bool>;
fn resized(&mut self, proxy: &Messenger, geometry: PaneGeometry) -> Result<bool>;
fn close_requested(&mut self, proxy: &Messenger) -> Result<bool>;
fn key(&mut self, proxy: &Messenger, event: KeyEvent) -> Result<bool>;
```

Be used `WindowActivated()`, `FrameResized()`, `QuitRequested()`.
Pane keeps the past-participle convention, adapted:
- `WindowActivated(bool)` splits into `activated()` / `deactivated()`
  (Rust tagged unions are better than bool flags)
- `FrameResized()` becomes `resized()` (Wayland has no frame)
- `QuitRequested()` becomes `close_requested()` (unified with
  `Message::CloseRequested`)

### Commands — imperative

```rust
fn quit(&self);
fn show(&self);
fn hide(&self);
```

Direct actions on an object. Be's `Quit()`, `Show()`, `Hide()`.

Note: pane merged `Show()`/`Hide()` into `set_hidden(bool)` on
`Messenger` — a tier 2 divergence. The imperative pattern still
applies where individual commands make sense.

### Collections — iterators over Count+At

Be's `CountHandlers()` + `HandlerAt(index)` existed because 1996
C++ didn't have a good iteration story. The *purpose* was
iteration. Express the purpose:

```rust
fn handlers(&self) -> impl Iterator<Item = &Handler>
```

When the index has semantic meaning (z-order, tab order), provide
direct access:

```rust
fn handler_count(&self) -> usize;
fn handler(&self, index: usize) -> Option<&Handler>;
```

Don't provide both `handlers().len()` and `handler_count()`. Pick
one path.

### Message variant ↔ handler method correspondence

Every `Message` variant has a corresponding `Handler` method.
The variant is CamelCase; the method is the snake_case equivalent:

| Variant | Handler method |
|---------|---------------|
| `Message::CloseRequested` | `close_requested()` |
| `Message::Activated` | `activated()` |
| `Message::Key(event)` | `key()` |
| `Message::Pulse` | `pulse()` |
| `Message::PaneExited { .. }` | `pane_exited()` |

This correspondence is mechanical. If you add a `Message` variant,
the handler method name follows automatically.

---

## Enum Variants

- **Message variants**: noun or past-participle matching the handler
  method. `CloseRequested`, `Activated`, `Resize`, `Key`.
  No prefix — the enum name is the namespace.

- **Error variants**: descriptive noun. `Disconnected`,
  `InvalidGeometry`, `HandlerExit`.

- **Action/result variants**: verb or adjective. `FilterAction::Pass`,
  `FilterAction::Consume`.

Be's `B_QUIT_REQUESTED`, `B_PULSE`, etc. become typed enum variants.
The four-char codes were clever for hex dumps, but typed variants
are strictly superior: type-safe, exhaustive, scoped, with typed
payloads.

---

## Module and Crate Names

Crate names in code follow `pane-{kit}` convention:

| Kit (docs/conversation) | Crate (code) | Be Kit |
|------------------------|-------------|---------|
| Application Kit | `pane-app` | Application Kit |
| Notification Kit | `pane-notify` | (new; replaces `StartWatching`) |
| Optics | `pane-optic` | (new; composable state accessors) |
| Protocol | `pane-proto` | (new; wire format) |
| Session Types | `pane-session` | (new; handshake protocol) |

"Kit" is valuable vocabulary — it implies a curated, designed
collection with intentional relationships. Use it in documentation
and conversation: "the Application Kit (`pane-app`)", not just
"the pane-app crate."

---

## Divergence Protocol

When you deviate from a Be name:

1. Record the divergence in serena memory `pane/beapi_divergences`:
   Be name, pane name, rationale.

2. Add a `# BeOS` section to the type or method's doc comment
   (see `kit-documentation-style.md` for format).

3. Valid reasons for divergence:
   - The concept is architecturally different in pane
   - A widely established contemporary convention justifies it
   - Rust's type system makes the Be pattern unnecessary
     (e.g., iterators replacing Count+At)

4. Invalid reasons:
   - "Sounds better"
   - "More modern"
   - "I didn't check what Be called it"

Early design decisions are not immutable. Rename now while the
cost is zero.

---

## What Be Would Have Changed

Context for why tier 3 (Rust idiom) wins in specific cases. These
are patterns that Be engineers adopted out of C++ necessity, not
design conviction. Porting to a language with traits, ownership,
and algebraic types, they would have changed these eagerly:

| Be pattern | Rust replacement | Why Be did it that way |
|-----------|-----------------|----------------------|
| `status_t` return codes | `Result<T, E>` | C++ had no standard error type |
| `InitCheck()` two-phase init | Fail at construction | C++ constructors can't return errors |
| `CountFoos()` + `FooAt(i)` | Iterators | C++ had no standard iteration protocol |
| `BArchivable` | serde | No serialization framework existed |
| `BMessage` dynamic fields | Typed enum variants | No algebraic data types |
| `AddInt32`/`FindInt32`/... | Enum payloads | No generics, no sum types |
| `SetNextHandler()` chain | Trait with default methods | Single-dispatch workaround |

**What they would have kept sacred:**

- One looper, one thread (design principle, not language artifact)
- Messaging as the primary inter-component pattern
- Set/Get naming convention (already convergent with Rust)
- Kit structure and vocabulary
- Scripting as first-class
