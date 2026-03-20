# Unix Multi-User UX Patterns — Research for Pane Agent Model

Research for pane spec-tightening. The thesis: pane treats AI agents as additional users of the system — sandboxed environments, shared protocols, declarative specifications. This reintroduces the multi-inhabitant condition that classic Unix timesharing systems were designed for. A whole category of UX patterns became vestigial when PCs went single-user. Some of these patterns may be exactly what's needed for human-agent interaction, and they've been sitting in the Unix toolbox unused for decades.

Primary sources: Unix man pages (V7, 4.2BSD, 4.3BSD), RFC 742 (Name/Finger), RFC 1288 (Finger User Information Protocol), Ritchie's "The Evolution of the Unix Time-sharing System" (1984), the CSRG BSD documentation, Les Earnest's account of the finger program's creation (Stanford AI Lab), MIT Project Athena documentation on Zephyr.

---

## 1. Inter-User Communication Primitives

### write(1) — Direct terminal message

**The original.** write(1) appeared in the First Edition of Research Unix (1971). It is the most primitive inter-user communication tool: type a message, it appears on another user's terminal.

**How it works.** The mechanism is disarmingly simple: write(1) opens the target user's tty device file (e.g., `/dev/ttyp3`) and writes text to it. That's it. The Unix "everything is a file" principle means that a terminal is a file, and if you have write permission on that file, you can send bytes to it. The recipient sees your text appear on their screen, interspersed with whatever they were doing. No framing, no protocol, no negotiation — just bytes arriving on a character device.

When you invoke `write alice`, the system:

1. Looks up alice in the utmp database (who's logged in, on which tty)
2. If alice has multiple sessions, selects the one with the shortest idle time
3. Opens alice's tty device for writing (checking group write permission)
4. Prints a header on alice's terminal: `Message from bob@hostname on ttyp2 at 14:32 ...`
5. Copies your stdin to alice's tty, line by line
6. On EOF or interrupt, prints `EOF` on alice's terminal

**The etiquette.** Because write(1) is half-duplex (you write, they write back to you with their own `write bob` invocation), users developed a protocol borrowed from radio communication:

- `-o` at the end of a line: "over" — your turn to talk
- `oo` on a line by itself: "over and out" — conversation finished

This is a human protocol layered on top of a machine protocol. The machine provides raw byte delivery; the humans negotiate turn-taking through convention. The convention emerged because the medium demanded it — without visual separation of speakers, you needed an explicit signal for turn boundaries.

**mesg(1) — The availability toggle.** Each user controls whether others can write to their terminal via `mesg y` (accept messages) or `mesg n` (refuse). The mechanism is a file permission change: `mesg n` removes group-write permission from your tty device file. When someone tries to `write` to you, the open() call fails. The sender gets no indication that you refused — the message simply doesn't arrive. This is notable: the refusal is silent. There's no "user is unavailable" response. You're just... not reachable.

Certain programs (nroff, pr) automatically set `mesg n` to prevent incoming messages from corrupting formatted output. This is the machine equivalent of "do not disturb during a presentation."

### talk(1) — Split-screen real-time chat

**The evolution.** talk(1) appeared in 4.2BSD (1983). Where write(1) is half-duplex text injection, talk(1) is a full-duplex, visual, real-time conversation.

**How it works.** talk uses a client-server architecture:

1. You type `talk alice`
2. Your talk client sends a UDP announcement to the talkd (talk daemon) on alice's machine
3. talkd writes a message to alice's terminal: `Message from Talk_Daemon@hostname... talk: connection requested by bob@hostname. talk: respond with: talk bob@hostname`
4. Alice types `talk bob`
5. Her talk client contacts your talkd, which matches the two requests
6. Both clients establish a direct TCP connection between them
7. The screen splits horizontally — your text appears in the top half, alice's in the bottom half

The split-screen display uses curses. You see each other's keystrokes in real time — not line-by-line like write(1), but character-by-character. You can see the other person typing, pausing, backspacing, correcting typos. As one nostalgic user described it: "You can see characters appear on screen as soon the person on the other side of the line types them... cursor movements, typos being corrected, pauses, and hesitations." This is more intimate than modern chat. You're not seeing polished messages; you're seeing the act of composition itself.

**ntalk** replaced the original talk protocol in 4.3BSD (incompatible with the 4.2BSD version — a reminder that even simple protocols can have versioning problems).

### ytalk — Multi-party talk

ytalk extended talk to support more than two participants. As each new user joins, the screen subdivides further — three users get three horizontal bands, four get four. Under X11, ytalk could use separate windows instead of terminal subdivision. ytalk was also the first to handle conversations between machines with different byte ordering (endianness), a practical concern in heterogeneous university networks.

### wall(1) — Broadcast to all

wall ("write all") sends a message to every logged-in user's terminal. Its canonical use: the system administrator warning users before a shutdown.

```
The system will go down for maintenance in 5 minutes.
Please save your work and log out.
```

wall respects `mesg n` for non-root users, but root can override — the sysadmin's broadcast is authoritative. This establishes a hierarchy: peer-to-peer communication (write, talk) can be refused; authoritative broadcast (wall from root) cannot.

### How these map to pane's agent model

The inter-user communication primitives map to agent interaction in ways that are surprisingly direct:

**write(1) as agent notification.** An agent `write`s to the user's pane — a message appears in the user's workspace, perhaps in a dedicated notification pane or overlaid on the current focus. The mechanism is the same: the agent has permission to write to the user's display, and it exercises that permission to deliver information. The key design question is the equivalent of "opening the tty device" — what is the protocol surface through which an agent delivers a notification to a human?

**talk(1) as agent conversation.** The user `talk`s to an agent in a split view. The top half is the user's input; the bottom half is the agent's response. The real-time character-by-character nature of talk is interesting here — the user could see the agent "thinking" (streaming tokens), which provides the same intimacy that talk provided between humans. The split-screen model is native to pane's tiling layout. A `talk agent.reviewer` command could open a split pane: human on top, agent on bottom, real-time bidirectional.

**wall(1) as system-wide agent broadcast.** A human user `wall`s instructions to all their agents: "Stop what you're doing, we're changing direction." Or the system broadcasts policy changes that all agents must acknowledge. The hierarchical override maps naturally: human authority over agents parallels root authority over users.

**mesg(1) as agent availability.** An agent sets `mesg n` when it's in a critical section — don't interrupt me, I'm in the middle of a complex operation. A human sets `mesg n` to suppress agent notifications during focused work. The silent refusal is interesting: an agent trying to notify a busy human gets no response, and must decide whether to queue the message, escalate, or wait. This is a real design pattern for agent interruption management.

**The etiquette conventions.** The `-o` / `oo` turn-taking protocol is a human solution to a coordination problem that agents also face. When a human and an agent are in a conversation, who speaks next? The agent could adopt the convention: end its response with a signal indicating "your turn" vs. "I'm done, conversation over." Session types formalize this — the type specifies whose turn it is — but the UX presentation could echo the old conventions.

---

## 2. Presence and Identity

### who(1) — Who's logged in

**The original presence indicator.** who(1) reads the utmp file and prints one line per logged-in user: username, terminal, login time, and (on networked systems) the remote host. It is the simplest possible answer to the social question: who else is here?

```
alice    ttyp0    Mar 18 09:14
bob      ttyp1    Mar 18 10:32
carol    ttyp2    Mar 18 08:45 (lab.example.com)
```

### w(1) — What are they doing

w(1) extends who with activity information: idle time, current process, CPU usage. It answers not just "who's here?" but "what are they up to?"

```
USER     TTY      FROM          LOGIN@   IDLE   WHAT
alice    ttyp0    :0            09:14    0.00s  vim paper.tex
bob      ttyp1    :0            10:32    3:22   (idle)
carol    ttyp2    lab.example   08:45    0.00s  make -j4
```

The idle time is a social signal. If alice has been idle for 3 hours, she's probably not at her terminal. If carol has 0 seconds idle, she's actively working right now. These are the same presence signals that modern messaging apps display (online, away, busy), but computed from actual terminal activity rather than self-reported status.

### users(1) — Just the names

The simplest variant: `users` prints a single line of space-separated usernames. `alice bob carol`. The minimal query — who's on the system right now?

### finger(1) — The user profile

**Origin.** Les Earnest created finger in 1971 at Stanford's Artificial Intelligence Laboratory. The name came from watching users "run their fingers down the output of the WHO command" to find someone. Earnest built finger to translate cryptic user IDs and terminal numbers into readable names and physical locations within the lab.

**What finger shows.** For each user, finger displays:

- Login name, real name, terminal, idle time
- Login time, office location, phone number
- Whether they have unread mail (and when mail last arrived)
- Contents of their `~/.plan` file
- Contents of their `~/.project` file

The `.plan` and `.project` files deserve their own section (see below). The rest of the information comes from the system password database (gecos field), utmp, and the mail spool.

**The network protocol.** finger became a network service (fingerd, port 79, RFC 742 in 1977, revised as RFC 1288 in 1991). A TCP connection, a username query in ASCII terminated by CRLF, and a text response. Simple enough to implement in any language. `finger alice@remote.host` queries a remote machine's fingerd. Without a username, the server returns a summary of all logged-in users — ambient awareness across the network.

**Decline.** finger's openness became a liability as the internet grew. The Morris worm of 1988 exploited a buffer overflow in fingerd as one of its attack vectors. The protocol exposes login patterns, email status, and user-written content to anyone who can reach port 79. By the late 1990s, most sites had disabled fingerd. Security killed the first social network.

### rwho(1) — Presence across the network

rwho extended who across a local network. The rwhod daemon on each machine periodically broadcast its user list via UDP. Every machine maintained a database of who was logged in where. `rwho` queried this local database and showed users across all machines on the LAN.

### ruptime — Machine presence

ruptime showed the status of all machines on the network: hostname, up/down, uptime, number of users, load average. It was presence detection for machines rather than users — is the compute server up? How loaded is it?

Both rwho and ruptime were part of the Berkeley r-commands (4.2BSD, 1982). They were retired for security reasons (replaced by nothing — the capability simply disappeared).

### How these map to pane's agent model

**who(1) for agents.** `who` shows which agents are active. The output might look like:

```
lane       pane/0     Mar 19 09:14   (human)
agent.rev  pane/1     Mar 19 09:14   reviewing pr #42
agent.ci   pane/2     Mar 19 09:30   running test suite
agent.fmt  --         Mar 19 08:00   (idle since 08:45)
```

The presence information is richer for agents because agent state is inspectable: we know not just that agent.rev is logged in, but what it's doing. The `w(1)` variant becomes especially interesting — agent activity is programmatically accessible, not inferred from idle time.

**finger for agents.** `finger agent.reviewer` shows the agent's profile:

```
Login: agent.reviewer          Name: Code Review Agent
Specification: ~/.config/pane/agents/reviewer.spec
Active since: Mar 19 09:14
Current task: Reviewing PR #42 (3 files, 2 comments drafted)
Plan:
  I review pull requests for style, correctness, and test coverage.
  I flag issues but do not approve — approval requires a human.
  I prioritize PRs from the main branch over feature branches.
Project:
  PR #42: src/compositor.rs refactor
  Status: 2/3 files reviewed, drafting comment on lifetime issue
```

The `.plan` file IS the agent's behavioral specification — a human-readable declaration of what it does and how. The `.project` file IS the agent's current task state. These are not metaphors; they are literally files in the filesystem, readable by any user (human or agent), queryable through the same infrastructure. finger becomes a way to inspect agent state through the same interface that inspected human state in 1971.

**rwho for agents.** In a system with agents distributed across machines (local models, remote APIs, different compute nodes), rwho's pattern becomes relevant again: which agents are running where? The periodic broadcast model maps to agent heartbeats — each agent periodically announces its existence and status.

**The presence model is pull-based.** This is important. finger, who, w — these are all queries. You ask "who's here?" and get an answer. There's no push notification of presence changes (that's what Zephyr added later — see section 11). The pull-based model is simpler and more private: your presence is visible when someone looks, not broadcast continuously. For agents, this means: an agent's status is queryable when you want it, not constantly streaming updates into your attention. The old model may actually be better for human attention management than the modern always-on presence indicators.

