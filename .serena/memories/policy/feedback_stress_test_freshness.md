---
type: policy
status: current
supersedes: [auto-memory/feedback_stress_test_freshness]
created: 2026-04-06
last_updated: 2026-04-10
importance: high
keywords: [stress_test, freshness, stale_assertion, codec, dispatch, wire_format]
agents: [pane-architect, formal-verifier]
---

# Stress tests go stale after production changes — verify explicitly

**Rule:** After any wire format, codec, or dispatch change, run stress tests and check assertions match the new behavior.

Stress tests can assert stale behavior after production changes. Session 2: the S3 codec desync test documented the OLD bug (returns "another Oversized error") after the poison fix changed the behavior to return Poisoned. The S9 session exhaustion test expected 254 after the u16 widening. Lane caught the staleness — the process didn't.

**Why:** Stress tests are often written to document specific failure modes. When the failure mode is fixed, the test still passes (the assertion is loose enough) but the comment and intent are wrong. Or worse, the test breaks silently because the new behavior doesn't match the old assertion.

**How to apply:** After ANY production change to wire format, codec, dispatch, or obligation handles, explicitly run `cargo test -- --ignored` and review the stress test output. Check that test comments match the current behavior, not the historical behavior they were written against.
