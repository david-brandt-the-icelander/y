# Concurrency and time

> Status marks: [`../development/knowledge-driven-development/README.md`](../development/knowledge-driven-development/README.md)

**Written for:** whoever writes Y.

## What this is for

Y removes `async` from signatures → [Effects and authority](effects-and-authority.md). That leaves the
question the colouring was incidentally answering: if a call can suspend and nothing in the source says
so, what stops it waiting forever?

The answer is that nothing in Y suspends without a bound, and the bound is checked rather than
conventional. A deadline is not a parameter a careful programmer remembers to pass; it is a property of
the scope, inherited by everything inside it, and a suspension with no finite bound above it does not
compile.

Everything else here follows: if suspension is bounded, tasks need lifetimes; if tasks have lifetimes,
cancellation needs to propagate; and if cancellation propagates, failure isolation needs a policy.

## Bounded suspension

- ⚠️ Every potentially suspending operation has a finite bound, from an explicit deadline on the operation
  or an enclosing scope with one.
- ⚠️ `within` bounds a single operation:

  ```y
  let result = queue.receive() within 5s
  ```

- ⚠️ `within` bounds a scope, and every suspension inside it shares the one deadline:

  ```y
  within 30s {
      let user = load_user(id)
      let orders = load_orders(user.id)
      render(user, orders)
  }
  ```

- ⚠️ A scope's deadline is absolute, not a per-operation budget: three calls inside `within 30s` have
  thirty seconds between them, not ninety.
- ⚠️ A nested `within` may only shorten the effective deadline. A scope cannot extend beyond its parent's
  deadline, and one that asks to is a compile error where the bound is known and a runtime failure where
  it is not.
- ⚠️ Exceeding a deadline is a typed failure that names the deadline, propagated like any other failure
  → [Types and typed failure](types-and-failures.md).
- ⚠️ A suspension with no finite bound above it is a compile error that names the operation and the scope
  where a deadline must be introduced.
- ⚠️ An operation that must genuinely wait indefinitely says so explicitly, and the construct that says
  so is distinct, greppable, and requires a capability — so "this waits forever" is a reviewable claim
  rather than an omission.
- ⚠️ A transaction and a suspended `retry` are bounded by the same mechanism
  → [Shared state](shared-state.md).

## Tasks

- ⚠️ `task` starts concurrent work belonging to the enclosing scope:

  ```y
  within 10s {
      task user = load_user(id)
      task orders = load_orders(id)

      render(user, orders)
  }
  ```

- ⚠️ A scope does not exit until every task started in it has finished or been cancelled. There are no
  orphan tasks and no detached work by default.
- ⚠️ A task inherits its scope's deadline and cancellation state.
- ⚠️ Reading a task's value waits for it, bounded by the scope's deadline.
- ⚠️ A task that fails cancels its siblings and propagates its failure out of the scope, so a scope's
  failure type includes the failures of the tasks inside it.
- ⚠️ A task's ownership of the values it captures is checked like any other transfer, and two concurrent
  tasks cannot hold conflicting borrows of the same value
  → [Ownership and resources](ownership-and-resources.md).
- ⚠️ Work that must outlive its starting scope is detached through a deliberate, distinct construct that
  requires a capability and an explicit lifetime, rather than by omitting a join.

## Cancellation

- ⚠️ Cancellation propagates down: cancelling a scope cancels its tasks and their tasks.
- ⚠️ Cancellation is observed at suspension points. A cancelled task resumes with a cancellation failure
  at its next suspension rather than being terminated between arbitrary instructions.
- ⚠️ A cancelled task runs its cleanup — every owned value is destroyed in order, and declared cleanup
  runs → [Ownership and resources](ownership-and-resources.md).
- ⚠️ Cleanup during cancellation has its own bound, so a cancelled scope cannot be held open indefinitely
  by a cleanup path that itself suspends.
- ⚠️ A region may be declared uncancellable so that a short critical sequence completes, and the region
  is bounded — an uncancellable region with no finite bound is a compile error.
- ⚠️ A committed transaction is not undone by cancellation arriving afterwards, and a durable command
  that was persisted with it is not withdrawn
  → [External effects](external-effects.md).

## Supervision

- ⚠️ A long-lived component declares a restart policy: whether it restarts on failure, how often before
  the failure escalates, and over what window.
- ⚠️ A supervisor owns a set of children, observes their failures, and applies its declared policy —
  restarting the failed child alone, or restarting the whole set where they share state that a partial
  restart would leave inconsistent.
- ⚠️ A failure that exceeds a restart policy escalates to the supervisor's own supervisor rather than
  being retried forever or swallowed.
- ⚠️ A restarted component starts from its declared initial state, and shared state it had committed to
  survives the restart, so restart is not a recovery mechanism for transactional data.
- ⚠️ Failure isolation is structural: a component's failure affects its supervision subtree and the tasks
  in its scopes, and reaches nothing else.
- ⚠️⛔ **Supervision has a source model** — whether a supervisor is a declaration, a value, a scope
  construct, or a property of a workflow, and how it relates to task scopes, which already propagate
  failure and cancellation. The working design says the source model is open, and the overlap with task
  scopes is unresolved: a task scope that cancels its siblings on failure is already a supervisor with a
  fixed policy. ⛔ **Unblocked by** the language owner. Until it is settled, nothing here can be
  implemented without inventing the construct.

## What the compiler rejects

| Program | Diagnostic | Status |
| --- | --- | --- |
| A suspension with no finite bound above it | The operation, and where a deadline belongs | ⚠️ |
| A nested scope extending its parent's deadline | Both deadlines | ⚠️ |
| An uncancellable region containing an unbounded suspension | The suspension | ⚠️ |
| A scope exiting with a task neither finished nor cancelled | Which task | ⚠️ |
| Two concurrent tasks holding conflicting borrows | The borrows and both task sites | ⚠️ |
| Detaching work without the capability and an explicit lifetime | What is missing | ⚠️ |

## What this deliberately does not do

- **No thread API.** Y programs do not create, name, pin or join operating-system threads; a task's
  placement is the runtime's decision.
- **No manual `await`, and no promise or future type in the surface language.** A task's value is read by
  reading it.
- **No unstructured cancellation token threaded through signatures by hand.** Cancellation is a property
  of the scope.
- **No timeout that silently abandons work still running.** A deadline cancels, and cancellation completes.

## Where it lives

- Why there is no `async` in signatures → [Effects and authority](effects-and-authority.md).
- Transactions, `retry`, and their bounds → [Shared state](shared-state.md).
- Durable suspension that outlives the process, which is not a task
  → [External effects § Durable workflows](external-effects.md#durable-workflows).
- How scopes, deadlines and cancellation are represented for a backend
  → [`../toolchain/core-ir.md`](../toolchain/core-ir.md).
- Controlling the scheduler, injecting cancellation, and reproducing an interleaving
  → [`../toolchain/oracles.md`](../toolchain/oracles.md).
