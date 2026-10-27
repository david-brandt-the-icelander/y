# Effects and authority

> Status marks: [`../development/knowledge-driven-development/README.md`](../development/knowledge-driven-development/README.md)

**Written for:** whoever writes Y.

## What this is for

Two questions a reviewer of generated code keeps having to ask — *what can this function actually do*,
and *how did it get permission* — are the same question asked about capability and about authority. Y
answers both mechanically. The effect system computes what a function may do; capabilities decide what it
was allowed to be given.

The design constraint is that the answers must be cheap to obtain and nearly free to write. An effect
system that requires every signature to carry a list of effects is one an agent will get wrong and a
human will stop reading, so Y infers effects and prints them on request.

## Effects

- ⚠️ Every function has an effect set, computed by the compiler from its body and the effect sets of
  everything it calls. The set is part of the function's type for purposes of checking, whether or not
  it is written in the source.
- ⚠️ The effects Y distinguishes are: local mutation; shared-state mutation; suspension; filesystem
  access; network access; database access; reading time; randomness; durable command submission; process
  spawning; and unsafe or native operations.
- ⚠️ A function's effect set may be written explicitly in a signature, and where it is, the compiler
  checks the written set against the inferred one and rejects a signature that claims fewer effects than
  the body has. A signature claiming more than the body needs is accepted, because widening is how an
  interface reserves room.
- ⚠️ An interface, a contract or a published API declares its effects explicitly, so that an
  implementation cannot quietly acquire an effect its callers were not told about.
- ⚠️ The compiler answers, for any function, which effects it has and which call path introduced each
  one — the diagnostic names the path, not just the effect, because "this function can touch the network"
  is not actionable without knowing where.
- ⚠️ An effect boundary can be declared to exclude effects: a region of code may be required to have an
  effect set within a stated bound, and a call that would exceed it is a compile error at the call site.
- ⚠️ A pure function — empty effect set — is recognised as such without annotation, and purity is what
  licenses caching, reordering and replay of a computation elsewhere in the toolchain.
- ⚠️⛔ **Explicit effect annotations have a syntax**, and whether effects are named individually, grouped
  into named sets, or arranged in a lattice with subsumption decides what an interface author writes and
  how precisely an effect bound can be stated. § 31 lists effect annotation syntax as open.
  ⛔ **Unblocked by** the language owner.

## Suspension is an effect, not a colour

- ⚠️ A function that may suspend is not marked in its own signature, and its callers are not marked
  either. Suspension is an inferred effect like any other, visible to the compiler and to anyone who asks
  for a function's effect set.
- ⚠️ There is no `await`. A call to a suspending function is written like any other call, and the
  compiler transforms the caller into whatever continuation or state-machine representation the backend
  needs.

  ```y
  response = http.get(url) within 2s
  ```

- ⚠️ A suspension point is nevertheless a semantic fact the compiler will name on request, because a
  reviewer asking "where can this yield" needs a list and not a promise.
- ⚠️ Every suspension is bounded, which is the one thing removing the colour does not remove
  → [Concurrency and time](concurrency-and-time.md).

Removing `async` from signatures is not a claim that suspension is invisible. It is a claim that
*propagating* it through signatures is clerical work with no reader, and that the compiler is a better
place to keep it than a thousand call sites.

## Authority

- ⚠️ A function has no ambient access to the process's resources. The filesystem, the network, the clock,
  randomness, a database, a secret store and the ability to submit a durable command are all reached
  through a capability value that was passed in.
- ⚠️ A capability is an ordinary value with an ordinary type, subject to ownership and borrowing, so the
  compiler already knows which functions hold which authority and a reviewer can read it off the
  signatures.
- ⚠️ A capability is attenuable: a capability over a filesystem subtree yields a capability over a
  subtree of that subtree, and no operation on a derived capability reaches outside it.
- ⚠️ Capabilities name the authority they carry at the granularity the resource has — a filesystem
  subtree, a class of durable command, one database, one network service, one secret, one shared
  resource — rather than a single blanket permission per resource kind.
- ⚠️ The program's entry point receives the full set of capabilities the process was started with, and
  every other capability in the program is derived from those by attenuation or by being passed along.
- ⚠️ A capability cannot be forged, obtained from a global, or recovered from a type — there is no
  operation that produces one from nothing.
- ⚠️ The compiler answers, for any function, the complete set of authority reachable from it, which is
  the question a security review asks and the one an agent's output most needs answered.
- ⚠️⛔ **Capabilities have a declaration and passing syntax**, and whether they are ordinary parameters,
  an implicit context threaded by the compiler, or a declared requirement satisfied at the call site
  decides whether adding one authority to a deep function edits every frame in between. § 31 lists
  capability syntax as open. ⛔ **Unblocked by** the language owner. The plumbing cost of the wrong answer
  here is what has killed capability systems in other languages, so it deserves more care than its
  position on a list of open items suggests.

## What the compiler rejects

| Program | Diagnostic | Status |
| --- | --- | --- |
| A signature declaring fewer effects than its body has | The effect, and the call path that introduced it | ⚠️ |
| A call exceeding a declared effect bound | The bound, the offending effect, the path | ⚠️ |
| An external effect inside a transaction | → [Shared state](shared-state.md) | ⚠️ |
| Reaching a resource without holding a capability for it | Which authority is missing | ⚠️ |
| Using a derived capability outside what it was attenuated to | The derivation and its bound | ⚠️ |
| An unbounded suspension | → [Concurrency and time](concurrency-and-time.md) | ⚠️ |

## Where it lives

- The rule that transactions perform no external effect, and what happens instead
  → [Shared state](shared-state.md), [External effects](external-effects.md).
- Bounding suspension → [Concurrency and time](concurrency-and-time.md).
- Whether a resource's *lifetime* is arena-managed, which is a different question from whether its
  operations may execute → [Ownership and resources](ownership-and-resources.md).
- How an effect set is represented once the source is gone
  → [`../toolchain/core-ir.md`](../toolchain/core-ir.md).
- Why a function's effect set is also its hot-swap compatibility signature
  → [`../toolchain/runtime-generations.md`](../toolchain/runtime-generations.md).
