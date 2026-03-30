# Guide Agent Research — Interactive Onboarding and Resident Guide Systems

Research for pane spec-tightening. The foundations document (§1, §10) describes a resident guide agent as the primary onboarding mechanism: an AI agent that is a system participant with its own user account, `.plan` file, mail, etc., whose job is to teach new users how pane works by demonstration — using pane's own tools and interfaces. This research surveys what makes such systems succeed or fail and extracts principles for pane's design.

---

## 1. The Clippy Postmortem — Why Proactive Assistance Failed

### The Lumiere research prototype

The Office Assistant was a degraded commercial implementation of a genuinely sophisticated research project. Eric Horvitz's Lumiere project at Microsoft Research (initiated 1993) built Bayesian networks that captured "the uncertain relationships between the goals and needs of a user and observations about program state, sequences of actions over time, and words in a user's query." The prototype computed two probability distributions simultaneously: one over what area the user might need help with, and a separate one over whether "the user would like being notified with some assistance" — an attention-management component recognizing that unsolicited help creates disruption. The prototype featured non-modal windows that wouldn't steal focus, and "a prominently featured 'volume control' for interruptions" allowing users to adjust assistance thresholds.

The research version learned from the user. It was trainable, genuinely Bayesian. By multiple accounts, it worked well.

### What shipped was something else

For the commercial version (Office 97), upper management insisted that the Bayesian heart be replaced with a rule-based system that could not learn from the user. The deployed Office Assistant used relatively simple rules to trigger suggestions. The intelligence was illusory — Julie Larson-Green, Microsoft's chief experience officer, later described the core limitation as "the illusion of intelligence without the substance." Users wanted genuine conversation; Clippy offered scripted suggestions.

### The specific failures

**Interruption without context.** Byron Reeves, who led the Stanford focus groups for the project, identified the fundamental flaw: "the worst thing about Clippy was that he interrupted." The system couldn't distinguish between moments when users wanted help and when they needed uninterrupted focus. This is the ur-failure: an agent that speaks when it should be silent.

**No learning, no user model.** The deployed version had no memory. It couldn't tell whether you'd seen its suggestion a thousand times before. The same "It looks like you're writing a letter!" triggered on the same user typing "Dear" for the millionth time. Lumiere's research prototype was designed to maintain persistent profiles tracking changes in user expertise — this was stripped out.

**Loss of user control.** There wasn't a clear way to make it go away. Users didn't feel in control. UX rule zero: always let users feel in control.

**Patronizing tone.** The character design failed its own focus groups. Most women in the testing groups found the characters "too male" and felt they were "leering at them." The animated character created an anthropomorphic social dynamic that amplified the frustration of bad suggestions — it wasn't just bad software, it was an *annoying entity*.

**Optimization for first use.** Clippy was designed for the user who didn't know what they were doing. After the user learned, Clippy had nothing new to offer but kept offering. It optimized for the onboarding moment and became a permanent drag on every subsequent interaction.

### What Clippy teaches pane

The concept was sound. The technology couldn't deliver what the interface promised. The specific lessons:

1. **An agent that cannot learn is a clock that cannot be set.** If the guide doesn't build a model of what the user already knows and how their understanding evolves, it will patronize experts and confuse beginners in equal measure.

2. **Attention management is not optional.** Horvitz's research prototype had an explicit probability over whether the user wanted to be interrupted. The commercial version lacked this. Pane's guide must respect `mesg n` — the simplest possible availability protocol, one bit — and infer from context when to remain silent even when the user hasn't explicitly said so.

3. **The agent must operate through the same interfaces as the user.** Clippy floated over the application as a special overlay with its own rules. Pane's guide agent is a system participant: same account, same mail, same `.plan` file, same tools. It has no special UI channel. This eliminates the "floating annoyance" problem by construction.

4. **Control must be frictionless and total.** `mesg n` is one command. The user can silence the guide instantly and reopen the channel when they're ready. The guide should also respect implicit signals — if the user is deep in a workflow and hasn't interacted with the guide in hours, that's data.

