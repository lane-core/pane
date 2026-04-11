---
type: reference
status: current
supersedes: [plan9/papers_writing_voice]
sources: [plan9/papers_writing_voice, plan9/foundational_paper]
created: 2026-04-10
last_updated: 2026-04-10
importance: high
keywords: [plan9, voice, writing_style, three_tier, rhetoric, technical_writing, manifesto, engineering_report]
related: [reference/plan9/_hub, reference/plan9/foundational, policy/technical_writing]
agents: [pane-architect, be-systems-engineer, plan9-systems-engineer]
---

# Plan 9 papers — writing voice

Detailed analysis of 12 papers from `reference/plan9/papers/`.
The three-tier voice model used by `policy/technical_writing`.

---

## Three tiers of the Plan 9 voice

Most expansive to most compressed:

### Tier 1: The paper (architecture exposition)

**Opens with what things do, not what they are:**

> "A central file server stores permanent files and presents them
> to the network as a file hierarchy exported using 9P."

**States properties as flat facts:**

> "There is no ftp command in Plan 9."
> "Plan 9 has no notion of 'teletype' in the UNIX sense."
> "There is no superuser."

**Concrete examples immediately after abstraction:** every
mechanism is illustrated with a command, a file path, or a code
snippet. Never describes a concept without grounding it in
something you can type.

**Justifications follow descriptions, stated as consequences:**

> "This is a different style of use from the idea of a 'uniform
> global name space'."

The distinction is stated after the mechanism is described, as a
clarifying note, not a preamble.

**Self-critical where warranted:**

> "Nonetheless, it is possible to push the idea of file-based
> computing too far."

Honest about limits without hedging.

**Numbers given matter-of-factly:**

> "about 100 megabytes of memory buffers, 27 gigabytes of magnetic
> disks, and 350 gigabytes of bulk storage"

No commentary on whether this is large or small.

**Active voice, present tense, third person for the system.**
First person plural for design decisions: "we adopted", "we
designed", "we decided".

### Tier 2: Man pages (reference documentation)

rio(1), plumber(4), pipe(3), exportfs(4) — the compressed
reference variant. Flat declarative sentences, conditionals
stated as facts, no justification or meta-commentary.

> "Data written to one channel becomes available for reading at
> the other."

> "If none has it open, the message is discarded and a write
> error is returned to the sender."

### Tier 3: Permuted index NAME entries (maximum compression)

From the First Edition Programmer's Manual. Each entry is a verb
phrase, no articles, describes what the thing does:

```
clone − duplicate a fid
clunk − forget about a fid
pipe − two-way interprocess communication
bind, mount, unmount − change name space
alarm − delay, ask for delayed note
walk − descend a directory hierarchy
flush − abort a message
exportfs − network file server plumbing
```

---

## Core voice characteristics (all 12 papers)

**Claim, then immediately substantiate.** Never a claim without an
example, a measurement, or a structural argument in the next
sentence.

**Honest about limitations.** Every paper identifies things that
don't work well or aren't solved yet. This is load-bearing — it
makes the positive claims credible. Not false modesty.

**Comparative rather than absolute.** Ideas positioned relative to
what they replace, not described in isolation. The comparison
does the argumentative work.

**Concrete before abstract.** Examples precede generalizations.
When an abstraction leads, a concrete instance follows within
1–2 sentences.

---

## Rhetorical moves

### Absence as design

State what you don't have as a feature, not an omission:

- "Unlike `make`, `mk` has no built-in rules." (`mk.ms`)
- "The `#if` directive was omitted because it greatly complicates the preprocessor, is never necessary, and is usually abused." (`comp.ms`)

### Tricolon with ascending severity

Three-part constructions where the final element hits hardest:

- "complicates the preprocessor, is never necessary, and is usually abused" (`comp.ms`)
- "compile quickly, load slowly, and produce medium quality object code" (`compiler.ms`)

### Concession as credential

Acknowledge the weight of what you're replacing before changing it:

- "Any successor of the Bourne shell is bound to suffer in comparison." (`rc.ms`)
- "The integration of devices into the hierarchical file system was the best idea in UNIX." (`names.ms`)

### Analogy as structural argument

Analogies do real argumentative work, not decoration:

- "People lock their front doors when they leave the house, knowing full well that a burglar is capable of picking the lock." (`auth.ms`)
- "Acme tells a client what changes that activity wrought... Putting it another way, 8½ enables construction of interactive applications; Acme provides the interaction." (`acme.ms`)

### Self-criticism as a section

Dedicate space to what went wrong, as structure not aside:

- "We implemented solutions several times over several months and each time convinced ourselves — wrongly — they were correct." (`sleep.ms`)
- "Despite five year's experience... we remain dissatisfied with the stream mechanism." (`net.ms`)

### Judgments stated as observations

- "The system seemed restrictive" — not "the system was bad" (`acme.ms`)
- "failed to follow through on some of its own ideas" (`acme.ms`)

### State the size

Size as evidence of simplicity, not bragging:

- "33 lines of C" (`8½.ms` — window creation)
- "560 lines long and has no graphics code" (`acme.ms` — Win)
- "847 lines of code vs 2200 for TCP" (`net.ms` — IL protocol)

### Personal voice with restraint

First person for chronological testimony, not opinion:

- "When I first saw Oberon, in 1988, I was struck by..." (`acme.ms`)

Used sparingly; most Plan 9 papers are third person.

### Parenthetical humor

Dry, brief, in service of the point:

- "(rather, it would be if it weren't so silly.)" (`rc.ms`)
- "Structures are now almost first-class citizens of the language." — "almost" does real work (`compiler.ms`)

---

## Voice by author

- **Pike (solo)** — Most varied: declarative manifesto
  (foundational paper), personal polemic (`acme.ms`), engineering
  trade-offs (`8½.ms`). Common thread: structural comparisons as
  argument.
- **Pike + Thompson + Presotto + others** — More measured,
  slightly longer sentences. Foundational paper and `names.ms`
  are in this collaborative voice.
- **Presotto + Winterbottom (`net.ms`)** — Plainest, most
  transactional. Strongest self-criticism. Engineering-log style.
- **Thompson (`compiler.ms`)** — Most compressed. "Medium quality
  object code" is the most brutally honest self-assessment in
  the corpus.
- **Duff (`rc.ms`)** — Driest humor. Arguments organized around
  a single invariant (no rescanning) with all consequences
  derived from it.
- **Cox, Grosse, Pike, Presotto, Quinlan (`auth.ms`)** — Most
  formal. Broadest authorship, most measured tone.
  Analogy-as-proof pattern.

---

## Application to pane

See `policy/technical_writing` for the operative rule. The voice
maps to pane's three output types:

| Tier | pane output |
|---|---|
| Tier 1 (paper) | `docs/architecture.md`, design specs |
| Tier 2 (man pages) | Rust doc comments (`///`), kit API docs |
| Tier 3 (permuted index) | One-line type briefs, `# Brief` |

For Tier 1, use the **engineering-report voice** (trade-offs,
limitations, sizes), not the manifesto voice of the foundational
paper alone.
