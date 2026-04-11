# Polarity Classifications: Be and Plan 9 Abstractions

Reference tables classifying the heritage systems' abstractions by duploid polarity. Use these when translating Be or Plan 9 concepts into pane's architecture.

## Plan 9 Classification

| Abstraction | Polarity | Subcategory | Notes |
|---|---|---|---|
| Files (general) | Negative | Co-Kleisli | Demand-driven; cache flag (-C) is explicit shift to positive |
| Synthetic files (/proc, /net) | Strongly negative | Co-Kleisli | No stored value; computed on demand |
| /env variables | Positive (negative wrapper) | Kleisli content, co-Kleisli interface | Shift-down is the file protocol itself |
| /srv entries | Positive refs to negative | Shift boundary | Reified capabilities; stale entry = broken shift (predictive!) |
| T-messages | Co-Kleisli morphisms | Co-Kleisli | Demands applied to comonadic servers |
| R-messages | Positive results | Kleisli values | Data returned from co-Kleisli evaluation |
| Fids | Comonadic references (!A) | Co-Kleisli | Non-consuming handles; walk = comonad duplicate |
| Qids | Positive identifiers | Kleisli | Pure data; comparable, inspectable |
| Pipes | Cuts | Cut rule | Bidirectional = tensor of two cuts |
| Namespace (Γ) | Ordered context | Structural | Weakening + contraction; exchange FAILS in unions → non-commutative logic |
| bind/mount | Context extension | Structural rules | REPLACE = substitution; -b/-a = ordered extension |
| rfork(RFNAMEG) | Context copy | Structural | Full duplication of Γ |
| Limbo channels | Cuts (synchronous) | Cut rule | Buffered = shift-mediated cut |
| rc variables | Positive | Kleisli | Lists of strings; data |
| rc pipelines | Composed cuts | Cut composition | Process subst = shift-down to name |

### Plan 9 key insights
- Plan 9 is **primarily co-Kleisli**. The system presents everything through negative/demand-driven interfaces using 9P as the universal co-Kleisli morphism structure.
- **Fids are !-typed** (comonadic). Twalk is the comonad's duplicate: !A → !!A. Clunk is dereliction.
- **Per-process namespace IS the sequent calculus context Γ.** Ordered, with implicit weakening and contraction, but exchange fails in union directories (non-commutative logic).
- **Twrite is the crossover**: carries positive data into negative server. 9P is not purely co-Kleisli.
- **/srv stale entries are a broken shift** — theory predicts the failure mode: shifts from negative to positive lose liveness information.

## BeOS/Haiku Classification

| Abstraction | Polarity | Subcategory | Notes |
|---|---|---|---|
| BMessage | Positive | Kleisli | Flat data container, Flattenable, copyable |
| BHandler | Negative | Co-Kleisli | Defined by MessageReceived, ResolveSpecifier (copatterns) |
| BLooper | Negative | Co-Kleisli | Running computation; dispatch pipeline is cut elimination |
| BMessenger | Positive (shifted negative) | ↑(BHandler) | Thunked reference to negative endpoint; copyable |
| BApplication | Negative | Co-Kleisli | Specialized BLooper; be_app is ↑ shift |
| BRoster | Negative | Co-Kleisli | Proxy to registrar via ↑-shifted reference |
| BPropertyInfo | Positive | Kleisli | Static schema of negative handler's copattern structure |
| BView | Negative (mixed) | Co-Kleisli core | Handler with embedded positive state (frame, color, font) |
| BWindow | Negative | Co-Kleisli | Specialized BLooper |
| reply_port | ↑(continuation) | Shift boundary | Negative one-shot demand wrapped as positive data in BMessage header |
| Scripting protocol | Negative | Co-Kleisli | Demand-driven property traversal; resolve_specifier = iterated ↓ |
| BQuery (non-live) | Positive | Kleisli | Predicate data, iterable results |
| BQuery (live) | Negative | Co-Kleisli | Running kernel computation pushing updates; ↓ shift from positive query |
| Node monitoring | Negative | Co-Kleisli | Watchpoint → notification; installed via ↑-shifted handler |

### BeOS key insights
- BeOS is **already a duploid**, NOT primarily Kleisli. Both subcategories are well-populated.
- **Control flow is CBV/Kleisli** (messages fully evaluated before dispatch). **Type structure** spans both subcategories.
- **Shifts are pervasive**: BMessenger = ↑(handler), reply_port = ↑(continuation), SendMessage = ↓, resolve_specifier = iterated ↓.
- **BMessage polarity confusion**: compound type `positive_data × ↑(negative_continuation)` that failed to decompose. Caused real bugs (aliased continuations, thread-unsafe reply, coupled lifetimes).
- **Be's design achievement**: making a fundamentally bi-polar system feel like simple CBV programming.

## pane Mapping

| pane Abstraction | Polarity | Heritage | Notes |
|---|---|---|---|
| Message trait | Positive | Be (BMessage, decomposed) | Pure data, no continuation |
| ReplyPort<T> | ↑(negative) | Be (reply_port, decomposed) | !Clone, #[must_use], Drop compensation |
| Handles<P> | Negative | Be (BHandler) | Defined by receive copattern |
| Handler | Negative | Be (BHandler) | Lifecycle copatterns |
| ServiceHandle<P> | ↑(negative) | Be (BMessenger) | !Clone, #[must_use], Drop fires RevokeInterest |
| MonadicLens view | Negative → Positive | Both | co-Kleisli extraction (pure) |
| MonadicLens set | Positive → Negative | Both | Kleisli map (effectful) |
| AttrReader<S> | ≈ ↓ (positive shift) | Plan 9 (/proc read) | Extracts positive value from negative state |
| AttrWriter<S> | ≈ ↑ (negative shift) | Plan 9 (/proc/ctl write) | Injects positive data into negative state |
| pane-fs namespace | Negative interface | Plan 9 (9P, synthetic fs) | Demand-driven reads |
| Protocol dispatch | Kleisli (CBV) | Be (BLooper dispatch) | Messages fully evaluated before handler runs |
| LooperCore | Cut elimination engine | Both | Juxtaposes positive message with negative handler |
| Snapshot (ArcSwap) | wrap operator | Both | Encapsulates negative state as positive observable |