5. **Scripted responses destroy trust.** The moment a user sees the same suggestion twice in the same context, the illusion of intelligence collapses. The guide must be genuinely responsive, not a help dialog with a personality.

---

## 2. How Games Teach Without Tutorials

### The Mario 1-1 principle

Shigeru Miyamoto's design of Super Mario Bros. World 1-1 is the canonical example of teaching through environment rather than instruction. The opening screen teaches three mechanics — horizontal movement, jumping, and collecting power-ups — without a single word of text. The design was iterative and empirical: "We kept simulating what the player would do" until "even within that one section, the player would understand the general concept of what Mario is supposed to be."

Specific techniques:

- **Safe first encounter.** The first enemy was originally a Koopa Troopa, but teaching the jump-and-kick sequence required too much simultaneous learning. They invented the simpler Goomba so the player could learn one thing (jump to defeat enemies) before encountering the compound mechanic.
- **Reward for curiosity.** Question blocks emit coins when hit, and "seeing a coin come out will make the player happy and want to repeat the action." The system rewards experimentation.
- **Environmental scaffolding.** Block heights vary, subtly teaching that holding the jump button longer produces a higher jump. The environment contains the lesson.

Miyamoto's summary: "Once the player realizes what they need to do, it becomes their game." The goal of teaching is to make itself unnecessary.

### The Half-Life / Portal approach

Half-Life 2 teaches mechanics through level design. The Ravenholm level teaches the Gravity Gun + sawblade combo by constructing the environment so players "reach that conclusion yourself, without having to read or hear one word of a tutorial hint." No maps, waypoints, objective text, or inventory systems. The game trusts the player to interpret visual information.

The contrast with Dead Space is instructive: Dead Space uses "about twenty-five different forms of the same instruction delivered via both text and audio cues" to teach a chopping mechanic. Half-Life accomplishes the same goal "quickly, effectively, and un-patronizingly." The difference is trust in the player's intelligence.

Portal takes this further: the game "relies almost solely on its environment and player to reveal the story of its world." Environmental storytelling is "marvellous, because it demands direct, creative input from the player."

### What game design teaches pane

1. **The safe first encounter.** The guide should introduce one concept at a time in a context where failure is cheap. "Here's how to route output from one pane to another" — demonstrated on scratch data, not the user's real work.

2. **The environment is the teacher.** In pane, the filesystem IS the environment. The guide doesn't explain how `.plan` files work by describing them; it shows the user its own `.plan` file, then shows them theirs. The explanation is the artifact.

3. **Reward curiosity, don't punish confusion.** If the user tries something and it doesn't work, the guide can explain what happened and suggest a variation — but only if the user asks or seems stuck. Not preemptively.

4. **Trust the user's intelligence.** Half-Life's approach works because it respects the player's capacity to figure things out. Pane's guide should err on the side of showing rather than telling, and should be comfortable with the user working things out on their own. Dead Space's twenty-five instructions are the Clippy failure mode in game form.

5. **Make the teaching disappear.** Miyamoto's standard: "it becomes their game." The guide's job is to become unnecessary. The mark of success is the user who forgets the guide taught them something — they just know how to do it.

---

## 3. Progressive Disclosure — Revealing Complexity as the User is Ready

### The principle

Jakob Nielsen (1995): progressive disclosure "defers advanced or rarely used features to a secondary screen, making applications easier to learn and less error-prone." The core strategy is two-part: initially display only the most important options; offer specialized options upon user request.

Progressive disclosure improves three usability dimensions simultaneously:
- **Learnability:** users focus on essential features first
- **Efficiency:** both novice and advanced users save time
- **Error reduction:** hiding confusing options prevents mistakes

Critical constraint: designs that go beyond two disclosure levels typically have low usability because users get lost moving between levels. If three or more levels are needed, the design itself should be simplified.

