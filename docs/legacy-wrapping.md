# Legacy Application Wrapping

How legacy applications (GTK, Qt, Electron, X11-via-Xwayland)
participate in the pane ecosystem. This document specifies the
compositor's role, the bridge process pattern, the `.app` bundle
format, and the enrichment protocol that connects them.

Design principle: pane does not attempt to automatically extract
rich semantics from legacy applications. The compositor provides
a baseline (synthetic panes from Wayland surfaces). Per-app
integration — the knowledge of how to best expose a specific
application's state — lives in community-maintained `.app`
bundles, following the source-based distribution model. pane
provides composable tools; the ecosystem provides per-app
expertise.

---

## 1. Synthetic Panes

When a Wayland client connects to pane-comp and creates an
`xdg_toplevel`, the compositor creates a **synthetic pane** — a
pane object with a UUID, a Tag, and namespace presence, managed
internally by the compositor rather than by an external Handler
process.

### What the compositor knows from Wayland alone

The Wayland protocol provides:

| Wayland event | Pane state |
|---|---|
| `xdg_toplevel.set_title(s)` | `tag` (plain text) |
| `xdg_toplevel.set_app_id(s)` | `attrs/signature` (already reverse-DNS by Wayland convention) |
| Surface commit (buffer) | Display view (composited by pane-comp) |
| Surface geometry + output scale | `Geometry { x, y, width, height, scale_factor }` |
| `xdg_toplevel.close` / client disconnect | `PaneExited` |

The synthetic pane appears in the namespace at its locally-
assigned numeric path (see `docs/pane-fs.md` §Pane numbering):

```
/pane/4/tag              -> "LibreWolf"
/pane/4/attrs/signature  -> "org.librewolf.Librewolf"
/pane/4/attrs/pid        -> 48291
/pane/4/attrs/synthetic  -> true
/pane/4/commands/close
/pane/4/commands/minimize
/pane/4/commands/maximize
/pane/4/commands/fullscreen
/pane/4/ctl              -> write command names to invoke
```

The stable UUID identity is available at
`/pane/by-uuid/<uuid>/` (symlink to `/pane/4/`).

`/pane/4/body` is empty for synthetic panes at baseline.
The display view (pixels) is the compositor's rendering of the
Wayland surface. Semantic content (text, structured data) is not
available from the Wayland protocol — it arrives through
enrichment (S3) if a bridge process provides it.

### Compositor-internal representation

Synthetic panes have no external looper, no calloop event loop,
no Protocol-speaking client process. The compositor manages them
internally:

```rust
struct SyntheticPane {
    id: Id,
    tag: Tag,
    geometry: Geometry,
    surface: WlSurface,
    toplevel: XdgToplevel,
    pid: u32,                    // from wl_client credentials
    enrichments: Vec<EnrichmentBinding>,  // from bridge processes (S3)
}
```

The compositor translates Wayland events into pane state changes
and emits the corresponding protocol events on the synthetic
pane's behalf (Tag changes, PaneExited, geometry updates). It
translates `/ctl` commands into Wayland protocol operations
(`xdg_toplevel.send_close()`, configure events for minimize/
maximize).

### What synthetic panes cannot do

Synthetic panes are not full pane citizens. They lack:

- **Handler methods**: no `ready()`, `pulse()`, `command_executed()`.
  The compositor provides a fixed command surface (`close`,
  `minimize`, `maximize`, `fullscreen`) — not extensible without
  a bridge.
- **Handles\<P\>**: no typed service protocol participation. A
  synthetic pane cannot open the clipboard service, respond to
  scripting queries, or implement Handles<Routing>. These require
  a bridge process.
- **Optic-governed properties**: the compositor exposes `tag`,
  `signature`, `pid`, and `geometry`. These are not DynOptic-
  backed — they are direct translations from Wayland state.
  GetPut/PutGet hold trivially (the compositor is the sole
  writer), but the properties are not composable with the optic
  layer's composition operators.

The gap between synthetic panes and native panes is the space
that bridge processes fill.

### Namespace convention

Synthetic panes share the namespace with native panes. They are
distinguishable by `/pane/<n>/attrs/synthetic` (a boolean
attribute), but this is metadata, not an architectural boundary.
Scripts and tools that operate on `/pane/<n>/tag` work
identically for synthetic and native panes.

The per-signature index works for synthetic panes:
`/pane/by-sig/org.librewolf.Librewolf/` lists all LibreWolf
instances (synthetic and native alike, if both exist).

---

## 2. Bridge Processes

