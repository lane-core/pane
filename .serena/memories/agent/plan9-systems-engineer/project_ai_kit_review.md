---
name: AI Kit design review findings
description: Review of docs/ai-kit.md — three rounds of findings covering identity, governance, communication, namespace, cpu analogy, event model, crash recovery
type: project
---

## Round 1 (2026-04-02) — implemented in commit 054a50e

1. **TLS-to-uid mapping**: recommended flat mapping file (/etc/pane/identity-map.toml) over naming convention or IdP. Rationale: mirrors Plan 9's /lib/ndb/auth, auditable, no CN-username coupling assumption.

2. **.plan/.access split**: .plan for finger display (free-form text), .access for structured Landlock/network declarations. Mixing human-readable and machine-parsed in one file creates parsing fragility. Plan 9 kept display (.plan) and enforcement (namespaces, permissions) fully separate. **Implemented.**

3. **Communication hierarchy**: pane-fs is primary agent communication, mail is async notification, write/talk/wall are legacy human-terminal compat. **Implemented — §3 reordered.**

4. **cpu analogy**: Plan 9 cpu projected local namespace onto remote compute; pane brings remote compute into local namespace. These are inverse patterns. **Resolved — subsection cut entirely.**

5. **Landlock execute gap**: .access [tools] section resolved — Nix profile is only $PATH entry, Landlock restricts execute to that profile path. **Implemented.**

## Round 2 (2026-04-03) — implemented in revision pass

6. **Remote identity scaling**: one-local-account-per-remote-agent doesn't scale beyond small deployments. Acknowledged as open question, deferred to pane-linux.md. **Acceptable.**

7. **Namespace composition not discussed**: Retracted — this belongs in the pane-fs spec, not AI Kit.

8. **Agent self-modification workflow**: .access [tools] tool request workflow now specified with interactive notification pane. **Implemented.**

9. **Communication framing inverted**: §3 restructured — pane-fs/protocol leads, mail second, unix terminal commands grouped under header. **Implemented.**

10. **cpu analogy imprecise**: Subsection cut. **Resolved.**

11. **Event subscription model**: /pane/<n>/event specified with blocking-read, one-line-per-event, Plan 9 pattern. **Implemented.**

12. **ctl command vocabulary**: ctl vs attrs distinction stated (§3). commands/ directory for discovery (§6). But relationship between ctl and commands/ not stated — see Round 3 issue A. **Partially addressed.**

13. **vacation(1) over-reaching**: Withdrawn. Current text is modest — describes mail auto-forwarding only.

14. **Crash recovery model**: Four-step crash sequence in §1 — drop compensation, PaneExited broadcast, pane-fs removal, presence. **Implemented.**

15. **Consistency model**: §9 states local sequential, remote eventual, send_request for synchronous. **Implemented.**

16. **Service/capability discovery**: §6 adds ls attrs/, ls commands/, cat commands/<name>. **Implemented.**

17. **Agent groups**: §2 adds unix groups with system example and Landlock interaction. **Implemented.**

## Round 3 (2026-04-03) — follow-up review, revision pass applied

A. **ctl vs commands/ relationship** (moderate): §3 introduces ctl as write-only line commands. §6 introduces commands/ as directory listing command metadata. Relationship still not explicitly stated — reader must infer that commands/ entries document what ctl accepts. **STILL OPEN** after revision pass. One sentence would close it.

B. **[models] enforcement gap** — **RESOLVED.** Lines 138–143 now state it in one place: advisory unless [network] restricts egress. Lines 213–219 reinforce.

C. **pane-notify vs /pane/<n>/event scope** — **RESOLVED.** Lines 679–683 state boundary: /pane/<n>/event is per-pane, pane-notify is per-path. Example given.

D. **Attribute auto-indexing** — **RESOLVED.** Lines 369–372: queryable "by configuration or on first encounter — the same mechanism as BeOS's mkindex."

**Why:** AI Kit frames agent participation in the system. Only one minor clarification remains (A).

**How to apply:** Issue A is the only remaining finding — affects agent scripting API design. Low severity but worth a sentence.
