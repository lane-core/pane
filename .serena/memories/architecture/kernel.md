---
type: architecture
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [pane-kernel, exokernel, Dev, DeviceRegistry, DevId, translate, input, display, audio, plan9, be, rio, rustix]
related: [architecture/proto, architecture/session, architecture/app, architecture/fs, decision/host_as_contingent_server, decision/kernel_naming, architecture/router]
agents: [plan9-systems-engineer, be-systems-engineer, session-type-consultant, optics-theorist, pane-architect]
---

# Architecture: pane-kernel (System Interface)

## Summary

pane-kernel is pane's exokernel — the thin foundational layer
between pane and whatever OS it runs on. Its architecture
draws from Plan 9's kernel design (Dev device interface,
device table, per-pane namespaces) with Be's kit model
providing typed domain APIs as the ergonomic surface. Both
access paths (file protocol and typed API) share the same
underlying device state.

Named "kernel" in the Plan 9 sense: the minimal system
interface layer, not the privileged core. Plan 9's kernel
provides syscalls, #devices, and namespace machinery;
pane-kernel does the same in userspace as a Rust trait suite.

## Design philosophy

**Universal device model (Plan 9) with typed domain APIs (Be)
as the ergonomic surface.** The file protocol is always
available for composition and scripting; the typed API is the
primary application interface. Both paths access the same
underlying device state.

This mirrors pane's general synthesis: pane-session marries
Plan 9's single-fd transport with Be's BLooper dispatch.
pane-app marries Plan 9's /proc observation with Be's Handler
lifecycle. pane-kernel marries Plan 9's Dev/device-table with
Be's typed kit interfaces.

## Architecture diagram

```
                Application code
                     │
          ┌──────────┴──────────┐
          │                     │
     Typed API              File protocol
  (Be kit heritage)      (Plan 9 heritage)
 Translator, Input,     open/read/write/close
 Display, Audio traits   via pane-fs mount
          │                     │
          └──────────┬──────────┘
                     │
                Dev trait
              (shared state)
                     │
              DeviceRegistry
                (devtab[])
                     │
           ┌─────────┼─────────┐
           │         │         │
        Linux     Darwin    Headless
       (rustix,  (Cocoa,   (synthetic
       Wayland,  CoreAudio, sources)
       PipeWire)  IOKit)
```

## Core types

### Dev trait — Plan 9 foundation

Heritage: Plan 9 `port/portdat.h` Dev struct (function
pointers for device operations).

Every device implements Dev. This is the universal interface
that makes devices mountable in the namespace and accessible
via the file protocol.

```rust
pub trait Dev: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn id(&self) -> DevId;
    fn open(&mut self, mode: OpenMode) -> Result<(), DevError>;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, DevError>;
    fn write(&mut self, data: &[u8]) -> Result<usize, DevError>;
    fn close(&mut self) -> Result<(), DevError>;
    fn stat(&self) -> Result<DevStat, DevError>;
    fn event_fd(&self) -> Option<RawFd> { None }
}
```

`event_fd()` enables calloop integration: devices with async
events expose an fd for polling.

### DevId — Plan 9's Qid

Heritage: Plan 9 Qid (path + version + type).

```rust
pub struct DevId {
    pub path: u64,
    pub version: u32,
    pub dtype: DevType,
}
```

Unique device identity. Stable across namespace operations.

### DeviceRegistry — Plan 9's devtab[]

Heritage: Plan 9 `devtab[]` device table.

```rust
pub struct DeviceRegistry {
    devices: HashMap<String, Box<dyn Dev>>,
}
```

Flat map of name → device. pane-fs consumes this to mount
devices at `/dev/pane/`. Per-pane device visibility is a
predicate (HashSet of visible device names), not a full mount
table. Headless = empty set.

**pane-kernel does NOT do path resolution.** That's pane-fs's
job. pane-kernel provides the device table; pane-fs mounts
it at `/dev/pane/` on the host filesystem.

### /srv — Service posting

Heritage: Plan 9 `/srv`. Absolute path on the host
filesystem. Panes post service file descriptors to `/srv/name`,
other processes (pane or otherwise) open them to connect.
This is how pane-session's service binding becomes
namespace-visible. `/srv` doesn't exist on Linux or Darwin
by default, so pane takes it with no conflict.

### Host filesystem mount points

pane occupies three positions on the host:

- `/srv` — service posting (Plan 9 /srv)
- `/dev/pane/` — devices from DeviceRegistry
- `/pane/` — pane state, compositor, computed views

## Typed domain APIs (Be kit layer)

Domain-specific traits extend Dev. The `: Dev` bound is
pane's compile-time enforcement of what Plan 9 enforced by
culture — every typed API is also a file server.

### translate — Translation Kit heir

Heritage: BTranslator, BTranslatorRoster, translation_format.

```rust
pub trait Translator: Dev {
    fn input_formats(&self) -> &[TranslationFormat];
    fn output_formats(&self) -> &[TranslationFormat];
    fn identify(&self, source: &[u8], hint: Option<&str>)
        -> Option<TranslationFormat>;
    fn translate(&self, source: &[u8], output_mime: &str)
        -> Result<Vec<u8>, TranslateError>;
}
```

TranslatorRoster: registry with quality×capability scoring
for format selection. Compiled-in for Phase 1; separate
processes (Plan 9 model, fault-isolated) for production.

StreamingTranslator extends with incremental write/read
(fixing Be's whole-input limitation, marrying Media Kit
streaming with Translation Kit negotiation).

### input — Input Server device tier

Heritage: BInputServerDevice + Plan 9 #c (devcons).

