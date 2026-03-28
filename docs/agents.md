# Pane — AI Agents as System Inhabitants

A guide for people who already use AI tools and want to understand what pane does differently.

## The problem with current AI interfaces

You know the pattern. You open a chat window, type a prompt, get a response, maybe copy-paste some code, close the tab. The next time you open it, the AI starts from scratch — or has a lossy summary of what you talked about last time. Your "assistant" has no memory of its own, no persistent presence, no way to notice something while you're away and tell you about it when you come back. Every interaction is an event. Between events, the AI doesn't exist.

The tooling has gotten better — copilots in your editor, agents that can run commands, assistants that write to files. But they all share the same fundamental limitation: **the operating system doesn't know they're there.** There's no account for your AI agent. No home directory. No inbox. No way for one agent to communicate with another. No way for you to check what your agents have been doing while you were asleep. The AI lives in the application layer, not in the system.

## What if the AI was a user?

Pane's answer is disarmingly simple: an AI agent is a user of the system. Not metaphorically — literally. It has a Unix user account, a home directory, file permissions, scheduled tasks. `who` shows which agents are logged in. `finger agent.reviewer` shows what it's working on and what its behavioral specification says. The agent's identity is a file you can read and edit.

This means everything the operating system already knows how to do for human users — authentication, permissions, process isolation, resource accounting, communication — works for agents too. No special AI infrastructure required.

## Communication: graduated, not monolithic

Current AI tools give you one interaction mode: the chat. Everything is a conversation — whether you need a quick status update, a focused working session, or an asynchronous handoff. Pane gives agents the same graduated communication model that Unix developed for human users over decades:

**Brief notifications.** Your build agent sends a one-liner to your screen: "build succeeded" or "found 3 issues in your diff." It appears, you glance at it, you continue working. No conversation opened, no context switch required.

**Focused sessions.** When you need to think through a design problem together, you open an interactive session with your agent — real-time, bidirectional, ephemeral. Like pair programming. When you're done, the session closes. No lingering thread.

**Asynchronous messages.** Your research agent found something interesting overnight. It left the summary in your inbox — a file with metadata (source, relevance, date) that you can query, filter, route to a reading list, or delete. You read it on your schedule, not when the agent decided to send it.

**Availability control.** You're in focus mode. One command — `mesg n` — tells all your agents: don't interrupt me. They respect it by queuing everything as asynchronous messages instead of notifications. When you're ready, `mesg y` opens the channel again.

This isn't a custom notification framework someone built for AI. These are communication patterns Unix has had since the 1970s, designed for coordinating between dozens of concurrent users on a shared system. They work because they were built for exactly this kind of multi-inhabitant coordination.

## The .plan file: governance you can read

Every agent has a `.plan` file in its home directory. This is the agent's behavioral specification — what it does, what it's allowed to do, what it's working on right now. It's a plain text file. You can read it with `cat`. You can edit it with your text editor. You can version-control it with git. You can share it with a colleague.

If you've ever struggled with the opacity of AI systems — wondering what permissions an assistant has, what data it can access, whether it's phoning home — the `.plan` file is the answer. The specification is a file. The governance is a file. When the system enforces the specification, it enforces what the file says. There's no gap between what you can inspect and what actually governs the agent's behavior.

## Agents build things

Current AI coding tools generate code in response to prompts. Pane's agents go further: they can modify the system itself. A user says "I want shell output lines matching this pattern routed to a scratchpad." The agent writes a routing rule — a small declarative file — and drops it in the right directory. The system gains the behavior immediately. No restart, no recompile, no deployment. The user didn't write code. The agent didn't modify any internals. It produced an artifact on the same surface that human developers use.

Over time, a user's collection of agent-built customizations becomes a personal configuration — shareable, versionable, composable. Think of it like the plugin ecosystems of vim or emacs, but with agents as contributors alongside humans, and with every artifact being a file you can inspect.

## Memory that lives in the filesystem

Current AI memory systems are opaque databases inside the application. You can't browse your AI's memories with `ls`. You can't query them with standard tools. You can't share them, back them up, or audit them except through the application's own interface.

In pane, an agent's memories are files. Each memory is a file in the agent's home directory with typed metadata — what kind of memory it is, how important it is, when it was created, what it's about. The system's metadata engine indexes these attributes and answers queries over them. "Show me all memories tagged 'debugging' from the last week" is a standard query, not a special AI feature.

The same infrastructure that makes email searchable in pane (metadata on files, indexed by the system, queryable) makes agent memory searchable. The agent's memory system is the filesystem. Nothing opaque, nothing proprietary, nothing locked inside an application.

## Local models are first class

Pane doesn't require a cloud API for AI functionality. A user running entirely on local models — on their own hardware, with their own data — gets the same agent infrastructure, the same communication patterns, the same tools as someone with API access to the most capable remote models.

More importantly: the choice of what data goes where is expressed as a routing rule. A rule might say: anything touching files in my work directory goes to the local model. General knowledge questions can go to a remote API. Credentials never leave the machine. These rules are files — you can read them, edit them, share them. The routing rule IS the privacy policy, expressed declaratively, not buried in settings.

## What this looks like in practice

You log in. "You have mail."

Three messages in your inbox. One from your build agent: the nightly build succeeded, one flaky test that's been intermittent for a week — log pattern attached. One from your review agent: notes on the commits you pushed yesterday evening, specifically a protocol change that's inconsistent with another component's assumptions. One from your research agent: it was reading a thread you bookmarked and thinks there's a cleaner approach to a problem you've been working on. Its thinking is in its project file if you want to look.

You haven't opened a single application. You've been informed of the state of your system through mail — files sitting in your inbox, queryable, dismissable. The system wasn't idle while you slept. It was inhabited.

## How this differs from what exists

| | Current AI tools | Pane |
|---|---|---|
| **Identity** | API key or OAuth token | Unix user account with home directory |
| **Persistence** | Conversation history in app database | Files in the filesystem, queryable |
| **Communication** | Chat window (one mode) | Graduated: notifications, sessions, mail, availability |
| **Governance** | Permissions dialog, opaque policies | `.plan` file — readable, editable, versionable |
| **Memory** | Proprietary database inside the app | Files with metadata, indexed by the system |
| **Extension** | Plugin APIs, custom integrations | Drop a file in a directory, gain a behavior |
| **Privacy** | Trust the provider's policy | Routing rules you control, enforced by the kernel |
| **Coordination** | One agent per app, no inter-agent communication | Multiple agents as system users, communicating through standard channels |

## The deeper idea

A personal computer has never really been inhabited by one entity. There were always the user, and there were always the system's own processes — daemons, services, scheduled tasks. What's changing is that some of those processes now have the capacity for judgment, conversation, and creative work.

The Unix multi-user model was designed for a world where multiple inhabitants shared a system, collaborated, communicated, and governed their shared resources through composable protocols. That world disappeared when personal computers became single-user. It's returning now, with different inhabitants than anyone expected.

Pane is ready for them because it was built on infrastructure that was always designed for multiple inhabitants. The mechanisms are the same. The inhabitants have changed. And making them first-class citizens of the operating system — not guests trapped inside application sandboxes — is what unlocks the next generation of human-AI collaboration.
