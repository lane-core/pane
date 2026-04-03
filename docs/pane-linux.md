# Pane Linux

The full desktop distribution. A sixos flake that layers pane's
personality — protocol, compositor, kits, desktop experience —
over sixos's init and service substrate and nixpkgs' 120,000
packages.

Pane Linux is the end state of the adoption funnel. It is not
a fork of anything. Each layer depends on the one below it;
none replaces the other.

---

## 1. The Three Layers

```
Layer 0: nixpkgs              120,000 packages (pkgs/, not nixos/)
Layer 1: sixos                s6 system builder, infuse combinator,
                              libudev-zero overlay
Layer 2: pane flake
         ├── pane-core        cross-platform: packages + service defs
         └── pane-linux       sixos consumer: pane-core + compositor
                              + desktop
```

**nixpkgs** provides application packages. The vast majority
(~95%) are init-system-agnostic — they don't care whether
systemd or s6 is PID 1. The systemd coupling in nixpkgs lives
in `nixpkgs/nixos/` (the module system), not in `nixpkgs/pkgs/`
(the packages). Pane uses pkgs, not nixos.

**sixos** (codeberg.org/amjoseph/sixos) replaces the NixOS
module system and systemd entirely. It provides:

- s6-linux-init boot chain
- s6-rc service compilation (service directories → binary
  service database)
- The `infuse` combinator (replaces the NixOS module system —
  services are fixpoint derivations with the same override/
  callPackage/overrideAttrs algebra as packages)
- libudev-zero overlay (breaks the systemd→libudev transitive
  dependency that makes ~hundreds of desktop packages depend
  on systemd)

Production-deployed on workstations, servers, routers, and a
768-core buildfarm. GPLv3.

**pane** provides the personality: the protocol, the compositor,
the application kit, the desktop experience.

---

## 2. The Seed Property

A user's pane configuration is a nix expression. It means the
same thing regardless of which platform interprets it.

```
nix flake on any unix-like
  → headless pane: server, kits, protocol, pane-fs
  → configuration accumulates in nix expressions
  → add compositor → add desktop
  → the flake IS the seed of a full Pane Linux installation
```

On Darwin, pane-headless is supervised by launchd. On NixOS, by
systemd. On Pane Linux, by s6-rc. The user's service
configuration is the same in all three cases — only the init
backend changes. Settings transfer because they were always nix
expressions. Graduation from headless-on-Darwin to full Pane
Linux is an upgrade, not a migration.

---

## 3. Service Architecture

Pane defines target-agnostic service definitions. Platform
backends consume these and generate native service
configurations.

The user writes service options at the pane level. The backend
generates the init-system-specific artifacts:

| Backend | Init system | Artifacts |
|---|---|---|
| Darwin | launchd | plist files |
| NixOS | systemd | unit files |
| Pane Linux | s6-rc | service directories |

### s6-rc service model (Pane Linux)

Each pane service is an s6-rc service directory compiled into
the binary service database:

- **Type:** `longrun` (supervised, restarted on failure)
- **Run script:** `execlineb` (small, auditable, no shell)
- **Readiness notification:** fd write (notification-fd 3) —
  the service signals when it is ready to accept connections,
  enabling dependency ordering without polling
- **Socket pre-registration:** `s6-fdholder` creates the unix
  socket at boot; the service retrieves it on startup. This
  enables zero-downtime restarts — the socket exists before
  the service does, and survives service restarts.
- **Dependencies:** declared in `dependencies.d/`, compiled
  into the service database. s6-rc resolves the dependency
  graph and starts services in the correct order, race-free.

### Pane services

| Service | Role | Dependencies |
|---|---|---|
| pane-headless | Protocol server (headless) | none |
| pane-comp | Compositor (display server) | pane-headless, elogind, seatd |
| pane-roster | Service registry, process supervision | pane-headless |
| pane-store | Attribute indexing, query engine | pane-headless |
| pane-fs | FUSE namespace at /pane/ | pane-headless, pane-store |
| pane-watchdog | Health monitoring, restart policy | pane-headless |
| agent services | Per-agent s6-rc longruns | pane-headless |

---

## 4. Platform Commitments

Pane Linux makes opinionated choices. The distribution
philosophy is: pick one of each, commit, optimize for it.

### Filesystem: btrfs exclusively

ext4's 4KB xattr limit is insufficient for pane-store's
attribute indexing. btrfs provides ~16KB per xattr value with
no per-inode total limit. pane-store's `full` backend writes
typed attributes as `user.pane.*` xattrs directly on files.
fanotify provides mount-wide change detection. The entire
attribute indexing model depends on btrfs.

### Init: s6 specifically

