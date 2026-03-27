# UX Review: The Modal Command Surface Tag Line

Review of the redesigned tag line concept from the perspective of BeOS's
interaction design philosophy. The reviewer shipped code in BeOS R3
through R5, worked on the app_server team, and knows the yellow tab
from the inside.

---

## 1. The BeOS Tab: What It Got Right

The BeOS window tab was one of the most successful pieces of desktop
chrome ever designed. It was a small, asymmetric, colored rectangle
sitting on the border of the window -- usually the top border, but it
could be left-titled too (the `kLeftTitledWindowLook` in the Haiku
decorator code). It contained exactly three things: the title, a close
button, and a zoom button. That's it.

What made it work:

**Identity at a glance.** The yellow tab was your beacon. In a screen
full of overlapping windows, the tab was how you found your window.
The color was unique in the GUI landscape -- not the grey of Windows,
not the candy of Mac OS 9's platinum. It was a warm, saturated signal
that said "this is a Be window." The focused tab and the unfocused
tab had different color intensities (look at `fFocusTabColor` vs
`fNonFocusTabColor` in Haiku's `TabDecorator.h`) -- you could tell
which window was active from across the room.

**Grabbability.** The tab was a grab handle. Drag the tab, move the
window. This is so fundamental that people forget it's a design
choice. The tab's shape -- smaller than a full title bar -- meant
windows could overlap and you could still reach the tab of a window
underneath. Macintosh title bars were the full width of the window;
they overlapped each other. Be's tab was compact enough to peek out
from behind another window.

**Minimal commitment.** The tab asked nothing of you. It didn't
demand interaction. It was there when you needed it (to move, to
close, to identify) and invisible when you didn't. It never
competed with the content for your attention. The gradient fill
(the linear gradient from `COLOR_TAB_LIGHT` to `COLOR_TAB` that
you can see in DefaultDecorator's `_DrawTab`) gave it depth and
presence without weight.

**The title was just a title.** It wasn't a button, it wasn't
editable, it wasn't a command surface. It was a label. This
matters. The cognitive load of a tab was near zero. You didn't
have to think about what would happen if you clicked on the text
vs. the button vs. the background. Click on the tab = grab the
window. Click the close widget = close. Click the zoom widget =
zoom. Three things, three actions, zero ambiguity.

What would the Be engineers think of making the tab also a command
input surface? Honestly -- they'd be nervous. The tab's power was
its simplicity. Adding an input surface to the tab changes its
fundamental nature from passive chrome to active interface. That
said, the Be engineers were pragmatists, not dogmatists. If you
could demonstrate that the tab remains simple at rest and only
becomes complex on invocation, they'd listen. But they'd want to
see it first.

---

## 2. Discoverability vs. Cleanliness

The acme tag line was always visible because Plan 9 had a
specific design philosophy: text IS the interface, nothing is
hidden, everything is manipulable. This was coherent within
Plan 9's world. It also meant every window had a line of inscrutable
command text across the top that newcomers found bewildering.

The new design hides commands until invoked. The trade-off is
real, but it's the right trade-off for pane, and here's why.

**How BeOS handled discoverability.** BeOS used three mechanisms:

1. **Menus.** Every application had a menu bar (BMenuBar) with
   hierarchical menus (BMenu) containing items (BMenuItem). Menus
   were the primary discovery mechanism. You didn't need to know
   keyboard shortcuts -- you opened the menu and read the options.
   The shortcut key was printed right there next to the menu item.
   Menus were persistent (always in the menu bar), discoverable
   (open them and browse), and self-documenting (labels + shortcuts).

2. **Keyboard shortcuts.** CUA-standard (Ctrl+C, Ctrl+V, Ctrl+S)
   plus Be-specific (Alt used as the shortcut modifier, which was
   actually the Command key on Be keyboards). The shortcut was the
   accelerator for something you'd already discovered through the
   menu. Experts used shortcuts; newcomers used menus. Same commands,
   two access paths.

3. **Context menus.** Right-click (or Control-click) popped up
   context-appropriate menus (BPopUpMenu). This was the secondary
   discovery path -- when you didn't know what you could do, you
   right-clicked and found out.

4. **Tooltips and Deskbar.** Minimal, but present. The Deskbar
   was the system-level discovery surface -- running apps, workspace
   switcher, system tray. Not per-window discoverability, but
   system discoverability.

The key insight: **BeOS's discoverability was layered.** The menu
was always there as a safety net. Shortcuts were the fast path.
Context menus were the "what can I do here?" path. You never
had to memorize anything -- you could always fall back to the
menu.

**The problem with the new design.** The modal command surface
has no menu-bar equivalent. There is no persistent visible surface
that tells you commands exist. The user must know that `:` (or
whatever the activation key is) opens the command surface. This
is the vim problem: `:` is incredibly powerful, but you have to
know it exists.

**The mitigation is completion.** The designer is right that
completion-driven discovery can substitute for menus. But it
requires the user to cross the initial chasm: they must invoke
the command surface at least once. After that first invocation,
completion teaches them what's available.

**My recommendation:** The activation mechanism needs a visible
affordance. Not a full acme tag line of commands, but something
that signals "there is more here." Consider:

- A small indicator glyph in the tab (when floating) or at the
  edge of the pane (when tiled) that signals "command surface
  available." A subtle `>_` or `:` glyph. Click it to activate.
  This is the menu bar equivalent -- a persistent visual cue that
  commands exist -- without the visual weight of acme's command text.

- On first focus of a pane that has commands, a brief hint overlay
  (like which-key's initial display) that fades after 2 seconds.
  First-run teaching, not permanent chrome.

- The Deskbar equivalent (system panel, notification area) should
  mention the command surface in its own help/welcome flow.

The worst outcome is a user who doesn't know the command surface
exists and thinks pane is a dumb window manager with no
application-level commands. BeOS never had this problem because
the menu bar was always visible.

---

## 3. The Hierarchical Tag Line

Commands at the container level vs. the leaf pane level. This is
genuinely new territory. BeOS didn't have it. In BeOS, windows
were independent objects. There was no concept of a "split
container" with its own command vocabulary.

But tiling window managers have taught us that container operations
are real. i3/sway users constantly perform operations on containers:
change split direction, move a container, resize splits, tabify
a group. These are operations on the *structure*, not on the content.

**Is this a genuine innovation?** Yes, conditionally.

The conditions under which container commands are useful:

- **Layout manipulation.** "Change this split from horizontal to
  vertical." "Equalize all panes in this container." "Add a pane
  to this group." These are real operations that i3 users perform
  daily.

- **Bulk operations.** "Close all panes in this container." "Move
  this container to another workspace." "Save the layout of this
  container as a preset." These make sense at the group level.

- **Context propagation.** "Set the working directory for all panes
  in this container." "Apply this color tag to this group."

The conditions under which it adds unnecessary complexity:

- If the user can't easily tell *which level they're commanding.*
  i3 has this problem -- `focus parent` / `focus child` is
  confusing. The hierarchical tag line must make the command target
  visually obvious. When you activate the container's command
  surface, the entire container should highlight. When you activate
  a leaf pane's command surface, only that pane highlights.

- If the command vocabularies at different levels overlap ambiguously.
  "Close" at the container level closes the container (and all
  children). "Close" at the pane level closes one pane. The user
  must not be confused about which "close" they're invoking.

**My recommendation:** This is worth doing, but the activation must
make the scope crystal clear. Visual feedback -- border highlighting,
a scope indicator in the command surface itself ("container: ..." vs
"pane: ...") -- is essential. And the default activation should
target the leaf pane, not the container. Container-level commands
should require an explicit "go up" gesture (like i3's `focus parent`
but less confusing -- maybe a modifier key during activation).

---

## 4. The Modal Concern

This is the question that matters most. The Be engineers cared
deeply about modelessness.

From the Be Newsletter Issue 4-22 (one of our last major UX
articles):

> "Not all questions and interactions deserve a user's undivided
> attention. One of the most frustrating things about software
> is when it limits you needlessly."

> "If modality is used just to call attention to something, I
> think that color and positioning are preferable to modal blocking."

From Issue 2-6, in a developer Q&A about when dialogs should be
modal:

> "I would take the position that a dialog should be modeless
> unless the interaction HAS to be executed before it is meaningful
> for other activities to continue."

The Be team's anti-modality stance was rooted in responsiveness.
BeOS's per-window threading meant that one window could never
block another. System-modal dialogs were anathema. The busy cursor
was rejected in favor of in-place status indicators precisely
because the busy cursor was a form of system-wide modality (you
can't interact normally while it's showing).

**But here's the nuance.** The Be engineers weren't against all
modes. They were against modes that *block the rest of the system.*
They were against modes that *persist invisibly.* They were against
modes that *exist because the programmer was too lazy to handle
the concurrent case.*

The modal command surface in this design has different properties:

1. **It doesn't block the rest of the system.** Other panes continue
   to function. The compositor continues to respond. Only the
   invoking pane enters the command state. This is per-pane modality,
   which is no worse than a text input field having focus.

2. **It's visible.** When the command surface is active, you can
   see it. The tab is expanded or the overlay is showing. There is
   no hidden mode. Compare this to vim, where the only indication
   of mode is a status line label (or Insert mode's cursor shape
   change). The pane command surface is as visible as a search
   bar in a web browser.

3. **It's transient.** Press Escape, it's gone. It has the
   temporal profile of a dialog box, but without the blocking
   semantics. This is closer to a popup menu (which BeOS used
   heavily via BPopUpMenu) than to a modal dialog.

4. **It exists because the user explicitly requested it.** The
   user pressed `:` or clicked the indicator. They chose to enter
   command mode. This is opt-in modality, like opening a menu.

**My verdict:** This is acceptable modality. It passes the Be
test on every count. It doesn't block, it's visible, it's
transient, and it's user-initiated. The Be engineers would
recognize this pattern -- it's structurally identical to the
BPopUpMenu interaction: invoke, choose, dismiss. The difference
is that the command surface accepts typed input rather than
pointer clicks.

There is one thing to get right: **Escape must ALWAYS dismiss.**
No exceptions, no confirmation dialogs, no "are you sure."
The command surface must vanish instantly on Escape. This is the
transient promise. If you break it, you've built a modal dialog.

---

## 5. Completion as Discovery

The designer wants completion to be the primary way users discover
commands. This is the telescope/fzf/which-key pattern transported
to the window chrome.

**How this compares to BeOS's approach:**

BeOS used menus for discovery. Menus have properties that completion
partially replicates and partially lacks:

| Property | Menus | Completion |
|---|---|---|
| Browse without knowing what you want | Yes (scan the menu) | Partial (type nothing, see all?) |
| Hierarchical organization | Yes (submenus) | Partial (categories as prefixes?) |
| Keyboard shortcuts visible | Yes (printed in menu) | Depends on implementation |
| Works without typing | Yes (click) | No (must type or at least invoke) |
| Muscle memory formation | Positional (menu item is always in the same place) | Nominal (you remember the command name) |
| Cognitive load to browse | Low (read a list) | Low-medium (type, scan results) |

Completion is better than menus at one thing: **fuzzy discovery.**
If you half-remember a command ("something about split..."), you
type "split" and completion shows you everything relevant. With
menus, you have to scan through hierarchies hoping the command is
categorized the way you'd expect. Completion finds commands by
substring, by intent, by association. This is genuinely better
for power users.

Completion is worse than menus at one thing: **zero-knowledge
browsing.** A new user who doesn't know any command names can't
type anything useful. With menus, they open "File" and see "Save."
With completion, they type... what? The empty string? Does that
show all commands?

**My recommendation:** The completion surface should have a
browsable mode. When activated with no input (just `:` followed
by nothing), it should show a categorized list of all available
commands -- essentially a menu rendered as a completion list.
This is what VS Code's command palette does, and it's the right
synthesis. Type to narrow, or browse to explore.

Additionally, the completion entries should show keyboard shortcuts
alongside command names, exactly as BeOS menus did. This teaches
the fast path while providing the slow path. "close-pane (Alt+W)"
tells the user they can skip the command surface next time.

**For whom is this better?** For intermediate and advanced users,
completion-as-discovery is strictly better than menus. For absolute
beginners, it's slightly worse (requires knowing the activation
key, requires typing). The browsable-mode mitigation closes most
of that gap. The remaining gap is the initial activation discovery
problem addressed in section 2.

---

## 6. The Floating vs. Tiled Visual Difference

Floating windows get an expanding tab. Tiled panes get an overlay.
Two visual presentations of the same interaction.

**Is this a problem?**

BeOS had multiple window looks. Look at the `window_look` enum
in Haiku's `Window.h`:

```c
enum window_look {
    B_BORDERED_WINDOW_LOOK       = 20,
    B_NO_BORDER_WINDOW_LOOK      = 19,
    B_TITLED_WINDOW_LOOK         = 1,
    B_DOCUMENT_WINDOW_LOOK       = 11,
    B_MODAL_WINDOW_LOOK          = 3,
    B_FLOATING_WINDOW_LOOK       = 7
};
```

A `B_TITLED_WINDOW_LOOK` had the full tab + borders.
A `B_FLOATING_WINDOW_LOOK` had a smaller tab. A `B_MODAL_WINDOW_LOOK`
had no tab at all. A `B_BORDERED_WINDOW_LOOK` had a thin border.
Different visual presentations for different window types. The
interaction was appropriate to the context.

So no, different visual presentations for floating vs. tiled is
not inherently a problem. It's contextually appropriate. A floating
window has a tab already -- expanding it is natural. A tiled pane
has no tab to expand -- it needs an alternative.

**The risk is interaction inconsistency.** The visual presentation
can differ, but the interaction semantics must be identical:

- Same activation gesture (`:` or click)
- Same completion behavior
- Same dismiss behavior (Escape, Enter)
- Same command vocabulary for the same pane type

If the user doesn't have to think about "am I floating or tiled"
when using the command surface -- if the behavior is identical and
only the chrome differs -- this is fine. The user learns one
interaction model and it works everywhere. The visual presentation
adapts to the context.

**The one thing to watch:** The overlay position in tiled mode
(top/bottom/center, configurable) introduces a choice that the
floating mode doesn't have. If the user configures it to appear
at the bottom of the pane, it mirrors vim's command line. If they
configure it at the top, it's more like a search bar. If at center,
it's like Spotlight. This variability is fine for personal preference,
but the default should be consistent with the floating case: top of
the pane, because that's where the tab would be. Spatial consistency
between modes reduces cognitive load.

---

## 7. What Would Gassee Think?

Gassee's test was always: does this *invite* interaction? He talked
about the BeOS feeling "responsive" and "fun." He rejected the busy
cursor not on philosophical grounds but on experiential ones -- it
made the system feel slow. He wanted software that made you want to
click on things.

His quote from Newsletter Issue 4-10 is relevant: "you know you've
done something right when your platform is perverted -- programmers
use your product in ways you hadn't thought of."

The command surface design has the right shape for this. It's a
platform primitive that pane developers fill with their own
vocabulary. It's extensible by design. A file manager will have
different commands than a text editor, which will have different
commands than a media player. Developers can put interesting,
surprising, powerful things in the command surface, and users will
discover them through completion. This is fertile ground for the
kind of creative abuse Gassee loved.

But -- and this is important -- **the design as described doesn't
quite have warmth.** The aesthetic spec commits to "controls that
look like controls -- affordances are visible" and "structure is
always visible." A command surface that is invisible until invoked
is, by definition, not a visible affordance.

At rest, the floating window is fine -- the BeOS-style tab is warm,
identifiable, present. But the tiled pane at rest has nothing. No
tab, no chrome indicator, nothing that says "I'm here, I can do
things." The content fills the space, which is clean, but it's
also anonymous. Where's the personality?

**Concrete suggestions for warmth:**

1. **Tiled panes should have a minimal chrome strip.** Not a full
   tab, but a thin bar (3-4px, the border width) at the top with
   the pane name in a condensed form. This serves as the identity
   element that the floating tab provides. When the command surface
   activates, this strip expands into the overlay. This is the
   equivalent of the BeOS border treatment -- structure is always
   visible.

2. **The command surface should animate.** The tab expanding
   (floating) or the overlay appearing (tiled) should be a
   smooth, fast animation -- 150ms, ease-out. Not instantaneous
   (feels mechanical) and not slow (feels sluggish). BeOS didn't
   animate much (hardware limitations), but the Frutiger Aero
   aesthetic that pane commits to is a system that would have
   had animation if it could.

3. **The completion dropdown should have visual personality.**
   Not a flat list of strings. Entries with icons where appropriate,
   type indicators (built-in vs. user-defined vs. routing action),
   and the warm color palette. Completion should feel like browsing
   a lovingly designed menu, not like grepping a command list.

4. **Typing in the command surface should feel immediate.**
   Zero perceived latency between keystroke and completion update.
   This is the responsiveness test that BeOS applied to everything.
   If the command surface feels sluggish, the whole interaction
   fails. The per-pane threading model helps here -- completion
   computation runs on the pane's thread, not the compositor's.

---

## Summary Assessment

The redesign is sound. Moving from acme's persistent tag line to
a modal command surface is the right call for a system that wants
to be approachable. Acme's model is coherent within Plan 9 but
alien to every mainstream desktop convention. The modal command
surface is familiar -- it's the pattern of VS Code's command palette,
Spotlight, Rofi, dmenu, vim's `:`. Users of modern tools already
know this interaction.

The design preserves what matters from the original tag line concept:
context-specific commands, developer-defined vocabulary, text-as-interface.
It discards what didn't map well: permanent visibility of command
text, the assumption that users will middle-click executable words.

**What the design gets right:**

- Opt-in for developers (panes without commands just work)
- Context-specific vocabulary (the developer defines it)
- Hierarchical scope (container vs. leaf commands)
- Completion as the discovery mechanism
- Per-pane modality that doesn't block the system
- Two visual modes appropriate to their contexts

**What needs attention:**

- **Activation discovery.** There must be a visible affordance
  that tells users the command surface exists. A glyph, an
  indicator, a first-run hint. Something.

- **Tiled pane identity.** Tiled panes need minimal chrome at
  rest. A thin name strip. Without it, they lose identity and
  warmth.

- **Empty-query browsability.** Activating the command surface
  with no input should show a browsable, categorized list of
  all commands. This is the menu-bar safety net.

- **Shortcut visibility in completions.** Completion entries
  should show keyboard shortcuts alongside names, teaching
  the fast path while providing the slow path.

- **Scope clarity for hierarchical commands.** Visual feedback
  must make command scope unambiguous. Border highlighting,
  scope labels in the command surface.

- **Animation and polish.** The transition between dormant and
  active should be smooth. This is where warmth lives.

None of these are fundamental objections. They're refinements
of a design that has the right structure. The Be team would
recognize the heritage and approve of the evolution.

---

## Sources

- Haiku source: `src/servers/app/decorator/DefaultDecorator.cpp` (tab rendering, gradient fill, frame colors)
- Haiku source: `src/servers/app/decorator/TabDecorator.h` (tab color enums, component structure)
- Haiku source: `src/servers/app/decorator/Decorator.h` (Tab struct, window_look enum usage)
- Haiku source: `headers/os/interface/Window.h` (window_look enum: B_TITLED, B_FLOATING, B_MODAL, etc.)
- Be Newsletter Issue 4-22: Modality ("Not all questions deserve a user's undivided attention"), feedback philosophy, consistency
- Be Newsletter Issue 2-6: Modal dialog Q&A ("modeless unless the interaction HAS to be executed")
- Be Newsletter Issue 4-10: Gassee on platform perversion and expressive power
- Be Newsletter Issue 1-2: Schillings on programming model clarity
- Architecture spec: tag line as "editable text that serves as title, command bar, and menu simultaneously"
- Aesthetic spec: "controls that look like controls -- affordances are visible"
- Input Kit research: discoverability analysis (which-key, acme tag line, completion menus, progressive disclosure)
