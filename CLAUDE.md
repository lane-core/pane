# pane project instructions

## Style

Follow `STYLEGUIDE.md` for code, comments, prose. Run `cargo fmt` and `cargo clippy --workspace` before committing.

## Agent Workflow

New subsystems/significant changes: four-agent workflow in serena `pane/agent_workflow`:

1. Four design agents parallel (plan9-systems-engineer, be-systems-engineer, session-type-consultant, optics-theorist)
2. Synthesize → present to Lane → Lane refines
3. pane-architect implements (one task per dispatch, review between — not bulk)
4. formal-verifier validates
5. Memory + doc freshness

No generic execution skills for steps 3-4. Project agents know pane's theory + conventions.

## Committing

Multi-phase task, tests + doc builds pass → commit without asking. Descriptive message summarizing work. Single-file/ad-hoc changes → check first.

### Commit message format

Two-paragraph body after subject. First paragraph: user's provenance third person, using name ("Lane decided...", "Lane asked..."): decision procedure, thought process, design direction, prompting history. Include concrete summary of initiating prompt/directive (what was asked, not just decided). Second paragraph: "Agent steps:" — what agent did, including model, agent consultations, verification.

Every commit with AI-generated code must end with `Generated-with:` trailer:

    Generated-with: Claude opus-4-6 (Anthropic) via Claude Code