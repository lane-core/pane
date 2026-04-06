# Session/Optic Boundary Rules

From full crate audit of pane-proto, pane-session, pane-app, pane-fs (2026-04-05).

## Boundary location

pane-proto is the membrane crate. Session vocabulary (Protocol, Message, Handles<P>) and optic vocabulary (Attribute<S,A>) coexist in separate modules. The looper in pane-app mediates between both worlds at runtime.

## Key findings

1. Attribute<S,A> physically cannot enter session types (Rc-based closures are !Send, not Serialize).
2. Obligation handles belong in pane-app (they contain LooperSender, a runtime type).
3. Drop-sends-failure is session discipline, not optic discipline.
4. Value/obligation split maps exactly to cartesian/linear optic split (Clarke et al. def:linearlens).
5. Linear lens insight is explanatory, not prescriptive — no API change needed.

## Ten rules (R1-R10)

1. No Attribute in Message
2. No AttrValue on wire
3. Obligations are !Message
4. receive doesn't touch optics
5. AttrReader closures must be pure
6. update_state looper-only
7. Filters never see obligations
8. ServiceHandle in handler not optic layer
9. No Arc<Mutex<Option<obligation>>> smuggling
10. ServiceHandle<P> methods are the outgoing protocol surface

**How to apply:** Reference these rules in any code review touching pane-proto property.rs, pane-fs attrs.rs, or obligation handle types in pane-app.