# Why Pane Exists

This document is the historical and political context for pane's design. The foundations document (spec.md) describes what pane stands for positively — its theoretical commitments and design principles. This document describes what it stands *against*, and why 2026 is the moment to act.

---

## The Fork

At the turn of the millennium, personal computing faced a fork in the road. On one side were platforms that had answered the question "what should a computer be?" with coherent, principled designs: BeOS, with its message-passing discipline and infrastructure-first architecture that made a Pentium 3 outperform machines twice its spec. Plan 9, with its protocol uniformity and per-process composition that made distributed computing transparent. These were not academic exercises — they were working systems that proved, empirically, that better design was possible.

On the other side were the entrenched platforms: Apple and Microsoft. They survived the 90s platform wars not because their designs were superior, but because applications were vendor-locked to their ecosystems. Without applications you don't have users; without users you don't have developers. The alternatives were pushed to obscurity — BeOS to a quiet acquisition, Plan 9 to a research lab, Haiku to a two-decade reconstruction effort with a skeleton crew.

This was not a neutral outcome. It profoundly distorted the course of personal computer design.

## The Distortion

The survivors consolidated. Convenient interfaces were accompanied by increasingly opaque abstractions that limited users' understanding of how their computer works. The App Store model — couched in terms of security assurances ("unidentified developers are potentially untrustworthy") — made it structurally inconvenient for users to design their own solutions or run software outside the walled garden. The monolithic architecture that justified this control didn't actually improve security; it increased the attack surface by requiring every user to run the entire kitchen-sink ecosystem.

OS design stagnated. Inertia led to development cycles that accumulated features at the expense of coherent design philosophy, without being attentive to accumulating cruft. macOS in 2026 is geological strata — Mach, BSD, Cocoa, Metal, SwiftUI, AppKit, and whatever shipped this year — not a designed system. Windows is worse; it was already extractive in the 90s. The Linux desktop inherited the design-by-committee problem from freedesktop.org: D-Bus, systemd, PulseAudio, PipeWire, X11, Wayland, GTK, Qt, dozens of clipboard protocols, three notification systems. Nobody designed "the Linux desktop." It accreted.

Meanwhile, concerning political trends have made the situation more urgent. The very capacities which could define a golden age of computing — AI, ubiquitous connectivity, powerful hardware in every pocket — have been stymied by actors who have historically committed to restraining progress out of their own self-interest. The controlling and extractive nature of the major platform enterprises became the main driving force behind new feature development, displacing the genuine innovation that characterized the early 2000s. The dreams nurtured in the 90s and 2000s — of computing as an empowering, creative, transparent medium — remain unfulfilled.

The evolution of mainstream platforms actively and deliberately reduced the tech literacy of users in general. Users who never knew anything else didn't know how their needs were already being failed compared to the potentials offered by alternatives they were distracted from. This compounded the initial adoption barrier for alternatives: the learning curve didn't just stay steep — it was multiplied by a generation of users trained to be passive consumers of computing rather than active participants in it.

## Why Linux, Why Now

Every year someone writes an article about why this will be the year of the Linux Desktop. It never is. This is not quite an accident.

The Linux desktop never broke through because no actor was visionary enough to put forth a decisive and satisfactory word about how Linux could be something *more* than it already was. And what it already was determined that it would remain a niche. Piecemeal refinement — better themes, better installers, better hardware support — addressed symptoms without touching the core problem: Linux desktops are assembled from parts, not designed as systems. They replicate the conventions of the platforms they're trying to replace, inheriting the downsides along with the familiar patterns.

All the attempts to kickstart broader adoption through incremental improvement have failed. The hypothesis: to break with the trend, a powerful, forceful, and evocative gesture is needed — of the caliber of early 2000s Apple keynotes. Not ego, but necessity. Given the precarious state of personal computing in 2026, this is the advantageous moment to make a bold step.

## What Pane Proposes

Pane picks up the thread that was dropped in the early 2000s. Not by rebuilding BeOS (Haiku has spent twenty years on that, with lessons we learn from). Not by porting Plan 9 to Linux (wio tried a subset of this and hit fundamental impedance mismatches). By applying a unified OS design philosophy over the Linux base — the way Apple applied NeXT's philosophy over Unix to create Mac OS X, but grounded in BeOS's design convictions rather than NeXT's, and committed to transparency and user empowerment rather than control.

The core system is compact, efficient, and bulletproof. Every other part can be rewritten as needs evolve. The commitment to architecture is minimized for the greatest possible gain in expressive design potential — and being minimal means the actual cost of that commitment can be understood by its users in its consequences and facets.

The name says it: a pane is a transparent object. Its purpose is to let you see clearly.
