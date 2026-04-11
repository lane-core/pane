---
type: policy
status: current
supersedes: [auto-memory/feedback_workflow_prominence]
created: 2026-04-06
last_updated: 2026-04-10
importance: high
keywords: [workflow_prominence, project_workflow, generic_skills, default_path]
agents: [all]
---

# Project workflow takes precedence over generic skills

**Rule:** The pane project has a specific four-agent workflow (`policy/agent_workflow`) that MUST be followed for significant changes. Twice in session 2, the agent defaulted to generic skills (writing-plans, subagent-driven-development) instead of following the project workflow.

**Why:** The project workflow exists because it was tested and validated in earlier sessions. It produces better results through specialized agents (pane-architect, formal-verifier) that understand the project's theoretical foundations and coding conventions.

**How to apply:** When starting implementation work on pane, ALWAYS check `policy/agent_workflow` first. The steps are:

1. Four design agents in parallel
2. Lane refines
3. pane-architect implements
4. formal-verifier validates
5. Memory + doc freshness

Generic skills like `superpowers:writing-plans` or `superpowers:subagent-driven-development` do NOT replace steps 3–4.
