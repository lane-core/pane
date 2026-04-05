# Plan 9 Papers — Writing Voice Analysis

Detailed analysis of 12 papers from `reference/plan9/papers/`.
Extends the three-tier model in `plan9/foundational_paper`.

---

## Core Voice Characteristics (all papers)

**Claim, then immediately substantiate.** Never a claim without an
example, a measurement, or a structural argument in the next
sentence.

**Honest about limitations.** Every paper identifies things that
don't work well or aren't solved yet. This is load-bearing — it
makes the positive claims credible. Not false modesty.

**Comparative rather than absolute.** Ideas positioned relative to
what they replace, not described in isolation. The comparison does
the argumentative work.

**Concrete before abstract.** Examples precede generalizations.
When an abstraction leads, a concrete instance follows within
1-2 sentences.

---

## Rhetorical Moves (new patterns beyond the foundational paper)

### Absence as design

State what you don't have as a feature, not an omission:
- "Unlike `make`, `mk` has no built-in rules." (mk.ms)
- "The `#if` directive was omitted because it greatly complicates
  the preprocessor, is never necessary, and is usually abused."
  (comp.ms)

### Tricolon with ascending severity

Three-part constructions where the final element hits hardest:
- "complicates the preprocessor, is never necessary, and is
  usually abused" (comp.ms)
- "compile quickly, load slowly, and produce medium quality
  object code" (compiler.ms)

### Concession as credential

Acknowledge the weight of what you're replacing before changing it:
- "Any successor of the Bourne shell is bound to suffer in
  comparison." (rc.ms)
- "The integration of devices into the hierarchical file system
  was the best idea in UNIX." (names.ms)

### Analogy as structural argument

Analogies do real argumentative work, not decoration:
- "People lock their front doors when they leave the house,
  knowing full well that a burglar is capable of picking the
  lock." (auth.ms)
- "Acme tells a client what changes that activity wrought...
  Putting it another way, 8½ enables construction of interactive
  applications; Acme provides the interaction." (acme.ms)

### Self-criticism as a section

Dedicate space to what went wrong, as structure not aside:
- "We implemented solutions several times over several months
  and each time convinced ourselves — wrongly — they were
  correct." (sleep.ms)
- "Despite five year's experience... we remain dissatisfied with
  the stream mechanism." (net.ms)

### Judgments stated as observations

- "The system seemed restrictive" not "the system was bad" (acme.ms)
- "failed to follow through on some of its own ideas" (acme.ms)

### State the size

Size as evidence of simplicity, not bragging:
- "33 lines of C" (8½.ms — window creation)
- "560 lines long and has no graphics code" (acme.ms — Win)
- "847 lines of code vs 2200 for TCP" (net.ms — IL protocol)

### Personal voice with restraint

First person for chronological testimony, not opinion:
- "When I first saw Oberon, in 1988, I was struck by..." (acme.ms)
- Used sparingly; most Plan 9 papers are third person

### Parenthetical humor

Dry, brief, in service of the point:
- "(rather, it would be if it weren't so silly.)" (rc.ms)
- "Structures are now almost first-class citizens of the
  language." — "almost" does real work (compiler.ms)

---

## Voice by Author

**Pike (solo):** Most varied — declarative manifesto (foundational
paper), personal polemic (acme.ms), engineering trade-offs (8½.ms).
Common thread: structural comparisons as argument.

**Pike + Thompson + Presotto + others:** More measured, slightly
longer sentences. The foundational paper and names.ms are in this
collaborative voice.

**Presotto + Winterbottom (net.ms):** Plainest, most transactional.
Strongest self-criticism. Engineering-log style.

**Thompson (compiler.ms):** Most compressed. "Medium quality object
code" is the most brutally honest self-assessment in the corpus.

**Duff (rc.ms):** Driest humor. Arguments organized around a single
invariant (no rescanning) with all consequences derived from it.

**Cox, Grosse, Pike, Presotto, Quinlan (auth.ms):** Most formal.
Broadest authorship, most measured tone. Analogy-as-proof pattern.

---

## Application to pane's Three Tiers

**Tier 1 (architecture.md, design specs):**
- Use the engineering-report voice, not the manifesto voice
- State trade-offs explicitly: name the tension, state the choice,
  explain the audience that motivated it
- Show the boundary of every abstraction — what it does NOT cover
- Compare by structure, not by rhetoric
- State sizes when compact

**Tier 2 (Rust doc comments):**
- Claim then substantiate (example or measurement within 1-2 lines)
- Absence as design when applicable
- Concrete examples over abstract description

**Tier 3 (one-line briefs):**
- Verb phrase, no articles, what it does
- The permuted index entries
