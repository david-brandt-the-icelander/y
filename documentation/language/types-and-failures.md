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

- ⚠️ `record` declares a product type with named, typed, ordered fields.

  ```y
  record Customer {
      id: CustomerId
      name: String
  }
  ```

- ⚠️ A record with few fields may be declared positionally on one line, and the two forms are the same
  type: `record Point(x: Float, y: Float)`.
- ⚠️ `type` declares a closed sum type. Each case names itself and carries zero or more payload types.

  ```y
  type PaymentResult {
      .Approved(Authorization)
      .Declined(DeclineReason)
      .Unavailable
  }
  ```

- ⚠️ A case is written `.Name`, and the type it belongs to is taken from the context the expression
  appears in — the declared type of the binding, the parameter it is passed to, or the return type of the
  enclosing function. Where context does not determine a unique type, the case is written with its type
  qualified, and an ambiguous case with no qualification is a compile error rather than a guess.
- ⚠️ A sum type is closed: its cases are fixed at declaration, and no other module adds one. Exhaustive
  matching depends on this and Y does not offer an open variant.
- ⚠️ `Void` is the type of an expression that produces no value, and is an ordinary type: it may be
  written as a return type, and a `T ! E` function that produces nothing on success is `Void ! E`.

## Absence

- ⚠️ `T?` is a value that is either a `T` or absent. It is a distinct type from `T`, there is no null,
  and no `T` is implicitly an absent `T?` or the reverse.
- ⚠️ `??` supplies a value for the absent case: `user.nickname ?? user.name` has type `String` where
  `nickname` is `String?`.
- ⚠️ Absence is matched like any other case where the two branches do more than substitute a value, and
  `match` over a `T?` is exhaustive in exactly the way it is over any sum type.

  ```y
  match customer {
      .Some(c) => use(c)
      .None    => notFound()
  }
  ```

- ⚠️ `?.` reaches through absence: where `a` is `A?`, `a?.field` is the field's type made optional, and
  the chain produces absence without evaluating the rest.

## Failure

- ⚠️ `T ! E` is the type of a computation that produces a `T` or fails with an `E`. It appears in a
  signature exactly where a return type appears:

  ```y
  function loadCustomer(id: CustomerId) -> Customer ! CustomerError
  ```

- ⚠️ A failure is an ordinary typed value. There is no unwinding, no side channel, no ambient handler,
  and no way for a failure to leave a function whose signature does not name it.
- ⚠️ `?` propagates: applied to a `T ! E` expression inside a function returning `U ! E`, it yields the
  `T` on success and returns the failure unchanged otherwise.
- ⚠️ Where the enclosing function's failure type is a sum type with exactly one case whose payload is
  the inner failure type, `?` converts through that case, and the conversion is derived rather than
  written.

  ```y
  type StartupError {
      .Configuration(ConfigurationError)
      .Database(DatabaseError)
      .Network(NetworkError)
  }

  function startup() -> Application ! StartupError {
      let configuration = loadConfiguration("app.json")?
      let database = connect(configuration.database)?
      Application(configuration, database)
  }
  ```

- ⚠️ Where two cases could carry the same inner failure type, the conversion is ambiguous and the
  program is rejected, naming both candidate cases. The compiler does not pick one.
- ⚠️ A failure is narrowed by matching it, and a function may convert a failure set into a smaller one
  by handling the cases it can.
- ⚠️⛔ **`T ! E` is either a distinct semantic channel in Core IR or sugar for a two-case sum type**,
  and which one decides whether a single `match` can range over a call's successes and failures
  together, whether `T ! E` and `T?` compose into one construct or two, and what a backend has to
  lower. ⛔ **Unblocked by** the language owner. Everything above holds under either answer; the
  matching syntax for a failing call does not.

## Matching

- ⚠️ `match` is exhaustive. A match that does not cover every case of the scrutinised type is a compile
  error that names the missing cases, and adding a case to a `type` turns every non-exhaustive match on
  it into an error at the sites that need updating.
- ⚠️ Patterns nest, binding payloads positionally or by field name, and `_` matches without binding.
- ⚠️ A `when` clause guards an arm with a boolean condition. A guarded arm never counts toward
  exhaustiveness, because the compiler does not reason about the guard's truth.
- ⚠️ Arms are tried in order, and an arm that cannot be reached because an earlier arm subsumes it is a
  compile error rather than dead code.

## What the compiler rejects

| Program | Diagnostic | Status |
| --- | --- | --- |
| A `match` missing a case of a closed sum type | The missing cases, by name | ⚠️ |
| An arm subsumed by an earlier arm | Which arm shadows it | ⚠️ |
| Exhaustiveness claimed by a guarded arm alone | That the guard is not evidence, and which case is still uncovered | ⚠️ |
| `?` where the enclosing signature names no compatible failure | The failure type produced and the failure type permitted | ⚠️ |
| `?` where two cases of the enclosing failure type could wrap the inner one | Both candidates | ⚠️ |
| A bare `.Case` whose type context is ambiguous | The candidate types | ⚠️ |
| A `T` used where `T?` is expected, or the reverse | That absence is a distinct type | ⚠️ |

## Where it lives

- Physical units are types too, and are a different concern
  → [Specification in types](specification-in-types.md).
- Persistent shapes and their versions → [Data and schemas](data-and-schemas.md).
- How much of this the semantic layer keeps and how much it erases
  → [`../toolchain/core-ir.md`](../toolchain/core-ir.md).
- Whether a value is copied, moved or borrowed when matched
  → [Ownership and resources](ownership-and-resources.md).
