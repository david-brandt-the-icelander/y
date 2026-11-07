# Shared state

> Status marks: [knowledge-driven development](https://github.com/david-brandt-the-icelander/knowledge-driven-development)

**Written for:** whoever writes Y.

## What this is for

Shared mutable state in Y is transactional by construction. The primary abstraction is a transaction over
shared memory, not a lock, because a lock is a correctness obligation distributed across every site that
takes it in the wrong order, and an agent will get that wrong.

The cost of choosing transactions is that a transaction can be retried, and a retried computation must be
repeatable. That single consequence is the most opinionated thing in the language: it is why Y forbids
external effects inside a transaction, and why an entire second mechanism exists to carry them out
→ [External effects](external-effects.md).

## Declaring shared state

- ⚠️ `shared` declares state that more than one task may reach, and every read and write of it happens
  inside a transaction.

  ```y
  shared inventory: inventory
  ```

- ⚠️ Shared state is reached only through a capability for it, so which components can touch a given
  shared resource is visible in the signatures
  → [Effects and authority](effects-and-authority.md).
- ⚠️ Reading or writing shared state outside a transaction is a compile error. There is no unsynchronised
  fast path and no escape hatch that performs a bare read.
- ⚠️ State that is not `shared` is owned by exactly one task, is not transactional, and costs nothing —
  the transactional machinery applies to what is actually shared, not to all mutation.

## `atomic`

- ⚠️ An `atomic` block executes its body as one logical transaction over shared state. Its reads observe
  a single consistent snapshot, its writes become visible together at commit, and no other transaction
  observes a partial state.

  ```y
  atomic:
      item = inventory[item_id]

      if item.available == 0:
          retry

      item.available := item.available - 1
  ```

- ⚠️ The runtime records what the transaction read and wrote, detects conflict with a committed
  transaction, and re-executes the body from the start against a fresh snapshot.
- ⚠️ A transaction that commits does so atomically, and one that does not commit leaves no trace in
  shared state.
- ⚠️ `atomic` blocks nest, and a nested block joins the enclosing transaction rather than starting a new
  one — there is no partial commit of an inner block.
- ⚠️ The body of an `atomic` block is repeatable: the compiler admits only operations whose re-execution
  is unobservable, which is where the rule below comes from.
- ⚠️ A transaction is bounded by its enclosing deadline, and a transaction that cannot commit within it
  fails rather than retrying indefinitely → [Concurrency and time](concurrency-and-time.md).

## `retry`

- ⚠️ `retry` abandons the current attempt and suspends the transaction until some value it read has
  changed, at which point the body re-executes from the start.
- ⚠️ `retry` is a semantic wait, not a spin: the runtime knows the transaction's read set and wakes it
  when that set is invalidated, so a waiting transaction consumes no CPU.
- ⚠️ A suspended `retry` is bounded by the enclosing deadline or cancellation scope exactly as any other
  suspension is, and a `retry` with no finite bound above it is a compile error
  → [Concurrency and time](concurrency-and-time.md).
- ⚠️ `retry` composes with the condition that preceded it: a transaction that reads a value, finds it
  unready and retries is a blocking operation on that value with no separate condition-variable protocol
  to get wrong.

## `or_else`

- ⚠️ `or_else` composes transactional alternatives. The first block runs; if it reaches `retry`, its
  effects are discarded and the second block runs; if that also reaches `retry`, the composed transaction
  waits on the union of both read sets.

  ```y
  atomic:
      take(primary_queue)
  or_else:
      take(secondary_queue)
  ```

- ⚠️ An alternative that commits does so as the whole transaction, and the discarded alternative's writes
  are never observable.
- ⚠️ `or_else` chains, and the alternatives are tried left to right.

## The rule: no external effect inside a transaction

**This is the load-bearing semantic rule of the language.** It is stated here because this is where it is
enforced; what to do instead is [External effects](external-effects.md).

- ⚠️ Transactional code performs no operation on a non-transactional external system. Network calls,
  filesystem writes, device commands, payment authorisations and log emissions are all rejected inside an
  `atomic` block, at compile time, by the effect set of the block's body.

  ```y
  atomic:
      account.balance := account.balance - amount
      payment_provider.charge(card, amount)   -- rejected
  ```

- ⚠️ The diagnostic for this names the effect, the call path that introduced it, and the durable mechanism
  that replaces it, because a rule this restrictive has to point at the alternative or it will be worked
  around.
- ⚠️ A transaction likewise does not synchronously query an external system and treat the answer as part
  of its snapshot. An external answer is not versioned, cannot be re-observed identically on retry, and
  therefore cannot participate in transactional consistency
  → [External effects § External queries](external-effects.md#external-queries).
- ⚠️ Reading the clock and drawing randomness are external effects for this purpose, and a transaction
  that needs either receives it as a value fixed before the transaction began.
- ⚠️ Computation with no externally visible effect is permitted however expensive it is, including native
  compute over private memory, because re-executing it is unobservable
  → [Ownership and resources § Arenas](ownership-and-resources.md#arenas).

The reasoning is worth keeping in one sentence, because every part of the design that looks like ceremony
descends from it: **an `atomic` block may run more than once, so anything inside it that cannot be undone
may happen more than once.** A charge that cannot be rolled back with the transaction is a charge the
customer may pay twice.

## What the compiler rejects

| Program | Diagnostic | Status |
| --- | --- | --- |
| Shared state read or written outside a transaction | That access requires a transaction | ⚠️ |
| An external effect inside `atomic` | The effect, its call path, and the durable mechanism to use instead | ⚠️ |
| A synchronous external query inside `atomic` | That an external answer cannot join the snapshot | ⚠️ |
| Clock or randomness inside `atomic` | That the value must be fixed before the transaction | ⚠️ |
| `retry` with no finite bound above it | Where a deadline must be introduced | ⚠️ |
| Shared state reached without a capability for it | Which authority is missing | ⚠️ |

## What this deliberately does not do

- **No locks, mutexes, condition variables or memory-ordering primitives in the surface language.** They
  exist in the runtime's implementation and are not a vocabulary Y programs speak.
- **No lock-free data structures written by hand.** A hand-rolled concurrent structure is exactly the
  code whose correctness cannot be reviewed, which is the case this language is trying to remove.
- **No transactional guarantee over anything that is not `shared`.** A transaction does not roll back
  local variables, files, or another process's state.

## Where it lives

- Carrying an external effect out of a transaction, durably → [External effects](external-effects.md).
- Bounding a transaction and a `retry` → [Concurrency and time](concurrency-and-time.md).
- Why a transaction's body must be repeatable, and what is repeatable
  → [Ownership and resources](ownership-and-resources.md).
- How transactions, read sets, write sets and `retry` are represented for a backend
  → [`../toolchain/core-ir.md`](../toolchain/core-ir.md).
- Forcing a conflict on purpose, in a test → [`../toolchain/oracles.md`](../toolchain/oracles.md).
- Multi-version concurrency control, and migrating shared state while it is in use
  → [`../toolchain/runtime-generations.md`](../toolchain/runtime-generations.md).
