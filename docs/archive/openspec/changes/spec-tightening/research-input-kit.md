# Input Kit Research — Keybinding Models and Generalization

Research for pane spec-tightening. Primary sources: Vim documentation and `:help`, Kakoune "why kakoune" design document (kakoune.org), Emacs Lisp Reference Manual (keymaps chapter), Hydra/Transient package documentation, Pike's acme and sam papers, Pike's "Structural Regular Expressions," Helix editor documentation, i3 User's Guide (binding modes), Be Book (Input Server), Cocoa Text System documentation (Apple), evil-mode and evil-collection source documentation.

Sources:

- Vim grammar: <https://learnvim.irian.to/basics/vim_grammar/>
- Vim intro/modes: <https://vimdoc.sourceforge.net/htmldoc/intro.html>
- Kakoune rationale: <https://kakoune.org/why-kakoune/why-kakoune.html>
- Emacs keymaps: <https://www.masteringemacs.org/article/mastering-key-bindings-emacs>
- Emacs keymap basics (GNU): <https://www.gnu.org/software/emacs/manual/html_node/elisp/Keymap-Basics.html>
- Emacs local keymaps: <https://www.gnu.org/software/emacs/manual/html_node/emacs/Local-Keymaps.html>
- Persistent prefix keymaps: <https://karthinks.com/software/persistent-prefix-keymaps-in-emacs/>
- Consistent structural editing: <https://karthinks.com/software/a-consistent-structural-editing-interface/>
- Hydra: <https://github.com/abo-abo/hydra>
- Sam/structural regexps: <https://doc.cat-v.org/bell_labs/structural_regexps/se.pdf>
- Acme paper: <https://plan9.io/sys/doc/acme/acme.html>
- Helix docs: <https://docs.helix-editor.com/usage.html>
- i3 user guide: <https://i3wm.org/docs/userguide.html>
- BeOS Input Server: <https://www.haiku-os.org/legacy-docs/bebook/TheInputServer_Introduction.html>
- Cocoa key bindings: <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/TextDefaultsBindings/TextDefaultsBindings.html>
- IBM CUA: <https://en.wikipedia.org/wiki/IBM_Common_User_Access>
- Vim keybindings everywhere: <https://github.com/erikw/vim-keybindings-everywhere-the-ultimate-list>
- vim-textobj-user: <https://github.com/kana/vim-textobj-user>
- evil-mode keymaps: <https://evil.readthedocs.io/en/latest/keymaps.html>

---

## 1. Vim's Modal Model

### The mode system

Vim is a modal editor. The meaning of a keystroke depends on which mode is active. The primary modes:

**Normal mode** is the default and the mode where the user spends most of their time. It is for navigation and manipulation of existing text. Keys are commands, not characters. `d` means "delete," `w` means "next word," `j` means "down."

**Insert mode** is for entering new text. Entered via `i`, `a`, `o`, and other insertion commands. Exited via `Esc` back to Normal. In Insert mode, keys produce characters — the editor becomes a conventional text input surface.

**Visual mode** is for selection. The user moves the cursor and the region between the initial position and the current position is highlighted. Visual mode has sub-modes: character-wise (`v`), line-wise (`V`), and block-wise (`Ctrl-V`). Once a region is selected, an operator acts on it.

**Command-line mode** (`:`, `/`, `?`) is for entering ex commands, search patterns, and filter expressions. A one-line text input at the bottom of the screen. This is where the full ex language lives.

**Operator-pending mode** is the transient state after typing an operator (like `d`) and before providing the motion or text object that completes it. This mode is invisible to casual users but is the structural hinge of vim's grammar — it is the pause between verb and noun.

**Replace mode** (`R`) is a minor variant of insert: typed characters overwrite rather than insert.

The critical insight: Normal mode is not "command mode" in contrast to "editing mode." Normal mode IS the editing mode. Text entry (Insert mode) is the special case. This inversion of the conventional model — where typing characters is the default and commands are accessed through modifiers — is what gives vim its power and its learning curve.

### The grammar: verb-object composition

Vim's command language has the structure of a composable grammar. The basic form is:

```
[count] operator [count] motion/text-object
```

**Operators** are verbs — actions to perform on text:
- `d` — delete
- `c` — change (delete and enter insert mode)
- `y` — yank (copy)
- `>` — indent
- `gU` — uppercase
- `gu` — lowercase
- `!` — filter through external command
- `=` — auto-indent

**Motions** are movements that define a range from the cursor's current position:
- `w` — to next word start
- `e` — to word end
- `b` — to previous word start
- `$` — to end of line
- `}` — to next paragraph
- `f{char}` — to next occurrence of {char}
- `/pattern` — to next match of pattern

