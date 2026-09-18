# Compiler pipeline

> Status marks: [`../development/knowledge-driven-development/README.md`](../development/knowledge-driven-development/README.md)

**Written for:** whoever implements or debugs a compiler stage.

## What it is

The Y compiler is a pipeline whose centre of gravity is deliberately not the parser. Source becomes an
AST, the AST becomes a high-level IR that still resembles the source, elaboration makes every implicit
thing explicit, and the result is [Core IR](core-ir.md) — the backend-neutral representation that is the
real definition of what a Y program means.

```text
Y source → parser / AST → HIR → semantic elaboration → Core IR
                                                         ├→ analyses and optimisation
                                                         ├→ oracle projections
                                                         └→ backend lowering
```

The shape matters because of what it implies about where work belongs. **A backend must never have to
recover a semantic fact from surface syntax.** If a backend needs to know whether a value is borrowed,
whether a block is a transaction, or which effects a call has, elaboration put it in Core IR explicitly.
Every violation of that rule duplicates semantics into each backend, and the duplicates diverge.

## The boundary

- ⚠️ The compiler's inputs are Y source and a manifest describing the compilation unit's dependencies; its
  outputs are Core IR, diagnostics, and whatever a selected backend produces.
- ⚠️ Everything downstream of Core IR is replaceable: a backend, an oracle projection, an analysis. Nothing
  downstream may extend the language.
