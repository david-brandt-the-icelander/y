# External effects

> Status marks: [`../development/knowledge-driven-development/README.md`](../development/knowledge-driven-development/README.md)

**Written for:** whoever writes Y.

## What this is for

[Shared state](shared-state.md) forbids a transaction from touching the outside world. This document is
the other half of that rule: how a transaction causes something to happen outside, exactly once, given
that the transaction itself may run several times and the process may die between any two instructions.

The pattern Y implements is the outbox, which every serious system eventually rebuilds by hand out of a
table, a worker, a correlation column and a retry loop. Y's claim is that the hand-built version is
reinvented badly often enough to belong in the language, and that the four steps it decomposes into are
worth naming:

1. change transactional state;
2. record durable intent to act;
3. perform the external action, outside the transaction;
4. record its outcome durably.

## Durable commands

- ⚠️ `enqueue` submits a durable command from inside a transaction. The command is persisted as part of
  the same commit, so it exists if and only if the transaction committed.

  ```y
  atomic:
      order.status = order_status.payment_pending

      enqueue charge_card(order_id: order.id, amount: order.total, payment_method: order.payment_method)
  ```

- ⚠️ A command is a declared type with typed fields, and submitting one requires a capability for that
  class of command → [Effects and authority](effects-and-authority.md).
- ⚠️ A command carries a stable identity assigned at submission, which survives process restart and is the
  key every later record about the command is filed under.
- ⚠️ A command has a declared handler that executes outside any transaction, receives the command, and may
  perform external effects.
- ⚠️ A handler's result is recorded durably against the command's identity, whether it succeeded, failed,
  or exhausted its retries.
- ⚠️ A command declares a retry policy — how many attempts, with what backoff — and a deadline after which
  it is abandoned rather than retried.
- ⚠️ A command declares whether its handler is idempotent. A handler that is not idempotent is delivered
  with the command identity available to it so that the external system can deduplicate, and Y does not
  silently promise exactly-once delivery to a system that cannot support it.
- ⚠️ Duplicate submissions of the same logical command deduplicate on a declared key, so a transaction that
  retries and re-runs its `enqueue` produces one command rather than several.
- ⚠️ A command carries a correlation identity linking it to the work that submitted it, so that a durable
  action can be traced back to the transaction that intended it.
- ⚠️ An abandoned command is visible as such, with its failure recorded, rather than disappearing — a
  command that exhausted its retries is a fact the program can observe and act on transactionally.
- ⚠️⛔ **A handler's failure escalates somewhere**, and whether that is a declared compensating command, a
  transition of the submitting workflow, a supervisor, or a dead-letter store the program must poll decides
  how a Y program recovers from an external system that stayed broken. Nothing in the working design settles
  it. ⛔ **Unblocked by** the language owner. This is the largest undecided question in the durable model:
  everything above describes getting an intent out reliably, and none of it describes what happens when the
  intent cannot be fulfilled.

## `afterCommit`

- ⚠️ `afterCommit` declares work that runs only if the enclosing transaction commits, and never during a
  speculative attempt.

  ```y
  atomic:
      order.status = order_status.confirmed

      afterCommit:
          enqueue send_confirmation(order.id)
  ```

- ⚠️ An `afterCommit` block observes the committed state, not the speculative state, and runs once per
  commit regardless of how many attempts preceded it.
- ⚠️ An `afterCommit` block that a retried attempt registered is discarded along with the rest of that
  attempt.
- ⚠️⛔ **`afterCommit` lowers to a durable mechanism rather than an in-memory callback**, so that a crash
  between commit and callback does not lose the action. Whether it always lowers durably, whether a
  non-durable form exists for work that is genuinely disposable, and how the durable form differs from
  writing the `enqueue` directly into the transaction is undecided; § 31 lists `afterCommit` syntax and
  lowering rules as open. ⛔ **Unblocked by** the language owner. Read as a durability guarantee the
  construct is nearly redundant with `enqueue`; read as a convenience it is a crash-loses-your-email bug
  with syntax, and the two readings are not reconcilable.

## External queries

