# Ownership and resources

> Status marks: [`../development/knowledge-driven-development/README.md`](../development/knowledge-driven-development/README.md)

**Written for:** whoever writes Y.

## What this is for

Y takes the useful half of Rust's ownership model — one owner, aliasing that excludes mutation,
deterministic destruction — and declines to make the programmer spell out the parts a compiler can work
out. The goal is not to resemble Rust. It is that a program which aliases unsafely does not compile, and
that a value's lifetime is a fact rather than a convention.

Two things make this matter more for generated code than for hand-written code. An agent will happily
produce a plausible aliasing bug, and an agent will happily produce a hidden deep copy to get past a
borrow error. Y rejects the first and refuses to perform the second.

## Ownership

- ⚠️ Every value has exactly one owner at a time. Assignment and argument passing transfer ownership,
  and using a value after it has been transferred is a compile error naming the transfer site.
- ⚠️ `&T` is a shared borrow: any number may exist at once, and none of them permits mutation.
- ⚠️ `&mutable T` is an exclusive borrow: while one exists, no other borrow of that value exists, and it
  permits mutation.

  ```y
  function inspect(order: &Order) { ... }

  function update(order: &mutable Order) { ... }
  ```

- ⚠️ A borrow may not outlive the value it borrows, and a function may return a borrow derived from a
  borrowed parameter.
- ⚠️ `move` states a transfer that would happen anyway, for a reader who wants it visible:
  `let transferred = move value`. It changes nothing about what the program means.
- ⚠️ **The compiler never inserts a clone to make a program type-check.** Where a program needs a copy,
  the copy is written, and a program that would only work with a hidden copy is rejected with the
  ownership conflict that caused it. Silent deep copies are the mechanism by which a pleasant surface
  syntax turns into a program with surprising cost, and Y does not have them.
- ⚠️ Types that are cheap and safe to duplicate bit-for-bit are copied rather than moved, and the set of
  such types is closed and part of a type's declaration rather than inferred from its size.
- ⚠️⛔ **Ownership is inferred as far as inference stays predictable, and annotations are written where
  it does not.** The compiler may infer substantially more than Rust does — how much more, and what the
  rule is for when a signature must say `&` out loud, is undecided; § 31 lists the degree of ownership
  inference as open. ⛔ **Unblocked by** the language owner. It decides whether the borrow markers in
  every example in this corpus are mandatory or merely permitted, so it is a precondition for writing
  the first real Y program rather than a later refinement.

## Lifetime and cleanup

- ⚠️ A value is destroyed at the end of the scope that owns it, deterministically and in reverse order
  of creation. There is no collector and no finalisation queue.
- ⚠️ A type may declare cleanup that runs on destruction, and that cleanup runs exactly once, on the
  owner, whether the scope exits normally or through a failure.
- ⚠️ Cleanup that can itself fail declares a failure type, and a scope whose cleanup fails propagates
  that failure rather than discarding it.
- ⚠️⛔ **A bounded set of types opts out of single ownership** where a graph genuinely needs shared
  lifetime, through reference counting or a region discipline rather than a tracing collector. Which
  types, whether the choice is per-type or per-allocation, and whether cycles are rejected or leaked is
  undecided; § 31 lists garbage collection versus region and ownership strategies as open.
  ⛔ **Unblocked by** the language owner.

## Arenas

- ⚠️ An arena owns a set of values with one lifetime and destroys them together, so that a computation
  which allocates heavily does not pay for individual destruction.
- ⚠️ A value allocated in an arena may not outlive the arena, and a borrow of it may not escape the
  arena's scope.
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
| Use of a value after ownership transferred | Where it was transferred | ⚠️ |
| `&mutable` alongside any other live borrow | The conflicting borrow and its extent | ⚠️ |
| Mutation through a shared borrow | That `&` excludes mutation | ⚠️ |
| A borrow outliving its referent | Where the referent dies | ⚠️ |
| A program that would need an implicit clone | The ownership conflict, not a suggestion to copy | ⚠️ |
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
