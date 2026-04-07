# pane project instructions

## Style

Follow `STYLEGUIDE.md` for all code, comments, and prose. Run `cargo fmt` and `cargo clippy --workspace` before committing.

## Agent Workflow

For new subsystems and significant changes, follow the four-agent workflow in serena `pane/agent_workflow`:

1. All four design agents in parallel (plan9-systems-engineer, be-systems-engineer, session-type-consultant, optics-theorist)
2. Synthesize → present to Lane → Lane refines
3. pane-architect implements (one task per dispatch, review between — not bulk)
4. formal-verifier validates
5. Memory + doc freshness

Do not substitute generic execution skills for steps 3-4. The project agents understand pane's theoretical foundations and coding conventions.

## Committing

After completing a planned multi-phase task where all tests and doc builds pass, commit the results without asking. Use a descriptive commit message summarizing the work. For single-file or ad-hoc changes, still check before committing.

### Commit message format

Two-paragraph body after the subject line. First paragraph describes the user's provenance in third person, using their name ("Lane decided...", "Lane asked..."): the decision procedure, thought process, design direction, and/or summary of the prompting history that led to the change. Include a concrete summary of the prompt or directive that initiated the work (what was asked, not just what was decided). Second paragraph begins with "Agent steps:" and describes what the agent did to meet that objective, including model, agent consultations, and verification.

Every commit containing AI-generated code must end with a `Generated-with:` trailer:

    Generated-with: Claude opus-4-6 (Anthropic) via Claude Code
