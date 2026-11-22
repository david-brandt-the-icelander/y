# The Y language

> Status marks: [knowledge-driven development](https://github.com/david-brandt-the-icelander/knowledge-driven-development)

**Written for:** whoever writes Y — the agent that emits it, and the human who reviews what it emitted.

These nine documents describe what a Y program can say. Each one is written as Y ought to be, in the
present tense, with every statement marked for whether it is true yet. None of them is true yet
→ [`../README.md` § Nothing here is built](../README.md#nothing-here-is-built).

## Syntax here is illustrative

**Every example in these documents is spelled provisionally, and no mark refers to a spelling.** The
current design freezes fifteen semantic commitments and explicitly leaves the surface syntax open
→ [working design § 31](../../Y-current-language-design.md#31-strongly-favored-but-not-yet-fully-frozen).
So a statement about behaviour whose token is unchosen is a gap, not a blocked gap: an implementer needs
a decision about the token, not about the meaning, and the meaning is what the paragraph specifies.

This is said once, here, so that no other page in the corpus has to repeat it and no reader has to
wonder whether `within 5s` is a commitment. It is not. The bound is.

The two statements that *are* about syntax, and are blocked:

- ⚠️ **Y has a grammar** — a lexical structure, a reserved-word list, an expression precedence table,
  and a rule for how blocks are delimited. The language owner has decided the block rule: a colon and
  indentation, as in Python, with `--` and `---` comments, `lower_snake_case` identifiers, fields written
  `name is type`, `conditional` choices with named conditions, and `public` written only on a facet
  → [`../../proposal/Y-proposal.md` § 3.1](../../proposal/Y-proposal.md#31-source-form). The rest of the
  grammar is still to be written.
- ⚠️ **Y has a macro or metaprogramming model**, or a decision that it has none.
  ⛔ **Unblocked by** the language owner. It is listed as open in § 31 with no stated leaning, and it
  constrains the parser, the HIR and what an agent is allowed to generate, so it cannot be deferred past
  the grammar.

## Which document owns what

A capability is described once and referenced from everywhere else it appears. Where two documents could
hold a fact, the more specific one holds it.

| Document | Owns |
| --- | --- |
| [Types and typed failure](types-and-failures.md) | The shape of data and the shape of failure |
| [Ownership and resources](ownership-and-resources.md) | Who holds a value, when it dies, what may alias it |
| [Effects and authority](effects-and-authority.md) | What a function is permitted to do, and how it got permission |
| [Shared state](shared-state.md) | Transactional memory, and the boundary transactions must not cross |
| [External effects](external-effects.md) | Everything that crosses that boundary, durably |
| [Concurrency and time](concurrency-and-time.md) | Suspension, deadlines, task lifetime, failure isolation |
| [Specification in types](specification-in-types.md) | Contracts, protocol state, units |
| [Data and schemas](data-and-schemas.md) | Persistent shape and its evolution |
| [Queries and collections](queries-and-collections.md) | Bulk and derived computation |

The boundary that causes the most confusion is between the last two and the runtime: a `schema`, a
migration and a `converge` declaration are language, and the convergent copying that makes `converge`
finish is runtime → [`../toolchain/runtime-generations.md`](../toolchain/runtime-generations.md).

## What Y deliberately does not do

Non-goals, unmarked. A non-goal is not a gap, and marking one invites a gap mark that will never close.

- **Y is not a natural-language programming system.** English is not a source language, and no part of
  compilation consults a model.
- **AI is not in the trusted semantic core.** An agent may generate, refactor, optimise, synthesise
  contracts, propose backends and minimise failures. It may not redefine semantics, waive a failed
  check, reinterpret unsafe code, or make correctness depend on model output.
- **Y is not a Rust dialect and does not transpile to Rust as its definition.** Rust is a reference
  backend and an ownership oracle → [`../toolchain/oracles.md`](../toolchain/oracles.md).
- **No exceptions.** Failure is a typed value in a signature
  → [Types and typed failure](types-and-failures.md).
- **No inheritance, and no object model with implicit dispatch through a class hierarchy.**
- **No reflection, and no framework that discovers behaviour at runtime.** If the compiler cannot see
  it, it is not a Y semantic.
- **No ambient authority.** A function reaches the filesystem, the network or a secret because authority
  was passed to it → [Effects and authority](effects-and-authority.md).
- **No undefined behaviour in safe Y**, and no implicit global mutable state.
- **No garbage collection as the default memory strategy.** Whether a bounded set of types opts into
  collection at all is open → [Ownership and resources](ownership-and-resources.md).
- **No source-level `async`/`await` colouring.** Suspension is a semantic property, not a signature
  you have to propagate → [Effects and authority](effects-and-authority.md).
- **No syntactic novelty for its own sake.** Y spends its budget on semantic leverage; where a familiar
  form works, Y uses the familiar form.
