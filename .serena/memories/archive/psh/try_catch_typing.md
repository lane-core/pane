---
type: archive
status: archived
archived_from: pane/psh_try_catch_typing
archived_at: 2026-04-10
note: psh content. Lane has seeded psh's own serena project with this and similar deliberations. Preserved here for historical reference. Read psh's serena for the current canonical version (the syntax has since changed from `try { } else e { }` to `try { } catch (e) { }` per psh deliberations.md).
---

# try/catch typing analysis (2026-04-07)

**Archived snapshot.** Original at `pane/psh_try_catch_typing`. Preserved
for historical reference. The canonical version lives in psh's serena
project, where the syntax has since been refined.

## Verdict
`try { body } else e { handler }` is ErrorT over command sequences, not ⅋. Not case elimination. Not a restricted `if`.

## Typing rule
```
Γ ⊢ body : cmd    [body under ErrorT sequencing: ;ₜ checks status between commands]
Γ, e : Str ⊢ handler : cmd
─────────────────────────────────────
Γ ⊢ try { body } else e { handler } : cmd
```

- `try` changes the sequencing combinator within its body from unconditional `;` to monadic `;ₜ` (Kleisli composition for ErrorT Status).
- `else e` is a μ̃-binder on the error case.
- Without `else`, error Status propagates to enclosing scope.
- `if` is single ⊕-elimination on one command's Status. `try` is a natural transformation on the sequencing combinator.
- Boolean contexts (if conditions, &&/|| LHS, ! commands) are exempt from the ErrorT check.

## Relationship to traps
`try/catch` is the synchronous special case of lexical trap where the "signal" is nonzero status and delivery is at each semicolon. Traps are asynchronous μ-binders. Keep as separate constructs — synchronous/asynchronous distinction matters for effect ordering.

## Match arm bindings
Standard ⊕-elimination. Each arm is μ̃x.c — covariable binding scoped to arm body. `ok val =>` syntax (two bare words = structural) is correct. Consistent with all other binders (for x, let x, else e, \x).

## Lexical traps
μ-binder (Curien-Herbelin §2.1). Inner traps shadow outer for same signal within block scope. Nesting = substitution-based composition. Dynamic trap capabilities lost: callee-installed persistent handlers (actually better — makes obligations visible at call site).
