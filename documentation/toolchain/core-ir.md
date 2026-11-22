# Core IR — the contract

> Status marks: [knowledge-driven development](https://github.com/david-brandt-the-icelander/knowledge-driven-development)

**Written for:** whoever consumes Core IR from outside the compiler — a backend, an oracle projection, an
analysis, a debugger, a physical optimiser.

## What this is

Core IR is the semantic centre of Y. It is not an implementation detail of the compiler and not a stage
that may be reshaped for a stage's convenience: it is **the definition of what a Y program means**, and
everything else in the toolchain is a consumer of it.

This document is a contract in the sense the taxonomy means → [taxonomy](https://github.com/david-brandt-the-icelander/knowledge-driven-development/blob/main/04-document-taxonomy.md#the-usual-kinds).
It says what a consumer may rely on and what it must not. The obligation runs the other way too: a
consumer that needs a semantic fact Core IR does not carry has found a defect in Core IR, and the fix is
to add it here rather than to recover it from surface syntax.

## What a consumer may rely on

Core IR represents each of these explicitly. A consumer never infers one from another and never parses
anything.

| Core IR carries | So a consumer never has to | Status |
| --- | --- | --- |
| Values and their types, with units erased | Reconstruct a type from a name | ⚠️ |
| Ownership, borrows and moves, per value and per use | Guess whether a value is owned or borrowed | ⚠️ |
| Control flow as an explicit graph | Recover structure from nesting | ⚠️ |
| Typed failures, and every path a failure propagates along | Distinguish a failure return from a normal one | ⚠️ |
| Effect sets, per function and per call site | Infer what a call may do | ⚠️ |
| Suspension points | Discover where a function may yield | ⚠️ |
| Deadlines and cancellation scopes, and their nesting | Work out what bounds a suspension | ⚠️ |
| Tasks and task scopes, with their parent relationships | Reconstruct a task's lifetime | ⚠️ |
| Transaction boundaries, with their read and write sets | Identify a transaction from a function name | ⚠️ |
| `retry`, and what it waits on | Model a retry as a loop | ⚠️ |
| Post-commit actions, and their durability | Guess whether an action survives a crash | ⚠️ |
| Durable commands, their identity, and their handlers | Recover an outbox from a table write | ⚠️ |
| Workflows and their suspension points | Distinguish durable suspension from a task's | ⚠️ |
| State machines and their legal transitions | Infer a protocol from control flow | ⚠️ |
| Contracts and invariants, with their obligations | Parse an assertion out of a body | ⚠️ |
| Schemas, their versions and their identities | Derive a persistent shape from a record | ⚠️ |
| Physical decisions, as attached parameters rather than baked choices | Re-decide a layout, or be unable to change one | ⚠️ |
| Stable logical identity for every function | Track a function by name or address | ⚠️ |
| Source spans on everything | Produce a diagnostic that cannot point at source | ⚠️ |

## Properties of the representation

- ⚠️ Core IR is backend-neutral. It contains nothing specific to Rust, to C, to a calling convention or to
  a machine — a construct that only one backend can lower is a defect.
- ⚠️ Core IR is deterministic: identical source and dependencies produce byte-identical Core IR on any
  machine, which is what makes projections and replay comparable at all
  → [oracles](oracles.md), [runtime generations](runtime-generations.md).
- ⚠️ Core IR is serialisable and versioned, so a consumer states which version it reads and a mismatch is
  detected rather than misinterpreted.
- ⚠️ Core IR is self-contained: a consumer needs no access to source, to the AST, or to HIR to do its job
  correctly. Source spans exist for diagnostics, not for semantics.
- ⚠️ Core IR is explicit in preference to compact. Where a fact could be represented once and derived
  elsewhere, it is represented, because a derivation is a semantic rule duplicated into every consumer.
- ⚠️ Core IR carries provenance for inferred facts, so a consumer — or a reviewer using one — can report
  why the compiler concluded something.

## What a consumer must not do

- ⚠️ **Not extend the language.** A backend may choose how to lower a construct and may not add one.
- ⚠️ **Not reinterpret a semantic.** Where Core IR says a transaction may retry, a consumer that assumes it
  will not is wrong, not optimising.
- ⚠️ **Not depend on a physical decision being fixed.** Layout, indexing, partitioning, vectorisation and
  caching are parameters that change between runs of the same program
  → [runtime generations](runtime-generations.md).
- ⚠️ **Not treat surface syntax as available.** If a consumer wants a fact, it comes from Core IR or it is
  a gap in Core IR.

## The compatibility signature

- ⚠️ Every function's Core IR carries a compatibility signature: its parameter and return representations,
  its effect set, its transactional writes, the classes of durable command it may submit, and the
  contracts it is obliged to satisfy.
- ⚠️ The signature is what decides whether one implementation of a function may substitute for another at
  runtime, so hot swap is a type-checkable question rather than an operational gamble
  → [runtime generations](runtime-generations.md).
- ⚠️ A change to any component of the signature is detected as a change of deployment boundary — adding an
  external command to a function that had none is not a transparent optimisation, and the toolchain says so
  rather than permitting the swap.

## Failure modes

| Symptom | Cause | What it looks like from outside | Handled? |
| --- | --- | --- | --- |
| Two backends produce different observable behaviour | A semantic Core IR left implicit, and each backend guessed | A program that works on one target and not another → [oracles](oracles.md) | ⚠️ |
| A projection cannot be generated for a construct | Core IR carries the construct but not the facts a verifier needs | Whole classes of program silently unverified | ⚠️ |
| Replay of a captured workload diverges | Non-determinism reached Core IR | Every downstream comparison becomes unsound → [runtime generations](runtime-generations.md) | ⚠️ |
| A hot swap is accepted and changes behaviour | Compatibility signature under-specified | A production incident from a promotion that type-checked | ⚠️ |
| A consumer reads surface syntax | A fact missing from Core IR, worked around rather than reported | The missing fact stays missing, and the workaround spreads | ⚠️ |

## Where it lives

- The stages that produce this → [compiler-pipeline.md](compiler-pipeline.md).
- The consumers that check it against independent implementations → [oracles.md](oracles.md).
- The consumer that varies physical parameters and swaps implementations
  → [runtime-generations.md](runtime-generations.md).
- The language constructs each entry above corresponds to
  → [`../language/README.md`](../language/README.md).
