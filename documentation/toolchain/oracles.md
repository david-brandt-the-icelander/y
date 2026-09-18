# Oracles and deterministic testing

> Status marks: [`../development/knowledge-driven-development/README.md`](../development/knowledge-driven-development/README.md)

**Written for:** whoever implements a projection, or is holding a disagreement between two of them.

## What it is

A new compiler's hardest problem is that it has no second opinion. Y's answer is to borrow other people's:
project Core IR into the input language of a mature tool, and ask that tool whether Y's conclusion holds.
Rust's borrow checker, a C model checker, a proof assistant and a model checker for protocols are each
decades of work Y does not have to repeat, and each of them will disagree with Y in a way that is
informative.

This makes the surrounding ecosystem a differential-testing harness for Y itself. It is also the reason
[Core IR](core-ir.md) must be explicit: a projection can only be generated from facts that are recorded.

## The boundary

Every tool on the other side of a projection was built by someone else for a different purpose. Two
consequences hold throughout:

- ⚠️ A projection is **evidence about Y's implementation, never a definition of Y.** If rustc rejects a
  projection of a program Y accepted, the possibilities are that Y's ownership analysis is wrong, that the
  projection is wrong, or that Y is deliberately more permissive than Rust in a way Y can justify. Nothing
  about that list makes rustc the definition of Y's ownership model.
- ⚠️ A projection's *absence* of a complaint is weak evidence. A tool that could not model the construct,
  timed out, or was given a projection that discarded the interesting part reports success, and a harness
  that counts that as verification is lying → [failure modes](#failure-modes).

## Projections

- ⚠️ Each projection states which Y properties it is evidence about, and the harness records that mapping
  rather than treating "the tool was happy" as a global pass.
- ⚠️ **Ownership and undefined behaviour** — Core IR is projected to Rust, and rustc's acceptance is
  evidence about Y's ownership and borrowing conclusions. Miri checks the projection for undefined
  behaviour that rustc's types do not catch.
- ⚠️ **Memory and control flow at C level** — where a C backend exists, CBMC and Frama-C are given the
  generated C to check memory safety and reachability on paths Y's own analysis approximates.
- ⚠️ **Contracts and functional correctness** — contracts and their obligations are projected to Why3 and
  Dafny, so a postcondition Y cannot discharge may still be discharged by a prover built for it
  → [`../language/specification-in-types.md`](../language/specification-in-types.md).
- ⚠️ **State machines and protocols** — `state` and `transition` declarations are projected to TLA+ and
  checked with Apalache, answering reachability, deadlock and invariant questions that a type system
  cannot: a typestate declaration says which transitions are legal, and a model checker says whether the
  bad state is reachable through legal ones.
- ⚠️ **Concurrent execution** — concurrent programs are explored with Loom and Shuttle against the Rust
  projection, and with Y's own scheduler → [below](#deterministic-testing).
- ⚠️ **Optimisation correctness** — where a transformation is LLVM-shaped, Alive2 checks that the
  transformation itself is sound rather than that one output happened to work.
- ⚠️ **Backend equivalence** — the same program is run through every available backend and the results,
  failures, observable effects, state transitions and serialised output are compared.
- ⚠️ A high-assurance C path through CompCert is available for selected compilation paths where its
  guarantees justify its constraints.
- ⚠️ Projections run in CI on a corpus, not on demand, so a regression in Y's analysis is caught by the
  build rather than by whoever next remembers to ask.

## Disagreement is a bug signal

- ⚠️ **Oracle results are never combined by majority vote.** Two agreeing tools and one dissenter is three
  pieces of evidence and one open question, not a two-to-one verdict.
- ⚠️ Any of these is an investigation rather than a tolerated difference: Y accepts a program its Rust
  projection rejects; a C verifier finds an invalid memory path; a prover refutes a contract Y discharged; a
  model checker reaches a state the typestate declaration forbids; two backends produce different
  observable behaviour.
- ⚠️ A disagreement is automatically minimised into a small reproducer, and the reproducer is committed to
  the corpus so the case is checked forever after.
- ⚠️ A disagreement Y decides is acceptable is recorded with its justification and suppressed specifically —
  by construct and by reason, never by disabling the projection.
- ⚠️ An agent may generate candidate programs to find disagreements, and may propose the fix. The
  disagreement is decided by the tools; whether the fix is correct is decided by the tools again.

## Deterministic testing

Concurrency and durability are where Y's semantics are most ambitious, and both are untestable without a
scheduler that can be controlled.

- ⚠️ The runtime's scheduler is controllable: a test drives the interleaving rather than hoping for one.
- ⚠️ The explorer enumerates the interleavings that matter for a program, using the read and write sets and
  effect information Core IR already carries to avoid exploring equivalent schedules.
- ⚠️ A test injects cancellation at a chosen suspension point, so cancellation paths — the ones production
  reaches rarely and cleanup code gets wrong — are exercised deliberately
  → [`../language/concurrency-and-time.md`](../language/concurrency-and-time.md).
- ⚠️ A test injects a deadline expiry at a chosen point.
- ⚠️ A test forces a transaction conflict, so retry paths execute without needing a race to occur
  → [`../language/shared-state.md`](../language/shared-state.md).
- ⚠️ A test simulates a crash at a chosen point around a durable boundary — between commit and handler,
  between handler and outcome record, mid-workflow — and asserts what survived
  → [`../language/external-effects.md`](../language/external-effects.md).
- ⚠️ **A schedule that produced a failure once is reproducible from a recorded seed**, and the schedule is
  committed with the test. A concurrency bug found and not reproducible is a bug that will be closed as
  unexplained.
- ⚠️ Determinism here is the same property the physical optimiser depends on, obtained the same way
  → [runtime-generations.md](runtime-generations.md).

## Failure modes

| Symptom | Cause | What it looks like from outside | Handled? |
| --- | --- | --- | --- |
| A projection passes everything | It discarded the property it was meant to test | Broad green coverage over nothing | ⚠️ |
| A tool cannot model a construct | Core IR carries it, the target language has no equivalent | Whole classes of program silently unverified, reported as passing | ⚠️ |
| A projection is unsound rather than Y being wrong | Bug in the projection, not the analysis | Investigation of the wrong component; a correct Y change reverted | ⚠️ |
| Minimisation does not terminate | Reducer fights a non-deterministic failure | A disagreement nobody can act on | ⚠️ |
| A flaky exploration is re-run until green | Seed not recorded, or recorded and ignored | A real interleaving bug, closed as flake | ⚠️ |
| Suppressions accumulate | Each individually justified, collectively a disabled oracle | An oracle that reports success and checks nothing | ⚠️ |

## Where it lives

- What projections are generated from → [core-ir.md](core-ir.md).
- Where projections sit in the build → [compiler-pipeline.md](compiler-pipeline.md).
- The other consumer of determinism and comparison → [runtime-generations.md](runtime-generations.md).
- The language properties being checked → [`../language/README.md`](../language/README.md).