---

## 3. The .plan and .project Files

### The original social media

The `.plan` and `.project` files lived in each user's home directory. They were plain text files, readable by anyone with access (finger checked permissions but typically they were world-readable). There was no format, no schema, no length limit. You wrote whatever you wanted.

**`.project`** was conventionally a one-line summary of your current work. It appeared on the first line of finger output, next to your name.

**`.plan`** was conventionally longer — a paragraph or more describing your plans, your status, your availability. But "conventionally" did all the work. In practice, .plan files became whatever their authors wanted them to be.

Les Earnest, who created finger, described the .plan feature's origin: "Some people asked for the Plan file feature so that they could explain their absence or how they could be reached at odd times, so I added it." A practical feature for researchers keeping irregular hours at Stanford's AI lab. You weren't at your terminal, but your .plan said "At the dentist until 3pm, back by 3:30."

The .plan file evolved from status message to personal expression. As Earnest noted, it "later evolved into a forum for social commentary and amusing observations." Poets wrote poems. Wits wrote jokes. System administrators wrote warnings. And eventually, game developers wrote development blogs.

### John Carmack's .plan files

The most famous .plan files in computing history belonged to John Carmack at id Software. During the development of Quake (1996-1997) and subsequent games, Carmack used his .plan file as a public development log. Fans would `finger carmack@idsoftware.com` to read his latest update.

Carmack's organizational system was simple plaintext:

- **No prefix**: issues mentioned but unresolved
- **\***: tasks completed that day
- **+**: previously mentioned items fixed later
- **-**: features decided against

