# Duploid Analysis of pane's Architecture (2026-04-05)

Based on Mangel/Melliès/Munch-Maccagnoni 2025 ("Classical Notions of Computation and the FH Theorem"), MM14b (foundational duploid definitions), and roundtable with session-type, Plan 9, Be, and duploid research agents.

## Core result

pane's architecture is a duploid — a non-associative polarized category where:
- **Positive subcategory** (Kleisli, CBV): wire types (ServiceFrame, ControlMessage, Message), serialized values, produced outputs
- **Negative subcategory** (co-Kleisli, CBN): handlers (Handles<P>), dispatch tables, obligation endpoints, demand-driven reads
- **Cross-polarity composition** is non-associative: (h ∘⁻ g) ∘⁺ f ≠ h ∘⁻ (g ∘⁺ f)

The server deadlock was exactly a non-associative bracket realized concurrently. The actor model prevents non-associative composition from arising by serializing all polarity crossings.

## Polarity assignments (verified by duploid research agent)

### Positive (Kleisli, value-driven)
- ServiceFrame::Request, Reply, Failed, Notification (all wire types are positive)
- ControlMessage variants (serialized envelope)
- P::Message (typed protocol values)
- Command text (data being written to ctl)

### Negative (co-Kleisli, demand-driven)
- Handles<P>::receive (handler waiting for input)
- Dispatch table entries (pending callbacks)
- read_frame() (blocks waiting for transport data)
- AttrReader::view (pure co-Kleisli extraction)

### Oblique / cross-polarity
- AttrWriter::write(state, text) — takes positive input, applies to negative state
- ctl dispatch — positive command → negative handler → effects
- Looper dispatch — positive frame → negative handler → Flow

### Special
- ReplyPort<T> — ↑(continuation): positive wrapper around negative one-shot channel. `.reply()` = ↓ (force). Drop forces with error value.
- ActivePhase<T> — ω_X : Handshake → ActivePhase. Shift operator. One-way, carries negotiated state.

## Two-phase structure

- **Handshake**: dialogue duploid (par's Dual = involutive negation). Central = thunkable (FH theorem applies).
- **Active phase**: plain (non-dialogue) duploid. Writer monad Ψ(A) = (A, Vec<Effect>) on positives, identity comonad on negatives. Central ≠ thunkable in general.
- **Transition**: shift operator ω_X, should be explicit as `ActivePhase<T>` newtype carrying max_message_size, PeerAuth, known_services.

## Composition laws

- (+,+): Kleisli composition. Associative. Safe to batch/reorder when thunkable.
- (-,-): co-Kleisli composition. Associative (identity comonad). Safe to reorder freely.
- (+,-) or (-,+): Cross-polarity. NOT associative. Must serialize at actor boundary.
- Thunkable ⟹ central holds in every duploid (MM14b Proposition 6). Use this for batch optimization.

## Plan 9 reading
- 9P is a collage category (degenerate duploid, cross-polarity morphisms factor through transverse maps)
- Per-fid serialization = actor model for each fid (avoids non-associative bracket)
- Namespace composition is non-commutative monoid (ordered union dirs) — outside standard duploid framework
- Filesystem and protocol share the duploid's state object but use different subcategories (co-Kleisli for reads, Kleisli for writes). MonadicLens bridges them. Keep them separate.

## Design principles derived

1. **Polarity discipline**: classify every operation. Same-polarity composes associatively. Cross-polarity requires serialization.
2. **ActivePhase<T> newtype**: make the handshake→active shift explicit in the type system.
3. **Thunkability criterion**: for batch optimization, reorder only thunkable operations (sound without dialogue structure).
4. **Actor model at polarity boundaries**: the single-threaded actor prevents non-associative composition from arising.
5. **ServiceFrame is all-positive**: polarity crossings happen at dispatch (LooperCore), not at framing.
6. **Protocol and filesystem are different subcategories**: MonadicLens is the bridge, not a unification.

## Corrections to initial analysis
- Reply is positive (not negative) — all wire types are positive
- ReplyPort is ↑(continuation) (not a transverse map)
- Writes are oblique/cross-polarity (not purely positive)
- Actor prevents non-associativity from arising (doesn't select a bracketing within it)

## Related psh concept anchors

psh's `.serena/memories/analysis/polarity/` cluster maintains
keyword-shaped anchors for the polarity discipline this memo
summarizes. For theoretical depth:

- **`../psh/.serena/memories/analysis/polarity/plus_minus_failure`**
  — names the (+,−) non-associativity equation directly and
  grounds the proof sketch in the vendored duploids paper
  (MMM25 lines 7100–7185). The "server deadlock was exactly a
  non-associative bracket" claim in §"Core result" above is
  this anchor's operational witness.
- **`../psh/.serena/memories/analysis/polarity/shifts`** — the
  focused-calculus reading of ↑ / ↓ as shift operators and
  their F/U adjunction (see also
  `analysis/foundations/cbpv_f_u_separation`). Pane's
  `ReplyPort<T>` (↑) and `ActivePhase<T>` (ω_X) are shift
  operators in this sense.
- **`../psh/.serena/memories/analysis/polarity/frames`** —
  the save/restore-around-shift engineering principle, with
  historical precedents (ksh93's `sh_polarity_enter/leave`,
  sfio's Dccache). Pane's single-threaded actor is the
  runtime enforcement of the frame discipline; the type system
  makes frames implicit by serializing all polarity crossings.

Not vendored into pane. Two pane spokes in this cluster port
adjacent material with pane-concrete types:

- [`analysis/duploid/oblique_maps`](oblique_maps.md) — every
  producer-consumer crossing (`send_request`, ctl write,
  `AttrWriter::write`, `pane_exited` dispatch) has the
  structure of an oblique map P → N.
- [`analysis/duploid/cbv_focusing`](cbv_focusing.md) — one
  focused scope per `Handles<P>::receive` dispatch; effects
  bounded to the scope, `MonadicLens` laws meaningful within
  the scope.
