# Data and schemas

> Status marks: [`../development/knowledge-driven-development/README.md`](../development/knowledge-driven-development/README.md)

**Written for:** whoever writes Y.

## What this is for

A record that outlives the process it was written by has a second type — the one on disk — and in most
languages that second type is maintained in a different file, in a different language, by a different
tool, and checked by nothing. Y makes the persistent shape a declaration the compiler understands, so that
"is this change additive" and "can version 3 read version 2" are questions with mechanical answers.

The reason this belongs in the language rather than a library is the same reason durable commands do
→ [External effects](external-effects.md): the hand-built version is rebuilt on every project, and its
failure mode is a deploy that cannot be rolled back.

## Schemas

- ⚠️ `schema` declares a persistent shape with a version:

  ```y
  schema customer v3:
      id: customer_id
      name: string
      email: email?
  ```

- ⚠️ Every version of a schema is retained in the source, so the compiler can compare them rather than
  inferring intent from a diff.
- ⚠️ The compiler classifies the difference between consecutive versions as additive or breaking, and says
  which field made it breaking.
- ⚠️ Serialisation codecs are generated from the declaration, and a schema type's wire representation is
  not hand-written.
- ⚠️ A field may only be persisted if its type is durably representable, and a schema containing a borrow,
  a capability, a task handle or an arena-allocated value is a compile error
  → [Ownership and resources](ownership-and-resources.md).
- ⚠️ A schema declares its identity — which field or fields address an instance — because migration,
  dependency tracking and deduplication all need to name one.
- ⚠️ A schema's invariants are declared with it and checked on read as well as write, so data that
  violates them is detected where it enters the program rather than where it later breaks something
  → [Specification in types](specification-in-types.md).
- ⚠️ A persisted state machine's legal states are the `state` declaration's states, and stored data in a
  state the current declaration does not list is a detected error rather than an unmatched value
  → [Specification in types § Typestate](specification-in-types.md#typestate).
- ⚠️ A database projection is derived from the schema — tables, columns and indexes follow the
  declaration, and the schema is not a description of a table maintained elsewhere.
- ⚠️⛔ **Which storage engines a schema projects onto, and how much of a projection is declared rather than
  chosen** — whether a schema names a store, whether one schema may be projected into several, and what a
  program says when it needs a representation the projection would not pick. Physical representation is
  meant to be the optimiser's decision → [`../toolchain/runtime-generations.md`](../toolchain/runtime-generations.md),
  which makes the boundary between a declared projection and a chosen one the thing to settle.
  ⛔ **Unblocked by** the language owner.

## Migrations

- ⚠️ `migrate` declares how one schema version becomes the next, as an expression per field of the target:

  ```y
  migrate customer v1 to v2:
      display_name = first_name + " " + last_name
      normalized_name = normalize(display_name)
  ```

- ⚠️ A migration declares what it reads and what it writes, and may read schemas other than the one it is
  migrating:

  ```y
  migrate customer v1 to v2
      reads customer_v1
      reads country_rules
      writes customer_v2
  ```

- ⚠️ A migration is deterministic, pure with respect to external systems, and idempotent. It reads only
  versioned persistent state, and the compiler rejects a migration that reads the network, the filesystem,
  the clock or randomness, because each of those makes re-running the migration produce a different answer.
- ⚠️ A migration is therefore replayable and re-runnable per instance, which is what permits it to be
  retried at the granularity of a single record rather than restarted wholesale.
- ⚠️ An additive version change with a total default needs no migration, and the compiler says so rather
  than requiring an empty declaration.
- ⚠️ A breaking change with no migration declared is a compile error naming the fields that need one.
- ⚠️ A migration's failure on a particular record is recorded against that record's identity, and the
  migration continues — a single unmigratable record does not abort the rest.
- ⚠️ A migration is reversible where a reverse expression is declared, and the compiler reports whether a
  version pair can be rolled back, because a one-way migration is a deployment that cannot be undone and
  a programmer should know which they have written.

## `converge`

`converge` is the declaration; the machinery that makes it terminate is runtime
→ [`../toolchain/runtime-generations.md`](../toolchain/runtime-generations.md).

- ⚠️ `converge` moves live persistent state from one representation to another while the program continues
  to read and write it, and completes with a cutover whose cost is proportional to the remaining
  difference rather than to the size of the data.
- ⚠️ A `converge` declaration names a source, a target, and the migration that relates them. The runtime
  owns the rest: snapshotting, copying, observing changes to the source, invalidating only the affected
  target fragments, recomputing them, and repeating until the difference is small enough to cut over.
- ⚠️ Invalidation is per record and per dependency. A migration's read set is recorded, and a change to
  something it read makes exactly that record's output stale — a change to an unrelated record makes
  nothing stale.
- ⚠️ Dependencies between schemas follow declared references, so a change to a customer invalidates the
  migrations of the orders that reference it and the invoices derived from those orders, and nothing else.
  This is the payoff for references being declared relationships rather than arbitrary pointers.
- ⚠️ A `converge` may run indefinitely: a representation change may migrate hot records eagerly and cold
  records on access, with reads understanding both representations, until no record remains in the old one
  and support for it is removed.
- ⚠️ A `converge` is observable — how much remains, what is failing, how far behind the target is — because
  a migration that will not converge needs to be diagnosable before its cutover window is chosen.
- ⚠️ A `converge` is abandonable up to the cutover, leaving the source untouched.
- ⚠️⛔ **How a `converge` is triggered, and who decides when to cut over** — whether it is a statement a
  program executes, a declaration the deployment applies, an operator action, or something the optimiser
  initiates on its own → [`../toolchain/runtime-generations.md`](../toolchain/runtime-generations.md).
  ⛔ **Unblocked by** the language owner. The mechanism is described in the transcript and the trigger is
  not, and it is the difference between a language feature and an operations tool.

## What the compiler rejects

| Program | Diagnostic | Status |
| --- | --- | --- |
| A schema field whose type is not durably representable | The field and why | ⚠️ |
| A breaking version change with no migration | The fields that need one | ⚠️ |
| A migration reading the clock, randomness or an external system | The read, and that migrations must be replayable | ⚠️ |
| A migration whose target fields are not all produced | The unset fields | ⚠️ |
| A schema version that is edited rather than superseded | That prior versions are retained | ⚠️ |
| A `converge` whose migration does not relate the named source and target | The mismatch | ⚠️ |

## What this deliberately does not do

- **Y does not generate a migration from a schema diff.** It reports what changed and what must be
  written; the expression that fills a new field is a design decision.
- **Y does not own the database.** A schema projects onto storage; it does not require Y to be the only
  writer, and it does not make an external system's schema Y's business.
- **A schema is not a record.** An in-memory `record` has no version, no codec and no migration
  → [Types and typed failure](types-and-failures.md).

## Where it lives

- In-memory shapes → [Types and typed failure](types-and-failures.md).
- Invariants and persisted protocol state → [Specification in types](specification-in-types.md).
- Transactional access to persistent state → [Shared state](shared-state.md).
- Convergent copying, cutover, physical representation changes and system generations
  → [`../toolchain/runtime-generations.md`](../toolchain/runtime-generations.md).
- Derived and incrementally maintained values over persistent data
  → [Queries and collections](queries-and-collections.md).
