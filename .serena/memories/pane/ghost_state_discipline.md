# Ghost State Discipline

Each time a correlation ID appears at the API surface, ask whether a typestate handle could replace it. When it can, compile-time protocol enforcement replaces runtime token-matching. When it can't (async gap ownership can't bridge), keep the token but recognize it as ghost state — correctness depends on matching logic, not types.

Derived from dependent linear session type theory against pane's messaging model. The theory formalizes what pane does well in places (ReplyPort, TimerToken) and reveals where it doesn't yet (CompletionRequest token correlation).

Use as a design frame for new subsystems. Define the protocol, derive the handles, push ghost state below the API surface wherever ownership can carry the weight. Flag exposed correlation IDs as candidates for ownership promotion.
