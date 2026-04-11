---
type: reference
status: current
citation_key: JHK24
aliases: [LinearActris, dlfactris]
created: 2026-04-10
last_updated: 2026-04-11
importance: high
keywords: [dlfactris, linearactris, jacobs, hinrichsen, krebbers, popl_2024, linearity, global_progress, connectivity_graph, acyclicity, iris, dependent_separation_logic, higher_order_message_passing]
related: [reference/papers/_hub, reference/papers/eact, reference/papers/forwarders]
agents: [session-type-consultant, pane-architect, optics-theorist, formal-verifier]
---

# Deadlock-Free Separation Logic: Linearity Yields Progress for Dependent Higher-Order Message Passing

**Authors:** Jules Jacobs, Jonas Kastberg Hinrichsen, Robbert Krebbers
(Radboud University Nijmegen / Aarhus University)
**Venue:** POPL 2024 (Proc. ACM Program. Lang., Vol. 8, Article 47, January 2024)
**Path:** `~/gist/2024-popl-dlfactris.pdf`

## Summary

Introduces **LinearActris**, a *linear* concurrent separation
logic for message-passing concurrency, designed to guarantee
deadlock and leak freedom. LinearActris amends Actris (which
is affine) with linearity restrictions sufficient to prove a
**global-progress adequacy theorem**. The logic is strong
enough to verify higher-order programs with mutable state
through dependent protocols — channels can carry channels,
closures, and protocol-level ownership assertions.

The key theorems in the introduction:

- **Theorem 1.1** (Actris/Iris adequacy, quoted from Jung et
  al. 2018b §6.4): a proof of `{True} e {True}` implies `e`
  is *safe* — for every reachable configuration, each thread
  is either a value or can step. This is a safety property;
  it does **not** preclude deadlock because stuck threads
  waiting on empty channels count as "safe."
- **Theorem 1.2** (the paper's contribution): a proof of
  `{Emp} e {Emp}` in LinearActris implies `e` enjoys **global
  progress** — for every reachable configuration, either all
  threads are values with an empty heap, *or* the
  configuration can step as a whole. Global progress is
  strictly stronger than per-thread safety: it requires the
  system as a whole to make progress, which rules out the
  deadlocked-stuck case.

Two ingredients are necessary for Theorem 1.2, and both fail
in Actris alone:

1. **Linearity.** Affinity (Actris) allows dropping
   resources, which permits a thread to abandon a send /
   receive obligation and leave the peer waiting forever.
   Linear resource use forces every channel to be drained,
   ruling out this deadlock shape. (§1 "The need for
   linearity.")
2. **Acyclicity of the connectivity graph.** Linearity alone
   is not enough: two threads can hold the endpoints of two
   separate channels and cross-wait (each thread sends on
   one and receives on the other in opposite order), forming
   a cycle that deadlocks despite both being linear. The
   paper adapts Jacobs's **connectivity graphs** as the
   semantic invariant: as long as the ownership topology
   stays acyclic, cross-wait cycles cannot form. (§1 "The
   need for acyclicity.")

**Linearity + acyclicity ⇒ global progress** is the paper's
central claim, proved via a step-indexed model of LinearActris
on top of Iris. All results plus examples are mechanised in
Coq.

## Concepts informed

- **Connectivity-graph acyclicity invariant.** The central
  structural insight pane relies on: if the "who-can-talk-to-
  whom" topology is a DAG, the system cannot deadlock
  regardless of message ordering. Pane's star topology
  (ProtocolServer at the center, clients as leaves) is
  trivially acyclic, which is the shape Theorem 1.2 covers.
- **Linearity as a path to global progress (not just
  safety).** Pane's obligation handles (`ReplyPort`,
  `CompletionReplyPort`, `CancelHandle`) are move-only and
  `#[must_use]` — a Rust-affine realization of LinearActris's
  linearity discipline. Drop-sends-failure compensation
  provides the protocol-level equivalent of the paper's
  type-level "must return `End`" requirement: both ensure
  every obligation is resolved.
- **Why Rust's affine types are enough for pane.** The paper
  discusses how linearity is achieved in languages that
  provide only affine types (§ on Rust-style ownership): the
  "must return a terminal token" closure-capability trick
  converts affine into linear-behavioured at the API layer.
  Pane uses the same trick with a different terminal object
  (the `ReplyFailed` / `CompletionFailed` compensation path).

## Used by pane

- `architecture/session` — `ProtocolServer` as single-mailbox
  actor at the center of an acyclic connectivity graph. The
  star topology is why the deadlock-freedom theorem applies.
- `decision/server_actor_model` — cites Theorem 1.2 as the
  formal grounding for "star topology ⇒ global progress."
  Watch/PaneExited creates a one-shot terminal reverse edge
  that does not violate acyclicity (the watched thread is
  gone; no ongoing loop).
- `decision/messenger_addressing` — direct pane-to-pane
  communication would take us outside the single-star
  topology. If Phase 2 implements multi-hop routing, the
  acyclicity argument needs re-examination (candidate: chain
  of stars is still acyclic; cross-server cycles are not).
- `analysis/session_types/coprocess_corrected` — per-tag
  binary sessions over one transport. Each tag is an
  independent linear channel; acyclicity is per-tag and
  per-connection. Cited as theoretical backing for "wire
  multiplexing ⊥ protocol structure."
- `agent/optics-theorist/linearity_gap` — agent-private
  reference material on LinearActris's connectivity-graph
  debug tool; the gap analysis on what Rust gives us
  statically vs what the paper gives us in Iris.
- `reference/papers/eact` — cross-reference. EAct and
  LinearActris cover complementary safety properties: EAct
  gives session fidelity + per-thread termination for the
  actor calculus; LinearActris gives global progress via the
  linearity + acyclicity theorem. Pane relies on both.

## Not to be confused with

The vendored gist directory at
`~/gist/deadlock-free-asynchronous-message reordering-in-Rust-with-multiparty-session-types/`
is a **different** paper: **Rumpsteak** by Cutner, Yoshida,
Vassor (PPoPP / PACMPL 2022 / ECOOP), about asynchronous
subtyping for message reordering via k-multiparty
compatibility model checking. Rumpsteak and LinearActris
solve different problems — Rumpsteak permits runtime
reordering of sends/receives provided an async-subtyping
check passes; LinearActris proves global progress for the
unreordered setting via connectivity-graph acyclicity. Pane
does not use Rumpsteak's reordering machinery; pane's single-
mailbox actor processes frames in causal order. If pane ever
wants out-of-order replies (multiple in-flight
`send_request`, Phase 3+ streaming), Rumpsteak's soundness
theorem becomes relevant.