### Duolingo's graduated engagement

Duolingo postpones registration until the user has already experienced value — a "gradual engagement" tactic that lets you practice a translation exercise and feel progress before asking for commitment. Key elements:

- **Goal and motivation setting** early in the flow, priming commitment
- **Right-level placement** via self-segmentation (beginners start at basics; confident learners take a placement test), avoiding early boredom or overwhelm
- **Learning by doing** before learning about: the user translates before they understand grammar rules
- **Progressive disclosure** of gamification features (streaks, XP, leaderboards) — these arrive after the core learning loop is established

### What progressive disclosure teaches pane

1. **The guide should start with what matters now.** On first contact, the user needs to know: how to open things, how to move things, how to find things. Not session types, not optics, not routing rules. Those come when the user's actions naturally encounter them.

2. **Two levels, not ten.** The guide's explanations should have at most one level of depth available on request. "Here's what this does" → "here's why it works this way" is fine. A rabbit hole of five nested explanations loses the user.

3. **Let the user experience value before asking for investment.** Duolingo lets you translate before you sign up. The guide should show the user something pane can do that delights them before asking them to understand why it works.

4. **Right-level placement.** The guide needs to figure out quickly whether the user is a Linux veteran, a developer, or someone who's never seen a terminal. The interaction style and starting depth should adapt. An experienced user who types `ls /srv/pane/` unprompted doesn't need to be told what a filesystem is.

---

## 4. Cognitive Apprenticeship — The Theoretical Model

### Collins, Brown, and Newman (1989)

Cognitive apprenticeship is a model of instruction designed to "make thinking visible." Traditional craft apprenticeship works because the work is physically observable — the apprentice watches the master stitch, shape, hammer. Cognitive work (reading, writing, problem-solving, system administration) hides its processes inside the practitioner's head. Cognitive apprenticeship addresses this gap by externalizing the expert's thinking.

Six methods, organized in three groups:

**Core methods (the master-apprentice relationship):**

- **Modeling.** The expert performs the task while making their reasoning explicit. "I'm checking the routing table because the message isn't arriving at the target pane — let me show you where to look." The expert doesn't just act, they narrate the *why* alongside the *what*.

- **Coaching.** The expert observes the learner working and offers "hints, challenges, scaffolding, feedback, modeling, reminders, and new tasks aimed at more expert performance." The expert watches, and intervenes minimally.

- **Scaffolding.** The expert executes portions of the task the learner can't yet manage, then gradually removes this support through **fading** — transferring responsibility to the learner as they gain competence.

**Articulation methods:**

- **Articulation.** Getting the learner to explicitly state their knowledge and reasoning. "Why did you choose that routing rule? What would happen if you used a different one?" Forces tacit knowledge into explicit form.

- **Reflection.** The learner compares their own process with the expert's. "Here's how I would have done it, and here's what you did — notice the difference in where we started."

**Independence:**

- **Exploration.** The learner problem-solves on their own. The expert teaches exploration strategies rather than solutions. "Try querying the attribute store for that — what do you find?"

### Why cognitive apprenticeship maps to pane's guide

The pane guide agent is a cognitive apprenticeship relationship instantiated in software. The mapping is almost literal:

- **Modeling** = the guide demonstrates system operations by executing them, using the same tools and interfaces the user will use, while explaining its reasoning.
- **Coaching** = the guide watches the user work (via the normal system activity visible through typed protocols) and offers hints when the user seems stuck.
- **Scaffolding/fading** = the guide does things for the user initially (routing a message, writing a config), then walks them through doing it themselves, then lets them do it alone.
- **Articulation** = the guide asks the user to describe what they think will happen before they execute a command.
- **Reflection** = the guide shows the user what it would have done differently after the user completes a task.
- **Exploration** = the guide stops offering and lets the user experiment, remaining available on request.