**Text objects** define regions relative to cursor position, independent of where the cursor sits within the object:
- `iw` — inner word (the word itself)
- `aw` — a word (word plus surrounding whitespace)
- `i(` — inside parentheses
- `a{` — a brace block (including the braces)
- `it` — inside XML tag
- `is` — inner sentence
- `ip` — inner paragraph

The composition is algebraic. Given N operators and M motions/objects, you have N*M combinations. Learning a new operator multiplies your capabilities by the number of motions you already know, and vice versa. `d3w` = delete 3 words. `ciw` = change inner word. `ya}` = yank a brace block. `gUiw` = uppercase inner word. None of these are special-cased. They all fall out from the grammar.

**Count** is a multiplier: `3dw` and `d3w` both delete three words. Counts compose with both operators and motions.

**The dot command** (`.`) repeats the last change. Because changes are composable commands (operator + motion/text-object), `.` effectively replays a structured edit. This transforms one-off compositions into repeatable operations — the user constructs a transform, then applies it repeatedly with a single keystroke.

### Extensibility of the grammar

The grammar is open to extension on both axes — new operators and new text objects:

**Custom operators** are defined via `operatorfunc`. A plugin defines a function that receives a motion type (characterwise, linewise, blockwise) and operates on the text between `'[` and `']` marks. Once defined, the custom operator composes with every existing motion and text object. The `vim-commentary` plugin defines a "comment" operator (`gc`): `gcip` comments a paragraph, `gcj` comments two lines, `gc3w` comments three words. One definition, infinite compositions.

**Custom text objects** are defined via `onoremap` (operator-pending mode mapping) and `xnoremap` (visual mode mapping). The `vim-textobj-user` plugin provides a declarative framework: define a text object by specifying how to find its boundaries (via regex or function), and it automatically works with every operator. The `targets.vim` plugin extends the built-in text objects with separator-based objects (e.g., `i,` for content between commas), next/last modifiers, and seeking behavior.

This extensibility is the strongest evidence that vim's grammar is genuinely algebraic: it admits new terms on both sides of the composition, and the new terms compose with everything that already exists.

### Why modality is powerful

Modality gives every key on the keyboard a command meaning without requiring modifier keys. In Normal mode, `d` is delete, `w` is word, `f` is find — no Ctrl, no Alt, no chord. This means the command vocabulary is vast (52 case-sensitive letters plus symbols and digits) while each command requires exactly one keystroke. The entire keyboard becomes a command surface.

