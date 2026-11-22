# Y

> Status marks: [knowledge-driven development](https://github.com/david-brandt-the-icelander/knowledge-driven-development)

**Written for:** whoever is deciding what Y is, and whoever implements it.

## What Y is

Y is a systems and application programming language arranged around one assumption: that most Y source
will be written by an AI agent, read by a human reviewer, and trusted by neither — the compiler holds
authority over meaning, safety and correctness.

That is not a claim that Y contains a model, understands intent, or accepts English. Y's semantics are
deterministic and implementable, and an agent's output is a proposal the compiler either accepts or
rejects. The division is the whole design:

> AI proposes. Deterministic tooling verifies.

Everything else follows from taking that seriously. If a generated program must be checkable rather
than trusted, then the properties a reviewer would otherwise have to hold in their head — what this
function may mutate, whether it can suspend forever, whether a retry can charge a customer twice, which
errors escape, which state transitions are legal — have to be facts the compiler can compute. Y's job
is to make those facts cheap to compute and expensive to violate, while keeping the source short enough
that a human will actually read the diff.

## What is built, and what the marks mean

A prototype toolchain exists, and it is small next to the design. [`../tools/y-parse`](../tools/y-parse/) parses
Y source. [`../tools/y-compile`](../tools/y-compile/) lowers the Y programs in [`../examples/`](../examples/) to
Rust through [`../tools/bare-rust`](../tools/bare-rust/), and those programs build and pass their tests. It
lowers to Rust only. There is no backend-neutral Core IR as a separate artifact, no Zig or C backend, no oracle
projection, and none of the runtime-generation machinery. The syntax in every example is illustrative.

This corpus is nevertheless written under the annotation convention, and the convention still applies to it: **no
statement anywhere in this corpus claims to be implemented and verified.** What the prototype does is written
up in the tools' own READMEs, not here, and a capability here stays a gap until a reader can check it against a
running toolchain and the corpus marks it so. A reader who finds a statement of verified implementation in this
corpus has found a bug in the corpus, not a working feature.

That is worth stating plainly, because a page with almost no green on it normally means nobody adopted the
convention, and here it means the opposite: the convention is adopted, and the prototype covers too little of
the design to graduate a mark.

It also means the gap marks carry the whole load, and a corpus where every line is a gap discriminates
nothing. That is the graduation rule from the other end — ticks get deleted once they stop carrying
information, and a uniform wall of gaps has the same problem. What actually varies across Y is not
whether a capability is built, but **whether it is decided well enough to build:**

- **A gap mark alone** means the statement is specified. An implementer could start from the paragraph
  without asking much, which is the only bar
  [02](https://github.com/david-brandt-the-icelander/knowledge-driven-development/blob/main/02-annotation-convention.md#what-each-mark-obliges-you-to-write)
  sets for it.
- **A gap followed by a blocked mark** means the semantics are still open. The blocked mark names the
  decision that would unblock it.

Spelling is not a blocker and never earns the blocked mark — the syntax in every example here is
illustrative, stated once in [`language/README.md`](language/README.md) and nowhere else. Only
undecided *meaning* blocks.

Search this corpus for the blocked glyph and you have Y's design agenda, each item sitting in the
document where that decision will bite. That is the one thing this corpus does that
[`../Y-current-language-design.md`](../Y-current-language-design.md) § 31 cannot: a flat list of
sixteen open questions tells you what is unresolved, not which paragraph stops being writable because
of it.

## Where Y is written down

| Record | What it is | Status |
| --- | --- | --- |
| This corpus | Y as a set of documents, each written for one reader, each statement marked | The convention applies here |
| [`../Y-current-language-design.md`](../Y-current-language-design.md) | The working design, 2026-09-17: goals, settled decisions, open questions | Unmarked. Not under the convention |
| `riff.md` | The conversation the design came out of | A transcript, not part of this repository. **Never annotated** — it is a record of a past state, like a changelog entry |

Two things about the transcript, because it is long and it is load-bearing:

- It calls the language **Rill** and frames it as ergonomic notation that transpiles to Rust. The
  current design supersedes both: the name is Y, and
  [§ 4.1](../Y-current-language-design.md#41-rust-as-an-oracle-not-as-ys-definition) makes Rust an
  oracle rather than Y's definition. Where the transcript and the design disagree, the design wins.
- The second half of it — deterministic replay, the split between a semantic and a physical optimizer,
  function-level hot swap, and live state migration by convergent copying — is not in the design
  document at all. It is written up here in
  [`toolchain/runtime-generations.md`](toolchain/runtime-generations.md), which is consequently the
  least settled document in the corpus.

## Suggested reading order

1. [`../Y-current-language-design.md`](../Y-current-language-design.md) §§ 1–2: what Y is and what it optimises for.
2. This page, to the end of "What is built, and what the marks mean".
3. [`language/README.md`](language/README.md), then the capabilities in the order its index gives them. Start with
   [types and typed failure](language/types-and-failures.md) and
   [ownership and resources](language/ownership-and-resources.md), which the others assume.
4. [`journeys/agent-writes-a-function.md`](journeys/agent-writes-a-function.md): the premise of the language, worked end to end.
5. [`toolchain/compiler-pipeline.md`](toolchain/compiler-pipeline.md), then [`toolchain/core-ir.md`](toolchain/core-ir.md),
   then [oracles](toolchain/oracles.md).
6. Last, and as the least settled document, [`toolchain/runtime-generations.md`](toolchain/runtime-generations.md).
7. To see something run, the [`../examples/`](../examples/) and their READMEs.

## The document kinds, for a language

The [taxonomy](https://github.com/david-brandt-the-icelander/knowledge-driven-development/blob/main/04-document-taxonomy.md) is written for a running service: features, journeys,
subsystems, contracts, and any other kind a system needs. The kinds survive the change of domain; the
examples do not. These are the four this corpus uses.

| Kind | Here | Perspective | Lives in |
| --- | --- | --- | --- |
| **Vertical feature** | A **language capability** a Y programmer would name: typed failure, shared state, bounded suspension | Whoever writes Y — the agent that emits it, the human who reviews it | [`language/`](language/) |
| **User journey** | A **pass through the toolchain**, start to finish | One actor: an agent, a compiler engineer, an operator | [`journeys/`](journeys/) |
| **Subsystem** | A **compiler or runtime stage that fails independently** | Whoever implements or debugs that stage | [`toolchain/`](toolchain/) |
| **Contract** | **Core IR**, and the foreign interface | A backend, an oracle projection, a tool outside the compiler | [`toolchain/core-ir.md`](toolchain/core-ir.md) |

## The documents

**Language capabilities** — what you can express, and what the compiler does about it.
→ [`language/README.md`](language/README.md) holds the index, the illustrative-syntax rule, and Y's
non-goals.

| Document | Covers |
| --- | --- |
| [Types and typed failure](language/types-and-failures.md) | `record`, `type`, `T?`, `T ! E`, exhaustive `match`, failure propagation |
| [Ownership and resources](language/ownership-and-resources.md) | Owned values, borrows, moves, arenas, deterministic cleanup, no hidden clones |
| [Effects and authority](language/effects-and-authority.md) | The effect system, why there is no `async` colour, capabilities instead of ambient access |
| [Shared state](language/shared-state.md) | `shared`, `atomic`, `retry`, `orElse`, and the rule that no external effect happens inside a transaction |
| [External effects](language/external-effects.md) | Durable commands, `afterCommit`, external queries, durable workflows |
| [Concurrency and time](language/concurrency-and-time.md) | Bounded suspension, `within`, task scopes, cancellation, supervision |
| [Specification in types](language/specification-in-types.md) | Contracts, typestate, units of measure |
| [Data and schemas](language/data-and-schemas.md) | `schema`, migration declarations, `converge` |
| [Queries and collections](language/queries-and-collections.md) | Pipelines, `derived`, `rule` |

**Toolchain** — the compiler and runtime that have to make the above true.

| Document | Covers |
| --- | --- |
| [Compiler pipeline](toolchain/compiler-pipeline.md) | Parser, HIR, semantic elaboration, and what each stage owes the next |
| [Core IR](toolchain/core-ir.md) | The contract: what every backend and oracle may rely on |
| [Oracles](toolchain/oracles.md) | Independent verifiers, and treating their disagreement as a bug signal |
| [Runtime generations](toolchain/runtime-generations.md) | Replay, the physical optimizer, hot swap, live state migration |

**Journeys**

| Document | Actor |
| --- | --- |
| [An agent writes a function](journeys/agent-writes-a-function.md) | A coding agent, from prompt to accepted commit |

## The state of this corpus

There is no separate backlog for the documentation itself. These are it.

- ⚠️ Y's capabilities are described here but its **goals and design principles are not** — they stay in
  [`../Y-current-language-design.md`](../Y-current-language-design.md) §§ 1, 2, 32, unmarked, because a
  goal has no build state and marking one invites a tick that means nothing.
  → [02 § What doesn't get a mark](https://github.com/david-brandt-the-icelander/knowledge-driven-development/blob/main/02-annotation-convention.md#what-doesnt-get-a-mark)
- ⚠️ **Y is described in two places and the duplication is not resolved.** Every capability statement
  here restates a section of the working design, so the two will contradict each other on the first
  change that lands in only one. The convention's answer is that a fact belongs in exactly one
  document; the decision this corpus is not entitled to make is which of the two that is.
  ⛔ **Unblocked by** the language owner either retiring the working design in favour of this corpus, or
  declaring the corpus a derived view that gets regenerated. Until then, prefer the working design on
  any disagreement and fix this corpus.
- ⚠️ **No enforcement exists.** The convention's one build gate, against `TODO`, needs a build to
  fail, and nothing in this repository runs it over the corpus. The prototype's tests check the examples, not
  these documents. Every mark in this corpus is therefore unchecked by anything but a reader, or an agent asked
  to check it.
  → [06](https://github.com/david-brandt-the-icelander/knowledge-driven-development/blob/main/06-enforcement.md#the-deferred-work-gate)
- ⚠️ **Journeys here are anchored to a toolchain invocation that is only partly runnable**, because the
  prototype covers a subset of the language, which is a weaker anchor than a running system and may turn out to
  be the wrong one.
  → [`journeys/agent-writes-a-function.md`](journeys/agent-writes-a-function.md#what-this-journey-is-anchored-to)
