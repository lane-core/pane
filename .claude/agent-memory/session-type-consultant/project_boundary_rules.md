---
name: Session/optic boundary rules
description: Ten implementer rules for session-type vs optic boundary in pane redesign, plus obligation handle placement and linear lens analysis
type: project
---

Produced 2026-04-05 from full crate audit of pane-proto, pane-session, pane-app, pane-fs.

**Boundary location:** pane-proto is the membrane crate. Session vocabulary (Protocol, Message, Handles<P>) and optic vocabulary (Attribute<S,A>) coexist in separate modules. The looper in pane-app is the runtime mediator between both worlds.

**Key findings:**
1. Attribute<S,A> physically cannot enter session types (Rc-based closures are !Send, not Serialize).
2. Obligation handles belong in pane-app (they contain LooperSender, a runtime type).
3. Drop-sends-failure is session discipline, not optic discipline.
4. Value/obligation split maps exactly to cartesian/linear optic split (Clarke et al. def:linearlens).
5. Linear lens insight is explanatory, not prescriptive -- no API change needed.

**Ten rules (R1-R10):** No Attribute in Message; no AttrValue on wire; obligations are !Message; receive doesn't touch optics; AttrReader closures must be pure; update_state looper-only; filters never see obligations; ServiceHandle in handler not optic layer; no Arc<Mutex<Option<obligation>>> smuggling; ServiceHandle<P> methods are the outgoing protocol surface.

**Why:** Lane requested concrete implementer guidance bridging the prior optics-x-session-types deliberation to actual crate code.
**How to apply:** Reference these rules in any code review touching pane-proto property.rs, pane-fs attrs.rs, or obligation handle types in pane-app.
