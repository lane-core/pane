## ADDED Requirements

### Requirement: Textual interface layer
pane-shell is a library and a program. As a library (`pane-shell-lib`), it provides terminal emulation that other pane clients can build on. As a program, it is the default textual interface to the system — a shell session with pane's semantic interface, routing, and extension model.

Terminal emulation (VT parsing, screen buffers, PTY management) uses existing libraries (vte, alacritty_terminal, or equivalent). pane-shell's value is the semantic layer above the terminal, not the terminal itself.

**Crate**: `pane-shell` (binary), `pane-shell-lib` (library)

#### Scenario: Standalone shell pane
- **WHEN** a user opens a shell pane
- **THEN** they interact with commands, output, and a working directory — a terminal integrated with pane's tag line, routing, and execution

#### Scenario: Library reuse
- **WHEN** a developer builds a "git pane" using pane-shell-lib
- **THEN** they get terminal emulation for free and add git-specific semantics (branch in tag, commit routing, staged files interface)

### Requirement: Semantic interface
A shell pane exposes its interface at the semantic level of what it does:

- **Tag line**: working directory as name, shell commands as actions
- **Text interaction**: B1 select, B2 execute, B3 route — on any visible text
- **Filesystem** (`/srv/pane/<id>/`): `cwd`, `command`, `output`, `status`, `env` — not cells or buffers

The compositor sees cells. The user sees commands and output. The filesystem interface sees shell state. Each consumer gets the abstraction relevant to their purpose.

**Crate**: `pane-shell`

#### Scenario: Execute from output
- **WHEN** the user B2-clicks `cargo build --release` in shell output
- **THEN** it executes in the shell

#### Scenario: Route a compiler error
- **WHEN** the user B3-clicks `src/main.rs:42` in a compiler error
- **THEN** pane-route matches it and opens the editor at line 42

#### Scenario: Script drives the shell
- **WHEN** a script writes to `/srv/pane/<id>/command`
- **THEN** the command executes in the shell session

### Requirement: Extension model
pane-shell's behavior SHALL be extensible via plugins that operate on the semantic layer. A plugin can:

- **Transform output**: enrich cell regions with attrs (compiler errors gain structured data, URLs gain metadata) by sitting between the terminal layer and the compositor
- **Add routing rules**: drop a file in `~/.config/pane/route/rules/` — the system gains new B3-click behavior
- **Add translators**: drop a binary in `~/.config/pane/translators/` — the system gains a new content type
- **Define a pane mode**: wrap pane-shell-lib with domain-specific semantics, providing a custom tag line, custom filesystem interface endpoints, and custom routing patterns

Plugins compose because they operate on typed interfaces (pane protocol, attrs bag, filesystem), not on internal state. The extension surface is the same surface the system itself uses.

**Crate**: `pane-shell`, plugin interface TBD

#### Scenario: Git mode
- **WHEN** a "git pane" plugin is active
- **THEN** the tag line shows the branch, B3-clicking a commit hash opens the diff, `/srv/pane/<id>/branch` and `/srv/pane/<id>/staged` become available

#### Scenario: Compiler error enrichment
- **WHEN** an output transformer plugin detects a `rustc` error pattern
- **THEN** it attaches structured attrs (file, line, error code) to the relevant cell region, enabling one-click navigation

### Requirement: Tag line as shell interface
The tag line presents the shell's semantic state:
- **name**: current working directory (updated via OSC 7 or `/proc/pid/cwd`)
- **built-in actions**: Del, Snarf, Get
- **user actions**: shell commands added by the user, B2-clickable, persisted across sessions

#### Scenario: Tag command execution
- **WHEN** the user B2-clicks "make test" in the tag line
- **THEN** `make test` executes in the shell

### Requirement: Mouse reporting coexistence
When the shell enables mouse reporting (vim, htop), mouse events forward to the application. B2/B3 pane semantics suspend. When reporting disables, pane semantics resume. The tag line is always available for execution regardless.

**Hazard**: Users lose B2/B3 in mouse-aware apps. Correct behavior — the app asked for mouse.

#### Scenario: vim
- **WHEN** vim enables mouse reporting
- **THEN** all clicks forward to vim until it exits

---

## Internal Implementation Contracts

### Requirement: Terminal emulation
pane-shell SHALL use an existing terminal emulation library for VT parsing, screen buffer management, and PTY I/O. The specific library (vte, alacritty_terminal, or equivalent) is an implementation choice. Terminal emulation is commodity infrastructure, not pane-shell's differentiator.

**Crate**: `pane-shell-lib`

### Requirement: Dirty region updates
Modified screen content SHALL be sent to the compositor as CellRegion writes. Only changed regions are sent. The compositor receives positioned cell data — it has no knowledge of VT sequences or terminal state.

**Crate**: `pane-shell-lib`