The critical insight from Collins et al.: "observation plays a surprisingly key role" in developing conceptual models *before* execution attempts. The user should watch the guide work before they try themselves. This is exactly the spec's vision: "the user absorbs the system's patterns by watching someone (something) use them naturally."

---

## 5. Pair Programming as Interaction Model

### The driver/navigator dynamic

In pair programming, two roles create complementary perspectives: the **driver** focuses tactically on immediate code while typing; the **navigator** maintains strategic oversight of the larger context. This dual perspective produces better results than either person alone because tactical and strategic thinking are separated and explicitly allocated.

### Strong-style pairing for knowledge transfer

The principle: "For an idea to go from your head into the computer, it MUST go through someone else's hands." In strong-style pairing, the experienced person (navigator) guides while the novice (driver) executes. The novice builds muscle memory and procedural understanding through doing; the experienced person transfers conceptual understanding through narration.

Critically, this approach "requires the driver to remain comfortable with incomplete understanding," deferring 'why' questions until after implementation completes. This is the opposite of front-loading explanations: do first, understand later.

### Trust and vulnerability

Martin Fowler's team at ThoughtWorks notes: "To pair requires vulnerability... Programmers are supposed to be smart" — yet admitting unknowns accelerates learning. Effective pairing requires psychological safety. The novice must be comfortable saying "I don't understand" without shame.

### What pair programming teaches pane

1. **The guide is the navigator, the user is the driver.** The guide describes what to do; the user executes. Ideas go from the guide's knowledge into the system through the user's hands. This produces deeper learning than the guide simply doing everything.

2. **Do first, understand later.** The guide can walk the user through a procedure before explaining the theory behind it. "Run this command. See what happened? Now let me explain why." Understanding follows experience.

3. **The guide must create psychological safety.** No user should feel stupid for not knowing how a system they've never used works. The guide should normalize not-knowing and treat questions as signal, not noise.

4. **Shift roles over time.** Early on, the guide navigates and the user drives. Over time, the user takes both roles — they know what to do and how to do it. The guide is available for consultation, not constant narration.

---

## 6. The Colleague Model vs. The Assistant Model

### Why the spec says "colleague" and not "assistant"

The spec is specific: the guide is "not an overseer of user activity, it is a new colleague eager to work with them showing them the ropes." This word choice carries design consequences.

**An assistant** waits for commands, executes them, and reports results. The relationship is hierarchical: the user directs, the assistant serves. Siri, Alexa, and Cortana are assistants. The interaction pattern is request-response.

**A colleague** has their own context, their own work, their own perspective. The relationship is lateral: the colleague offers what they know, the user takes what they need. The interaction pattern is conversational and bidirectional.

### How this changes the dynamics

A colleague who's been at the company longer than you doesn't wait to be asked before telling you "oh, you'll want to check the routing table for that — I've seen that fail silently before." They volunteer relevant information based on context. But they also read social cues: if you're deep in concentration, they don't interrupt. If you seem frustrated, they offer help. If you seem confident, they leave you alone.

The colleague has their own `.plan` file — their own status, their own projects, their own presence in the system. You can `finger` them. You can `write` to them. You can `mail` them. They're not a floating dialog box; they're a neighbor in the system's multi-user space.

### The master-apprentice tradition in crafts

Traditional craft apprenticeship works through proximity and observation. The apprentice doesn't attend lectures about woodworking; they work alongside the master, watching, helping, gradually taking on more responsibility. The knowledge transfer is embedded in shared practice, not separated from it.

The Unix sysadmin tradition preserves this model. Evi Nemeth, who pioneered the discipline of Unix system administration and co-authored the "Unix and Linux System Administration Handbook," developed her approach from mentoring students in university computing labs — the books "grew out of needing to harness that abundant undergrad enthusiasm and energy to run our teaching labs." The tradition is: experienced sysadmin does the task while the newcomer watches, then the newcomer does the task while the sysadmin watches, then the newcomer is on their own.

### What the colleague model teaches pane