A **bridge process** is a headless pane that enriches a synthetic
pane with capabilities the Wayland protocol alone cannot provide.
It is an ordinary pane — Handler, Messenger, protocol
participation — whose purpose is to project knowledge about a
specific legacy application into the pane namespace.

### Lifecycle

```
pane-roster launches the bridge (from the .app service definition)
  -> bridge starts the legacy application (with configured env)
  -> legacy app connects to pane-comp as a Wayland client
  -> pane-comp creates a synthetic pane (automatic)
  -> bridge connects to pane-server as a headless pane
  -> bridge requests enrichment binding to the synthetic pane (S3)
  -> bridge populates attributes via its app-specific knowledge
```

The bridge process is a sibling of the legacy app, not a parent.
If the bridge crashes, the legacy app continues (it's a Wayland
client — the compositor keeps it alive). If the legacy app
crashes, the bridge receives notification (process monitoring or
D-Bus signal) and exits. The synthetic pane's enrichment
attributes become stale and are cleared by the compositor when
the enrichment binding is revoked.

### What bridges know

A bridge author — someone who uses the application and
understands its interfaces — encodes app-specific integration
logic:

- **D-Bus properties**: many GTK/Qt apps expose state over D-Bus.
  The bridge reads properties, maps them to pane attributes.
- **CLI queries**: some apps support `--dump-state` or similar.
  The bridge invokes these and parses output.
- **Config file watching**: the bridge watches config files
  (via pane-notify) and exposes settings as attributes.
