# Ownership and resources

> Status marks: [`../development/knowledge-driven-development/README.md`](../development/knowledge-driven-development/README.md)

**Written for:** whoever writes Y.

## What this is for

Y keeps what Rust's ownership model is for — nothing changes behind a reader's back, aliasing never
permits a data race, and a value's lifetime is a fact rather than a convention — without asking the
programmer to spell out references, borrows or moves. A Y program reads as if every value were
garbage-collected, and the compiler works out ownership from it
→ [`../../proposal/Y-proposal.md` § 3.6](../../proposal/Y-proposal.md#36-values-and-ownership).

Two things make this matter more for generated code than for hand-written code. An agent writing Rust
spends most of its failed attempts on borrows and lifetimes, and the annotations it writes to get past
them tell a reviewer about memory, not about intent.

## Values

- ⚠️ Y source has no references, no borrows, no lifetimes and no moves. A value is passed, stored and
  returned as freely as in a garbage-collected language.
- ⚠️ **Values have value semantics.** A value never changes under someone who holds it: keeping a value
  keeps it as it is, and changing a copy changes only that copy.
- ⚠️ **Parameters are immutable by default.** A parameter `x` is read and never changed. `mutable x` lets the
  callee change the caller's value, and the call says so too, `update(mutable order)`. While the call
  runs, nothing else can reach that value.

  ```y
  -- order: The order to look at, as order.
  function inspect(order) { ... }

  -- order: The order to change, as order.
  function update(mutable order) { ... }
  ```

- ⚠️ A `let` binding may not change, and a `variable` may. Method receivers are declared `self` or
  `mutable self`, and are not marked at the call.
- ⚠️ **The compiler chooses the representation.** A parameter the callee only reads is passed as a
  shared borrow; a value the callee keeps is moved when the caller no longer uses it and copied when it
  does; a value that would be copied often may be counted and copied on write; a `mutable` parameter is
  an exclusive borrow. `y explain` lists every copy the compiler made and why.
- ⚠️ Borrows exist only in the generated code, where an independent checker verifies them
  → [`../toolchain/oracles.md`](../toolchain/oracles.md).

## Lifetime and cleanup

- ⚠️ A value is destroyed at the end of the scope that owns it, deterministically and in reverse order
  of creation. There is no collector and no finalisation queue.
- ⚠️ A type may declare cleanup that runs on destruction, and that cleanup runs exactly once, on the
  owner, whether the scope exits normally or through a failure.
- ⚠️ Cleanup that can itself fail declares a failure type, and a scope whose cleanup fails propagates
  that failure rather than discarding it.
- ⚠️ **A type with cleanup is a resource, and a resource is never copied.** Giving it to a callee that
  keeps it ends the caller's use of it, and a later use is a compile error naming where it was given
  away. It is cleaned up once, at the end of the scope of its last holder.
- ⚠️ Values share freely without a type that opts out of single ownership: there are no references to
  share. Mutable state between tasks is `shared<t>` → [Shared state](shared-state.md), and a graph is a
  collection or arena of nodes that refer to each other by typed index.

## Arenas

- ⚠️ An arena owns a set of values with one lifetime and destroys them together, so that a computation
  which allocates heavily does not pay for individual destruction.
- ⚠️ A value allocated in an arena may not outlive the arena, and may not escape the arena's scope.
- ⚠️ **An arena may own an external resource, and owning it does not make it transactional.** A GPU
  buffer, a socket and a file handle can all have their destruction managed by an arena while remaining
  operations that must not execute speculatively. The two questions are separate and answered by
  different parts of the language: the arena answers *when is this destroyed*, and the effect system
  answers *when may this execute* → [Effects and authority](effects-and-authority.md).
- ⚠️ A computation over private arena memory with no externally visible effect is repeatable, and may
  therefore run inside a transaction that might be retried — including native compute whose inputs and
  outputs are both arena-private. Retrying discards the arena result and recomputes it, which is
  permitted and may be expensive → [Shared state](shared-state.md).

## What the compiler rejects

| Program | Diagnostic | Status |
| --- | --- | --- |
| A change to a parameter that is not `mutable` | That the parameter is immutable, and how to declare it `mutable` | ⚠️ |
| A call that passes a `mutable` parameter without `mutable` at the call | The parameter, and the call to mark | ⚠️ |
| The same value passed `mutable` twice in one call, or `mutable` and read | The two arguments | ⚠️ |
| Use of a resource after it was given away | Where it was given away | ⚠️ |
| An arena-allocated value escaping its arena | The arena's scope | ⚠️ |
| A speculative context performing a non-repeatable operation on an arena-owned external resource | That arena ownership is not transactional | ⚠️ |

## Where it lives

- Whether an operation may execute at all → [Effects and authority](effects-and-authority.md).
- Why repeatability matters inside a transaction → [Shared state](shared-state.md).
- How ownership survives into the semantic layer, and what a backend receives
  → [`../toolchain/core-ir.md`](../toolchain/core-ir.md).
- How Y's ownership conclusions get checked against an independent implementation
  → [`../toolchain/oracles.md`](../toolchain/oracles.md).
- Which physical layout a collection of owned values actually gets
  → [`../toolchain/runtime-generations.md`](../toolchain/runtime-generations.md).