```rust
pub trait InputSource: Dev {
    fn info(&self) -> &DeviceInfo;
    fn configure(&mut self, setting: DeviceSetting)
        -> Result<(), ConfigError>;
}
```

Input filters (BInputServerFilter heir) live in pane-router,
not pane-kernel. IME is a pane service, not a kernel concern.

### display — Buffer-based compositor interface

Heritage: app_server protocol + Plan 9 #i (devdraw). Buffer
management, not drawing commands. app_server's ~370-opcode
command model was right for vertical integration; Wayland's
buffer model is right when applications own their GPU context.

```rust
pub trait DisplayBackend: Dev {
    type Surface: Surface;
    type Buffer: Buffer;
    fn create_surface(&mut self)
        -> Result<Self::Surface, DisplayError>;
    fn create_buffer(&mut self, w: u32, h: u32, fmt: BufferFormat)
        -> Result<Self::Buffer, DisplayError>;
}
```

### audio — Device level only

Heritage: Media Kit device level + Plan 9 #A (devaudio).
Node graph (BBufferProducer/Consumer, latency tracking) is
a separate pane-media concern if ever needed.

```rust
pub trait AudioDevice: Dev {
    fn info(&self) -> &AudioDeviceInfo;
    fn configure(&mut self, format: &AudioFormat)
        -> Result<(), AudioError>;
    fn start(&mut self) -> Result<(), AudioError>;
    fn stop(&mut self) -> Result<(), AudioError>;
}
```

## Event model

Two consumption paths, same device:

- **Typed path (apps):** calloop polls `event_fd()` via
  `DeviceEventSource` wrapper → typed event enum → dispatch.
  Hot path, direct call on device object, no registry lookup.
- **File path (scripts):** pane-fs read on `/pane/dev/keyboard`
  → FUSE thread blocks on broadcast channel from looper.

Device events are NOT Messages. They're a lower-level type
translated at the pane-router/compositor boundary. Keyboard
events from local hardware should not be wire-routable.

Flow:
```
device fd → calloop poll → DeviceEventSource
           → typed event enum
           → pane-router filter chain
           → Protocol Message
           → target pane handler
```

## The rio connection

This design enables a rio-style compositor. rio multiplexes
`/dev/draw`, `/dev/cons`, `/dev/mouse` into per-window
namespaces. pane's compositor does the same: it's a file
server that mediates Dev devices, presenting per-pane virtual
devices. Each pane sees its own `/pane/dev/draw` and
`/pane/dev/keyboard`, and the compositor is the device
multiplexer.

## Platform strategy

`cfg(target_os)` modules within pane-kernel, rustix as the
primary POSIX abstraction:

- **Linux:** rustix, Wayland, libinput/evdev, PipeWire/ALSA
- **FreeBSD:** rustix, Wayland, evdev compat, OSS/sndio.
  capsicum maps naturally to pane-router sandboxing.
- **OpenBSD:** rustix (partial), Wayland, wscons, sndio.
  pledge/unveil maps to pane-router ACLs.
- **NetBSD:** rustix (partial), Wayland, wskbd/wsmouse, audio(4)
- **macOS/Darwin:** rustix + Obj-C frameworks, Cocoa/CALayer,
  IOKit, CoreAudio
- **Haiku:** native Be API (app_server, Input Server, Media Kit).
  Full circle — pane abstracting over the system that inspired
  half its architecture, as just another backend.
- **Headless:** synthetic sources (per headless_strategic_priority)
- **Redox OS:** redox-syscall, Orbital. The Dev trait abstraction
  means platform backends can be swapped from "host kernel" to
  "microkernel syscalls" — pane-kernel becomes the native
  userspace interface directly.

The Dev trait boundary is OS-agnostic by construction. Any
system with a POSIX-like surface (or its own syscall layer)
is a viable backend. The exokernel boundary stays clean
because pane-kernel is mechanism, not policy.

## What's IN vs OUT

| IN (pane-kernel) | OUT (other crate) |
|---|---|
| Dev trait | Path resolution (pane-fs) |
| DeviceRegistry (devtab[]) | Input filter chain (pane-router) |
| DevId (Qid) | IME (pane service) |
| Per-pane device view | Drawing/rendering (future pane-render) |
| Typed domain traits | Media node graph (future pane-media) |
| Platform backends | Process management (pane-server) |
| TranslatorRoster | Signal-flow policy (pane-router) |

## Essential vs deferrable

| Concept | Status | Rationale |
|---|---|---|
| Dev trait | Essential | Universal device interface |
| DeviceRegistry | Essential | Discovery + pane-fs binding |
| DevId | Essential | Unique identity (Qid) |
| Per-pane device view | Essential | Predicate-based visibility |
| Typed domain traits | Essential | Be kit ergonomics |
| Per-open state (Chan) | Deferrable | Multiple openers need independent cursors |
| Full Pgrp mount table | Deferrable | Predicate view sufficient Phase 1 |
| namec() | Not needed | pane-fs handles this |
| pipe/dup/rfork | Not needed | Different abstraction level |

## Provenance

Design established 2026-04-12 via four-agent roundtable
(plan9-systems-engineer, be-systems-engineer,
session-type-consultant, optics-theorist). Two rounds:
initial trait-suite design, then Lane redirected to
exokernel model based on the insight that a rio-style
compositor requires Plan 9's kernel primitives as substrate.
The synthesis (Dev foundation + Be typed APIs) follows the
same pattern as the rest of pane's architecture.

## See also

- `architecture/router` — signal-flow policy (pane-router)
- `architecture/fs` — namespace, pane-fs
- `architecture/proto` — Message, Protocol, MessageFilter
- `decision/kernel_naming` — why "kernel"
- `decision/host_as_contingent_server` — host has no privilege