- ⚠️ Transactional logic that needs an answer from outside gets it by persisting a request, letting a
  handler execute it outside the transaction, persisting the result, and resuming against transactional
  state that now contains the result. The wait is a `retry` on the result's absence, not a synchronous
  call → [Shared state § `retry`](shared-state.md#retry).
- ⚠️ The request, the correlation between request and response, the worker, and the resumption are provided
  by the language rather than assembled per use site out of a table and a polling loop.
- ⚠️ A request declares a deadline, and its expiry is a durable outcome the waiting logic observes like any
  other result.
- ⚠️ A crashed process resumes a pending external query from durable state, because the request, its
  identity and its response are all durable and the waiting side holds no in-memory state that matters.

## Durable workflows

- ⚠️ `workflow` declares a long-running operation whose progress is durable. Its position, its local values
  and the results of the commands it submitted are all reconstructable from durable state.

  ```y
  workflow fulfill(order_id):
      let order = load_order(order_id)

      let payment = submit charge_card(order.payment)
      let shipment = submit create_shipment(order)

      complete(order, payment, shipment)
  ```

- ⚠️ `submit` submits a durable command and suspends the workflow until its outcome is recorded, yielding
  that outcome. The suspension may outlive the process.
- ⚠️ A workflow survives process restart, machine restart, worker reassignment and a dependency that is
  unavailable for longer than any in-memory timeout would tolerate.
- ⚠️ A workflow resumes at most once per suspension point: a restarted workflow does not re-submit a command
  whose outcome is already recorded.
- ⚠️ A workflow's code is subject to the constraint that makes resumption possible — it is deterministic
  given its durable inputs, and every non-deterministic value it uses is obtained through a recorded
  command rather than read directly.
- ⚠️ A workflow is distinct from a task. A task is in-memory, cheap, and dies with its scope
  → [Concurrency and time](concurrency-and-time.md); a workflow is durable, expensive, and outlives the
  process.
- ⚠️ A workflow suspended across a deployment resumes under a version compatible with the one that
  suspended it, and persists its logical identity and a semantic continuation version rather than any
  address → [`../toolchain/runtime-generations.md`](../toolchain/runtime-generations.md).
- ⚠️⛔ **Workflow syntax and the boundary of what a workflow body may contain** are open — in particular
  whether a workflow may open an `atomic` block directly, and whether its determinism constraint is
  enforced by the effect system or by a separate check. § 31 lists workflow syntax as open.
  ⛔ **Unblocked by** the language owner.

## What the compiler rejects

| Program | Diagnostic | Status |
| --- | --- | --- |
| `enqueue` without a capability for that command class | Which authority is missing | ⚠️ |
| A command type whose fields are not durably representable | The offending field and why | ⚠️ |
| A non-idempotent handler with no deduplication key declared | That exactly-once cannot be provided | ⚠️ |
| A command with no deadline | That an unbounded retry policy is not permitted | ⚠️ |
| `afterCommit` outside a transaction | That there is no commit to follow | ⚠️ |
| A workflow reading the clock, randomness or an external system directly | That the value must arrive through a recorded command | ⚠️ |
| `submit` outside a workflow | That durable suspension requires a durable context | ⚠️ |

## What this deliberately does not do

- **Y does not promise exactly-once external effects.** It promises exactly-once *durable intent*, at-least
  once delivery, and the identity an external system needs to deduplicate. A provider with no idempotency
  key cannot be made exactly-once by a language.
- **Y does not distribute transactions across external systems.** There is no two-phase commit over a
  payment provider, and the durable command model exists precisely because there is not.
- **Y does not make the outside world transactional.** A committed transaction means the intent is durable,
  not that the effect has happened.

## Where it lives

- The rule that creates the need for all of this → [Shared state](shared-state.md).
- Tasks, deadlines, and how a workflow's suspension differs from a task's
  → [Concurrency and time](concurrency-and-time.md).
- The authority to submit a class of command → [Effects and authority](effects-and-authority.md).
- How commands, handlers and workflows are represented for a backend
  → [`../toolchain/core-ir.md`](../toolchain/core-ir.md).
- Simulating a crash between commit and handler → [`../toolchain/oracles.md`](../toolchain/oracles.md).
