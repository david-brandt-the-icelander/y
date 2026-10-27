# Queries and collections

> Status marks: [`../development/knowledge-driven-development/README.md`](../development/knowledge-driven-development/README.md)

**Written for:** whoever writes Y.

## What this is for

A loop that filters, projects and aggregates says two things at once: what the answer is, and how to
compute it. The second is almost always a guess about data the programmer cannot see — how many records,
in what order, changing how often — and it is the guess a physical optimiser could make from measurement
instead → [`../toolchain/runtime-generations.md`](../toolchain/runtime-generations.md).

So Y's bulk operations are written as what the answer is. The point is not brevity; it is that a
declaratively stated computation is one the compiler may reorder, fuse, index, vectorise or maintain
incrementally, and an explicit loop is one it must execute as written.

## Pipelines

- ⚠️ Collection operations compose as a chain, each stage naming what it does:

  ```y
  names =
      users
          .where(active)
          .map(name_of)
          .sorted()
  ```

- ⚠️ Each stage takes a named callable — a `rule` such as `active`, a function, or a lambda bound with
  `adult = lambda adult(user) returns grown_up: …`. Y has no inline lambda expression
  → [`../../proposal/Y-proposal.md` § 3.1](../../proposal/Y-proposal.md#31-source-form).
- ⚠️ A chain is fused into a single traversal where the stages permit it, rather than materialising an
  intermediate collection per stage.
- ⚠️ **A chain reads its receiver and produces a new value.** It never changes the receiver: changing
  the elements of a collection is a `for` loop whose body says so, with `:=` or a `mutable` call
  → [Ownership and resources](ownership-and-resources.md).
- ⚠️ A stage's laziness is not a property the programmer manages: the semantics are those of the whole
  chain, and whether an intermediate is materialised is a decision recorded in the semantic layer rather
  than implied by which method was called.
- ⚠️ A chain whose stages are pure and whose element operations are independent may be evaluated in
  parallel, and the compiler decides whether to, from the effect sets it already has
  → [Effects and authority](effects-and-authority.md).
- ⚠️ Ordering is explicit. A chain that does not sort makes no promise about order beyond what its source
  guarantees, and a program relying on an incidental order is relying on an optimiser decision.
- ⚠️ A chain over transactional state reads within the enclosing transaction's snapshot, and its reads join
  that transaction's read set → [Shared state](shared-state.md).

## `derived`

- ⚠️ `derived` declares a value defined by an expression over other state, maintained rather than
  recomputed:

  ```y
  derived active_user_count =
      users.where(active).count()
  ```

- ⚠️ A derived value is always consistent with the state it is defined over, and reading it inside a
  transaction observes a value consistent with that transaction's snapshot.
- ⚠️ A derived value is maintained incrementally where the runtime can — a change to one record updates the
  derived value without re-reading the collection — and recomputed where it cannot, as a performance
  decision with no semantic difference.
- ⚠️ A derived value's definition is pure. It reads state and calls pure functions, and a `derived` with any
  effect is a compile error.
- ⚠️ A derived value may be defined over another derived value, and a cyclic definition is a compile error
  naming the cycle.
- ⚠️ A derived value over persistent state is itself persistent, and its maintenance participates in the
  same transaction as the write that invalidated it, so a crash cannot leave it stale
  → [Data and schemas](data-and-schemas.md).
- ⚠️ Whether a derived value is stored, cached or recomputed on each read is a physical decision the
  optimiser makes from measurement, not a keyword
  → [`../toolchain/runtime-generations.md`](../toolchain/runtime-generations.md).

## `rule`

- ⚠️ `rule` declares a named predicate over data, holding exactly when its condition does:

  ```y
  rule high_risk_customer(customer) =
      (customer.failed_payments > 3) and (customer.outstanding_balance > 100_000kr)
  ```

- ⚠️ A rule is usable as a predicate in a pipeline, in a contract, and in another rule.
- ⚠️ A rule's condition is pure, and a rule is evaluated by whichever strategy the runtime chooses —
  per-element, as an index lookup, or maintained as a set.
- ⚠️ Rules compose: one rule may be defined in terms of others, and the set of customers satisfying a rule
  is queryable rather than only testable one at a time.
- ⚠️⛔ **Whether rules may be recursive** decides what the declarative subset actually is. Recursion makes it
  a fixpoint language with the evaluation, termination and stratification obligations that implies;
  forbidding it keeps rules as named predicates and reusable conditions. The working design says the goal
  is not to turn the language into Datalog, and lists the declarative facilities as open without settling
  which side of that line they fall on. ⛔ **Unblocked by** the language owner. This is the widest open
  question in the language by implementation cost: a non-recursive rule is a macro over a predicate, and a
  recursive one is a second evaluator.

## What the compiler rejects

| Program | Diagnostic | Status |
| --- | --- | --- |
| A chain that would need an implicit clone to satisfy ownership | The conflict, and where the receiver's mode decides it | ⚠️ |
| A `derived` or `rule` with an effect | The effect | ⚠️ |
| A cyclic `derived` or `rule` definition | The cycle | ⚠️ |
| A mutating stage on a borrowed receiver | That mutation requires a mutable borrow | ⚠️ |

## What this deliberately does not do

- **Y is not a relational language and does not aim to be Datalog.** The declarative subset exists for
  computations that are naturally relational or incremental, not as the primary way to write a program.
- **No query strings.** A query is Y code, type-checked against declared shapes, not text assembled at
  runtime.
- **No user-visible index management.** An index is a physical decision; a program that overrides it is
  overriding an optimiser, and doing so is deliberate and visible rather than routine
  → [`../toolchain/runtime-generations.md`](../toolchain/runtime-generations.md).

## Where it lives

- Ownership of elements, and why the receiver's mode matters
  → [Ownership and resources](ownership-and-resources.md).
- Reading collections inside a transaction → [Shared state](shared-state.md).
- Persistent collections, their identity and their evolution → [Data and schemas](data-and-schemas.md).
- Choosing a layout, an index or a vectorisation for any of this
  → [`../toolchain/runtime-generations.md`](../toolchain/runtime-generations.md).
