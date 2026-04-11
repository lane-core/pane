---
type: policy
status: current
supersedes: [auto-memory/feedback_technical_writing]
created: 2026-04-05
last_updated: 2026-04-10
importance: high
keywords: [technical_writing, plan9_voice, present_tense, active_voice, machine_description]
agents: [all]
---

# Technical writing standard — Plan 9 voice

**Rule:** Write in Plan 9's documentation voice. Describe the machine. Present tense, active voice, concrete behavior. No formalism buzzwords.

Three tiers of the voice, matched to pane output types (see serena `reference/plan9/foundational_paper`):

- **Tier 1 (paper):** `docs/architecture.md`, design specs.
  "Plan 9 from Bell Labs" (Pike et al., 1995) — `~/gist/plan9.pdf`
- **Tier 2 (man pages):** Rust doc comments (`///`), kit API docs.
  rio(1), plumber(4), pipe(3) via https://9p.io/sys/man/
- **Tier 3 (permuted index):** One-line type briefs, `# Brief`.
  First Ed. manual: https://doc.cat-v.org/plan_9/1st_edition/manual.pdf
  "clone − duplicate a fid" / "pipe − two-way interprocess communication"

Describe the machine — what it does, how it behaves, what the reader can expect. Present tense, active voice, concrete. Trust the reader to infer concepts from behavior.

## Avoid

- Formalism names as primary framing ("CLL types", "EAct actor framework", "profunctor optics")
- Emphatic "IS" constructions ("pane-app IS the EAct framework")
- Justifying why before explaining what
- "Design reference:" / "Design heritage:" citations inline
- Concept-mapping tables (theory term → implementation term)
- Adjective-heavy descriptions ("the universal message contract")

## Prefer

- State what things do, then stop
- Short sentences. Active voice. Present tense.
- Code examples over prose explanation
- If a formalism name is needed, use it once, briefly, then move on — don't build the paragraph around it
- Let the behavior speak for itself

## Compare

  Bad:  "pane-app provides single-threaded actor dispatch over
        multiple session endpoints. Each pane is an actor: one
        thread, sequential message dispatch, multiple protocol
        bindings. The handler store maps protocols to dispatch
        functions."

  Good: "Each pane runs on one thread. The looper dispatches
        messages sequentially. Protocol bindings are registered
        at setup; reply callbacks are installed per-request and
        consumed on reply."

The bad version names the concept then describes it. The good version describes the machine. The reader understands the concept from the behavior.

## Exemplary patterns (full analysis in `reference/plan9/papers_writing_voice`)

  Open with what things do:
    "A central file server stores permanent files and presents
    them to the network as a file hierarchy exported using 9P."

  State properties as flat facts:
    "There is no ftp command in Plan 9."
    "There is no superuser."

  Absence as design — state what you chose NOT to include:
    "Unlike `make`, `mk` has no built-in rules." (mk.ms)

  Tricolon with ascending severity:
    "complicates the preprocessor, is never necessary, and is
    usually abused" (comp.ms)

  Concession as credential — acknowledge what you're replacing:
    "Any successor of the Bourne shell is bound to suffer in
    comparison." (rc.ms)

  Self-critical where warranted:
    "We implemented solutions several times over several months
    and each time convinced ourselves — wrongly — they were
    correct." (sleep.ms)

  State the size as evidence of simplicity:
    "33 lines of C" (8½.ms) / "560 lines long" (acme.ms)

  Compare by structure, not rhetoric:
    "files are already provided with naming, access, and
    protection methods that must be created afresh for objects"
    (names.ms) — not "files are better than objects"

  Name the tradeoff explicitly:
    "Since the plumber is part of a user interface, and not an
    autonomous message delivery system, the decision was made to
    give the non-blocking property priority over reliability of
    message delivery." (plumb.ms)

  Show the boundary of every abstraction:
    Three things NOT mapped to files: process creation, network
    name spaces, shared memory. Explain why. (names.ms)

  Ground every abstraction in something you can type:
    Never describe a mechanism without a command, file path,
    or code snippet.

**Why:** Lane directed Plan 9 prose style for all pane documentation. Derived from analysis of 12 papers in `reference/plan9/papers/` plus the foundational paper and man pages. The engineering-report voice (trade-offs, limitations, sizes) is the target — not the manifesto voice of the foundational paper alone.

**How to apply:** When writing docs, specs, or code comments — describe behavior first. If you catch yourself explaining *why* before *what*, invert the order and often the *why* can be cut. For architecture.md: state trade-offs explicitly, show boundaries, state sizes, compare by structure. Use the engineering-report voice, not the manifesto voice.