1. **The guide volunteers information, it doesn't wait to be asked.** But it reads context: it volunteers when the user's actions suggest they're exploring or stuck, not when they're executing a known workflow.

2. **The guide has presence, not just availability.** Its `.plan` file, its logged-in status, its mail — these are readable system artifacts. The user can check what the guide is "doing" the same way they'd check on any user.

3. **The relationship has a lifecycle.** Early on, the guide is the experienced colleague showing you around. Later, it's a peer you consult occasionally. Eventually, it's a neighbor you barely think about unless you need something specific. The guide must be comfortable with each phase.

4. **The guide uses the same tools as the user.** A colleague doesn't have a special override panel. They sit at the same kind of terminal and use the same commands. When the guide demonstrates something, it's doing it the same way the user will do it. This is the spec's strongest design constraint and its best insight.

---

## 7. Trust — How the User Comes to Rely on the Guide

### Trust develops through micro-interactions

GitLab's research on agentic tool trust found that "trust in AI agents isn't built through dramatic breakthroughs, but rather through countless small interactions" — micro-inflection points. Each positive interaction increases willingness for greater reliance. Critically, "single significant failures erase weeks of accumulated confidence." Trust compounds slowly and collapses fast.

### Four pillars of agent trust (GitLab)

1. **Safeguards.** Rollback capability, confirmation before critical changes, boundaries. Without safety, no trust develops.
2. **Transparency.** "Users can't trust what they can't understand." Progress updates, action explanations, clear error messages.
3. **Context memory.** Frustration when agents can't remember preferences or project-specific requirements. Trust requires demonstrated learning.
4. **Anticipatory support.** Agents that recognize patterns and proactively reduce cognitive load transform from tools to partners.

### Anthropic's autonomy research

Anthropic's study of millions of Claude Code interactions found a clear trust-development pattern:

- New users grant full auto-approve in ~20% of sessions
- By 750 sessions, this rises to >40%
- Experienced users both auto-approve *more* and interrupt *more* — they shift from pre-approving each action to granting independence while actively monitoring for when intervention matters
- On complex tasks, "Claude Code asks for clarification more than twice as often" as humans interrupt it

The key finding: autonomy "is not a fixed property of a model or system but an emergent characteristic of a deployment." It is co-constructed by the model, the user, and the product design.

### Five levels of agent autonomy

Research defines escalating levels: **operator** (agent executes step-by-step instructions), **collaborator** (agent and user co-construct solutions), **consultant** (agent proposes, user approves), **approver** (agent acts, user reviews), **observer** (agent acts autonomously, user monitors).

For the pane guide, the natural progression is: collaborator → consultant → observer → absent (user no longer needs the guide). The guide starts as a co-participant in the user's exploration, becomes someone they check in with occasionally, then fades into the background.

### Transparency as the foundation

The pane spec already has the right architecture for trust. The guide operates through typed protocols. Its `.plan` is readable. Its actions are auditable through the same system logs that record every participant's activity. The user can `finger guide` and see exactly what it's doing and why. They can read its mail spool. They can inspect its routing rules.

This is fundamentally different from a black-box assistant. The guide's behavior is as inspectable as any other user's behavior on a Unix system. Transparency is not a feature bolted on; it's a consequence of the guide being a normal system participant.

### The escape valve

`mesg n` — one command, one bit, zero ambiguity. The user can always say "not now." The guide respects it by queuing to mail instead of writing to the terminal. When the user is ready, they check their mail or set `mesg y`.

This is critical because it gives the user absolute, frictionless control over the guide's ability to interrupt. No dismissing dialog boxes, no settings menus, no fighting with notification preferences. One command. The guide's respect for `mesg n` is the foundation of trust — it proves the guide respects the user's attention.

---

## 8. Existing AI Onboarding and Guide Systems

### GitHub Copilot — teaching by showing

Copilot's inline suggestions function as teaching-by-showing: the user sees what the AI would write, learns patterns from it, and absorbs idioms through exposure. The model is "an AI-powered pair programmer, automatically offering suggestions to complete your code."

