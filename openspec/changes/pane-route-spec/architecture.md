## Context

Plan 9's plumber matched text fragments against rules and routed them to handler ports. pane-route extends this with service registry awareness (from pane-roster) and a multi-match chooser (transient scratchpad pane). Rules are files in well-known directories, watched by pane-notify for live addition/removal.

The existing RouteCommand typed view defines the message shape: `data` (text to route), `wdir` (working directory), optional `src` (source app) and `content_type` (hint). RouteQuery asks "what handlers match this?" for the multi-match chooser.

## Goals / Non-Goals

**Goals:**
- Define the rule format (what fields, how patterns work)
- Define the matching algorithm (ordered rules, first match, multi-match)
- Define the port model (named ports, listeners, delivery)
- Define service registry integration
- Define rule file storage and live reload

**Non-Goals:**
- Implementation
- The RouteMessage wire type (already defined in pane-proto)
- Transport layer (socket connections — that's pane-app)

## Decisions

### 1. Rule format: structured data, not a DSL

Plan 9's plumber used a text-based rule file with a custom syntax. We use one JSON file per rule in a well-known directory. Each rule is a self-contained file — add a file to add a rule, remove to remove. pane-notify watches the directory.

This follows the filesystem-as-configuration principle. No rule parser needed beyond JSON.

### 2. Matching algorithm: ordered by priority, first match wins by default

Rules have a numeric priority (lower = higher priority). Within the same priority, rules are evaluated in filename sort order. The first rule that matches dispatches immediately — unless the `multi` flag is set, in which case all matching rules AND all matching services are collected and presented via the chooser.

### 3. Port model: named string ports with listener registration

A port is a string name (e.g., "edit", "web", "image"). Applications connect to pane-route and register interest in one or more ports. When a message is routed to a port, it's delivered to all listeners on that port. If no listener is registered, pane-route queries pane-roster to see if a handler app should be launched.

### 4. Regex patterns with named captures

Rule patterns are regular expressions. Named capture groups become message attributes. Example: a pattern `(?P<file>[a-zA-Z_./]+):(?P<line>\d+)` matching `parse.c:42` produces attrs `file=parse.c` and `line=42`, which are attached to the routed message.

### 5. Service registry as fallback matching

After rule matching, pane-route queries pane-roster for registered services whose `content_type` pattern matches the message data. Services found this way are added to the match set. If the combined set (rules + services) has multiple matches, the chooser pane appears.

## Risks / Trade-offs

**[Regex complexity]** → Complex regexes can be slow. Mitigation: rules are evaluated infrequently (on user click), not in a hot loop. Regex compilation happens once at rule load time.

**[Rule ordering]** → Priority-based ordering is less intuitive than Plan 9's "first match in file order." Mitigation: most users will have few rules. Priority is optional — default is 50, and filename order breaks ties.

## Open Questions

- Should rules support negation? (match if pattern does NOT match)
- Should rules support chaining? (output of one rule feeds into another)
