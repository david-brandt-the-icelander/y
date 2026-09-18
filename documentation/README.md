# Y

> Status marks: [`development/knowledge-driven-development/README.md`](development/knowledge-driven-development/README.md)

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

## Nothing here is built

There is no compiler, no parser, no runtime, and no source file of any kind in this repository — only
these documents. Every capability statement in this corpus is therefore a gap, and **no statement
anywhere in this corpus claims to be implemented and verified.** A reader who finds one has found a bug
in the corpus, not a working feature.

That is worth stating plainly, because a page with no green on it normally means nobody adopted the
convention, and here it means the opposite: the convention is adopted, and the honest answer is that
none of it exists.

It also means the gap marks carry the whole load, and a corpus where every line is a gap discriminates
nothing. That is the graduation rule from the other end — ticks get deleted once they stop carrying
information, and a uniform wall of gaps has the same problem. What actually varies across Y is not
whether a capability is built, but **whether it is decided well enough to build:**

- **A gap mark alone** means the statement is specified. An implementer could start from the paragraph
  without asking a question, which is the only bar
  [02](development/knowledge-driven-development/02-annotation-convention.md#what-each-mark-obliges-you-to-write)
  sets for it.
- **A gap followed by a blocked mark** means the semantics are still open. The blocked mark names the
  decision and who makes it.

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
| [`../riff.md`](../riff.md) | The conversation the design came out of | A transcript. **Never annotated** — it is a record of a past state, like a changelog entry |

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

## The five kinds, for a language

The [taxonomy](development/knowledge-driven-development/05-document-taxonomy.md) is written for a
car-wash SaaS and names its admin tables, its hardware and its receipts. The five kinds survive the
change of domain; the examples do not.

| Kind | Here | Perspective | Lives in |
| --- | --- | --- | --- |
| **Vertical feature** | A **language capability** a Y programmer would name: typed failure, shared state, bounded suspension | Whoever writes Y — the agent that emits it, the human who reviews it | [`language/`](language/) |
| **User journey** | A **pass through the toolchain**, start to finish | One actor: an agent, a compiler engineer, an operator | [`journeys/`](journeys/) |
| **Subsystem** | A **compiler or runtime stage that fails independently** | Whoever implements or debugs that stage | [`toolchain/`](toolchain/) |
| **Contract** | **Core IR**, and the foreign interface | A backend, an oracle projection, a tool outside the compiler | [`toolchain/core-ir.md`](toolchain/core-ir.md) |
| **Component** | Source files, annotated in place | The maintainer with the file open | Nothing to annotate yet |

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
  → [02 § What does not get a mark](development/knowledge-driven-development/02-annotation-convention.md#what-does-not-get-a-mark)
- ⚠️ **Y is described in two places and the duplication is not resolved.** Every capability statement
  here restates a section of the working design, so the two will contradict each other on the first
  change that lands in only one. The convention's answer is that a fact belongs in exactly one
  document; the decision this corpus is not entitled to make is which of the two that is.
  ⛔ **Unblocked by** the language owner either retiring the working design in favour of this corpus, or
  declaring the corpus a derived view that gets regenerated. Until then, prefer the working design on
  any disagreement and fix this corpus.
- ⚠️ The **knowledge-driven-development folder still describes the car-wash system it was imported
  from** — its inventories, waves, gates, `UserJourney` rows, hardware drivers and Asana history are
  all festi's, and its templates cite paths that do not exist here. The convention itself transfers
  cleanly; nothing around it does.
- ⚠️ **No enforcement exists.** All seven gates in
  [07](development/knowledge-driven-development/07-enforcement.md) are festi's, one of them built there
  and none of them here, and there is no build in this repository to fail. Every mark in this corpus is
  therefore unchecked by anything but a reader.
- ⚠️ **Journeys here have no backing record.** The taxonomy requires a journey document to correspond to
  a row in an admin database, and calls a journey without one "a feature document wearing a journey's
  clothes". That database is festi's. A Y journey is rooted in a reproducible toolchain invocation
  instead, which is a weaker anchor and may turn out to be the wrong one.
- ⚠️ **The templates contradict the convention they ship with, and this corpus follows the convention.**
  Every template opens with a four-glyph legend and the feature template asks an unimplemented
  statement to say "what happens instead today";
  [02](development/knowledge-driven-development/02-annotation-convention.md) forbids both — a page links
  to the convention rather than restating it, so the greps stay clean, and the mark is the whole
  commentary. No page here carries a legend.
- ⚠️ There is **no component documentation and no coverage vocabulary in use**, because there is no
  source. The three coverage marks appear nowhere in this corpus and should not appear until something
  compiles.
