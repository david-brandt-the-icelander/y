# Journey: an agent writes a function

> Status marks: [`../development/knowledge-driven-development/README.md`](../development/knowledge-driven-development/README.md)

**Actor:** a coding agent, working from a human's request, with a human reviewing the result.
**Starts when:** the agent is asked to add a capability to a Y program.
**Ends when:** the change is committed, or the agent gives up and says why.

This is the journey Y exists for. If it works, the language's premise holds: an agent can produce code
whose safety a reviewer does not have to establish by reading it. If any step needs the reviewer to know
something the compiler could have known, that is a defect in Y and not in the agent.

The example throughout is one function: reserve an item from shared inventory and charge the customer.
Both hazards Y is built around are in it — a transaction that may run twice, and a payment that must not.

## Steps

| # | The agent does | The toolchain does | Status |
| --- | --- | --- | --- |
| 1 | Reads the capability documents for the area it is changing | Serves them as ordinary files, with each statement marked for whether it is true yet | ⚠️ |
| 2 | Writes a function, guessing at a signature | Infers types, effects and ownership; checks the written signature against what it inferred | ⚠️ |
| 3 | Puts the charge inside the `atomic` block, because that reads naturally | Rejects the program, naming the effect, the call path that introduced it, and the durable mechanism that replaces it → [shared state](../language/shared-state.md) | ⚠️ |
| 4 | Moves the charge to an `enqueue` of a durable command | Accepts it; records that the command becomes durable with the commit, and that its handler runs outside the transaction → [external effects](../language/external-effects.md) | ⚠️ |
| 5 | Forgets a deadline on the call that loads the customer | Rejects the program, naming the suspension and the scope where a bound belongs → [concurrency and time](../language/concurrency-and-time.md) | ⚠️ |
| 6 | Adds `within`, and handles the failure case it introduced | Checks exhaustiveness over the new failure set → [types and typed failure](../language/types-and-failures.md) | ⚠️ |
| 7 | Asks what the function may now do | Reports the effect set, the authority reachable from it, the suspension points, and the commands it may submit → [effects and authority](../language/effects-and-authority.md) | ⚠️ |
| 8 | Asks what the compiler concluded that the source does not say | Prints the elaborated program as explicit Y — inferred borrows, transaction boundaries, failure propagation → [compiler pipeline](../toolchain/compiler-pipeline.md) | ⚠️ |
| 9 | Runs the tests, including the failure paths it cannot provoke by hand | Forces a transaction conflict, expires the deadline, and crashes between commit and handler, on demand and reproducibly → [oracles](../toolchain/oracles.md) | ⚠️ |
| 10 | Generates property tests | Derives them from the contracts on the function → [specification in types](../language/specification-in-types.md) | ⚠️ |
| 11 | Asks whether an independent tool agrees | Projects to Rust and reports rustc's verdict on the ownership structure → [oracles](../toolchain/oracles.md) | ⚠️ |
| 12 | Reads what will actually run | Shows generated Rust a competent Rust programmer might have written → [compiler pipeline](../toolchain/compiler-pipeline.md) | ⚠️ |
| 13 | Updates the marks on the statements its change made true | Nothing — this is the one step with no mechanical support, and the reviewer is the only check on it | ⚠️ |
| 14 | Commits | — | ⚠️ |

Steps 3 and 5 are the journey's whole argument. Each is a bug that a reviewer would have had to catch by
knowing a convention — *never call a payment provider inside a transaction*, *always pass a timeout* —
and each becomes a compile error with the remedy in the diagnostic. Neither depends on the agent having
read anything.

## Where it can stop

| Step | Why it stops | What the actor sees | Recovery | Status |
| --- | --- | --- | --- | --- |
| 2 | Ownership inference concludes something the agent did not expect | A borrow error naming both use sites | Provenance for the inference: what was concluded and from where | ⚠️ |
| 3 | The agent takes the rejection as a reason to move state out of `shared` | An accepted program with a race instead of a rejected one with a diagnostic | None — a diagnostic that names only the prohibition invites this, which is why it names the replacement | ⚠️ |
| 5 | The agent satisfies the bound with an absurd deadline | An accepted program | None mechanical. A deadline is a reviewable claim and this is a case for the reviewer | ⚠️ |
| 7 | The effect set is larger than expected and the reason is not local | An effect introduced several calls away | The path that introduced it, named in the report | ⚠️ |
| 9 | A failure appears only under one interleaving | An intermittent test | The schedule, recorded as a seed and committed with the test | ⚠️ |
| 11 | rustc rejects a projection of a program Y accepted | A disagreement | Automatic minimisation to a reproducer; the disagreement is investigated, not voted on → [oracles](../toolchain/oracles.md) | ⚠️ |
| 13 | The agent ships the code and leaves the documentation claiming the capability is missing | A corpus that contradicts the code | None. No gate exists in this repository → [`../README.md` § The state of this corpus](../README.md#the-state-of-this-corpus) | ⚠️ |

## Variants

- **A human writes the function.** Steps 1 to 12 are identical, which is the point — Y does not have an
  agent mode. Step 13 is likelier to happen.
- **The agent is fixing a defect rather than adding a capability.** The journey starts at a reproducer and
  ends at a promoted function version rather than a commit
  → [runtime generations](../toolchain/runtime-generations.md). ⚠️⛔ Blocked on whether Y has that
  mechanism at all. ⛔ **Unblocked by** the language owner
  → [`../toolchain/runtime-generations.md` § Status of this document](../toolchain/runtime-generations.md#status-of-this-document).
- **The change alters a persistent shape.** Additional steps for a schema version and a migration
  → [data and schemas](../language/data-and-schemas.md).

## Capabilities this traverses

[Types and typed failure](../language/types-and-failures.md) ·
[Ownership and resources](../language/ownership-and-resources.md) ·
[Effects and authority](../language/effects-and-authority.md) ·
[Shared state](../language/shared-state.md) ·
[External effects](../language/external-effects.md) ·
[Concurrency and time](../language/concurrency-and-time.md) ·
[Specification in types](../language/specification-in-types.md)

## What this journey is anchored to

The taxonomy requires a journey document to correspond to a row in an admin database, and calls one
without a row a feature document wearing a journey's clothes
→ [taxonomy](../development/knowledge-driven-development/05-document-taxonomy.md#user-journey-documents).
That database belongs to the system the convention was imported from.

- ⚠️ This journey is anchored instead to a reproducible toolchain invocation: a committed Y program, a
  sequence of compiler runs, and the expected diagnostic at each step. Every row in the steps table above
  is then a test rather than a claim, and the marks on it are computed rather than asserted.
  ⛔ **Unblocked by** anything existing to run. Until then this document is the specification of that test
  and the weakest-anchored page in the corpus.