- **IPC protocols**: apps with custom IPC (e.g., Firefox's
  Marionette protocol, Emacs's `emacsclient`) can be driven
  by the bridge.
- **Command translation**: pane commands written to `/ctl` or
  invoked through the command surface are translated by the
  bridge into app-specific actions (D-Bus method calls, CLI
  invocations, IPC messages).

### Example: LibreWolf bridge

```rust
struct LibreWolfBridge {
    dbus: DbusProxy,
    target_pane: Id,
    enrichment_id: Option<EnrichmentId>,
}

impl Handler for LibreWolfBridge {
    fn ready(&mut self) -> Flow {
        self.dbus = DbusProxy::new("org.mozilla.firefox")
            .expect("D-Bus connection to firefox");

        // Declare enrichment for the synthetic pane.
        self.messenger.send_request::<Self, EnrichmentGrant>(
            &self.server_messenger,
            EnrichRequest {
                target: self.target_pane,
                properties: vec![
                    AttrInfo::new("url", ValueType::String)
                        .operations(&[OpKind::Get]),
                    AttrInfo::new("tabs", ValueType::String)
                        .operations(&[OpKind::Get]),
                ],
                commands: vec!["new-tab", "open"],
            },
            |bridge, grant| {
                bridge.enrichment_id = Some(grant.id);
                Flow::Continue
            },
            |bridge| {
                // Server denied enrichment — degrade to no-op.
                Flow::Stop
            },
        );

        // Poll D-Bus for state changes.
        // (Event-driven via D-Bus signal subscription is better;
        // polling shown here for clarity.)
        self.messenger.set_pulse_rate(Duration::from_secs(1));
        Flow::Continue
    }

    fn pulse(&mut self) -> Flow {
        if let Ok(url) = self.dbus.get_property("URL") {
            self.messenger.set_enrichment_attr("url", &url);
        }
        if let Ok(tabs) = self.dbus.call_method("GetTabs") {
            self.messenger.set_enrichment_attr("tabs", &tabs);
        }
        Flow::Continue
    }

    fn request_received(&mut self, service: ServiceId, msg: Box<dyn Any + Send>, reply: ReplyPort) -> Flow {
        if service == service_id!("com.pane.enrichment.command") {
            if let Some(cmd) = msg.downcast_ref::<EnrichmentCommand>() {
                match cmd.name.as_str() {
                    "new-tab" => {
                        self.dbus.call_method("NewTab")
                            .expect("D-Bus NewTab call");
                    }
                    "open" => {
                        if let Some(url) = cmd.args.get("url") {
                            self.dbus.call_method_with_args("Open", &[url])
                                .expect("D-Bus Open call");
                        }
                    }
                    _ => {}
                }
            }
        }
        drop(reply);
        Flow::Continue
    }
}
```

The bridge is specific to LibreWolf. It knows about
`org.mozilla.firefox` D-Bus interface, knows that `GetTabs`
returns tab data, knows that `NewTab` opens a tab. This
knowledge is maintained by the wrapper author, not by the pane
framework.

---

## 3. The Enrichment Protocol

Enrichment is the mechanism by which a bridge process attaches
attributes and commands to a synthetic pane it does not own.

### Protocol definition

```rust
struct Enrichment;
impl Protocol for Enrichment {
    fn service_id() -> ServiceId { ServiceId::new("com.pane.enrichment") }
    type Message = EnrichmentMessage;
}

/// Messages from the server to the bridge.
#[derive(Debug, Clone, Serialize, Deserialize)]
enum EnrichmentMessage {
    /// A command was invoked on the synthetic pane's command
    /// surface that belongs to this enrichment binding.
    CommandInvoked { command: String, args: HashMap<String, String> },
    /// The target synthetic pane was destroyed (app exited).
    TargetLost,
    ServiceLost,
}
```

### Binding lifecycle

1. **Request**: the bridge sends `EnrichRequest { target, properties, commands }` via `send_request`. The server validates:
   - The target pane exists and is synthetic
   - The bridge's PeerAuth has permission to enrich this pane
     (same uid, or explicitly granted via sandbox policy)
   - The property names don't collide with existing properties
     or other enrichment bindings

2. **Grant**: the server responds with `EnrichmentGrant { id }`.
   The bridge holds this grant for the lifetime of the
   enrichment. The granted properties appear in the synthetic
   pane's namespace under `/pane/<n>/attrs/<name>`.

3. **Updates**: the bridge writes attribute values via
   `set_enrichment_attr(name, value)` — a Messenger method
   scoped to the bridge's enrichment binding. The server
   updates the synthetic pane's attribute set. Reads from
   `/pane/<n>/attrs/<name>` return the bridge's last-written
   value.

4. **Commands**: when a user or script invokes a command that
   belongs to the enrichment binding (e.g.,
   `echo "new-tab" > /pane/4/ctl`), the server routes
   the command to the bridge via `EnrichmentMessage::CommandInvoked`.
   The bridge translates it into an app-specific action.

5. **Revocation**: when the bridge disconnects (exit, crash,
   `Flow::Stop`), the enrichment binding is revoked. The server
   removes the enriched attributes from the synthetic pane's
   namespace. The synthetic pane reverts to its baseline state
   (Wayland-derived properties only). Revocation is also
   explicit: the bridge can send `RevokeEnrichment { id }`.

### Permission model

Enrichment is a write operation on another pane's namespace. The
server enforces:

- **Same-uid default**: a bridge running as uid 1000 can enrich
  synthetic panes whose Wayland client is also uid 1000. This
  is the common case (user's bridge enriches user's apps).
  PeerAuth::Kernel provides the uid.
- **Cross-uid via policy**: an agent running as a different uid
  can enrich a pane if the user's sandbox policy grants
  `enrich` permission to that uid. The policy mechanism
  (Landlock rules, namespace configuration) is defined by the
  deployment, not by the enrichment protocol itself.
- **Property collision**: two bridges cannot claim the same
  property name on the same pane. First binding wins; second
  request is declined with `EnrichmentDeclined::PropertyConflict`.
  Bridges should namespace their properties (e.g., `dbus.url`,
  `marionette.tabs`) to avoid collisions when multiple bridges
  enrich the same pane.

### Optic law status

Enrichment attributes are DynOptic-backed (see
`docs/archive/optics-design-brief.md`). The bridge's
`set_enrichment_attr()` is a DynOptic `set()`; the namespace
read is a DynOptic `get()`.

- **GetPut**: if the bridge writes a value and nobody else
  writes, reading returns the written value. Holds.
- **PutGet**: writing and then reading returns the written
  value. Holds — the server stores exactly what the bridge
  wrote.
- **Fidelity to the legacy app**: the bridge's written value
  may not reflect the legacy app's actual state if the bridge
  is polling (stale between polls) or if the app rejects a
  write-through (e.g., the bridge tries to set a URL but the
  app navigates elsewhere). The optic laws hold for the pane's
  attribute store, not for the bridge-to-app round-trip.

This is the "bridge is where type safety ends" boundary. The
enrichment protocol is typed and law-governed on the pane side.
The bridge-to-app communication (D-Bus, CLI, IPC) is not.

---

## 4. The `.app` Bundle

A `.app` bundle is a directory containing a Nix flake that
describes how to install a legacy application and integrate it
with the pane ecosystem. It is the user-facing artifact for
application deployment.

### Structure

```
LibreWolf.app/
  flake.nix              # deployment recipe (required)
  flake.lock             # pinned dependencies (required)
  metadata.toml          # human-readable app info (required)
  icon.svg               # pane-native icon (optional)
  bridge/                # app-specific integration (optional)
    src/               # bridge process source (Rust, or any language)
    commands.toml      # command surface definition
    properties.toml    # attribute mappings
```

### metadata.toml

```toml
[app]
name = "LibreWolf"
signature = "org.librewolf.Librewolf"
description = "Privacy-focused Firefox fork"
license = "MPL-2.0"
upstream = "https://librewolf.net"

[maintainer]
name = "pane-apps contributors"
contact = "https://github.com/pane-apps/librewolf"

[integration]
# What level of pane integration this bundle provides.
# "synthetic" = compositor baseline only (no bridge)
# "bridge" = custom bridge process with enrichment
level = "bridge"
```

### flake.nix

The flake uses `pane-lib.wrapApp` to produce a wrapped package:

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    pane-lib.url = "github:pane-project/pane-lib";
  };

  outputs = { self, nixpkgs, pane-lib }:
  let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};
    pane = pane-lib.lib.${system};
  in {
    packages.${system}.default = pane.wrapApp {
      # The underlying application from nixpkgs.
      app = pkgs.librewolf;

      # Pane service identity.
      signature = "org.librewolf.Librewolf";

      # Bridge process (optional -- omit for synthetic-only).
      bridge = ./bridge;

      # Environment for the wrapped application.
      env = {
        MOZ_ENABLE_WAYLAND = "1";
      };

      # Icon for the launcher / pane chrome.
      icon = ./icon.svg;
    };

    # pane service module: registers with the roster.
    pane.services.${system}.librewolf = {
      package = self.packages.${system}.default;
      signature = "org.librewolf.Librewolf";
      categories = [ "browser" "network" ];
      autostart = false;
    };
  };
}
```

### Installation

Drag to app folder (or `pane app install ./LibreWolf.app`):

1. The pane app manager evaluates the flake, building the
   wrapped package and its transitive closure.
2. The package is added to the user's profile (Nix user
   environment or Home Manager).
3. The service definition is registered with pane-roster.
4. The icon and metadata are indexed for the launcher and
   pane-fs app directory.

The app is now launchable. On launch:
1. pane-roster starts the bridge process (if the bundle
   includes one).
2. The bridge starts the legacy application with the
   configured environment.
3. The legacy app connects to pane-comp (Wayland).
4. pane-comp creates the synthetic pane.
5. The bridge binds enrichment to the synthetic pane.

### Uninstallation

Remove from app folder (or `pane app remove librewolf`):

1. Running instances are sent `close` (graceful shutdown).
2. The service definition is removed from pane-roster.
3. The package is removed from the user's profile.
4. `nix store gc` cleans up unreferenced store paths.

### Updates

The `.app` folder contains `flake.lock`. Updating:

```
pane app update librewolf   # runs nix flake update in the .app dir
pane app update              # updates all installed .app bundles
```

The update rebuilds the wrapped package with newer nixpkgs
(updated application) and/or newer pane-lib (updated bridge
tooling). Running instances are not affected until restarted.

---

## 5. pane-lib: Wrapper Toolkit

`pane-lib` is a Nix flake providing the `wrapApp` function and
supporting utilities for `.app` bundle authors. It is the
composable toolbox, not a framework — wrapper authors use what
they need.

### wrapApp

```nix
pane.wrapApp {
  app : derivation;       # the upstream package
  signature : string;      # reverse-DNS service identity
  bridge : path | null;    # path to bridge source (optional)
  env : attrset;           # environment variables for the app
  icon : path | null;      # icon file (optional)
  wrapperScript : path | null;  # custom launch script (optional)
}
```

`wrapApp` produces a derivation that:
- Wraps the app's binary with the configured environment
- Builds the bridge process (if `bridge` is provided)
- Generates a launcher entry and roster service definition
- Produces an s6 service file for the bridge process

If no bridge is provided, the app runs as a plain Wayland client
and gets synthetic-pane treatment (S1) only.

### Bridge helpers (Rust crate: pane-bridge)

`pane-bridge` is a Rust crate providing ergonomic wrappers for
common bridge patterns. It depends on `pane-app` (the main SDK)
and adds bridge-specific utilities:

```rust
use pane_bridge::{DbusAttr, FileAttr, CliCommand, BridgeBuilder};