s6's philosophy — small tools, explicit dependencies, race-free
startup, process supervision as a first-class concept — aligns
with pane's design values. s6-rc's compiled service database
provides constant-time service lookup. s6-fdholder enables
zero-downtime socket handoff. Readiness notification via fd
write eliminates the polling-based readiness checks that
systemd's `Type=notify` only partially solves.

### FUSE: io_uring baseline

pane-fs uses FUSE-over-io_uring (Linux 6.14+). This is a
baseline requirement of the distribution, not an optional
optimization. The kernel interface is small (two io_uring
subcommands + standard FUSE opcodes). The performance target
is ~15-30μs per filesystem operation.

### Audio: PipeWire

PipeWire sits on ALSA and provides mixing, routing, network
audio, and video. Pane never interacts with ALSA directly.

### Sandbox: Landlock

Agent governance via `.access` files maps to Landlock rules.
Kernel-enforced, no daemon, no runtime overhead after setup.
The `.plan` file (separate) is the agent's human-readable
self-description displayed by `finger`. See `docs/ai-kit.md` §2.

### Session management: greetd

Lightweight, compositor-agnostic session manager.

---

## 5. The Core/Full Decomposition

Every pane server has two versions: a portable `core` that
runs on any unix-like, and a platform-optimized `full` that
leverages Pane Linux's specific infrastructure. Both expose the
same protocol interface. Clients do not know or care which
version they are talking to.

| Server | Core (portable) | Full (Pane Linux) |
|---|---|---|
| pane-store | SQLite backend, query interface | xattr backend (`user.pane.*`), fanotify mount-wide change detection |
| pane-fs | Standard FUSE (libfuse / FUSE-T on Darwin) | FUSE-over-io_uring |
| pane-roster | Portable process monitoring | pidfd, s6-rc integration, s6-fdholder socket activation |
| pane-watchdog | Portable health checks | s6-rc readiness integration |

The core versions are what users run on Darwin and NixOS
during the adoption funnel. The full versions are what Pane
Linux runs. The protocol interface is identical — a pane-native
application built against the core pane-store works unchanged
against the full pane-store.

---

## 6. Agent User Provisioning

Agents are unix users (see `docs/ai-kit.md`). On Pane Linux,
agent accounts and their s6-rc services are declaratively
defined in the system configuration:

