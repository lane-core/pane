---
type: policy
status: current
supersedes: [auto-memory/feedback_no_python_extraction]
created: 2026-03-27
last_updated: 2026-04-10
importance: low
keywords: [no_python_extraction, agent_write_permission, transcript_files]
agents: [all]
---

# No Python extraction from agent outputs

**Rule:** Agent subprocesses have Write permission — never use Python to extract content from agent transcript files.

Agents have Write permission and can write their output files directly. Do NOT use Python scripts to parse agent JSON transcript files and extract content. This was a workaround for a permission issue that has been resolved.

**Why:** The user granted Write permissions for agents. Using Python to extract from `/private/tmp/` transcript files is unnecessary, fragile, and confusing.

**How to apply:** If an agent fails to write its output, investigate why — don't fall back to Python extraction. The agent should retry or the permission should be checked.

**Importance: low.** This memory documents an obsolete workaround. Kept to prevent regression.
