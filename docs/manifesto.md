# Why Pane Exists

This document is the historical, political, and philosophical context for pane's design. The foundations document (`spec.md`) declares principles. This document explains why we hold them — what pane stands against, why 2026 is the moment to act, and the fuller arguments behind each commitment.

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

---

# Extended Discussion

The following sections carry fuller arguments that were condensed in the foundations spec for brevity. Each is keyed to the section it elaborates on.

---

## On §1: The Guide Agent Scenario

We imagine this sort of scenario: a new user's first encounter with pane is a resident guide agent — a system participant whose `.plan` file is to help users understand pane's interfaces by demonstration. The guide teaches pane _using pane_. Every explanation can be accompanied by a live demonstration of the system's own capabilities, and catered to the user's specific interests or knowledge gaps. The ideal is that the user doesn't need to study abstract principles to start to understand how their system works — they can learn by watching the system work from the inside, taught by a responsive assistant who can inform them in minute detail how their system is configured, who can also walk them through how to alter it step by step to suit their own needs. If the user wants to know where their agent learned these things from, they can show them exactly where in the manual to look and where to find related information. This agent is not an overseer of user activity, it is a new colleague eager to work with them showing them the ropes.

The user absorbs the system's patterns by watching someone (something) use them naturally. The system absorbs the user's patterns by being programmed for responsiveness to their areas of concern. When they outgrow the guide, their understanding of the system was guided by a source able to meet them where they were at, who encouraged inquiry and supported the user's initiative and experiment. To facilitate the smooth functioning of such endeavors, not only is a resident agent trained with the system's own documentation, its actions are executed by means of the protocol the system establishes, with faculties for sanity checking and safety guarantees provided by the system architecture and fundamental to its design. Not just through well thought out security and design principles, but also by contracts enforced via typechecked protocols governing each context under which system interaction takes place, by human users or otherwise.

All of this is achieved through principled systems design infrastructure whose compositional approach extends to its relationship to the host system, the Linux base and the variety of tools already populating the ecosystem. When the user asks "how do I customize this?" the guide modifies a config file and the user sees the change take effect live. When they ask "what are my agents doing?" the guide shows them `who` and `finger` — the same tools they'll use on their own when engaging with the user and other agents. In other words, the guide uses the same tools the user will eventually use directly to interact with the system.

---

## On §1: The Unix Heritage of Multi-User Interaction

This last example with `who` and `finger` is instructive as to a resourcefulness that permeates our design philosophy. Designed in the days when Unix systems were exclusively the province of mainframes located within large firms, government or educational institutions, `who` and `finger` are relics of an interface built to facilitate clean and efficient interaction on multi-user systems, which fell into obscurity when commodity hardware enabled single-user computing as the norm. We might be led to ask: could it not be the case that some of the most brilliant systems engineers of all time already solved in principle major design challenges presenting themselves to agentic workflow developers? These were workflows developed to coordinate activities between dozens of the most adversarial actors of all: human beings; they must certainly be sufficient to manage agents. And they were developed to facilitate communication using the same command line tools which are already the bread and butter of agents and users alike, which is already becoming the primary way they communicate in various settings.

Our aim is not to reinvent yet-another-suite of yet-another-\* tools, to have our own music or photo apps. Our ambition is to realize a powerful environment enabling startling new uses for existing tools. And who knows, maybe something amazing happens once you imagine each computer as a collective endeavor no different in principle from any other enterprise, an assemblage of interactions between human beings or AI agents alike within or across networks.

---

## On §8: The Power User Pipeline

We wish to set a high bar in the full expectation that power users will leap over it. And there is a false dichotomy we wish to refute: the convention that novice-friendly interfaces must necessarily come at the expense of advanced users. Central to not only our design sensibility, but also our strategy as a whole, is the idea that the conveniences we offer on the part of less experienced users are a means of _expanding_ the power users precisely by obeying the principle that our interfaces ought to promote the increase of tech literacy rather than lull users into complacency. Conveniences should be evocative of further conveniences which they may never have knew they wanted before, and their experience with using the system ought to impress upon them that such improvements to their workflow are close at hand, and give them the confidence that the challenge of building it is surmountable. Thus it should be clear that we do not wish to inconvenience or suppress power users. We seek to grow their ranks.