- User account (uid, home directory, shell, groups)
- `.plan` file (human-readable description, agent-writable)
- `.access` file (enforcement specification, owner-writable)
- s6-rc service (longrun, runs the agent's pane connection)
- Nix user profile (the agent's tools, declaratively specified)
- cron entries (scheduled tasks)

The `.access` file is compiled into Landlock rules and network
namespace configuration at service launch time. The s6-rc
service `run` script invokes the `.access` parser, which
resolves `[tools]` names against the agent's Nix profile and
produces Landlock rules that are applied before exec'ing the
agent binary. The agent cannot modify its own `.access` —
Landlock rules cannot be relaxed once applied.

---

## 7. Desktop Composition

The full Pane Linux desktop is a strict superset of a headless
deployment:

```
headless pane
  + pane-comp (compositor, Wayland, input, rendering)
  + greetd (session management)
  + fonts (inter for UI, monospace for terminals, noto for fallback)
  + default applications (pane-shell, file manager, etc.)
  + Xwayland (legacy X11 application support)
  = Pane Linux desktop
```

Remove any layer above headless and you still have a
functioning pane system. The compositor is a server, not the
center of the architecture. This is the correction of BeOS's
limitation: app_server was architecturally special. In pane,
nothing is.

---

## Open Questions

These require resolution before Pane Linux is implementable.

### O1. infuse combinator integration

The current service definition sketches use NixOS-style
`mkEnableOption` / `mkOption` / `lib.mkIf` — the NixOS module
system. sixos uses the `infuse` combinator instead. The target-
agnostic service definitions need to work with both:

- On NixOS: consumed by the NixOS module system (systemd backend)
- On Darwin: consumed by nix-darwin (launchd backend)
- On Pane Linux: consumed by infuse (s6-rc backend)

**Options:**
- (a) Write service definitions in the NixOS module system
  and translate to infuse for the sixos backend
- (b) Write service definitions in infuse natively and
  translate to NixOS modules for the NixOS/Darwin backends
- (c) Write a thin abstraction layer that both module systems
  can consume
- (d) Write separate definitions per backend (duplication,
  but simplest)

This determines the shape of `nix/lib/services.nix` and every
platform module.

### O2. libudev-zero overlay scope

sixos provides the libudev-zero overlay that replaces
`systemd.lib` (libudev) with a daemonless alternative. This
requires rebuilding all packages that transitively depend on
libudev — hundreds of packages including the Wayland stack,
PipeWire, dbus, polkit.

**Questions:**
- What is the rebuild cost in practice? (sixos has done this;
  their experience is the reference)
- Does libudev-zero's missing hwdb interface affect any pane
  dependency?
- Is the overlay applied at the flake level (all consumers
  see it) or scoped to Pane Linux builds only?

### O3. greetd session flow

The desktop module specifies greetd for session management.

**Questions:**
- Does greetd launch pane-comp directly, or does it launch a
  session script that starts pane-comp + other services?
- How does the session interact with s6-rc? (greetd manages
  the user session; s6-rc manages system services. The boundary
  between them needs definition.)
- Auto-login for single-user systems vs greeter for multi-user?
- How do agent users interact with greetd? (They don't need
  a graphical session — their s6-rc services start independently
  of any human login.)

### O4. .access compilation pipeline

The `.access` file is the agent's enforcement specification
(separate from `.plan`, which is the human-readable description
— see `docs/ai-kit.md` §2). `.access` must be compiled into:
- Landlock rules (filesystem access)
- Landlock execute permissions (resolved from `[tools]` names
  against Nix profile store paths)
- Network namespace configuration (network access)
- pane-fs view filters (pane visibility)
- Model routing rules (advisory unless network restricts egress)

**Resolved:**
- Compilation happens at service launch time (s6-rc `run`
  script, before exec).
- `.plan` is agent-writable (self-description). `.access` is
  owner-writable (governance). The agent cannot modify its
  own `.access`.
- `[tools]` names that don't resolve against the Nix profile
  cause the agent to refuse to start (loud failure).

**Remaining questions:**
- Is the `.access` format formally specified beyond the sketch
  in ai-kit.md (`[filesystem]`, `[tools]`, `[network]`,
  `[models]` sections)?
- How are parse errors in other sections handled? (Refuse to
  start? Fall back to maximally restrictive default?)
- Can `.access` be hot-reloaded (restart agent to apply), or
  must it be stable for the agent's lifetime?

### O5. Boot chain and verified boot

sixos supports ownerboot (coreboot + immutable pre-kexec kernel
from write-protected SPI flash). Pane Linux's boot story is
unspecified:

**Questions:**
- Does Pane Linux commit to ownerboot, or support both
  ownerboot and conventional UEFI boot?
- How does the btrfs commitment interact with boot? (GRUB
  supports btrfs; systemd-boot does not. sixos may use
  neither.)
- Is the initrd s6-linux-init, or something simpler?

### O6. Nix store and caching

Pane Linux is built with Nix. The libudev-zero overlay means
cache misses against the public NixOS binary cache for every
affected package.

**Questions:**
- Does the pane project run its own binary cache? (Cachix,
  Attic, or self-hosted?)
- Is the rebuild closure bounded? (How many packages does
  the libudev-zero overlay actually invalidate for a typical
  desktop?)
- Can the overlay be structured to minimize cache
  invalidation? (e.g., replace only the libudev output of
  the systemd derivation, not the entire systemd package)

### O7. Wayland compositor dependencies

pane-comp (smithay-based) needs: libinput, Mesa/GPU drivers,
seatd/elogind, Wayland protocols. These are in nixpkgs but some
have systemd integration points.

**Questions:**
- Does elogind (the logind fork without systemd) work with
  sixos, or does sixos use a different seat management approach?
- Does the libudev-zero overlay cover all of pane-comp's
  transitive dependencies?
- Are there GPU driver issues with the overlay? (Mesa links
  libudev for device enumeration.)

### O8. Configuration as files

pane-fs.md specifies that server configuration lives as files
in `/etc/pane/<server>/` on a persistent btrfs volume, with
xattr metadata.

**Questions:**
- How does this interact with Nix's declarative model? (Nix
  generates `/etc` at activation time. User-modified files in
  `/etc/pane/` need to survive system rebuilds.)
- Is `/etc/pane/` a persistent volume separate from the Nix
  store? (Like NixOS's `/etc/nixos` which is not managed by
  Nix itself.)
- How does "erase your darlings" (stateless root, persistent
  /home and selected /etc) interact with pane's config model?

---

## Sources

- sixos: codeberg.org/amjoseph/sixos
- Nix base layer research: `docs/archive/openspec/changes/spec-tightening/research-nix-base.md`
- Linux subsystem research: `docs/archive/openspec/changes/spec-tightening/research-linux-stack.md`
- Distributed pane: `docs/distributed-pane.md` §5-6
- Architecture spec: `docs/architecture.md` §Implementation Phases
- Adoption funnel: `docs/introduction.md`
- pane-fs: `docs/pane-fs.md`
- AI Kit (agent provisioning): `docs/ai-kit.md`
