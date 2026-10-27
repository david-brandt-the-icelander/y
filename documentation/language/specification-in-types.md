# Specification in types

> Status marks: [`../development/knowledge-driven-development/README.md`](../development/knowledge-driven-development/README.md)

**Written for:** whoever writes Y.

## What this is for

Three mechanisms here do the same job at different scales: they move a statement that would otherwise
live in a comment, a code review or a runtime assertion into a position where the compiler can act on it.
Contracts say what must hold across a call. Typestate says which operations a value currently permits.
Units say what a number means.

What they share is that each one converts a class of bug an agent reliably produces — the unchecked
precondition, the call in the wrong protocol state, the millisecond passed as a second — into a rejected
program. And each one gives a generating agent more structure to aim at, which is the reason they are
worth their syntax cost.

## Contracts

- ⚠️ `requires` declares a precondition, `ensures` a postcondition, and both are part of the function's
  signature rather than statements in its body.

  ```y
  function withdraw(account: mutable Account, amount: Money) -> Receipt ! WithdrawError
      requires amount > 0
      requires account.balance >= amount
      ensures account.balance == old(account.balance) - amount
  {
      ...
  }
  ```

- ⚠️ `old(e)` in a postcondition denotes `e` evaluated on entry, which is what makes a statement about
  change expressible.
- ⚠️ A contract expression is pure and terminating. It may read its arguments and call pure functions, and
  a contract with any effect is a compile error → [Effects and authority](effects-and-authority.md).
- ⚠️ A type declares an invariant that holds of every value of that type outside a mutation, and a
  function that takes the type by exclusive borrow may break the invariant inside its body and must
  restore it before returning.
- ⚠️ A contract is checked statically where the compiler can discharge it, and the program is rejected
  where the compiler can prove it false.
- ⚠️ A contract the compiler can neither prove nor refute becomes a runtime check whose failure is a
  distinct, non-recoverable program fault rather than a typed failure — a violated contract is a bug, not
  a condition to handle.
- ⚠️ The build reports which contracts were discharged statically and which became runtime checks, because
  a contract silently demoted to a runtime check in an optimised build is a guarantee that was never
  there.
- ⚠️ Contracts generate property tests: a precondition describes the valid input domain and a postcondition
  the property to assert, so a contract is a test generator as well as a check.
- ⚠️ Contracts are projected to independent verifiers rather than only to Y's own checker
  → [`../toolchain/oracles.md`](../toolchain/oracles.md).
- ⚠️⛔ **How much refinement reasoning the type system carries** — whether a refinement like
  `Money where amount > 0` is a type that propagates through inference, or whether contracts remain
  attached to functions and checked at call sites — decides whether an ordinary program needs a prover in
  its build. § 31 lists contract and refinement syntax as open.
  ⛔ **Unblocked by** the language owner. The working design's stated intent is richer reasoning without
  requiring full theorem proving for ordinary programs, which is a direction rather than a boundary.

## Typestate

- ⚠️ `state` declares the protocol states of a type, and a value of the type is always in exactly one of
  them.

  ```y
  state Order {
      .Created
      .PaymentPending
      .Paid
      .Cancelled
      .Fulfilled
  }
  ```

- ⚠️ A state is usable as a type: `Order.Created` is the type of an order in that state, and a function may
  take or return one.
- ⚠️ `transition` declares a legal move between states, and consumes the value in the source state to
  produce it in the target:

  ```y
  transition pay(order: Order.Created) -> Order.PaymentPending
  ```

- ⚠️ A transition not declared does not exist. Calling an operation on a value in the wrong state is a
  compile error naming the state the value is in and the states the operation accepts.
- ⚠️ An operation available in several states declares that set, and one available in all of them says so
  rather than enumerating.
- ⚠️ A state may carry data that only exists in that state, so a field that is meaningful only after
  payment is reachable only from the paid state rather than being optional everywhere.
- ⚠️ Where a value's state is not known statically — it came from storage, or from a branch that could
  produce either — it is matched to recover the state, exhaustively, like any sum type.
- ⚠️ A typestate value's transitions and its ownership interact: a transition consumes the value, so a
  stale reference to the pre-transition state is an ownership error rather than a logic error
  → [Ownership and resources](ownership-and-resources.md).
- ⚠️ A persisted state machine's states and transitions are the same declaration the schema uses, so the
  legal transitions of stored data are the legal transitions of the type
  → [Data and schemas](data-and-schemas.md).
- ⚠️ A state machine is projected to a model checker, which answers the questions a type system does not —
  reachability, deadlock, and whether an invariant holds over every path
  → [`../toolchain/oracles.md`](../toolchain/oracles.md).
- ⚠️⛔ **Typestate syntax and its scope** are open, in particular whether typestate applies to shared
  transactional state as well as to owned values. A state machine over `shared` data raises a question
  ownership does not answer: two transactions may each observe a legal transition from the same state and
  conflict only at commit. § 31 lists state-machine and typestate syntax as open.
  ⛔ **Unblocked by** the language owner.

## Units of measure

- ⚠️ A unit is part of a type, and a literal may carry one:

  ```y
  let timeout: Seconds = 5s
  let distance: Kilometers = 12.4km
  let amount: ISK = 1500kr
  ```

- ⚠️ Arithmetic respects units: adding two `Seconds` yields `Seconds`, dividing `Kilometers` by `Hours`
  yields a speed, and adding `Seconds` to `Kilometers` is a compile error.
- ⚠️ A plain number does not silently become a united value, and a united value does not silently become a
  plain number.
- ⚠️ Conversion between units of the same dimension is explicit at the point of conversion, and the
  conversion factor is part of the unit's declaration rather than written at each site.
- ⚠️ A program declares its own units and dimensions, so a currency, a wash cycle count or a request quota
  is a unit on the same footing as a metre.
- ⚠️ Units are erased before code generation and cost nothing at runtime.
- ⚠️ A currency is a dimension, not a scale factor — `ISK` and `EUR` are not interconvertible by a constant,
  and a program that adds them is rejected rather than converted at some ambient rate.
- ⚠️ A deadline is a united value, so `within 5s` is type-checked rather than conventional
  → [Concurrency and time](concurrency-and-time.md).

## What the compiler rejects

| Program | Diagnostic | Status |
| --- | --- | --- |
| A call whose precondition the compiler can prove false | The precondition and the argument that violates it | ⚠️ |
| A body that can exit without establishing its postcondition | The path and the unmet clause | ⚠️ |
| A contract expression with an effect | The effect | ⚠️ |
| An operation called on a value in a state that does not permit it | The current state and the accepted states | ⚠️ |
| Use of a value after a transition consumed it | The transition site | ⚠️ |
| Arithmetic mixing dimensions | Both dimensions | ⚠️ |
| An implicit conversion between units or into a plain number | Where the conversion must be written | ⚠️ |

## Where it lives

- The shapes contracts and states are written over → [Types and typed failure](types-and-failures.md).
- Persisted states and their versions → [Data and schemas](data-and-schemas.md).
- Independent verification of contracts and state machines
  → [`../toolchain/oracles.md`](../toolchain/oracles.md).
- How contracts and state transitions reach a backend
  → [`../toolchain/core-ir.md`](../toolchain/core-ir.md).
