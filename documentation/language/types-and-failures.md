# Types and typed failure

> Status marks: [`../development/knowledge-driven-development/README.md`](../development/knowledge-driven-development/README.md)

**Written for:** whoever writes Y.

## What this is for

Most of what a reviewer needs to know about a function is in its signature, or it is nowhere. Y puts the
data shape, the absence case and the failure set all in the signature as ordinary types, so that
"what can come back from this call" is a question the compiler answers rather than one the reader guesses
from a docstring.

The consequence that matters for generated code: an agent cannot forget to handle a case it was never
told about, because the compiler enumerates the cases.

## Data

- ⚠️ `structure` declares a product type with named, typed, ordered fields.

  ```y
  structure customer:
      id: customer_id
      name: string
  ```

- ⚠️ A structure with few fields may be declared positionally on one line, and the two forms are the same
  type: `structure point(x: float, y: float)`.
- ⚠️ `type` declares a closed sum type. Each case names itself and carries zero or more payload types.

  ```y
  type payment_result:
      case approved(authorization is authorization)
      case declined(reason is decline_reason)
      case unavailable
  ```

- ⚠️ A case is always written with its type, `payment_result.approved(...)`, in an expression, a
  pattern, a `return` and a `fail` alike. Y has no `.name` shorthand, so a reader never works out which
  type a case belongs to.
- ⚠️ A sum type is closed: its cases are fixed at declaration, and no other module adds one. Exhaustive
  matching depends on this and Y does not offer an open variant.
- ⚠️ `void` is the type of an expression that produces no value, and is an ordinary type: it may be
  written as a result's type; a function that produces nothing on success has no `returns`, and may still `fail`.

## Absence

- ⚠️ `optional<t>` is a value that is either a `t` or absent. It is a distinct type from `t`, there is no
  null, and no `t` is implicitly an absent `optional<t>` or the reverse.
- ⚠️ Absence is handled by a named decision, never by an operator: Y has no `??` default and no `?.`
  reach-through. An `if` with a named `else`, or a `conditional` over an `optional<t>`, which is exhaustive
  in exactly the way it is over any sum type, gives each way through a name for tests and traces.

  ```y
  conditional looked_up = customer:
      condition looked_up.present = (looked_up == optional.some):
          use(looked_up.value)
      condition looked_up.absent = (looked_up == optional.none):
          not_found()
  ```


## Failure

- ⚠️ A function that produces a `customer` or fails with a `customer_error` says so in words, after the
  name of its result; the result's type is in the documentation above it:

  ```y
  function load_customer(id) returns customer fails customer_error
  ```

- ⚠️ A failure is an ordinary typed value. There is no unwinding, no side channel, no ambient handler,
  and no way for a failure to leave a function whose signature does not name it.
- ⚠️ `or fail` passes a failure on: after a call that can fail with `e`, inside a function that also
  fails with `e`, it yields the call's result on success and fails with the failure unchanged otherwise.
- ⚠️ Where the enclosing function's failure type is a sum type with exactly one case whose payload is
  the inner failure type, `or fail` converts through that case, and the conversion is derived rather than
  written. `or fail as startup_error.database` names the case where the author wants it seen.

  ```y
  type startup_error:
      case configuration(error is configuration_error)
      case database(error is database_error)
      case network(error is network_error)

  function startup() returns started fails startup_error:
      configuration = load_configuration("app.json") or fail
      database = connect(configuration.database) or fail
      return started = application(configuration, database)
  ```

- ⚠️ Where two cases could carry the same inner failure type, the conversion is ambiguous and the
  program is rejected, naming both candidate cases. The compiler does not pick one.
- ⚠️ A failure is narrowed by matching it, and a function may convert a failure set into a smaller one
  by handling the cases it can.
- ⚠️ A call that can fail has a two-case sum as its outcome, `result.ok(value)` or `result.err(failure)`, so a
  single `conditional` ranges over a call's successes and failures together, and an optional result composes
  with failure without a special rule
  → [`../../proposal/Y-proposal.md` § 3.4](../../proposal/Y-proposal.md#34-failure).

## Choosing

- ⚠️ A choice over one value is a `conditional`: `conditional loaded = load_customer(id):` evaluates the
  value once and names it, and each branch is `condition loaded.known = (loaded == result.ok):`, a named
  boolean expression over that label. There is no pattern syntax and no guard clause
  → [`../../proposal/Y-proposal.md` § 3.5](../../proposal/Y-proposal.md#35-every-decision-has-a-name).
- ⚠️ A case written without its payload, `result.ok`, stands for every value of that case, and a condition
  that compares with it narrows the label in its branch: `loaded.value`, `loaded.failure.id`.
- ⚠️ When every condition compares the label with a case of a closed type, the conditional must cover
  every case, and adding a case to a `type` turns every conditional over it into an error at the sites
  that need updating. Otherwise a named `else` is required.
- ⚠️ Making the same call more than once in one conditional is a critical lint error, suppressed only by an
  inline linter directive on that line.
- ⚠️ Conditions are tried in order, and one that cannot be reached because an earlier one covers it is a
  compile error rather than dead code.

## What the compiler rejects

| Program | Diagnostic | Status |
| --- | --- | --- |
| A `conditional` over a closed type that misses a case, with no `else` | The missing cases, by name | ⚠️ |
| A condition covered by an earlier one | Which condition covers it | ⚠️ |
| The same call made twice in one conditional | Both sites; critical, suppressible only inline | ⚠️ |
| `or fail` where the enclosing signature names no compatible failure | The failure type produced and the failure type permitted | ⚠️ |
| `or fail` where two cases of the enclosing failure type could wrap the inner one | Both candidates | ⚠️ |
| A case written without its type | The type to write before it | ⚠️ |
| A `t` used where `optional<t>` is expected, or the reverse | That absence is a distinct type | ⚠️ |

## Where it lives

- Physical units are types too, and are a different concern
  → [Specification in types](specification-in-types.md).
- Persistent shapes and their versions → [Data and schemas](data-and-schemas.md).
- How much of this the semantic layer keeps and how much it erases
  → [`../toolchain/core-ir.md`](../toolchain/core-ir.md).
- Whether a value is copied, moved or borrowed when matched
  → [Ownership and resources](ownership-and-resources.md).