As chronicled in "Masters of Doom," Carmack turned to .plan files because he felt fans "had suffered months, years, of unsubstantiated hyperbole" from colleagues, and "it was time that they saw some hard data." The .plan file was a transparency mechanism — raw, unfiltered, technical, pull-based. No marketing department, no PR review, no platform. Just a text file in a home directory, queryable by anyone who knew the finger command.

This is the original developer blog. It predated web-based blogging. It was filesystem-native, protocol-accessible, and required zero infrastructure beyond what Unix already provided. The social network was the finger protocol; the content was a text file; the platform was the operating system itself.

### What made .plan files powerful

1. **Filesystem-native.** The .plan file is just a file. `cat ~/.plan` reads it. `echo "Working on the parser" > ~/.plan` updates it. Any tool that works on text files works on .plan files. No API, no app, no account.

2. **Pull-based.** Nobody gets notifications when you update your .plan. People check when they want to. This is the opposite of push-based social media, where every update demands attention from your followers. Pull-based status respects the reader's attention.

3. **Identity-linked.** The .plan file lives in your home directory, tied to your Unix account. Your identity is your username. There's no separate "profile" to maintain — your system identity IS your social identity.

4. **Network-accessible.** Through fingerd, your .plan is readable across the network. `finger alice@remote.host` retrieves alice's .plan from a remote machine. The network protocol is trivial (TCP to port 79, send username, read response).

5. **No platform.** There is no company between you and your readers. No algorithmic feed, no terms of service, no data collection, no ads. The protocol is open, the data is a file you own, and the "platform" is the operating system.

### How .plan/.project map to pane's agent model

This is one of the most direct and powerful mappings in this entire research.

**An agent's .plan IS its behavioral specification.** The pane architecture already defines agent behavior through declarative specification files. If these files live at a well-known path in the agent's home directory (e.g., `~agent.reviewer/.plan`), they become inspectable through the same mechanism that .plan files always used. `finger agent.reviewer` shows what it does, what it's working on, and how it behaves — pulled from the filesystem, readable by humans and other agents alike.

**An agent's .project IS its current task.** A one-line summary of what the agent is doing right now. `finger agent.ci` shows `Project: Running test suite for PR #42 (87% complete)`. This is live state, updated by the agent as it works, queryable by anyone.

**Humans have .plan files too.** A human user's .plan might say "Focusing on the parser refactor today. Please route code review requests to agent.reviewer." This communicates intent — to other humans, but also to agents. An agent that needs to route a request can `finger lane` to check what lane is working on and whether it's appropriate to interrupt.

**The composition with BeOS's mail model.** In BeOS, email messages were files with typed attributes, queryable through BFS. In pane, .plan files are text files in the filesystem. A system-wide query — "which agents have .project files mentioning PR #42?" — uses the same attribute indexing infrastructure that everything else uses. The .plan file is not a special social feature; it's a text file that the filesystem indexes, that finger displays, that other agents can read, and that the user can update with any text editor. The social network emerges from the filesystem.

---

## 4. Mail as System Infrastructure

### The Unix mail system

Unix mail(1) is local, filesystem-based, user-to-user message delivery. In its simplest form:

```
$ mail alice
Subject: Build results
The nightly build failed on test 47. Log attached.
.
```

A message is composed, delivered to alice's mail spool (`/var/spool/mail/alice` or `~/Strstrmail`), and stored as a file. The next time alice logs in, the login program checks for new mail and prints the iconic message:

```
You have mail.
```

Three words that were the notification system for an entire generation of Unix users.

### The mbox format

Messages are stored in mbox format: a single file where each message begins with a line starting `From ` (with a space, not a colon). Messages are concatenated. The entire mailbox is one text file, readable with any text tool. This is brutally simple and has obvious limitations (locking, concurrent access, large mailboxes), but it's also transparent: `cat /var/spool/mail/alice` shows all of alice's mail as plain text.

The Maildir format (qstrstrmail, 1995) improved on mbox by storing each message as a separate file in a directory structure (`new/`, `cur/`, `tmp/`). One file per message. No locking needed. Each message is an independent filesystem object.

### Mail as system notification

Mail was not just for human communication. It was the Unix system notification infrastructure:

**cron output.** By default, cron mails the output of every job to the job's owner. If your nightly backup script produces output or errors, you get mail. The MAILTO variable in crontab controls where this goes. This is automatic — you don't configure it, it just happens. Every scheduled task has a built-in notification channel: mail.

**at/batch results.** The at(1) and batch(1) commands schedule deferred execution. When the job completes, its stdout and stderr are mailed to the submitter. "It is historical practice to mail results to the submitter, even if all job-produced output is redirected." The system assumes you want to know when your deferred work is done, and mail is the delivery mechanism.

**System alerts.** Disk full warnings, security events, package update notifications — all delivered by mail. The sysadmin's mailbox was the system's event log.

**Build results.** CI systems mailed build results long before Slack notifications or GitHub checks. `make all 2>&1 | mail -s "Build results" team@` was a one-liner CI pipeline.

### biff/comsat — Instant mail notification

The "You have mail" prompt only appears at login. What about mail that arrives while you're working? biff(1) and comsat(8) solved this.

**biff** is a client-side toggle: `biff y` enables instant mail notification, `biff n` disables it. Named after a dog belonging to Heidi Stettner, a Berkeley CS student — the dog was known for barking at the mailman.

**comsat** is the server-side daemon. When new mail arrives, the mail delivery agent sends a UDP datagram to comsat, which writes a notification to the recipient's terminal — showing the From line, Subject, and first few lines of the body.

The biff/comsat pair is an early publish-subscribe system: mail delivery publishes an event (UDP datagram to comsat), comsat pushes notification to subscribed terminals (those with `biff y`). The subscription is per-terminal, toggled by the user. The notification is ephemeral — it appears on your terminal and scrolls away.

biff's idea survived long after biff itself: xbiff (X11 mailbox icon), kbiff (KDE), gnubiff (GNOME), and ultimately the notification badges on every modern email client. The concept of "notify me when mail arrives" is so fundamental that every platform reinvents it. But biff was the first, and it was one command.

### vacation(1) — Auto-reply

vacation(1) (4.3BSD, written by Eric Allman, author of sendmail) is an auto-responder. Configure it in your `.forward` file, and incoming mail gets an automatic reply from `~/.vacation.msg`. It tracks who it has replied to (in a dbm database) so each sender gets only one auto-reply per interval.

vacation is sophisticated about what NOT to reply to: it ignores mailing lists (checks for `Precedence: bulk`), system accounts (Mailer-Daemon, Postmaster), and messages that don't have you in To: or Cc:. These heuristics were developed over decades of real-world deployment and represent accumulated social wisdom about auto-reply etiquette.

### How mail maps to pane's agent model

**Agents mail results to users.** When an agent completes a task, it mails the result. `agent.ci` finishes a test run and mails the results to lane. The mail arrives as a file in the mail spool — a filesystem object with typed attributes (sender, subject, timestamp, status), queryable through pane-store, displayable in any pane that knows how to show mail.