- ⚠️ Nothing in the pipeline consults a model. An agent's involvement is upstream of the parser and
  downstream of a diagnostic, and never inside a stage
  → [`../language/README.md` § What Y deliberately does not do](../language/README.md#what-y-deliberately-does-not-do).

## Parser and AST

- ⚠️ The parser produces an AST that retains every source span, so any later diagnostic can point at the
  text a reader wrote.
- ⚠️ The parser recovers from errors and continues, reporting many syntax errors per run rather than
  stopping at the first.
- ⚠️ The AST retains comments and formatting well enough for a formatter and for a mechanical rewrite to
  round-trip source without reformatting unrelated lines — which an agent editing Y depends on more than a
  human does.
- ⚠️⛔ The parser implements Y's grammar, which is not yet chosen
  → [`../language/README.md` § Syntax here is illustrative](../language/README.md#syntax-here-is-illustrative).
  ⛔ **Unblocked by** the language owner freezing the surface syntax. This is the first thing in the
  pipeline and the first thing blocked.

## HIR

- ⚠️ HIR is a desugared, name-resolved form that still carries source structure — it is the level at which
  type inference, effect inference, pattern checking and ownership elaboration run, and the level most
  diagnostics are phrased in terms of.
- ⚠️ Names are resolved in HIR, and a reference is a resolved binding rather than a string.
- ⚠️ Types are inferred in HIR, and a signature's written types are checked against the inferred ones
  rather than trusted.
- ⚠️ Pattern exhaustiveness and reachability are decided in HIR, where the source's match arms still exist
  to point at → [`../language/types-and-failures.md`](../language/types-and-failures.md).
- ⚠️ HIR retains enough structure that a diagnostic can be phrased in the reader's vocabulary: a borrow
  error names the binding and the two use sites, not a temporary introduced by desugaring.
- ⚠️ Desugaring in HIR is semantics-preserving and is not where implicit behaviour is decided — a
  desugaring that changes what a program means belongs in elaboration, where it is recorded.

## Semantic elaboration

This is where Y earns the two-layer design. Elaboration's job is to make every implicit source behaviour
explicit, and its output is checkable in a way the source is not.

- ⚠️ Elaboration computes and attaches: inferred ownership and implicit borrows; effect sets; suspension
  points; cancellation and deadline scopes; transaction boundaries and their read and write sets; durable
  command submissions; typed failure propagation paths; and contract obligations.
- ⚠️ Elaboration is deterministic: the same source and dependencies produce identical Core IR, byte for
  byte, on any machine.
- ⚠️ Elaboration explains itself. For any elaborated fact, the compiler reports what it inferred and from
  where — the question "why does the compiler think this function can suspend" has an answer that names a
  call path.
- ⚠️ Every rule that rejects a program is enforced here or later, on elaborated facts, never on syntax: the
  no-external-effect-in-a-transaction rule is a check over an elaborated effect set, so it cannot be
  evaded by spelling the call differently
  → [`../language/shared-state.md`](../language/shared-state.md).
- ⚠️ Elaboration is where a source-level convenience becomes an explicit semantic construct — a missing
  `await` becomes a suspension point, a pipeline becomes an explicit traversal with a stated ownership
  mode, a derived conversion becomes a named one.
- ⚠️ An elaborated program can be printed back as explicit Y, so a reviewer can read what the compiler
  concluded rather than inferring it from a diagnostic. This is the "show me what this really means"
  counterpart to showing generated Rust → [backends](#backends).

## Analyses, optimisation, and what may not happen here

- ⚠️ Semantic analyses over Core IR prove the properties the rest of the toolchain relies on: purity,
  whether a transaction may retry, whether a reference escapes, whether two operations commute.
- ⚠️ **Semantic transformation must be correct; physical choice may be experimental.** Any transformation
  that changes what a program means is a bug, and all of representation, indexing, partitioning, layout,
  vectorisation and caching is out of scope for this pipeline and belongs to the physical optimiser
  → [runtime generations](runtime-generations.md).
- ⚠️ The pipeline records, rather than discards, the decisions a physical optimiser will later want to
  revisit — so a layout choice is a parameter attached to Core IR, not a fact baked into a backend's
  output.

## Backends

- ⚠️ A backend consumes Core IR and nothing else.
- ⚠️ The Rust backend comes first, because rustc independently checks the ownership and borrowing
  conclusions Y reached → [oracles](oracles.md).
- ⚠️ A Zig or C backend follows, and a direct native backend after that.
- ⚠️ Generated Rust is readable, and the toolchain can show it for any Y function on request — output a
  competent Rust programmer might have written, not machine-shaped output that happens to compile. This is
  what makes debugging, profiling, interop and gradual adoption possible, and it is a requirement on the
  backend rather than a nicety.
- ⚠️ Backends are compared against each other on the same program, and a difference in observable behaviour
  is a bug in Y rather than a platform quirk to document → [oracles](oracles.md).
- ⚠️⛔ **The direct native backend's architecture** is open, including whether it targets a framework such
  as Cranelift or emits machine code itself, and § 31 lists it as undecided. ⛔ **Unblocked by** the
  language owner, and not urgent: the Rust backend has to exist first, and its existence will inform this.
- ⚠️⛔ **The ABI and foreign function interface** are open. This blocks calling C from Y, calling Y from
  anything, and the shape of every binding to an external library. § 31 lists it as undecided.
  ⛔ **Unblocked by** the language owner.

## Failure modes

| Symptom | Cause | What it looks like from outside | Handled? |
| --- | --- | --- | --- |
| A backend needs a semantic fact Core IR does not carry | Elaboration under-specified | The backend re-derives it from syntax, and two backends disagree | ⚠️ |
| Identical source produces different Core IR | Non-determinism in elaboration — iteration order, a hash seed, a path | Oracle projections and replay stop being comparable | ⚠️ |
| A diagnostic names a desugared construct | The check ran too late, or HIR dropped a span | An error about a temporary the programmer never wrote | ⚠️ |
| Inference concludes something surprising and cannot say why | Elaborated facts recorded without provenance | "The compiler thinks this can suspend" with no path to inspect | ⚠️ |
| A rule is checked on syntax rather than elaborated facts | Check placed in the parser or HIR | The rule is evadable by rephrasing the call | ⚠️ |

## Where it lives

- What Core IR must contain, as a contract → [core-ir.md](core-ir.md).
- Independent checking of what this pipeline concluded → [oracles.md](oracles.md).
- Everything about physical representation, versioning and the running system
  → [runtime-generations.md](runtime-generations.md).
- What the language requires this pipeline to enforce → [`../language/README.md`](../language/README.md).
