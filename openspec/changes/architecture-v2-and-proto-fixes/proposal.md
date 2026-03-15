## Why

The deep design review surfaced architectural decisions, naming changes, protocol model updates, and a formal grounding in sequent calculus that need to be captured before building the compositor. A code review of pane-proto found critical state machine bugs. The architecture spec has accreted through three rounds of deltas and needs a clean rewrite as a single coherent document.

## What Changes

**Architecture spec full rewrite incorporating:**
1. PaneMessage gains an open attrs bag (typed core + extensible key-value attributes)
2. FUSE interface at `/srv/pane/` — separate pane-fs server with format-per-endpoint
3. Linux-only target, track latest kernel, s6/runit init, xattr baseline with filesystem abstraction layer
4. pane-notify abstraction over fanotify (broad) and inotify (targeted) for pane-store's index
5. pane-roster hybrid model — init supervises infrastructure servers, roster supervises desktop apps, roster is service directory
6. Router multi-match via transient scratchpad pane
7. Filesystem-based plugin discovery — servers scan well-known directories, pane-notify watches them
8. Filesystem-as-configuration — config values are files, xattrs carry metadata, pane-notify makes it reactive
9. Value/Compute polarity (from sequent calculus/CBPV) — formal grounding for protocol composition
10. Inter-server protocol: ServerVerb + attrs with typed views/builders (BMessage-inspired)
11. Multi-pane per connection — state machine tracks a pane set, not a single active pane
12. Rename plumb/plumber → route/router throughout
13. pane-input absorbed into pane-comp (input handling in-process, IME add-ons external)
14. Compositor↔router relationship clarified (native panes route via client kit)
15. Pillar 5 (Declarative State) merged into Filesystem as Interface, with caching invariant
16. pane-shell architectural constraints noted (xterm-256color, dirty regions, alternate screen)
17. Accessibility acknowledged as known gap

**pane-proto code fixes (from code review):**
- Add PendingCreate state to protocol state machine
- Add height field to CellRegion
- Track PaneKind in ProtocolState::Active
- Fix frame() truncation on large payloads
- Bound NamedKey::F to valid range
- Re-export frame/frame_length from crate root
- Remove unnecessary Serialize/Deserialize on ProtocolState
- Document Scroll delta convention

## Capabilities

### New Capabilities
- `pane-notify`: Filesystem notification abstraction over fanotify and inotify
- `pane-fs`: FUSE interface exposing compositor and plumber state at /srv/pane/
- `filesystem-config`: Configuration model using files + xattrs instead of config file formats
- `plugin-discovery`: Filesystem-based plugin/add-on registration for extensible servers

### Modified Capabilities
- `architecture`: Platform target, init system, 8 new architectural decisions
- `pane-protocol`: PaneMessage attrs bag, state machine fixes, CellRegion height, wire fixes
- `cell-grid-types`: CellRegion gains explicit height field

## Impact

- Architecture spec substantially expanded — new sections for platform, FUSE, config, plugins, notifications
- pane-proto code changes: state machine redesign (PendingCreate), CellRegion breaking change (height field), PaneMessage wrapper type
- New crate planned: pane-notify (can be built before or alongside pane-comp)
- New server planned: pane-fs (after pane-comp)
- Tests need updating for state machine and CellRegion changes