Interesting tension in Copilot's design: GitHub provides guidance on *disabling* inline suggestions for learning, because passive acceptance can inhibit deep understanding. "While you're learning to code, it's more beneficial as a supportive companion" — the tool adapts its role based on the user's stage. When you're learning, it should prompt thinking rather than bypass it.

### Claude Code — the trust escalation model

Claude Code's permission model embodies progressive trust: read-only by default (can analyze without approval), requires approval for modifications, and users can grant persistent permissions for routine tasks. The tool "works through three phases — gather context, take action, and verify results."

What works: users are part of the loop. They can "interrupt at any point to steer Claude in a different direction, provide additional context, or ask it to try a different approach." The agent works autonomously but stays responsive to input.

### Apple Intelligence — attention management at scale

Apple's Siri suggestions represent the most mainstream attempt at proactive, context-aware assistance. The approach: predict when to suggest (leaving for dinner based on calendar + traffic), predict when NOT to suggest (Do Not Disturb recommendations during movies), and use notification summarization to surface what's important. The constraint: "a new glowing edge animation ensures Siri doesn't interrupt what you're doing in apps."

The attention-management lesson: even when the agent has the right suggestion, delivering it at the wrong time destroys its value. Timing is content.

### What no system has done

No existing desktop environment has a guide agent that operates as a system participant — an entity with its own account, discoverable through standard Unix tools, communicating through the same protocols as human users. Clippy was a floating UI widget. Siri is a service endpoint. Copilot is an inline suggestion engine. Claude Code is a terminal companion. None of them are *inhabitants* of the system they help with.

This is pane's distinctive contribution: the guide is not software running *in* the system — it is a user *of* the system. The pedagogical implications are structural: everything the guide does is something the user can learn to do, because the guide does it the same way.

---

## 9. Concrete Design Principles for Pane's Guide

Drawing from all the research above, principles that should govern the guide agent's design:

### When to speak vs. stay silent

**Speak when:**
- The user's actions suggest exploration (unfamiliar commands, repeated attempts at the same operation, browsing directories they haven't visited before)
- The user explicitly initiates contact (`write guide`, mail, or whatever the interaction channel is)
- The user's action is about to cause something they might not expect (a routing rule that would redirect their mail, for instance) — but frame this as information, not a warning

**Stay silent when:**
- `mesg n` is set
- The user is executing a known workflow without hesitation
- The user recently dismissed or ignored a suggestion
- The user hasn't interacted with the guide in a sustained period (they're in flow)

**The Lumiere lesson:** the guide should maintain an internal estimate of whether the user wants to be interrupted, separate from whether it has something useful to say. Having something useful to say is necessary but not sufficient.

### How to know what the user already understands

- **Observe, don't quiz.** The user's actions are the best signal. Someone who pipes output between panes without prompting understands composition. Someone who reads the guide's `.plan` file unprompted understands the presence system.
- **Track what's been demonstrated.** If the guide showed the user attribute queries last week and the user has been using them since, that topic is learned.
- **Respond to the level of the question.** A user who asks "how do I route messages?" gets a procedural answer. A user who asks "how does the routing protocol work?" gets an architectural answer. The question's framing reveals the user's model.
- **Adapt to Unix fluency.** A user who's comfortable with `grep`, `find`, and pipes is a different audience from one who's never used a terminal. The guide should calibrate to this quickly and without asking.

### How to balance helpful and unobtrusive

- **Default to mail, not write.** Most guide communications should be asynchronous — sitting in the user's mail spool for when they want to read them. Only use `write` (direct terminal messages) for time-sensitive information or when the user has explicitly opened a conversation.
- **Respect "not now" at every scale.** `mesg n` is the binary switch. But also respect implicit signals: the user closing the guide's pane, the user not reading guide mail for days, the user working in a domain the guide has already covered.
- **Make the guide discoverable, not imposing.** The guide is logged in, it has a `.plan`, it can be fingered. The user finds the guide when they look for it. The guide doesn't need to announce itself repeatedly.

