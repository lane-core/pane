---
type: analysis
status: current
created: 2026-04-12
last_updated: 2026-04-12
importance: high
keywords: [SharedLens, Observable, concurrent_optics, lens_laws, writer_monad, shared_state, thread_per_pane]
sources: [architecture/shared_state, architecture/proto, architecture/fs, analysis/optics/_hub, analysis/optics/writer_monad, reference/papers/profunctor_optics]
verified_against: [CBG24 Definition 4.6, CBG24 Proposition 4.7, monadic_lens.rs assert_monadic_lens_laws]
related: [decision/thread_per_pane, analysis/optics/scope_boundaries]
agents: [optics-theorist]
---

# SharedLens and Observable: Optics Under Concurrency

## Key findings

1. **SharedLens is NOT a lens.** All three lens laws (GetPut,
   PutGet, PutPut) fail across separate lock acquisitions.
   The inner MonadicLens remains lawful; the concurrent wrapper
   breaks the sequential composition the laws require.
   architecture/shared_state's claim "Lens laws hold (the lock
   is orthogonal)" is incorrect — the lock prevents data races
   but not law violations.

2. **Observable<T> is not an optic.** It is a versioned
   concurrent store (identity comonad + generation counter).
   It composes *with* optics by providing the state they
   focus on, but is not itself in the optic formalism.

3. **SharedLens is a concurrent accessor containing a lens.**
   Option C from the literature analysis: no established
   formalism for concurrent optics exists in CBG24, Abou-Saleh
   et al., or the profunctor literature generally.

4. **Writer monad Ψ remains well-defined under shared state.**
   The monad structure is unchanged; what changes is the
   effect algebra (broadcast to all observers vs. local
   processing). The Effect::Notify target field already
   supports routing.

## Recommendation: Observable + MonadicLens on snapshots

Remove SharedLens as a type. Use Observable for concurrent
access and MonadicLens on owned/snapshot state:

- view: snapshot via Observable, apply lens to Arc<T>
- set: Observable.update(|t| { lens.set(&mut t_clone, v) })
- Laws hold within update closure (exclusive access)
- Generation counter notifies other panes to re-read

This preserves the invariant from analysis/optics/scope_boundaries
point 5: "Laws hold within linearizable scope."

## Corrected claim for architecture/shared_state

"Lens laws hold within a single lock acquisition (or within
an Observable update closure). Across concurrent operations,
the lens is a focusing mechanism, not a lawful optic."