The deeper point: modality separates the concerns of navigation, manipulation, and text entry. In a modeless editor, every key must serve double duty — it is either a character or, when combined with a modifier, a command. Modifier chords are inherently limited (Ctrl gives ~26 bindings, Ctrl+Shift ~26 more) and ergonomically costly (simultaneous key presses requiring hand contortions). Modality trades temporal context (knowing which mode you're in) for spatial efficiency (every key is a command).

### The ergonomic costs

Mode confusion is real. A user who doesn't know they're in Insert mode types commands that produce garbage text. A user who doesn't know they're in Normal mode types text that executes as commands. The escape to Normal mode (`Esc`, reaching for the corner of the keyboard) is the most frequent mode transition, and it's ergonomically awkward — hence the universal practice of remapping it (to `jk`, `Caps Lock`, etc.).

The learning curve is brutal. The grammar is powerful once internalized but opaque to newcomers. There is no visual affordance telling you what mode you're in (beyond a status line indicator), what commands are available, or what an operator is waiting for. The system is maximally powerful and minimally discoverable.

The mental model requirement is high: the user must maintain a running awareness of the current mode, the pending operator (if any), the count prefix (if any), and the register (if any). This is cognitive overhead that experienced users internalize to the point of unconsciousness, but the internalization period is steep.

---

## 2. Emacs's Chord Model

### Prefix keys and key sequences

Emacs does not use modes in the vim sense. Instead, it uses **key sequences** — multi-key combinations where some keys are **prefix keys** that open up further binding namespaces.

A key sequence like `C-x C-f` is two steps: `C-x` is a prefix key bound to a keymap (not a command), and `C-f` is looked up within that keymap to resolve to the `find-file` command. Prefix keys can nest: `C-x r` is a prefix for register/bookmark commands, and `C-x r t` is "string-rectangle."

The key prefixes in common use:
- `C-x` — file, buffer, window operations
- `C-c` — mode-specific commands (reserved by convention for major/minor modes)
- `C-h` — help
- `M-x` — command by name (the universal escape hatch)
- `C-x r` — registers and bookmarks
- `C-x 4` — other-window operations
- `C-x 5` — other-frame operations

This is a hierarchical namespace system. The prefix key acts as a directory; keys within it act as files. The user navigates a tree of bindings.

### The keymap hierarchy

Emacs resolves key bindings by searching a stack of keymaps in a defined order. The lookup chain (simplified, and roughly in order of precedence):

1. **`overriding-terminal-local-map`** — terminal-specific overrides (rarely used)
2. **`overriding-local-map`** — unconditional override (used by special modes, e.g., during isearch)
3. **Minor mode keymaps** (`minor-mode-map-alist`) — each enabled minor mode contributes a keymap
4. **Major mode keymap** (`current-local-map`) — the keymap for the buffer's current major mode
5. **Global keymap** (`current-global-map`) — the default bindings

The critical property: **minor modes shadow major modes, which shadow globals.** This creates composable layers. A minor mode can add, override, or intercept bindings without modifying the major mode or global keymaps. The user can stack multiple minor modes, each contributing its own bindings, and the first match wins.

Every major mode has exactly one keymap. A buffer has exactly one major mode. But a buffer can have an unlimited number of minor modes active simultaneously. Minor modes are the composition mechanism — they are how behavior is layered.

### Minor modes as composable layers

Minor modes are emacs's most powerful keybinding mechanism because they compose orthogonally with major modes. Consider:

- `flycheck-mode` adds `C-c !` prefix bindings for error navigation in any major mode
- `company-mode` adds completion bindings in any major mode
- `evil-mode` adds an entire modal editing layer on top of any major mode

Each minor mode is self-contained — it provides its own keymap and activates/deactivates as a unit. The user's effective keybinding environment is the composition of all active minor modes with the major mode and global map. This is genuine composability: the total is more than the sum of the parts, and new combinations work without explicit coordination.

The cost: conflicts. When two minor modes bind the same key, the one with higher priority in `minor-mode-map-alist` wins, and the other is silently shadowed. There is no conflict detection, no composition operator — it's last-writer-wins by list position. In practice this is manageable because well-designed minor modes use distinct prefix namespaces, but it's a real problem when modal systems (evil-mode) interact with mode-specific bindings.

### Hydra and transient: ephemeral modal states

Emacs's chord model has a discoverability and efficiency problem: complex operations require long key sequences, and the user must remember which prefix leads where. Two packages address this by introducing **ephemeral modality** within the chord model:

**Hydra** creates temporary modal states. You define a "hydra" — a group of related commands bound to single keys — and activate it via a prefix. Once active, the hydra's keys are live: you can invoke commands in rapid succession without re-typing the prefix. The hydra displays a hint showing available keys. Colors encode exit behavior: red heads keep the hydra active, blue heads exit after execution. The implementation uses Emacs's `set-transient-map` — a temporary keymap overlay that intercepts input until explicitly dismissed.

**Transient** (from the Magit project) provides structured menus: a grid of options displayed at the bottom of the screen with single-key invocation. Transient menus can have state (toggling flags before executing a command), hierarchy (sub-menus), and persistence. This is the interface Magit uses for git operations: `C-c g` opens the Magit dispatch, then `c` opens commit options, where you can toggle `--amend` or `--no-verify` before pressing `c` again to commit.

**which-key** takes a different approach: it is purely observational. After you type a prefix key and pause, which-key displays all available completions. It doesn't change behavior — it surfaces what's already there. The user types `C-x` and waits; which-key shows every binding under `C-x`. This is discoverability without modality.

The convergence is notable: all three mechanisms are responses to the same problem — the chord model's namespace is deep and opaque, and users need help navigating it. Hydra and Transient add ephemeral modality. which-key adds visibility. The emacs ecosystem discovered that pure chords need modal supplementation to remain usable at scale.

### Why chords work and where they break

Chords avoid mode confusion entirely. The user is always in the same "mode" — the meaning of a key is determined by whether it follows a prefix, not by ambient state. This is cognitively simpler in one specific way: the user never needs to track "where am I?"

But chords have ergonomic costs that scale with depth:
- **Modifier strain.** Ctrl and Meta require awkward hand positions. Emacs pinky (RSI from Ctrl usage) is a recognized occupational hazard.
- **Sequence length.** Powerful commands often require 3-5 keystrokes with modifiers: `C-c C-x C-l` is not unusual. Each keystroke requires a simultaneous chord.
- **Memorization.** The namespace is enormous and organized by convention rather than structure. `C-x` is global, `C-c` is mode-specific — but this is a guideline, not a mechanism.
- **Limited vocabulary per level.** A chord modifier gives ~26 bindings per modifier. Even with two modifiers (Ctrl, Meta), the total per-level vocabulary is maybe 60-70 practical bindings. This is why the hierarchy goes deep.

The fundamental trade-off: vim gets a vast command vocabulary per keystroke by trading temporal context (mode). Emacs gets modeless operation by trading per-keystroke vocabulary (only commands accessible via chords).

---

## 3. Alternative Models

### Kakoune/Helix: selection-first

Kakoune inverts vim's grammar. Where vim says "verb then object" (`dw` = delete word), Kakoune says "object then verb" (`wd` = select word, then delete). This is not merely aesthetic — it has deep consequences:

**Visual feedback before commitment.** After selecting, the user sees exactly what will be affected before applying the operator. In vim, `d3w` deletes three words sight-unseen; the user must predict the effect. In Kakoune, `3w` selects three words (highlighted), and only then does `d` delete them. Errors are caught before they happen.

**Unification of movement and selection.** In vim, movement and selection are distinct concepts — `w` moves in Normal mode but selects in Visual mode, and operators implicitly create a selection. In Kakoune, every motion is a selection. Moving IS selecting. There is no Visual mode because Normal mode already operates on selections.

**Multiple selections as a natural consequence.** Because everything operates on selections, multiple selections become a first-class feature. `%` selects the entire buffer, `s` keeps only the subselections matching a regex within the current selection, and then `d` deletes all of them. This is structural regular expressions applied to editing interaction — Pike's `x` command as a selection primitive.

**Simpler operator set.** Vim needs `x` (delete character) separate from `d` (delete motion) because the operator-motion grammar needs the motion to define scope. Kakoune only needs `d` — it deletes whatever is selected, period. Each command does one thing. Complexity emerges from composition of individually simple commands.

**Helix** follows the same selection-first model and adds tree-sitter integration: text objects are defined by the syntax tree, not by regex or delimiter matching. `mif` selects the inner function, `mic` selects the inner class — defined by tree-sitter queries, so they understand the actual code structure. This points toward a future where the grammar's nouns are semantically aware.

The cost of selection-first: an extra keystroke for simple operations. Vim's `dw` is two keystrokes; Kakoune's `wd` is also two, but `dd` (delete line) in vim is two keystrokes while Kakoune's equivalent `xd` is also two. The difference is in predictability and feedback, not in keystroke count.

### Sam/acme: structural operations and text-as-interface

Sam and acme represent a fundamentally different approach from either vim or emacs. They are not keyboard-first editors — they are **structural editors** where the primary interaction model is mouse-driven, with a command language for programmatic operations.

**Sam's structural regular expressions** are the key conceptual contribution. Pike: "the use of regular expressions to describe the structure of a piece of text rather than its contents." The `x` command extracts all matches of a pattern within the current selection, then runs a command on each match. The `y` command is the complement — it runs a command on the text between matches. These compose:

```
x/pattern/ command     — for each match of pattern, run command
y/pattern/ command     — for each non-match interval, run command
g/pattern/ command     — if selection matches pattern, run command
```

This is selection-oriented rather than line-oriented. Selections are contiguous strings of text — they may span multiple lines or be sub-line. The `x` and `y` commands are iterators over structure, and they nest. `x/\n/ { ... }` iterates over lines. `x/[^ ]+/` iterates over words. The structure is whatever the pattern says it is.

The philosophical point: in traditional Unix tools (ed, sed, awk), the line is a built-in structural primitive. Pike argues this is an arbitrary choice — why should the newline character have special status? Structural regular expressions let the user define what the structural units are. The pattern IS the structure.

**Acme's tag line model** is the other key contribution. There are no menus, no keyboard shortcuts (almost), no key bindings. Instead:
- B1 (left click) selects text
- B2 (middle click) executes text as a command
- B3 (right click) opens/searches text as a name
- B1-B2 chord: cut
- B1-B3 chord: paste

The tag line at the top of each window contains editable command text. Want a new command? Type it in the tag. The tag IS the menu, and it's mutable. The key insight: the command vocabulary is visible, editable, and contextual. You can see what commands are available because they're written in the tag. You can add commands by typing. You can remove them by deleting.

This is radical discoverability through a different axis: not "the system shows you what keys do" but "the commands are literal text on the screen." No keybindings to discover because there are almost no keybindings.

Acme's interaction model is relevant to pane not as a keybinding system but as a complementary philosophy: text as interface, visible commands, mouse-driven composition. Pane's tag line commitment descends directly from this.

### CUA: the mainstream modeless standard

IBM's Common User Access (1987) established the keybinding conventions that became universal in GUI applications: Ctrl+C (copy), Ctrl+V (paste), Ctrl+X (cut), Ctrl+Z (undo), Ctrl+S (save), Ctrl+O (open). Historically, the original CUA standard used Shift+Del for cut, Ctrl+Ins for copy, and Shift+Ins for paste — the Ctrl+C/V/X mappings came from Apple's Macintosh and were adopted by Windows, eventually displacing IBM's own standard.

CUA is a fixed vocabulary: each modifier+key combination is a specific command. There is no composition, no grammar, no multiplication. Learning Ctrl+C and Ctrl+V gives you exactly two operations. The vocabulary scales linearly with memorization, not multiplicatively with composition. The per-application command set is typically 20-40 bindings.

CUA's strength is its uniformity — the same bindings work in every application — and its zero learning curve for basic operations. Its weakness is that it provides no path to power: there is no way to compose CUA bindings into more complex operations, no way to extend the vocabulary, and the modifier-key space is small (roughly 60 practical bindings with Ctrl and Ctrl+Shift).

For pane, CUA represents the floor. Any keybinding system must at minimum accommodate CUA-accustomed users, but the design aspiration is far beyond CUA's expressive range.

---

## 4. The Common Structure

Across all these models, there are recurring structural elements. The differences are in how they're arranged, not in what they are.

### Four universal components

**1. A vocabulary of actions (verbs/operators).** Delete, yank, change, indent, comment, filter, execute. Every system has them. In vim they're operators. In emacs they're commands. In kakoune they're the same operators, applied after selection. In acme they're tag line text or mouse buttons. The set is extensible in all systems.

**2. A vocabulary of objects (nouns/motions/selections).** Word, line, paragraph, sentence, brace block, function, file. Every system needs to address regions of content. In vim they're motions and text objects. In emacs they're mark-and-point or region commands. In kakoune they're selections. In sam they're addresses/structural regex matches. In acme they're mouse-swept text.

**3. A composition mechanism that combines verbs with objects.** Vim's operator-pending mode. Kakoune's selection-then-action. Emacs's command that implicitly operates on the region. Sam's command language. The grammar that connects actions to targets.

**4. A scoping mechanism that determines which bindings are active.** Vim's modes. Emacs's keymap hierarchy. i3's binding modes. Kakoune's modes (Normal, Insert, Prompt). Some kind of context that determines which vocabulary is available at any given moment.

### Two axes of variation

**Composition order: verb-first vs. object-first.**
- Vim: verb then object (`dw`). Efficient, but blind — you don't see the effect until after.
- Kakoune/Helix: object then verb (`wd`). One extra step, but visual confirmation before commitment.
- Sam: verb wraps object (`x/pattern/ d`). The command language makes the composition explicit.

**Binding resolution: modal vs. hierarchical vs. textual.**
- Vim: mode determines the active vocabulary. Switching mode changes the entire keymap.
- Emacs: keymap stack determines precedence. All keymaps are potentially active; conflicts resolved by priority.
- Acme: visible text IS the binding. The "keymap" is the content of the tag line.
- i3: explicit named modes, switched by command, one active at a time — vim-style modality applied to window management.

### The spectrum from modality to hierarchy

These are not discrete alternatives. They form a spectrum:

**Pure modality** (vim): One keymap active at a time. Mode switch replaces the entire context. Maximum keys-per-action, maximum context dependence.

**Hierarchical layering** (emacs): Multiple keymaps active simultaneously, resolved by priority. Minor modes compose as layers. Context accumulates rather than switches.

**Ephemeral modality within hierarchy** (hydra/transient): Temporary modes that activate on demand and dismiss when done. The user is normally in the hierarchical model but can enter a brief modal state for rapid command sequences. This is the synthesis: hierarchical by default, modal when it helps.

**Textual** (acme): No keymaps at all. Commands are visible text. The "binding" is the act of clicking on text. Infinitely extensible (type any text), maximally discoverable (you can see the commands), but limited to mouse-accessible interfaces.

The most expressive systems tend toward the middle of this spectrum. Evil-mode (vim bindings in emacs) is modal editing within a hierarchical keymap system. Hydra and Transient are ephemeral modality within a hierarchical system. Kakoune is modality with visual feedback that reduces the cost of mode confusion. The pure extremes — full modality with no discoverability, or full hierarchy with no modality — both have serious usability costs that the hybrid approaches mitigate.

---

## 5. Generalizing Beyond Text Editing

### What does vim-style grammar mean in a file manager?

The existing ecosystem gives clear evidence that vim's grammar generalizes. Ranger, vifm, and lf are file managers with vim-style navigation (`hjkl`, `/` for search, `gg`/`G` for top/bottom). But most stop at navigation — they borrow vim's motions without its compositional grammar.

What would the full grammar look like? The objects would be: file, directory, selection, marked set, match set. The operators: delete, move, copy, rename, open, compress, chmod. The motions: next file, previous file, parent directory, child directory, next match. Counts: `d3j` = delete the next 3 files. Text objects: `di/` = delete inner directory (contents without the directory itself). `ya/` = yank a directory (contents plus the directory).

This is not hypothetical. Ranger already supports `dd` (cut), `yy` (yank), `pp` (paste) — but these are hardcoded operations, not compositions from a grammar. The grammar would mean that any new operator automatically works with all existing motions, and any new motion works with all existing operators.

### What does it mean for a process monitor?

Objects: process, process group, user's processes, all processes matching a pattern. Operators: kill, nice, strace, suspend, resume. Motions: next process, previous process, next process by CPU usage, parent process, child processes. `k3j` = kill the next 3 processes. `sip` = suspend inner process group. The grammar provides a uniform interaction vocabulary.

### What about widget-based interfaces?

Widget panes (settings panels, notification lists, form-based interfaces) introduce a new category of object: the widget. A list widget has items. A form has fields. A tree has nodes. The objects are the structural elements of the widget, and the operators are the actions that make sense for those elements.

The key question: can the grammar bridge text-based and widget-based panes? The answer is yes, if the Input Kit defines the grammar abstractly — verb + object — and each pane type registers the objects and operators that make sense for its content. A text pane registers words, lines, paragraphs, and text objects. A file manager pane registers files, directories, and selections. A widget pane registers its widget elements.

The operators have natural analogues across domains:
- `d` (delete): delete text / delete file / delete list item / dismiss notification
- `y` (yank): copy text / copy file path / copy list item
- `o` (open): open line below / open file / expand tree node
- `/` (search): search text / filter files / filter list items

The motions have natural analogues:
- `j`/`k`: next/prev line / next/prev file / next/prev item
- `w`: next word / next file / next widget
- `gg`/`G`: top/bottom of buffer / top/bottom of list / first/last item
- `{`/`}`: next/prev paragraph / next/prev directory / next/prev group

### The abstraction

The grammar generalizes when we recognize that "verb + object" is not specific to text. It is a general interaction grammar where:
- **Verbs** are actions meaningful in a given context
- **Objects** are addressable units in a given content type
- **Motions** are navigation across objects
- **Counts** multiply motions
- **The dot command** repeats the last verb+object combination

A pane registers its content type, which determines which objects and motions are available. The Input Kit provides the grammar engine — mode management, operator-pending states, motion resolution, count handling, dot-repeat — and the pane provides the vocabulary. The kit is the syntax; the pane is the semantics.

---

## 6. System-Wide Keybinding as a Kit Concern

### What would an Input Kit look like?

The Input Kit is a keybinding framework that provides:

**1. A grammar engine.** The mechanism for composing verbs with objects. This includes:
- Mode management (Normal, Insert, and any pane-defined modes)
- Operator-pending state (waiting for a motion after a verb)
- Motion resolution (converting keystrokes to object addresses)
- Count accumulation
- Repeat (dot) command

**2. A keymap hierarchy.** Layered binding resolution:
- System-wide bindings (compositor-level: workspace switching, pane management, system commands)
- Kit-level bindings (common to all panes using the Input Kit: navigation, standard operators)
- Content-type bindings (specific to the pane's content: text objects for text panes, file objects for file managers)
- Pane-local bindings (specific to a particular pane instance)

This mirrors emacs's global → major-mode → minor-mode → local hierarchy, but structured around pane's content-type system rather than emacs's mode system.

**3. An object registration mechanism.** Each pane type registers:
- The objects its content supports (words, files, widgets, etc.)
- The motions for navigating between objects
- The operators that make sense for its content
- Any content-specific modes

**4. Discoverability infrastructure.** which-key-style display of available bindings after a prefix or mode switch. Visible in the tag line, in a popup, or in a dedicated help pane. The system knows what bindings are active at any moment and can present them.

### The conflict problem

System-wide keybinding creates a conflict space that per-application keybinding does not. When the compositor has `Super+j` for "focus next pane" and a text pane has `j` for "move down," there is no conflict — they operate in different scopes. But when a text pane defines `Ctrl+w` for "delete word" and the compositor defines `Ctrl+w` for "close pane," the user's intent is ambiguous.

The resolution strategies, from the ecosystem:

**Scope separation** (sway/i3): compositor bindings use a modifier (Super) that applications never see. Application bindings use unmodified keys and standard modifiers (Ctrl, Alt). The two namespaces don't overlap.

**Priority hierarchy** (emacs): compositor bindings are checked first; if not matched, the event passes to the focused pane. This is essentially the overriding-local-map pattern.

**Mode separation** (vim/i3): compositor bindings only apply in a compositor mode. Application bindings only apply when a pane has focus and is in its own mode. The compositor mode is distinct from any pane mode.

Pane's approach should combine these. The Input Kit's keymap hierarchy naturally provides the resolution: system-wide bindings (compositor scope) take precedence, then kit-level bindings, then content-type bindings, then pane-local bindings. The compositor claims a modifier prefix (e.g., Super) for its own namespace. The grammar engine operates within the pane's scope after compositor bindings have been resolved. This is structurally identical to emacs's `overriding-terminal-local-map` → `minor-mode-map-alist` → `current-local-map` → `current-global-map` chain, translated to a compositor/kit/pane architecture.

### Relationship to BeOS's Input Server

BeOS's Input Server provides a pipeline model for input events: device add-ons generate events, filter add-ons intercept and modify them, and input method add-ons transform keyboard input into complex character sets. The pipeline is: **device → filters → method → app_server → application.**

The Input Server is about event transformation in a pipeline — taking raw hardware events and producing semantic input events. It is NOT a keybinding system. It is the layer below keybinding: it produces the key events that a keybinding system would then interpret.

Pane's architecture already handles event pipeline concerns through the compositor's input dispatch (libinput integration, xkbcommon for keyboard layout). The Input Kit operates above this layer — it receives key events and interprets them through the grammar engine and keymap hierarchy. The Input Server's filter concept is relevant, though, as a model for system-wide input transformation: a filter add-on that transforms key events before they reach any pane is the mechanism for system-wide keybinding at the event level.

### Relationship to i3/sway's binding modes

i3's binding modes are vim-style modality applied at the window manager level. The default mode contains normal keybindings. Named modes (e.g., `resize`) contain specialized bindings. Switching to a mode replaces the active bindings. A binding within a mode can switch back to default.

```
mode "resize" {
    bindsym h resize shrink width 10 px
    bindsym l resize grow width 10 px
    bindsym Escape mode "default"
}
bindsym $mod+r mode "resize"
```

This is directly relevant. Pane's compositor-level bindings should support named modes: a default mode for normal operation, a resize mode for pane resizing, a layout mode for rearranging, a launcher mode for application selection. The pattern generalizes: any set of related compositor operations can be grouped into a named mode with single-key bindings, entered via a prefix and exited via Escape.

The insight from i3: named modes at the compositor level are the same concept as vim's modes, hydra's transient keymaps, and emacs's transient-map — ephemeral contexts where a specialized vocabulary is temporarily available. The Input Kit should provide this mechanism uniformly across all levels: compositor modes, pane modes, and content-type modes are all instances of the same abstraction.

### How does this compose with agents?

An agent is a system participant. If an agent is operating a pane on behalf of a user, it sends the same key events (or, more precisely, the same semantic commands) that a user would. The agent doesn't need special keybinding infrastructure — it can invoke the same operators and motions programmatically via the protocol.

But there's a deeper question: can an agent define keybindings? Should it? Consider a code review agent that adds custom operators: `ga` for "approve hunk," `gr` for "request revision." These would be registered as custom operators for the pane type the agent is operating on, discovered via the same mechanism as any other extension.

This works naturally if the Input Kit's object registration is filesystem-based. An agent could drop keybinding definitions into the appropriate directory (just as routing rules and translators are dropped into directories), and the pane would gain the vocabulary. The extension surface is the same surface agents use for everything else.

---

## 7. Discoverability

### The discoverability problem

Power and discoverability are in tension. Vim's grammar is maximally powerful and minimally discoverable. CUA is maximally discoverable (Ctrl+C is written in every Edit menu) and minimally powerful. The design challenge is: can you have both?

### Strategies from the ecosystem

**which-key (emacs, neovim):** After typing a prefix and pausing, display all available completions. This is passive discoverability — the system shows you what's there when you ask (by pausing). It doesn't change behavior; it surfaces the existing structure. Neovim's which-key.nvim adds descriptions to keymaps, turning the display from a list of keys into a list of annotated actions.

**Hydra/Transient hint display (emacs):** Active discoverability — when entering an ephemeral mode, display the available commands as a menu. The user can read the menu and choose, or type from memory. The menu is generated from the keymap definition, so it's always accurate.

**Kakoune's auto-info boxes:** After typing certain prefix keys (e.g., `g` for goto), Kakoune displays a context menu showing available next keys and their effects. This is which-key applied at the individual prefix level.

**Acme's tag line:** Radical discoverability through visibility — the commands are literal text on screen. There is nothing to discover because nothing is hidden. The cost is that only a limited set of commands can be visible at once (the tag line is one line).

**Completion menus (kakoune, emacs):** When typing in command mode, display completions with fuzzy matching. The user doesn't need to remember the exact command name; they type a substring and the system narrows options.

### The tag line as visible vocabulary

Pane already commits to acme-style tag lines. This provides a natural discoverability surface for the Input Kit. The tag line can display:

- The current mode name (when not in default mode)
- Available commands for the current context
- A condensed which-key-style display after prefix input

The tag line is editable, so users can customize which commands are visible — their own "favorites bar" of commands. The tag line is also executable: B2-clicking a command in the tag line invokes it. This bridges the discoverable (visible text) with the efficient (keyboard shortcuts): the tag line shows what's available; the keyboard shortcut invokes it faster.

### Progressive disclosure

The design goal is a system that is approachable on first contact and reveals depth as the user grows. The Input Kit can achieve this through layered disclosure:

1. **Default mode with CUA bindings:** New users find familiar Ctrl+C/V/X/Z bindings. The tag line shows available commands as clickable text. The mouse works for everything. This is the zero-knowledge floor.

2. **which-key discovery:** Users who pause after a prefix see available options. They discover the compositional grammar gradually by seeing that `d` waits for a motion, that `w` and `e` and `$` are all valid completions. The system teaches itself.

3. **Modal efficiency:** Users who learn the Normal/Insert distinction gain access to the full grammar. The single-keystroke command vocabulary becomes available. The dot command enables repetition. This is where the power curve goes exponential.

4. **Custom objects and operators:** Power users extend the grammar with pane-type-specific or personal bindings. Agents add vocabulary on their behalf. The system grows with the user.

This progression mirrors how vim users actually learn: they start in Insert mode doing CUA-style editing, gradually learn Normal mode navigation, then discover operators, then discover text objects, then discover the dot command, then discover macros. The Input Kit can make this progression explicit and supported rather than accidental and unguided.

---

## 8. Synthesis: What This Means for Pane

### The core abstraction

The Input Kit provides a **generalized interaction grammar** — a composable system of verbs, objects, motions, counts, and modes that works uniformly across all pane types. The grammar is the kit; the vocabulary is the pane.

This is the same relationship as the Application Kit to application behavior, or the Interface Kit to visual rendering. The Input Kit does not define what keys do — it defines the structure within which keys have meaning. It provides mode management, operator-pending states, keymap layering, conflict resolution, discoverability display, and repeat/macro infrastructure. Individual panes fill in the vocabulary: what objects exist, what operators apply, what motions navigate.

### The compositional bet

Pane's foundations document commits to "input kits provided universal keybinding mechanisms of the same strength and power as those found in cult favorite text editor interfaces, generalized over multiple interfaces." This research clarifies what that means concretely:

**The grammar must be genuinely compositional.** N operators × M objects = N*M interactions. New operators compose with existing objects. New objects compose with existing operators. The dot command repeats compound operations. This is the specific property that makes vim-grade keybinding qualitatively different from CUA-grade keybinding, and it generalizes beyond text when the objects and operators are defined abstractly.

**The modes must be first-class.** Named modes at every level — compositor, pane type, pane instance. Modes are entered and exited explicitly. Ephemeral modes (hydra-style) for rapid command sequences. Mode transitions are visible (tag line, status indicator). The mode mechanism is provided by the kit; the mode definitions are provided by the pane.

**The keymap hierarchy must compose.** System → kit → content-type → local, with clean precedence rules and no silent shadowing. Conflicts are reportable. Extensions at any level compose with bindings at every other level.

**Discoverability must be architectural.** which-key-style display is not a plugin — it is a kit feature. The kit knows what bindings are active at every moment and can present them on demand. The tag line participates in discoverability. New users can click; experienced users can type. The same information is accessible through both channels.

### What this does NOT mean

The Input Kit is not a reimplementation of vim. It is not evil-mode for a desktop environment. The grammar engine is inspired by vim's compositional structure, but the vocabulary is not vim's vocabulary. A file manager pane does not need `ciw` — it needs operators and objects that make sense for files. A notification pane does not need Visual mode — it needs selection and action appropriate to notifications.

The point is not to make everything feel like vim. The point is to give every pane the same structural advantages that vim gives text editing: a compositional grammar where learning compounds multiplicatively, where new vocabulary composes with existing vocabulary, where repetition is first-class, and where the full power of the keyboard is available without modifier-key gymnastics. Whether the user's mental model is "this is like vim" or "this is pane's own thing" is a presentation question, not an architectural one.

### Open questions

1. **Default mode identity.** Should the default mode for new panes be Insert-like (CUA, immediate text input) or Normal-like (navigation/command)? Vim's choice of Normal-as-default is one of its most polarizing decisions. For a general desktop environment, Insert-as-default with explicit Normal mode entry (via `Esc` or a configurable key) is likely the right call — it matches user expectations from every other application. But the Normal mode vocabulary must be available and discoverable, not hidden.

2. **Selection-first or verb-first?** Kakoune's argument for selection-first is strong: visual feedback before commitment reduces errors and makes the system more approachable. But vim's verb-first is more efficient for experienced users who know what they want. The Input Kit could support both — verb-first composition in Normal mode (vim-style) with visual selection as an alternative path (Visual mode). Or it could commit to one model. This is a design decision with strong arguments on both sides.

3. **Mouse integration.** Acme's mouse model shows that keyboard and mouse interaction can be complementary rather than redundant. The tag line provides a mouse-accessible command surface. Can the grammar engine integrate mouse gestures — B2-click as "execute," B3-click as "route" — alongside keyboard operators? Should mouse and keyboard compose (select with mouse, operate with keyboard)?

4. **Agent-defined vocabulary.** If agents can extend the grammar by dropping definitions into directories, how do those extensions compose with user-defined extensions? What happens when two agents define conflicting bindings? The filesystem-based extension model needs a conflict resolution strategy.

5. **Cross-pane operations.** The grammar as described operates within a single pane. But pane's compositional nature suggests cross-pane operations: "yank from this pane, paste into that pane" is natural. "Delete 3 panes" at the compositor level uses the same grammar. How does the Input Kit's grammar engine scale from intra-pane to inter-pane operations?