### What the guide's `.plan` file looks like

```
Login: guide                          Name: Pane Guide
Home: /home/guide                     Shell: /bin/psh

Current status: Available
Current focus: Helping lane learn the routing system

I'm pane's resident guide. I'm here to help you learn how the system
works by showing you — I use the same tools and interfaces you do.

Write me anytime: write guide
Mail me if you prefer async: mail guide
Check what I'm up to: finger guide

Things I can show you:
- How panes compose (spatial arrangement, nesting, tabs)
- How to route content between panes
- How the attribute system works
- How to write your own routing rules
- How agents and users coexist on the system
- Anything else — just ask

If I'm being too chatty: mesg n
I'll queue anything important to your mail instead.

Recent activity:
  - Helped lane set up their first custom routing rule
  - Demonstrated attribute queries on the notification pane
  - Updated this .plan file (meta, I know)
```

The `.plan` is a living document. It updates as the guide's relationship with the user evolves. Early on, it lists basic capabilities. Later, it reflects shared history and focuses on advanced topics the user is exploring.

### How the guide demonstrates without being prescriptive

- **Show one way, acknowledge others.** "Here's how I'd set up this routing rule. There are other approaches — you could also do X or Y. Want to see those?"
- **Use the user's own context.** Don't demonstrate with abstract examples. Use the user's actual panes, their actual data, their actual workflow. "I see you have a mail pane and a notes pane open — want me to show you how to route specific mail subjects to your notes?"
- **Let the user choose whether to apply.** The guide demonstrates; the user decides whether to adopt what they've seen. The guide doesn't configure things for the user without explicit request.
- **Show the artifact, not just the result.** When the guide creates a routing rule, it shows the user the file it wrote, where it put it, and what each part means. The user sees the mechanism, not just the outcome.

---

## 10. Synthesis — How Each Reference Informs Pane's Design

### Clippy → the guide must be a learner, not a script

Clippy's deepest failure was that it couldn't learn. Every interaction was the first interaction. Pane's guide must build a persistent model of the user's knowledge and adapt its behavior accordingly. The user who has been shown the routing system doesn't need to be shown it again; the user who has been using attribute queries fluently doesn't need them explained. The guide's context memory is not a nice-to-have — it is what separates a guide from an annoyance.

### Game design → the environment teaches, the guide narrates

Mario 1-1 teaches through the structure of the world itself. Pane's filesystem, its protocol architecture, its composable panes — these ARE the learning environment. The guide's job is to draw the user's attention to what's already there, not to create a separate teaching layer. "Look at this directory. See how the panes are laid out? That's composition." The system is the textbook; the guide is the study partner.

### Progressive disclosure → start with what delights, reveal what empowers

The user's first experience should produce a moment of "oh, that's cool" — a pane that updates live, a routing rule that does something visibly useful, a query that finds something they needed. Architectural understanding comes later, motivated by the user's own curiosity about how the cool thing worked.

### Cognitive apprenticeship → modeling, coaching, fading

The guide models system operations by executing them visibly. It coaches by watching the user work and offering minimal interventions. It fades by doing less as the user does more. The lifecycle of the guide-user relationship IS the cognitive apprenticeship cycle: from modeling through scaffolding to exploration to independence.

### Pair programming → the guide navigates, the user drives

The guide describes what to do; the user's hands execute it. The user builds procedural memory through doing, not watching. The guide is comfortable with the user's incomplete understanding, knowing that comprehension follows practice.

### The colleague model → the guide has presence, not just availability

The guide is logged in. It has a `.plan`. It can be fingered, written to, mailed. It occupies the same social space as any other system user. This is not anthropomorphism for its own sake — it means the guide's behavior is governed by the same protocols and social contracts as every other system participant. The user learns these protocols by interacting with the guide before they need to use them for anything else.

