# Pane — An Introduction

## What is pane?

Pane is a new kind of operating environment for Linux. It's a desktop, a compositor, and a distribution — one integrated thing, not parts bolted together.

The name comes from what a pane of glass does: it lets you see clearly through it. Every piece of the system — every window, every notification, every running program — is a *pane*. A pane is transparent in both senses: you can see what it presents, and you can see how it works.

## Why does pane exist?

At the turn of the millennium, personal computing faced a fork in the road. On one side were platforms built from coherent design principles — BeOS, with its legendary responsiveness and elegance, and Plan 9, whose radical simplicity enabled capabilities no one had to plan for individually. On the other side were the entrenched platforms: Windows and macOS. The principled alternatives lost — not because their designs were inferior, but because they couldn't overcome the application ecosystem lock-in of the incumbents.

OS design stagnated. The survivors accumulated features at the expense of coherent philosophy. Meanwhile, the Linux desktop inherited a design-by-committee problem: dozens of competing standards, no unified vision, assembled from parts rather than designed as a system.

Pane picks up the thread from where it was dropped. It applies a unified design philosophy over a contemporary Linux base — drawing on what BeOS and Plan 9 proved works, while leveraging what computing makes possible in 2026.

## What makes pane different?

### Everything is a pane

In most desktops, there are windows, menus, notifications, tooltips, dialogs — each a different concept with a different interface. In pane, there is one thing: the pane. A notification is a pane. A terminal is a pane. A settings panel is a pane. They all speak the same language, compose the same way, and can be inspected with the same tools.

This uniformity is what made BeOS so powerful — when everything communicates the same way, combining features becomes natural. Users and developers can compose experiences that no designer anticipated, because the building blocks are universal.

### Your computer has inhabitants

Pane treats AI agents not as applications running inside the system, but as *inhabitants* of it — users with accounts, home directories, and their own tools. An agent on pane communicates the same way a human does: it can send you a brief message, have a focused conversation when you need one, or leave something in your inbox for when you're ready.

This isn't science fiction — it's the recovery of an old idea. Unix was designed in the 1970s for dozens of users sharing a single computer. The tools they built for coordinating between people — `who` shows who's logged in, `finger` shows what they're working on, `mail` leaves a message — work just as well for coordinating between a person and their agents. The infrastructure was always ready. The inhabitants have arrived.

Some inhabitants live on other machines. An agent running on a headless pane instance in the cloud is the same kind of system user as an agent on your laptop — same `.plan` governance, same communication patterns, same pane in the unified namespace. The only difference is latency. Plan 9 understood this: the `cpu` command let you run computation on a remote machine while I/O stayed local. Pane recovers this for a world where the remote machine might be running your AI agent ecosystem.

### The system teaches itself

A new user's first encounter with pane is a guide — an agent whose job is to show you around by demonstration, not by tutorial. The guide uses pane *using pane*. When you ask "how do I customize this?", it modifies a config file and you see the change take effect live. When you ask "what are my agents doing?", it shows you the same tools you'll eventually use on your own.

The user absorbs the system's patterns by watching someone use them naturally. When they outgrow the guide, they understand the system because they learned it from a fellow inhabitant, not from a manual.

### Infrastructure, not applications

BeOS's engineers didn't build a monolithic email application. They designed infrastructure — a filesystem that could store metadata on any file, a query engine, a file manager that could display anything — and email emerged naturally from the combination. Inboxes were just queries. The file manager became the mail client. Each component was independently useful, and the integration wasn't brittle.

Pane follows the same philosophy. The system provides infrastructure: routing content to handlers, indexing metadata, exposing state as files, managing application lifecycles. The experiences — file management, development workflows, communication — emerge from how that infrastructure composes. When a new capability is needed, the first question is: can existing infrastructure compose to provide it?

### The aesthetic matters

All this systems design is in service of something the user actually sees and touches. Pane reimagines desktop design from the early 2000s fork in the road — continuing what BeOS pioneered: power-user-friendly but elegant, dense with information but refined in presentation. A computer interface should invite interaction rather than demand it, and indulge your curiosity rather than evade it.

The visual consistency isn't cosmetic — it's architectural. Every pane renders through the same shared infrastructure, producing the same visual language without a central authority forcing it. The integrated feel comes from shared tools, not from imposed rules.

### Freedom is not too difficult

One of pane's central convictions is that user freedom and usability are not opposed — they reinforce each other, if the infrastructure is right. The best user experiences are not generated by imposing a particular view of what computing should be, but by providing a powerful and flexible foundation that users can build on to invent their own experiences.

The stock pane experience is one presentation of many possible experiences enabled by the underlying architecture. It sets a high bar — but it encourages its users to climb over it, and gives them the tools to do so. The convenience offered to newcomers is meant to *expand* the ranks of power users, not to lull anyone into complacency.

### Runs anywhere, installs nowhere

You don't reinstall your operating system to try pane. You add a nix flake. On macOS, on NixOS, on any Linux with nix — the same flake gives you headless pane: the protocol server, the application kit, the filesystem interface, the attribute store. Your configuration accumulates in nix expressions. When you're ready for the full desktop, the flake you've been building IS the seed of your Pane Linux installation. Settings transfer because they were always nix expressions.

This is possible because pane's architecture treats the local machine as one server among many. The headless deployment isn't a stripped-down version — it's the foundation that the full desktop extends. A headless pane instance in the cloud runs the same protocol, manages the same panes, participates in the same unified namespace as a local desktop. The only difference is whether there's a display attached.

The adoption path is architectural, not marketing: flake → headless → compositor → full desktop. Each step is an upgrade, not a migration.

### Local-first AI

Pane's AI capabilities are designed local-first. A user running entirely on local models gets the same agent infrastructure, the same communication patterns, the same tools as someone with API access to frontier models. The system doesn't phone home. Your data stays on your machine unless you explicitly choose otherwise — and that choice is expressed as a routing rule you can read, edit, and understand, not as a privacy policy buried in settings.

## Who is pane for?

Pane is for anyone who wants their computer to be a partner in their work rather than a passive tool. It's for people who are curious about how their system works and want the freedom to change it. It's for developers who want a platform that respects their craft. It's for users who have been told that freedom is too difficult to bother with, and who suspect that isn't true.

The system should be approachable on first contact and reveal depth as the user grows. Guard rails for new users. A ladder for power users. And a community that grows because the system itself encourages curiosity and rewards initiative.

## Where does the name come from?

> *What are we to do with these spring days that are now fast coming on?
> Early this morning the sky was grey, but if you go to the window now
> you are surprised and lean your cheek against the latch of the casement.*
>
> — Franz Kafka, "Absent-minded Window-gazing"

A pane is a transparent object. Its purpose is to let you see clearly.
