## Context

The inter-server protocol (`PaneMessage<ServerVerb>`) is defined but has no typed access layer. The only way to read attrs is `msg.attr("key")` — stringly typed, no compile-time checking, easy to typo. The architecture spec calls for typed views and builders that recover type safety. This change implements the pattern.

Reference: BeOS's BMessage had the same problem — `msg.FindInt32("widht", &val)` compiles fine, fails at runtime. Pane's answer: typed views that parse attrs once and expose typed accessors. The BMessage flexibility stays on the wire; Rust's type system governs the code.

## Goals / Non-Goals

**Goals:**
- Define the TypedView trait and ViewError type
- Define the typestate builder pattern
- Implement views/builders for pane-route messages (RouteCommand, RouteQuery)
- Implement views/builders for pane-roster messages (RosterRegister, RosterServiceRegister)
- Establish the pattern so all future server interactions follow it

**Non-Goals:**
- Implementing the servers themselves (pane-route, pane-roster)
- Protocol transport (socket connections, calloop integration)
- The RosterQuery view (deferred — query semantics need more design once roster exists)

## Decisions

### 1. TypedView as a trait, not a derive macro

A trait with a manual `parse()` implementation per view. Each view is 10-20 lines — the validation logic is specific to each message shape and not worth macro-generating. If the pattern proves repetitive across 10+ views, a derive macro can be added later.

**Alternative considered:** Derive macro from the start. Rejected — premature abstraction for 4-5 initial views.

### 2. Typestate builders for required-field enforcement

Builders use phantom type parameters to track which fields have been set. The `into_message()` method is only available when all required fields are present. This is a compile-time guarantee, not a runtime check.

```rust
struct RouteCommandBuilder<Data, Wdir> {
    data: Data,
    wdir: Wdir,
    src: Option<String>,
    content_type: Option<String>,
}

// Only callable when Data=Set and Wdir=Set
impl RouteCommandBuilder<Set, Set> {
    fn into_message(self) -> PaneMessage<ServerVerb> { ... }
}
```

**Alternative considered:** Runtime validation in `build()`. Rejected — the whole point is compile-time safety. Runtime validation is what we're replacing.

### 3. Views borrow the message, builders own their data

TypedView::parse takes `&PaneMessage<ServerVerb>` and returns a view that borrows attr values. This avoids cloning strings. Builders own their field values and move them into the constructed message.

### 4. Module structure: server::views, server::route, server::roster

The `server` module (already exists with `ServerVerb`) gains sub-modules:
- `views` — TypedView trait, ViewError, Set/Unset marker types
- `route` — RouteCommand, RouteQuery views and builders
- `roster` — RosterRegister, RosterServiceRegister views and builders

## Risks / Trade-offs

**[Typestate builder ergonomics]** → Typestate builders produce complex type signatures in error messages when required fields are missing. Mitigation: the error is a compile error (good) even if the message is verbose. Doc comments on the builder explain what's needed.

**[View lifetime ties to message]** → Borrowed views can't outlive the message. Mitigation: this matches the usage pattern — parse the message, use the view, drop both. If owned views are needed later, add `into_owned()`.

## Open Questions

- Should views implement `Value` marker trait? They're not wire types — they're typed accessors over wire types. Leaning no.
