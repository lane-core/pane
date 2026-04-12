---
type: reference
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [pane-router, pane-translator, plumber, device_drivers, namespace, security, routing_rules, crate_boundary]
related: [reference/plan9/man_pages_insights, reference/plan9/papers_insights, reference/plan9/divergences, decision/host_as_contingent_server, architecture/fs, architecture/proto]
agents: [plan9-systems-engineer]
---

# Consultation: pane-router and pane-translator (2026-04-12)

Seven-question analysis for Lane on two proposed subsystems.

## Key recommendations

1. **Plumber heritage:** Adopt content-based routing and user-configurable rules. Reject central-server architecture (plumber's fixed-namespace bug). Data-management rules ("copy file X to folder Z") are effects, not routing — separate them from signal dispatch.

2. **Namespace and #device:** pane-translator defines traits (InputSource, DisplayTarget, etc.), not file servers. pane-fs projects translator state into `/pane/dev/` as computed directories. Follows existing three-tier access model.

3. **The /dev question:** `/pane/dev/` is another computed directory in pane-fs, alongside `/pane/by-sig/`. Per-pane device visibility is server-enforced policy (ConnectionNamespace), not client-side bind/mount. Deliberate loss of Plan 9's client-side composition, acceptable given no kernel support.

4. **Import/export model:** Translator is local traits for local devices, protocol services for remote devices. No wire protocol between translator and local consumers — just Rust traits. Protocol enters only when device is actually remote (cpu reverse-export model via DisplayProtocol/InputProtocol services).

5. **Security model:** Router does authorization (exportfs -P patternfile model), not authentication (factotum model). Three-level precedence: per-connection > per-pane > system-wide. Adopt factotum's `confirm` pattern for interactive authorization of sensitive cross-boundary operations.

6. **Rule language:** Three categories with different match semantics: security (deny-first), routing (first-match-wins like plumb(6)), effect (all-match). Precedence: per-pane > user > system (with system override capability). Rules inspectable/modifiable via pane-fs.

7. **Crate boundary:** Separate crates, neither depends on the other. Both depend on pane-proto. pane-fs is the integration point (Plan 9 model: namespace unifies independent subsystems). Router compiles rules to MessageFilter impls on existing filter chain.

## Where I flagged "outside my expertise"

- Confirm-pattern UX details → be-systems-engineer
- Rule language concrete syntax → bikeshed, semantics matter more
- Data-management effect safety → novel territory, no Plan 9 precedent

## Rule evaluation location

Recommended: server-side evaluation (server has authoritative view, stays current — avoids plumber's frozen-namespace bug). Per-pane evaluation avoids the bug differently but introduces rule-consistency complexity.
