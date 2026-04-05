# Standard Development Workflow

Established 2026-04-05. Lane directed this after noticing agents were being bypassed.

## Step 1: Four design agents analyze (ALL FOUR, in parallel)

- **plan9-systems-engineer** — Plan 9 namespace model, 9P protocol, /proc and rio precedent, distributed state
- **be-systems-engineer** — BeOS/Haiku API design, BLooper/BHandler/BMessage, hey scripting, app_server
- **optics-theorist** — profunctor optics theory, lens/prism/traversal design, law verification, Clarke et al.
- **session-type-consultant** — session type safety, linear/affine types, protocol soundness, invariant analysis

Why all four: each brings a different perspective. Skipping one leaves a blind spot.

## Step 2: Lane refines

Discuss findings, resolve open questions, make decisions.

## Step 3: pane-architect implements

Writes Rust code faithful to project foundations. Has project-specific context about par, session types, Be/Haiku translation rules, optics layer.

## Step 4: formal-verifier validates

Audits implementation against architecture spec invariants and formal properties.

## Rules

- Don't skip any of the four design agents in step 1
- Don't substitute general-purpose agents for any named agent
- Design agents run in parallel; pane-architect and formal-verifier run sequentially