fn main() {
    BridgeBuilder::new("org.librewolf.Librewolf")
        // Attributes sourced from D-Bus properties
        .attr(DbusAttr::new("url")
            .interface("org.mozilla.browser")
            .property("URL")
            .poll(Duration::from_secs(1)))
        .attr(DbusAttr::new("tabs")
            .interface("org.mozilla.browser")
            .method("GetTabs")
            .poll(Duration::from_secs(5)))

        // Attributes sourced from config files
        .attr(FileAttr::new("profile")
            .path("~/.librewolf/profiles.ini")
            .parser(FileParser::Ini)
            .key("Profile0.Name")
            .watch(true))   // pane-notify, not polling

        // Commands translated to CLI
        .command(CliCommand::new("new-tab")
            .exec("librewolf --new-tab"))
        .command(CliCommand::new("open")
            .exec("librewolf {url}")
            .arg("url"))

        .run()   // starts the bridge event loop
}
```

`BridgeBuilder` generates a Handler implementation internally.
For bridges that need custom logic beyond declarative mappings,
authors implement Handler directly (as in S2's example).

### Declarative bridge (TOML-only, no Rust)

For simple bridges where all attributes are D-Bus properties
and all commands are CLI invocations, the bridge can be defined
entirely in TOML — no Rust code:

```toml
# bridge/properties.toml
[[property]]
name = "url"
source = "dbus"
interface = "org.mozilla.browser"
property = "URL"
poll_seconds = 1