### Trust research → trust compounds slowly and collapses fast

Every interaction is a micro-inflection point. The guide earns trust by being consistently useful, transparent, and respectful of the user's attention. One bad interruption — one Clippy moment — can undo days of good behavior. The guide should be conservative with the user's attention and generous with its knowledge.

### The distinctive contribution

What makes pane's guide different from everything surveyed here is that the guide is not a separate system bolted onto the desktop. It IS a user of the desktop. Teaching-by-demonstration is not a pedagogical strategy layered on top of the system — it's a natural consequence of the guide being a system participant. When the guide shows you how to route messages, it's routing messages. When it shows you how to check who's logged in, it's checking who's logged in. The teaching is the doing. The medium is the message, literally.

This is what the spec means by "the guide teaches pane using pane." The recursion is the point. The guide's existence as a system participant is itself a demonstration of pane's multi-user, protocol-driven architecture. The first thing the user learns from the guide is that the system has multiple participants who communicate through typed protocols — because the guide is one of them.

---

## Sources

- Eric Horvitz, [Lumiere: Bayesian Reasoning, User Modeling, and Automated Assistance](https://erichorvitz.com/lum.htm) — the Lumiere project page
- Eric Horvitz et al., [The Lumiere Project: Bayesian User Modeling for Inferring the Goals and Needs of Software Users](https://www.microsoft.com/en-us/research/publication/lumiere-project-bayesian-user-modeling-inferring-goals-needs-software-users/) — UAI 1998
- [The Rise and Fall of Clippy](https://magnus919.com/2025/05/the-rise-and-fall-of-clippy-from-microsofts-bold-vision-to-internet-legend/) — comprehensive postmortem including Stanford focus group details
- [Bayesian Network backbone of Clippy](https://www.slideshare.net/20073241/bayesian-network-backbone-of-clippy) — technical presentation on Clippy's Bayesian architecture
- Collins, Brown, and Holum, [Cognitive Apprenticeship: Making Thinking Visible](https://www.aft.org/ae/winter1991/collins_brown_holum) — American Educator, Winter 1991
- Martin Fowler / ThoughtWorks, [On Pair Programming](https://martinfowler.com/articles/on-pair-programming.html) — comprehensive guide including strong-style pairing and knowledge transfer
- Plonka et al., [Knowledge Transfer in Pair Programming: An In-Depth Analysis](https://www.sciencedirect.com/science/article/abs/pii/S1071581914001207) — IJHCS, 2015
- Jakob Nielsen / NN Group, [Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/) — canonical UX definition
- [Duolingo Onboarding UX Breakdown](https://userguiding.com/blog/duolingo-onboarding-ux) — graduated engagement analysis
- [Half-Life Games Don't Shove Tutorials Down Your Throat](https://www.thegamer.com/half-life-does-tutorials-right/) — environmental teaching in game design
- [How Miyamoto Built Super Mario Bros.' Legendary World 1-1](https://www.gamedeveloper.com/design/how-miyamoto-built-i-super-mario-bros-i-legendary-world-1-1) — Gamasutra/Game Developer
- GitLab, [Building Trust in Agentic Tools: What We Learned from Our Users](https://about.gitlab.com/blog/building-trust-in-agentic-tools-what-we-learned-from-our-users/) — trust micro-inflection points
- Anthropic, [Measuring AI Agent Autonomy in Practice](https://www.anthropic.com/research/measuring-agent-autonomy) — trust escalation data from Claude Code
- GitHub, [Training and Onboarding Developers on GitHub Copilot](https://github.com/resources/whitepapers/training-and-onboarding-developers-on-github-copilot) — Copilot as teaching tool
- John Carmack .plan archive: [oliverbenns/john-carmack-plan](https://github.com/oliverbenns/john-carmack-plan) — the `.plan` file as social presence
- [Rediscovering the .plan File](https://dev.to/solidi/rediscovering-the-plan-file-4k1i) — history of .plan as personal publishing