This connects directly to the BeOS email composition pattern already documented in the BeOS research: mail messages as files with indexed attributes, inboxes as live queries, the entire email UX emerging from general-purpose filesystem infrastructure. The agent's mail output is not a special notification system — it's mail, stored as files, with attributes, queryable, archivable, using the same infrastructure as everything else.

**Agents mail requests to other agents.** Agent-to-agent communication through mail is asynchronous, persistent, and auditable. agent.reviewer finishes a review and mails the results to agent.ci, which triggers a rebuild. The mail spool is the message queue. This is not a metaphor — it's using Unix mail as an actual inter-process communication mechanism, which is exactly what it was designed for.

**"You have mail" for agent results.** The login notification "You have mail" becomes a workspace notification: "agent.reviewer has completed its review." The biff pattern (instant notification of new mail) becomes instant notification of agent task completion. `biff y` for agents means: tell me the moment an agent finishes something. `biff n` means: I'll check when I'm ready (the pull-based model).

**vacation(1) as agent delegation.** When the primary agent for a task is busy (or the user has configured a do-not-disturb period), vacation-style auto-delegation kicks in. An incoming request to agent.reviewer gets an auto-reply: "I'm currently reviewing PR #42. Queuing your request, estimated availability: 15 minutes." Or better: the auto-reply delegates to a backup agent: "Forwarding to agent.reviewer-2 for immediate handling."

**The cron → mail pipeline.** Agents can use cron. An agent scheduled to run nightly code quality analysis uses cron for scheduling and mail for result delivery. The scheduling infrastructure is standard Unix; the notification infrastructure is standard Unix mail; the agent is just a user who happens to be software. No special agent scheduling framework needed.

---

## 5. The Unix Permission Model as Social Contract

### Permissions as inter-user boundaries

The Unix permission model (rwx for owner, group, others) is fundamentally a social contract encoded in the filesystem. It answers the question: what can each inhabitant of this system do?

```
-rw-r--r--  1 alice  staff  4096 Mar 19 09:14 paper.tex
drwxrwx---  2 alice  team   4096 Mar 19 09:14 shared/
-rwsr-xr-x  1 root   root  16384 Mar 19 09:14 /usr/bin/passwd
```

**Owner permissions** define what you can do with your own stuff. **Group permissions** define what your collaborators can do. **Other permissions** define what strangers can do. This three-tier model is social structure expressed as metadata.

**Groups as capability delegation.** Adding a user to a group grants them capabilities. The `staff` group can read alice's paper. The `team` group can write to the shared directory. Group membership is a social decision — who do you trust? — expressed as a system configuration.

**The shared directory.** `/tmp` is the town square — world-writable, everyone can leave things there. Project-specific shared directories (`drwxrwx---` owned by a group) are private collaboration spaces. The sticky bit on `/tmp` prevents users from deleting each other's files — you can write but you can't destroy others' work. This is a social norm (don't mess with other people's stuff) enforced by the kernel.

**setuid/setgid as controlled privilege escalation.** The passwd command runs as root even when invoked by a regular user, because it needs to write to `/etc/shadow`. The setuid bit enables this: the program temporarily escalates to the file owner's privileges. This is delegation with constraints — you can change your password, but only through the approved mechanism, and the mechanism runs with just enough privilege to do its job.

### How permissions map to agent governance

**Agents run as users.** Each agent in pane has a Unix user account (or equivalent sandboxed identity). The agent's permissions define what it can access. agent.reviewer can read source code (group membership in `developers`), but cannot write to production (no membership in `deploy`). The permission model IS the capability model.

**Group membership as agent roles.** An agent's group memberships define its role:

- `readers` group: can read source code, documentation, configs
- `reviewers` group: can read source and write review comments
- `builders` group: can read source and execute builds
- `deployers` group: can trigger deployments (probably no agents in this group without human approval)

Adding an agent to a group is granting it a capability. Removing it revokes the capability. The mechanism is the same `usermod -aG` that has managed human access for decades.

**setuid as controlled agent escalation.** An agent that needs to perform a privileged operation (modify system config, restart a service) could use a setuid helper — a program that runs with elevated privileges but validates the request before acting. The agent invokes the helper, the helper checks the agent's specification against the requested action, and either performs it or refuses. This is the principle of least privilege, using the same Unix mechanism that passwd uses.

