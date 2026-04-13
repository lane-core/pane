---
type: agent
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [pane-kernel, two-tier, typed_api, file_interface, translation_kit, input_server, display, audio, media_kit, format_negotiation, event_dispatch, calloop, synthesis]
sources: [TranslationDefs.h:27-34, TranslatorRoster.h:37-138, Translator.h:14-65, InputServerDevice.h:42-72, ServerProtocol.h:32-300, BufferProducer.h:24-160, BufferConsumer.h:25-130, MediaDefs.h:244-282,557-615, AppDefs.h:28-136, Window.h:1-80]
verified_against: [~/src/haiku/ as of 2026-04-12]
related: [agent/plan9-systems-engineer/pane_kernel_design_consultation, agent/plan9-systems-engineer/consultation_router_translator_2026_04_12, reference/haiku/internals, reference/haiku/source, architecture/app, architecture/fs, architecture/looper, decision/host_as_contingent_server]
agents: [be-systems-engineer]
---

# pane-kernel Design Consultation (2026-04-12)

Five-section analysis: Be's typed APIs on Plan 9's architectural skeleton.

## Core finding: two-tier interface pattern

Every pane-kernel device has:
1. A **typed trait** (Rust methods) — primary interface for application code.
   Captures domain protocol (phases, negotiation, state machine).
2. A **file projection** (pane-fs computed directory) — derived interface
   for scripting, inspection, automation.

Neither is implemented in terms of the other. Both derive from the same
underlying device state. Typed trait is in-process fast; file projection
goes through FUSE.

Inverse of Plan 9's libdraw/devdraw: Plan 9 had file-primary with typed
sugar. pane has typed-primary with file projection. Justified by: no
kernel support for zero-copy file protocol, FUSE round-trip cost.

## Where typed APIs add value over raw files

Verified from Haiku source:

- **Translation Kit** — `Identify()` is computational (probes format
  signatures, returns quality*capability scores). Roster selection is a
  comparison across candidates. File interface can carry data in/out but
  not the probing/scoring step. Typed API IS the translation.

- **Input Server** — `EnqueueMessage(BMessage*)` produces platform-
  independent typed events from platform-specific hardware. File reads
  give raw scancodes/evdev structs. Typed `InputEvent` enum IS the
  abstraction layer.

- **Display** — ServerProtocol.h defines ~370 stateful opcodes with
  phase ordering (create→configure→update→draw). Surface lifecycle is a
  state machine. File read/write can't enforce the state machine. Typed
  API IS the protocol discipline.

- **Audio/Media Kit** — Two phases with different requirements:
  (1) negotiation via FormatSuggestionRequested/FormatProposal (request-
  reply, wildcards, counter-proposals), (2) streaming via SendBuffer
  (low-latency pipe). Typed API separates the phases.

## Format negotiation: roster + explicit + preferences

- Default: roster-mediated (quality*capability scoring). Application code
  uses this 99% of the time.
- Explicit via file path: `/pane/translate/libjpeg/data` vs
  `/pane/translate/auto/image/jpeg`.
- User preferences override roster scores. Missing from original Be
  design — translator quality inflation was a real problem.

## Event dispatch architecture

Input goes through compositor, not direct to panes:
1. Hardware fd → InputSource trait (calloop EventSource on compositor loop)
2. Compositor applies focus policy
3. Routes InputEvent to target pane via pane-session wire protocol
4. Arrives as Handles<Input>::receive(InputEvent) on pane's looper

calloop sits at two levels:
- Compositor-level: polls device fds + pane connections
- Pane-level: polls connection to compositor, receives typed events

DeviceSource is a bridge trait: device impls produce calloop EventSources
for registration on the compositor's loop.

Display is NOT an event source — it's a command target. Compositor writes
TO display, not reads FROM it. Hotplug comes from separate udev monitor.

## Synthesis philosophy (one paragraph)

Every device presents a typed trait that captures the domain's protocol —
its phases, negotiation, state machine — because that protocol IS the
value the system adds over raw hardware access. pane-fs projects device
state into the namespace as computed directories, making every device
scriptable. The trait is the seam between hardware and system abstraction.
The file projection is the seam between system abstraction and human
tooling. Neither is implemented in terms of the other; both derive from
the same device state. This is neither Plan 9's "everything IS a file"
nor Be's "everything IS a typed API." It is: everything has a typed
interface for programs and a file interface for people, and they are two
views of the same truth.

## Haiku sources consulted

- translation_format struct: quality/capability floats (TranslationDefs.h:27-34)
- BTranslatorRoster::Identify/Translate overloads (TranslatorRoster.h:51-89)
- BTranslator::Identify/Translate pure virtuals (Translator.h:30-37)
- BInputServerDevice::EnqueueMessage (InputServerDevice.h:58)
- input_device_ref with type enum (InputServerDevice.h:15-19)
- ServerProtocol.h ~370 opcodes, phase-structured (lines 32-300+)
- media_raw_audio_format with wildcard pattern (MediaDefs.h:244-282)
- media_format union + Matches/SpecializeTo (MediaDefs.h:557-615)
- BBufferProducer negotiation: FormatSuggestionRequested, FormatProposal,
  PrepareToConnect, Connect (BufferProducer.h:54-111)
- BBufferConsumer: AcceptFormat, Connected, FormatChanged (BufferConsumer.h:78-101)
- System message codes B_KEY_DOWN through B_MOUSE_WHEEL_CHANGED (AppDefs.h:37-50)