---

## On §3: The Rust Analogy for Type System Adoption

There has traditionally been a tension perceived between type safety and good dev UX. Our contention is to structure pane's API such that type-safe design flows with and not against programmer intentions, so long as such intentions are reasonable. We can accomplish this by making more intentions reasonable to execute under such a framework by carefully chosen compromises. That was Rust's strategy, which paid dividends. Affine type systems existed for decades in the arcana of academic programming language research prior, but establishing real-world adoption required not only thoughtful presentation, it also had to be timely, well motivated by the specific challenges of complex software design in a world plagued with security risks owing to a well understood and easily exploitable class of bugs. The result of this confluence of initiative, practical design sensibility, and timing was a robust systems design programming language with a rich and ever growing developer ecosystem, without which our project may not have been possible to write. One could argue that the technology itself is dwarfed by its own userbase. We wager that a similar gesture is possible now in operating system design, whose emergent benefits are yet to be determined.

---

## On §6: Source-Based Distributions and the Wisdom of Modularity

We attribute the long-term success of source based Linux distributions precisely to the systemic problems with the monolithic development model. They designed systems that were built to handle the patchwork of system dependencies that constitute a Linux-based operating environment, flexible enough to meet the needs of users who for any reason wanted the very latest packages or updates as soon as they released. Such distributions are known for having the latest package bases on aggregate far outstripping other distributions. The lack of opinionation was _their_ uniform design principle, and also their only opinionated choice. Their opinion was that the right system was ready to go wherever its constituent parts were heading towards, or wherever its users wanted it to go in general. Initially these distributions were ridiculed as fools' errands for bored hackers, but now they are among the most popular style of linux distributions. They've even come to define the brand of what being a Linux user entails. The features derided as silly or impractical are exactly those which respective users love the most about their systems, and the communities united by their love of the personal curation a customized system enables forms the most distinctive quality of the Linux ecosystem as a whole. Any framework that isn't willing to embrace this principle in its soul will not thrive in this setting, but pushing the art forward may be among the best strategies for earning loyalty among this hardened core of linux enthusiasts, who have kept this platform alive and whose needs ought to be naturally accommodated.

---

## On §8: The Power Users

These are some of the most important users to win over, their initiative are invaluable to our efforts. These are the people who will dream up things pane's development team could have never imagined, the people who will discover killer apps driving forth further adoption. They are also the people who will poke and stress test every aspect of our system, who will break things and force us to harden and refine pane to meet their expectations of reliability, safety, and security to the same extent they also demand desirable user experiences for their particular use cases. They are also among the most demanding of particular features in other projects. The best way to compensate for our inability to accommodate the needs of each and every user is to provide a foundation for achieving any particular need, placing the responsibility for niche desired features in users' hands in ways that seem reasonable, familiar, and fair. The vast plugin ecosystems of emacs and (neo)vim are instructive historic examples of this virtuous (if at times chaotic) cycle.

---

## On §9: The Nostalgia Signal

Pane is meant to evoke the memory of the best aspects of this era of systems design; the nostalgia isn't entirely meant to be indulgent (although it certainly is in part ;)), it is a signal for those who remember this era of computing, and expresses our desire to be a credible standard bearer for this tradition for new generations of users who are tempted to know what it was like those days. The persistence of Linux and the unix-like ecosystem as a whole is the last remaining legacy of this era; what we want to bring back is the thrill that computing was actively going places noone has ever been before. Something new, something other than what we've been condition to accept as all we can realistically expect from our computing experience. We want to show that the best of that era was not a dead end, but a path forward — that the design principles that produced those systems are still valid, and that with the right architecture, they can be the foundation for a new generation of compelling user experiences.