**The /tmp pattern for agent collaboration.** Agents that need to exchange intermediate results can use a shared directory with appropriate group permissions. agent.formatter writes formatted output to `/shared/agent-work/`; agent.reviewer reads it. The filesystem mediates the exchange, permissions control access, and the data is inspectable by humans (it's just files).

**Permissions as specification enforcement.** The pane architecture defines agent behavior through declarative specifications. But how do you enforce those specifications? Partially through session types (compile-time). Partially through the runtime sandbox. And partially through the oldest enforcement mechanism in Unix: file permissions. An agent specified as "read-only access to source code" is literally given read-only filesystem permissions. The specification maps to permissions. The kernel enforces what the specification declares.

---

## 6. Terminal Multiplexing and Shared Sessions

### screen and tmux session sharing

GNU screen (1987) and tmux (2007) are terminal multiplexers — they create persistent terminal sessions that can be detached and reattached. But they also support multi-user session sharing: multiple users connected to the same session, seeing the same state, both able to type.

**screen multi-user mode.** `multiuser on` in screen's configuration enables multi-user access. `acladd alice` grants alice permission to attach. Access control lists (ACLs) specify per-user, per-window permissions (read-only, read-write, none). This means you can give someone read-only access to watch your terminal without being able to type, or full read-write for pair programming.

**tmux shared sessions.** tmux uses Unix socket permissions for sharing. Create a tmux session attached to a named socket in a directory both users can access, set group permissions on the socket, and both users can attach. The wemux wrapper adds modes: Mirror (read-only, everyone sees the leader's view), Pair (shared cursor, both can type), and Rogue (independent windows within the shared session).

### How shared sessions map to human-agent interaction

**A human and an agent sharing a pane session.** This is perhaps the most direct mapping. In pane, a human and an agent could share a pane — both seeing the same content, both able to act on it. The agent watches what the human types and offers suggestions. The human sees what the agent does in real time. This is pair programming between a human and an AI, using the same infrastructure that humans use to pair with each other.

**The permission modes from wemux are instructive:**

- **Mirror mode**: the agent watches the human work (read-only). Useful for agents that learn from observation or provide assistance only when asked.
- **Pair mode**: human and agent share a cursor. The agent can type suggestions that the human can accept or reject. This is the AI pair programming model.
- **Rogue mode**: human and agent have independent views of the same session. Each can work on different parts of the same codebase within the same session context. The agent works in one window while the human works in another, but they share the same session state (environment variables, working directory, etc.).

**Session sharing as the basis for agent assistance.** The key insight: session sharing doesn't require a special AI framework. It requires a multiplexer that lets two participants (human and agent) connect to the same session. If pane's compositor supports multiple connections to the same pane (one from the human's input, one from the agent's protocol connection), then shared sessions fall out naturally. The agent connects to the same pane the human is using, reads its state through the filesystem interface, and writes to it through the protocol. No special "AI assistant" mode — just two users sharing a session.

---

## 7. System Accounting and Auditing

### Process accounting

Unix process accounting (appearing in early BSD) logs every command executed on the system: who ran it, when, how long it took, how much CPU it consumed.

**accton** enables or disables process accounting. When enabled, the kernel writes a record to the accounting file for every process that exits.

**lastcomm** queries the accounting log: "what commands has alice run recently?"

```
$ lastcomm alice
vim          alice  ttyp0  0.12 secs  Mar 19 09:14
gcc          alice  ttyp0  2.34 secs  Mar 19 09:18
make         alice  ttyp0  0.87 secs  Mar 19 09:18
```

**sa** summarizes accounting data by command or by user — aggregate statistics rather than individual records. "Which users consumed the most CPU this month?" was a real question in shared computing environments where CPU time had actual cost.

**last** shows login history: who logged in when, from where, for how long. `lastb` shows failed login attempts.

The accounting records are binary files (for compactness and write speed) — the kernel writes one record per process exit, and it must be fast enough not to slow down process termination. The records include: command name, user, group, terminal, start time, elapsed time, user CPU time, system CPU time, memory usage, I/O operations, and exit status.

### The social context of accounting

Process accounting existed because shared computing resources had real costs. In university and corporate timesharing environments, CPU time was billed to departments or grants. Accounting answered: who used how much, and what did they do with it? The sysadmin reviewed accounting data to catch resource abuse, debug performance problems, and generate billing reports.

But accounting also served a security function. After a break-in, the accounting log is one of the few records that shows what the intruder actually did — it's harder to tamper with than shell history because it's written by the kernel, not by the user's shell.

### How accounting maps to agent auditing

**Every agent action is logged, attributable, reviewable.** Process accounting gives this for free — if agents run as Unix users, their commands appear in the accounting log. `lastcomm agent.reviewer` shows exactly what the agent did, when, and how much resource it consumed.

But pane can go further. Process accounting records commands; pane can record *decisions*. If agent actions flow through the pane protocol (session-typed messages), the protocol trace IS the audit log. Every message the agent sends, every file it reads, every modification it makes is a protocol event that can be recorded, timestamped, and attributed.

**The accountability model.** The old accounting question — "who used how much?" — becomes the agent accountability question: "what did each agent do, why, and was it within its specification?" The accounting infrastructure answers the "what" and "when." The protocol trace answers the "what" at a higher semantic level. The agent's .plan file (its specification) answers the "was it within bounds?" question. The three together — process accounting, protocol traces, declarative specifications — form a complete audit chain.

**Resource governance.** In timesharing, CPU and memory were scarce shared resources. In a system with multiple agents, compute and API calls are scarce shared resources. The old `sa` summary — which user consumed the most CPU — becomes: which agent consumed the most API tokens, the most compute time, the most disk I/O. The same resource governance patterns apply. An agent that's consuming disproportionate resources might be malfunctioning, or it might need a larger allocation — the same judgment call sysadmins have been making for decades.

---

## 8. The Login Sequence and Environment

### The ritual of logging in

The Unix login sequence is a ritual with specific stages, each serving a purpose:

1. **`/etc/issue`** — displayed before the login prompt. The pre-authentication banner. Machine identity, legal warnings, or just a hostname. This is the "you are approaching a system" message. Anyone can see it, even before authenticating.

2. **Login prompt** — username and password. Authentication. You prove you are who you claim to be.

3. **`/etc/motd`** — message of the day, displayed after successful authentication. System-wide announcements: scheduled maintenance, policy changes, new software installed, known issues. Every user sees it. It's the sysadmin's broadcast channel for non-urgent information (wall is for urgent; motd is for persistent).

4. **`You have mail.`** — notification of pending mail, checked by the login program.

5. **`.login`/`.profile`/`.bashrc`** — per-user environment initialization. PATH setup, aliases, prompt customization, environment variables. This is the user's personal configuration of their view of the system.

6. **Shell prompt** — ready to work.

### What each stage does socially

The login sequence is a *transition ritual* — it moves you from "outsider" to "participant" in the shared system. Each stage communicates something:

- **issue**: "This is the system. These are the rules."
- **authentication**: "Prove your identity."
- **motd**: "Here's what's happening in our shared space."
- **mail notification**: "People have been communicating with you."
- **profile**: "Set up your personal workspace."

The motd is particularly interesting. It's the system administrator's voice — a human communicating with all users through a text file. The content is whatever the sysadmin decides is important: "The NFS server will be down Saturday 2am-6am." "New version of gcc installed, see /usr/local/doc/gcc-4.0-notes." "Please clean up /tmp, we're running low on disk space." The motd is how the shared system's caretaker communicates with its inhabitants.

### How the login sequence maps to agent initialization

**Agent "login" initializes from specification.** When an agent starts, it goes through an analogous sequence:

1. **System policy** (`/etc/pane/agents/policy`): the equivalent of /etc/issue. System-wide agent constraints — what agents can and cannot do, resource limits, audit requirements. Every agent must acknowledge this.

2. **Authentication**: the agent proves its identity (API key, certificate, or specification signature). This is not password-based — it's specification-based. The system verifies that the agent's specification is valid and authorized.

3. **System state** (`/etc/pane/motd`): the equivalent of motd. Current system state relevant to agents — "deployment freeze in effect until Thursday," "primary API endpoint is rate-limited, use secondary." Agents read this and adjust behavior accordingly.

4. **Pending messages**: "You have mail." The agent checks for queued requests, notifications from other agents, and results from previous runs.

5. **Environment setup**: the agent's equivalent of .profile — loading its specification, initializing its tool access, setting up its filesystem view (namespace), establishing protocol connections.

6. **Ready**: the agent begins its work.

**motd as system policy for agents.** The motd pattern is powerful for agent governance. A human administrator updates `/etc/pane/motd` with a policy change, and every agent reads it on next "login" (or watches it via pane-notify for live updates). The policy is a text file, human-readable, human-editable, and machine-parseable. "All agents: deployment freeze until March 20. Do not merge PRs to main." An agent that reads this adjusts its behavior. The governance mechanism is a text file that agents are expected to read — exactly the same mechanism as the original motd.

---

## 9. Batch and Scheduling Systems

### at(1), batch(1), cron(1)

**cron** runs commands on a schedule. The crontab file specifies when:

```
0 2 * * * /home/alice/bin/backup.sh
30 8 * * 1 /home/alice/bin/weekly-report.sh
```

**at** runs a command once at a specified time: `echo "make release" | at 2am tomorrow`. When the job completes, stdout and stderr are mailed to the submitter.

**batch** runs a command when the system load drops below a threshold. It's at(1) with an implicit "when the system isn't busy" schedule.

### The mail notification on completion

The critical design pattern: when a scheduled job completes, the user gets mail. This is not optional — it's the default behavior. The system assumes that if you scheduled work for later, you want to know when it's done. The notification channel is mail, because mail is the system's universal asynchronous messaging layer.

The MAILTO variable in crontab allows directing output to specific addresses (or suppressing it entirely with `MAILTO=""`). This is per-job notification routing — each cron job can notify different people.

### How scheduling maps to agent orchestration

**Agents use cron, at, and batch.** If agents are users, they can use cron like any user. An agent scheduled to run nightly code quality analysis adds a crontab entry:

```
0 3 * * * /home/agent.quality/bin/analyze.sh
```

When it completes, results are mailed to the human user. The scheduling infrastructure is standard Unix. No agent-specific scheduling framework. No orchestration layer. Just cron.

**batch(1) for resource-aware agent scheduling.** The batch command's "run when the system isn't busy" semantics are directly useful for agents. Agents doing background analysis, code formatting, documentation generation — these are batch tasks that should yield to interactive human work. The Unix batch scheduler already implements this policy: wait for the system load to drop, then run.

**The MAILTO pattern for agent result routing.** Each agent's scheduled task can specify where results go. `MAILTO=lane` sends results to the human. `MAILTO=agent.ci` sends results to another agent for further processing. The existing cron MAILTO mechanism IS the agent notification routing system.

**Composition: cron + mail + .plan.** An agent runs a nightly analysis via cron. Results are mailed to the user. The agent updates its .project file: "Last run: Mar 19 03:00. Found 3 issues. See mail for details." The user can check immediately (via biff notification) or later (via mail). The user can check the agent's status anytime (via finger / .project). Three Unix facilities — cron, mail, .plan — compose into a complete agent task lifecycle without any agent-specific infrastructure.

---

## 10. Social Protocols and Etiquette

### The culture of shared systems

Dennis Ritchie articulated the social dimension of Unix explicitly: "What we wanted to preserve was not just a good environment in which to do programming, but a system around which a fellowship could form. We knew from experience that the essence of communal computing, as supplied by remote-access, time-shared machines, is not just to type programs into a terminal instead of a keypunch, but to encourage close communication."

This is the founding insight: Unix was designed as a *social system*, not just a technical one. The multi-user primitives — write, talk, mail, finger, who, motd — were not afterthoughts. They were part of the system's purpose: to enable "close communication" among a fellowship.

### The norms that emerged

**Resource courtesy.** On a shared system, your processes affect everyone. Running a CPU-intensive compilation at peak hours slowed down everyone's editor. The norm: use `nice` for background work, schedule heavy jobs for off-hours, be aware of your impact. The `w` command showed load averages — a public dashboard of system stress. If load was high, courteous users deferred their heavy work.

**Message etiquette.** `mesg n` was "do not disturb" — and it was respected. You didn't escalate to wall when someone had mesg off. The write/talk protocol conventions (-o for over, oo for over and out) were widely known and followed. Interrupting someone's terminal without warning was considered rude.

**The sysadmin as social role.** The system administrator was not just a technical role but a social one. The sysadmin maintained the shared environment: resolved conflicts between users competing for resources, enforced social norms (don't leave huge files in /tmp), communicated policy through motd, and used wall for urgent announcements. The sysadmin was the steward of the commons.

**Shared spaces.** Directories like `/usr/local/share` and `/tmp` were commons — shared spaces with implicit norms. You could leave things in /tmp, but don't expect them to persist. You could install software in /usr/local, but follow the conventions. These spaces had technical protections (the sticky bit on /tmp) but relied primarily on social norms for order.

### How social protocols translate to agent governance

**Resource courtesy for agents.** Agents should be "nice" — literally. An agent doing background analysis should use nice(1) to lower its scheduling priority. An agent should be aware of system load and defer heavy work when humans are active. The `w` command's load average is a signal: if load is high because a human is compiling, agents should back off. This is the old timesharing courtesy, now applied to agents.

**mesg n as agent interruption policy.** A human sets a "do not disturb" flag (the equivalent of `mesg n`), and agents respect it. No notifications, no suggestions, no questions. When the human clears the flag, queued messages are delivered. The old social norm becomes a system policy: agents MUST respect the human's availability flag.

**The sysadmin role for agent governance.** Someone (or some process) plays the sysadmin role for agents: monitoring agent behavior, enforcing policy, resolving conflicts between agents competing for resources, and communicating policy changes. The motd is the policy broadcast. The wall is the urgent directive. The accounting log is the audit trail. All of these are standard Unix sysadmin tools, now applied to agent governance.

**Agent-to-agent courtesy.** Agents on a shared system should be courteous to each other, just as users were. An agent that monopolizes the API endpoint affects other agents. The norms: share resources, yield to higher-priority work, and communicate status through .plan files so other agents know what you're doing.

---

## 11. Forgotten and Obscure Patterns

### Zephyr (MIT, 1986)

Zephyr was an institutional messaging system created at MIT as part of Project Athena. It was designed by Ciaran Anthony DellaFera as a solution to two problems: presence detection in a distributed computing environment, and scalable message delivery.

**Architecture.** Zephyr followed Unix's "do one thing, do it well" philosophy, decomposed into several separate programs:

- **zhm (HostManager)**: runs on each client workstation. Mediates between local applications and the Zephyr servers. Acts as a cache and relay.
- **zephyrd (server)**: runs on dedicated server machines. Maintains user subscriptions, presence state, and routes messages.
- **zwgc (WindowGram client)**: the display program. Shows incoming messages as transient X windows ("windowgrams") that appear and can be dismissed, or as text on a terminal.
- **zwrite**: the sending program. Command-line tool to compose and send messages.
- **zlocate**: presence query — is this user logged in? Where?
- **zctl**: subscription management — subscribe to message classes.

**Subscriptions.** Zephyr's subscription model used a three-tuple: (class, instance, recipient). Classes were topics (e.g., "cs-101", "help", "white-magic"). Instances were sub-topics within a class. Recipients were individual users or "*" for everyone subscribed to that class. Users subscribed to the tuples they cared about. Messages were delivered only to matching subscriptions.

**Presence.** Zephyr tracked user presence across the network — not just "logged in" but "logged in at which workstation." This was push-based presence (unlike finger's pull-based model): when you logged in, Zephyr announced it; when you logged out, Zephyr announced that too. Other users could see your presence state change in real time.

**Windowgrams.** Incoming messages appeared as transient X11 windows — small popup rectangles with the message text. They appeared, you read them, you dismissed them (or they timed out). This is the ancestor of every desktop notification toast in modern computing.

**Why Zephyr matters for pane.** Zephyr solved the institutional messaging problem — communication within an organization with hundreds or thousands of users, with topic-based routing, presence, and subscription management. The subscription model (class, instance, recipient) is a content-based routing system. The presence model is push-based awareness. The windowgram is a notification primitive.

For pane's agent model, Zephyr's subscription system maps to agent event subscriptions: an agent subscribes to (class="pull-request", instance="*", recipient="agent.reviewer") and receives all PR-related messages. The presence model maps to agent presence: when an agent starts, the system knows; when it stops, the system knows; other agents and humans can query who's active.

### comsat/biff — Instant notification

Already covered in section 4, but worth emphasizing as a pattern: the separation of notification into a client-side subscription (biff y/n) and a server-side event emitter (comsat) is a clean pub-sub design. The client decides whether to receive; the server decides when to emit. Neither knows about the other's implementation.

### notify in csh — Job completion notification

The C shell's `notify` variable (and the `notify` built-in command) controlled whether background job completion was reported immediately or deferred to the next prompt.

Default behavior: when a background job finishes, the shell waits until just before printing the next prompt to tell you. This avoids interrupting your current work — you find out about completed jobs at a natural pause point (when you press Enter).

With `set notify` or `notify %1`: the shell tells you immediately when the job finishes, interrupting your current line if necessary. This is the same "biff y/n" trade-off — immediate notification vs. deferred notification — applied to process completion rather than mail.

For agents: when an agent completes a background task, should the human be notified immediately (notify) or at the next natural pause (default)? The csh model suggests both options should be available, controlled by the human. The default should probably be deferred (don't interrupt focused work), with the option to enable immediate notification for urgent agents.

### last(1) — Login history

last(1) reads the wtmp file and shows login history: who logged in when, from where, for how long, and when they logged out. `lastb` shows failed login attempts.

For agents: `last agent.reviewer` shows when the agent was active, how long each session lasted, and whether it exited cleanly or crashed. This is the agent activity log — not what it did (that's process accounting), but when it was running.

### The "You have mail" pattern as a general notification

"You have mail" is the simplest possible notification system: a boolean check at a specific moment (login). It doesn't tell you who sent it or what it's about. It just tells you something is waiting. You decide when to deal with it.

This pattern generalizes beautifully for agents: "agent.reviewer has results." Not the results themselves — just the fact that results exist. The user decides when to look. This is the antithesis of the modern notification barrage. It respects attention. It says "something is waiting for you" and stops.

---

## 12. Unexpected Compositions

The patterns above are interesting individually, but the real power emerges when they combine. Here are compositions that become possible when agents are users on a Unix-like system:

### finger + .plan + mail = Agent status and communication

A human checks on an agent: `finger agent.reviewer`. Sees its .plan (what it does), its .project (what it's working on), its idle time (is it active?), and its mail status (has it received new requests?). From this single command, the human has a complete picture of the agent's state — using infrastructure from 1971.

### who + mesg + write = Attention-aware agent interaction

An agent wants to notify the human. It checks `who` to confirm the human is logged in. It checks the human's mesg status. If mesg is y, it `write`s a brief notification. If mesg is n, it sends mail instead (deferred notification). The agent respects the human's attention boundaries using the same mechanisms humans used to respect each other's boundaries in the 1980s.

### cron + mail + .project + finger = Autonomous agent lifecycle

An agent is scheduled via cron to run nightly analysis. When it runs, it updates its .project file to "Running nightly analysis (started 03:00)." When it finishes, it mails results to the human and updates .project to "Last run: Mar 19 03:00, 3 issues found." The human can check the agent's status at any time via finger, or wait for the mail. The entire lifecycle uses four standard Unix facilities, none of which know anything about agents.

### permissions + groups + .plan = Agent capability and intention

An agent's group memberships define what it CAN do (capabilities). Its .plan file defines what it WILL do (intentions). Its .project file defines what it IS doing (current state). These three pieces of information — capability, intention, activity — are the complete picture of an agent's relationship to the system. All three are filesystem-native, inspectable, and use infrastructure that predates the concept of AI agents by decades.

### wall + motd + vacation = Agent governance broadcast

A system-wide policy change: "Deployment freeze effective immediately." The human administrator:

1. Updates `/etc/pane/motd` (persistent policy for agents that restart)
2. Runs `wall` (immediate broadcast to all active agents)
3. Each agent that acknowledges updates its .project to note the freeze

An agent that's mid-task and can't stop immediately sets up a vacation-style response: "Acknowledged deployment freeze. Completing current review (ETA 5 minutes) then halting." Other agents or humans who message it get this auto-response. The governance uses wall for urgency, motd for persistence, and vacation for graceful acknowledgment.

### talk + tmux-sharing = Human-agent pair session

A human starts a `talk` session with an agent. The screen splits: human types in one pane, agent responds in the other. But unlike classic talk (which was just text), this talk session is attached to a shared tmux-style pane — both participants see the same editor buffer, the same file tree, the same terminal output. The human makes a code change; the agent sees it and comments in real time. The agent suggests a fix; the human sees it appear in the shared buffer and accepts or rejects it. Two classic patterns — talk (split-screen conversation) and tmux sharing (shared session) — compose into a collaborative workspace.

### process accounting + .plan = Agent audit chain

Every command the agent runs is logged by process accounting (lastcomm). The agent's .plan file declares what it's supposed to do. Post-hoc audit compares the accounting log against the specification: did the agent stay within its declared scope? This is compliance checking using 1980s infrastructure — the accounting log is the evidence, the .plan is the policy, and the comparison is the audit.

### rwho + finger = Distributed agent presence

In a system with agents running on multiple machines (local models, remote APIs, cloud compute), the rwho pattern provides cross-machine presence: which agents are running where. Combined with finger, you get distributed agent status: `finger agent.reviewer@gpu-server` shows the agent's status on the GPU machine — what it's running, how long it's been active, and its .plan.

---

## 13. Synthesis: What the Multi-User Past Offers the Multi-Inhabitant Future

### The core observation

Classic Unix multi-user UX patterns are not historical curiosities. They are solutions to the *exact problems* that arise when a computing system has multiple concurrent inhabitants who need to communicate, coordinate, share resources, respect boundaries, and maintain awareness of each other's activities.

These patterns became vestigial on single-user PCs because the problems they solved disappeared: there was only one user, so presence detection, inter-user messaging, resource courtesy, and permission boundaries were irrelevant.

Pane reintroduces multiple inhabitants. AI agents are not applications — they are users. They have identities, permissions, home directories, mailboxes, and .plan files. They log in, do work, and log out. They communicate with humans and with each other. They consume shared resources. They need governance.

Every one of these needs was addressed by the Unix multi-user infrastructure, designed and refined over two decades (1970s-1990s) of real-world use in timesharing environments with dozens to hundreds of concurrent users.

### What becomes charming

Some patterns are charming in the agent context — they produce interactions that feel right and delightful:

- **"You have mail."** Three words, after agent.reviewer finishes a code review. Not a notification banner, not a toast, not a badge count. Just: you have mail. Check it when you're ready.

- **The .plan file as agent personality.** An agent's .plan is its self-description, written in natural language, readable by anyone. It's the original social profile, applied to software. `finger agent.reviewer` gives you a sense of *who* (what) this agent is, what it cares about, and what it's working on — the same way finger once told you about the person down the hall.

- **The write(1) notification.** An agent has something to tell you. A brief message appears on your pane, attributed to the agent, at the bottom of your workspace. Like a colleague tapping you on the shoulder. Not a modal dialog, not a notification center — just a message from another inhabitant of your system.

- **The talk(1) conversation.** You want to discuss something with an agent. `talk agent.reviewer` opens a split-pane conversation. You type questions in your half; the agent responds in its half. You can see it "thinking" (streaming). The conversation is ephemeral — when you close it, it's gone, like hanging up a phone call. Or you can log it to mail for persistence.

### What becomes useful

Some patterns are directly useful — they solve real problems in human-agent interaction:

- **mesg n as do-not-disturb.** A single, well-understood mechanism for "agents, leave me alone right now." Every agent checks this before interrupting. Simple, universal, binary.

- **Permissions as agent capabilities.** The agent can do exactly what its file permissions allow. No special capability framework needed — Unix permissions are the capability framework.

- **Process accounting as audit trail.** Every agent action is logged by the kernel. Non-fakeable, non-erasable (if the accounting file has appropriate permissions), and queryable with standard tools (lastcomm, sa). Agent accountability using infrastructure from 1980.

- **cron as agent scheduling.** Agents use cron. Mail delivers results. No orchestration framework, no job queue system, no distributed scheduler. Just cron and mail, the way they've always worked.

- **motd as agent policy.** System-wide agent policy is a text file. Update the file, agents read it. Human-readable, machine-parseable, filesystem-native.

### What becomes powerful

Some patterns become more powerful in the agent context than they ever were in the human context, because agents can use them more systematically:

- **finger + .plan as a queryable agent state model.** Humans updated their .plan files sporadically. Agents can update them continuously — .project always reflects current task state, .plan always reflects current capability. finger becomes a real-time status dashboard, queryable by humans and agents alike, using a protocol from 1971.

- **mail as inter-agent messaging.** Human-to-human mail is slow and informal. Agent-to-agent mail can be structured, typed (attributes on the mail file, per BeOS's pattern), and processed programmatically. The mail spool becomes a message queue. Live queries over mail attributes become agent inboxes filtered by topic, sender, priority. The entire email-as-infrastructure pattern from BeOS, applied to agent communication, using the Unix mail delivery mechanism.

- **vacation(1) as auto-delegation.** Human vacation replies are "I'm away." Agent vacation replies can be "I'm busy, but I've forwarded your request to agent.reviewer-2, who will handle it." Active delegation, not just acknowledgment.

- **The subscription model (Zephyr-style) for agent events.** Agents subscribe to event classes. When a relevant event occurs (PR opened, build failed, deploy requested), subscribed agents are notified through the messaging infrastructure. The subscription model means agents only hear about what they care about — no polling, no global event bus, just targeted delivery based on declared interest.

- **Shared sessions as collaborative workspaces.** A human and an agent sharing a pane session is a strictly more powerful version of tmux sharing, because the agent can do things that a second human couldn't: instant code analysis, real-time type checking, automated refactoring — all within the shared session, visible to the human in real time.

### The deeper principle

Dennis Ritchie said Unix was designed "to encourage close communication" among "a fellowship." The multi-user UX patterns are the mechanisms of that fellowship — the ways inhabitants of a shared system communicate, coordinate, share, and maintain awareness of each other.

Pane introduces a new kind of fellowship: humans and AI agents inhabiting the same system. The mechanisms of the old fellowship — write, talk, finger, mail, permissions, accounting, motd, cron — may be exactly the mechanisms this new fellowship needs. Not because they're quaint or retro, but because they were designed for multi-inhabitant systems, and pane is a multi-inhabitant system.

The patterns are sitting right there in the Unix toolbox. They've been waiting for the inhabitants to return.

---

## Sources

### Historical and Technical Documentation

- [write(1) man page](https://man7.org/linux/man-pages/man1/write.1.html) — inter-user terminal messaging
- [RFC 742: Name/Finger Protocol](https://www.rfc-editor.org/rfc/rfc742) (December 1977) — original finger specification
- [RFC 1288: The Finger User Information Protocol](https://www.rfc-editor.org/rfc/rfc1288) (December 1991) — revised finger specification
- Ritchie, Dennis M. ["The Evolution of the Unix Time-sharing System."](https://www.read.seas.harvard.edu/~kohler/class/aosref/ritchie84evolution.pdf) AT&T Bell Laboratories Technical Journal, 1984.
- [GNU Accounting Utilities Manual](https://www.gnu.org/software/acct/manual/accounting.html) — process accounting

### Finger and .plan Files

- [Les Earnest, talk, gold medal for FINGER](https://exhibits.stanford.edu/ai/catalog/hg950nz1220) — Stanford AI Lab exhibit on finger's creation
- ["Finger: The First Social Software"](https://www.somanymachines.com/tx/finger-the-first-social-software/) — finger as social network precursor
- ["The Carmack Plan"](https://garbagecollected.org/2017/10/24/the-carmack-plan/) — John Carmack's .plan file practices
- ["Rediscovering the .plan File"](https://dev.to/solidi/rediscovering-the-plan-file-4k1i) — history and modern relevance
- [John Carmack .plan archive](https://github.com/ESWAT/john-carmack-plan-archive) — complete .plan file collection

### Inter-User Communication

- ["Communicating with other users on the Linux command line"](https://www.networkworld.com/article/968450/communicating-with-other-users-on-the-linux-command-line.html) — Network World overview
- ["Talk and ytalk nostalgia"](https://www.cambus.net/talk-and-ytalk-nostalgia/) — historical recollection with technical details
- [ytalk(1) man page](https://linux.die.net/man/1/ytalk) — multi-user chat
- [biff(1) man page](https://man.cx/biff) — mail notification

### Zephyr

- [Zephyr protocol — Wikipedia](https://en.wikipedia.org/wiki/Zephyr_(protocol)) — architecture and history
- [Zephyr source repository](https://github.com/zephyr-im/zephyr) — institutional messaging system

### Session Sharing

- ["Remote Pair Programming With SSH & tmux"](https://hamvocke.com/blog/remote-pair-programming-with-tmux/) — multi-user terminal sharing
- [wemux — Multi-User Tmux Made Easy](https://github.com/zolrath/wemux) — mirror/pair/rogue modes

### Berkeley r-commands and Network Presence

- [Berkeley r-commands — Wikipedia](https://en.wikipedia.org/wiki/Berkeley_r-commands) — rwho, ruptime, and the r-command suite

### Process Accounting

- ["Process Accounting" — Linux Journal](https://www.linuxjournal.com/article/6144) — accounting infrastructure
- [acct(5) man page](https://man7.org/linux/man-pages/man5/acct.5.html) — accounting file format

### Message of the Day and Login Sequence

- [motd(5) man page](https://www.man7.org/linux/man-pages/man5/motd.5.html) — message of the day