[[property]]
name = "profile"
source = "file"
path = "~/.librewolf/profiles.ini"
parser = "ini"
key = "Profile0.Name"
watch = true
```

```toml
# bridge/commands.toml
[[command]]
name = "new-tab"
exec = "librewolf --new-tab"

[[command]]
name = "open"
exec = "librewolf {url}"
args = ["url"]
```

`pane-lib` includes a generic bridge binary (`pane-bridge-generic`)
that reads these TOML files and runs the appropriate BridgeBuilder
pipeline. This means simple wrappers require zero compiled code.

### Wrapper authoring workflow

```
mkdir MyApp.app && cd MyApp.app
pane app init --signature com.vendor.myapp
# creates flake.nix template, metadata.toml, bridge/ skeleton

# Edit metadata.toml with app info
# Edit bridge/properties.toml and bridge/commands.toml
# Or write a custom bridge in bridge/src/

pane app build     # nix build, installs locally for testing
pane app test      # launches the app, checks enrichment binds
pane app publish   # submits to pane-apps community repo (if desired)
```

---

## 6. Compositor Responsibilities

The compositor's role in legacy wrapping is bounded. It handles
Tier 0 (Wayland protocol translation) and delegates everything
else to bridge processes.

### What pane-comp does

- Implements the Wayland protocol (wl_compositor, xdg_shell,
  wl_seat, xdg_decoration, wp_viewporter, and extensions as
  needed for GTK/Qt compatibility).
- Creates synthetic pane objects for each xdg_toplevel.
- Translates Wayland state changes to pane state changes
  (title, geometry, activation, close).
- Routes `/ctl` commands for synthetic panes:
  - Built-in commands (close, minimize, maximize, fullscreen):
    translated to Wayland configure/close events.
  - Enrichment commands: forwarded to the bound bridge process
    via the enrichment protocol.
- Manages enrichment bindings: accepts/revokes bindings,
  maintains the enriched attribute set, cleans up on bridge
  disconnect.
- Renders synthetic panes alongside native panes (both are
  surfaces to the compositor).

### What pane-comp does not do

- No D-Bus introspection of legacy apps.
- No accessibility tree scraping.
- No process injection or library preloading.
- No attempt to parse or interpret application content.
- No per-toolkit special cases in the compositor codebase.

All app-specific knowledge lives in bridge processes, which are
external to the compositor. The compositor's complexity budget
for legacy wrapping is bounded by the Wayland protocol — a
known, stable surface.

### Xwayland

X11 applications run through Xwayland, which presents them as
Wayland clients to pane-comp. Xwayland-mediated clients produce
synthetic panes the same way native Wayland clients do. The
Xwayland layer may provide less metadata (X11's `WM_NAME` and
`WM_CLASS` are less structured than Wayland's `app_id`). Bridge
processes for X11 apps may need to supplement via `xprop` or
EWMH properties.

Xwayland support in pane-comp follows standard practice
(smithay rootless Xwayland or equivalent). No pane-specific
X11 infrastructure.

---

## 7. Integration Levels

Every legacy application on pane falls into one of two
integration levels, determined by whether a `.app` bundle with
a bridge exists.

### Synthetic only (no bridge)

Any Wayland client gets this automatically:
- Appears in namespace (`/pane/<n>/...`)
- Tag, signature, pid, geometry exposed
- Basic commands (close, minimize, maximize, fullscreen)
- Participates in workspace layout, pane roster, pane-fs queries
- No semantic content, no app-specific attributes or commands

Sufficient for: casual use of any Linux GUI application.

### Bridge-enriched (with `.app` bundle)

Applications with a maintained `.app` bundle additionally get:
- App-specific attributes (`url`, `tabs`, `playing`, etc.)
- App-specific commands (`new-tab`, `open`, `pause`, etc.)
- Semantic body content (if the bridge provides it)
- Command surface integration (pane's command palette
  includes the app's commands)

The depth of enrichment depends entirely on the bridge author's
effort and the application's available interfaces. A bridge for
an app with a rich D-Bus interface (e.g., MPRIS-compliant media
players) can expose nearly everything. A bridge for an app with
no external interface can only add CLI-based commands.

### Incentive toward native

The integration levels create a natural incentive gradient:

```
Wayland client     -> synthetic pane (free, automatic)
Wayland + .app     -> enriched synthetic (community-maintained)
pane-native app    -> full citizen (Handler, optics, typed protocols)
```

Each level adds capability. The jump from synthetic to enriched
is low-cost (a `.app` bundle with TOML config). The jump from
enriched to native is a rewrite — but it's a rewrite toward
better architecture, not toward a proprietary API. The pane
protocol is session-typed and open; the kit libraries are
composable and well-documented.

The ecosystem expectation: most apps stay at synthetic or
enriched. Apps that benefit from deep system integration
(terminal, editor, file manager, media player) go native. This
mirrors the source-based distro pattern: most packages use
generic build systems; a few important ones have carefully
maintained build recipes.

---

## 8. Relationship to Architecture Spec

This document depends on and extends the following architecture
spec components:

| Architecture concept | Role in legacy wrapping |
|---|---|
| Handler (headless) | Bridge processes are headless panes |
| ServiceId (reverse-DNS) | Wayland `app_id` maps directly to service signature |
| pane-fs namespace | Synthetic panes appear at `/pane/<n>/` with numeric IDs |
| pane-fs `by-uuid` view | Stable cross-machine reference for synthetic panes |
| DynOptic + AttrInfo | Enrichment attributes are DynOptic-backed |
| DeclareInterest | Bridge declares `com.pane.enrichment` |
| PeerAuth::Kernel | Bridge authenticated by uid; same-uid enrichment is default |
| Dispatch + send_request | Bridge requests enrichment grant from server |
| ServiceTeardown (Control) | Enrichment revoked on bridge disconnect |
| `service_id!` macro | Compile-time ServiceId for enrichment protocol |

### New protocol: `com.pane.enrichment`

The enrichment protocol (`com.pane.enrichment`) is the one new
service this document introduces. It follows the existing
Protocol + Handles\<P\> pattern:

```rust
struct Enrichment;
impl Protocol for Enrichment {
    fn service_id() -> ServiceId { ServiceId::new("com.pane.enrichment") }
    type Message = EnrichmentMessage;
}
```

The protocol is between a bridge process and the pane server.
It does not affect the architecture spec's existing protocols
or invariants.

### New Nix infrastructure

The `.app` bundle format and `pane-lib.wrapApp` are Nix-level
infrastructure. They depend on the pane flake architecture
(described in `docs/distributed-pane.md`) but do not affect the
runtime protocol or the architecture spec.

### Phase mapping

| Component | Phase |
|---|---|
| Synthetic panes | Compositor track (requires pane-comp) |
| Enrichment protocol | Phase 2 (requires DeclareInterest + multi-server) |
| `.app` bundles + pane-lib | Phase 2 (requires enrichment protocol) |
| pane-bridge crate | Phase 2 (built on enrichment protocol) |
| TOML-only declarative bridges | Phase 2 (built on pane-bridge) |
| pane-apps community repo | Post-Phase 2 (ecosystem, not core) |

---

## Sources

- NeXTSTEP application bundles (`.app` directories)
- Nix flakes (RFC 0049)
- Wayland protocol: xdg-shell (stable), wl_compositor, wl_seat
- AT-SPI2 / D-Bus accessibility (reference, not dependency)
- MPRIS D-Bus interface (example of rich app introspection)
- Plan 9 synthetic filesystems (`/proc`, `/dev/cons`)
- Gentoo ebuilds, Nixpkgs derivations (community maintenance model)
