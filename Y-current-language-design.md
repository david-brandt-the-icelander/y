# Y: Current Language Design

**Status:** Working design  
**Date:** 2026-09-17  
**Purpose:** Capture the language as it currently stands, including settled semantic decisions, current architectural direction, and surface syntax that is still illustrative.

---

## 1. What Y is

Y is an **AI-oriented systems/application programming language** designed around a different assumption from most existing languages:

> The primary programmer may increasingly be an AI agent, while the compiler remains the authority on meaning, safety, and correctness.

Y is therefore not a natural-language programming system and not an LLM-driven compiler. The language must have precise, deterministic semantics that can be implemented, compiled, tested, model-checked, and reasoned about without trusting an AI.

The intended model is:

```text
human intent
    |
    v
AI agent writes/refactors Y
    |
    v
pleasant Y source
    |
    v
deterministic semantic elaboration
    |
    v
backend-neutral Core IR
    |
    +--> verification/oracle projections
    |
    +--> Rust / Zig / C / native backend
```

AI is free to propose implementations, refactorings, proofs, optimizations, test cases, or backend alternatives.

The compiler decides whether those results are valid.

---

## 2. Primary design goals

Y is intended to optimize for the following.

### 2.1 Make correct programs easier to generate than subtly wrong programs

The language should encode common correctness requirements directly into its semantics:

- ownership and aliasing rules;
- typed failures;
- explicit transaction boundaries;
- deterministic state transitions;
- structured concurrency;
- bounded suspension;
- durable external effects;
- contracts and invariants;
- schemas and migrations;
- machine-checkable effect information.

An AI agent should not need to remember dozens of external conventions to produce safe code.

### 2.2 Be pleasant for humans, but highly legible to machines

Source code should remain concise and familiar enough for human review.

At the same time, Y should avoid semantics that depend heavily on:

- hidden runtime conventions;
- framework magic;
- reflection;
- implicit global state;
- action at a distance;
- ambiguous overload resolution;
- undefined behavior.

The semantic representation behind the source should be even more explicit than the source itself.

### 2.3 Make important behavior mechanically inspectable

The compiler should be able to answer questions such as:

- Can this function mutate shared state?
- Can it suspend?
- Can it perform external I/O?
- Can it retry?
- Which values does it own or borrow?
- Can this transaction cause an external side effect?
- Which errors can escape?
- Which resources must be released?
- Which state transitions are legal?
- What happens if the process crashes here?

This is useful both to humans and to coding agents.

### 2.4 Eliminate unnecessary "coloring"

Y should avoid infecting large parts of the program with surface-level distinctions that are primarily implementation details.

The most important example is asynchronous programming.

A caller should not generally need a separate `async` version of a function simply because the implementation may suspend.

Potential suspension remains visible to the compiler and Core IR, but should not require the entire source program to be manually colored with `async`/`await`.

### 2.5 Treat retries, crashes, and concurrency as normal execution

Retries and failures are not exceptional edge cases in distributed or concurrent software.

Y should provide first-class semantics for:

- transactional retry;
- cancellation;
- deadlines;
- durable commands;
- durable workflows;
- supervision;
- restart;
- idempotence;
- conflict handling.

---

## 3. Source-language direction

The surface syntax is intentionally conventional.

The goal is not syntactic novelty. The goal is semantic leverage.

Unless otherwise stated, syntax in this document is **illustrative rather than frozen**.

### 3.1 Functions

```y
------------------------------------------------------------------------------------------------------------------------
x: The number to square, as int64.
squared: x times x, as int64.
------------------------------------------------------------------------------------------------------------------------
function square(x) returns squared:
    return squared = x * x
```

With several paths:

```y
------------------------------------------------------------------------------------------------------------------------
x: The number to limit, as int64.
minimum: The smallest allowed, as int64.
maximum: The largest allowed, as int64.
clamped: x, moved into the range, as int64.
------------------------------------------------------------------------------------------------------------------------
function clamp(x, minimum, maximum) returns clamped:
    if below = (x < minimum):
        return clamped = minimum
    if above = (x > maximum):
        return clamped = maximum
    return clamped = x
```

(The October proposal settles the spelling: types are written in the documentation above a function, the result is
named, and every identifier is `lower_snake_case`; `proposal/Y-proposal.md` § 3.1.)

### 3.2 Immutable and mutable bindings

```y
let name = "Ada"
variable attempts = 0
```

`let` is immutable.

`variable` permits reassignment, subject to ownership, borrowing, transaction, and effect rules.

### 3.3 Optional values

```y
customer?
```

This represents either a `customer` or absence.

Pattern matching is preferred where behavior differs materially between cases.

```y
match customer:
    optional.some(found) => use(found)
    optional.none        => not_found()
```

Exact optional-case syntax is not yet frozen.

### 3.4 Typed failures

The working notation is:

```y
function load_customer(id) returns customer fails customer_error
```

meaning:

> produces a `customer`, or fails with a `customer_error`.

Failures are ordinary typed program values, not an untyped exception side-channel.

Pattern matching and propagation syntax should make the success path concise without hiding the possible failure set.

### 3.5 Records and algebraic types

Working direction:

```y
record customer:
    id: customer_id
    name: string
```

Variants/sum types:

```y
type payment_result:
    case approved(authorization: authorization)
    case declined(reason: decline_reason)
    case unavailable
```

Pattern matching is a core control-flow mechanism:

```y
match result:
    payment_result.approved(authorization) => complete(authorization)
    payment_result.declined(reason) => reject(reason)
    payment_result.unavailable => retry_later()
```

Exhaustiveness should be compiler checked.

---

## 4. Ownership, borrowing, and memory safety

Y adopts the useful part of Rust's ownership model without requiring Y to expose all of Rust's source-level complexity.

The semantic model should be able to represent:

- owned values;
- moves;
- immutable borrows;
- mutable borrows;
- aliasing restrictions;
- resource lifetimes;
- deterministic cleanup.

Illustrative syntax may include familiar forms:

```y
-- order: The order to look at, as order.
function inspect(order): ...

-- order: The order to change, as order.
function update(mutable order): ...
```

The October proposal settles this further: source has value semantics and no references at all, and
borrows, moves and copies are the compiler's choices (`proposal/Y-proposal.md` § 3.6).

The compiler may infer substantially more than Rust does at the source level.

The important property is not syntactic similarity to Rust. It is that unsafe aliasing and lifetime behavior is rejected or made explicit.

### 4.1 Rust as an oracle, not as Y's definition

Rust is initially useful as:

- a reference backend;
- an ownership/borrow-checking oracle;
- a way to validate portions of Y's inferred ownership model.

Y's semantics must remain independent of Rust.

The compiler may project Core IR into Rust specifically to ask:

> Would `rustc` accept an equivalent ownership/borrowing structure?

That is evidence about Y's implementation, not the definition of Y itself.

---

## 5. Effects

Y should have an effect system, although much of it should normally be inferred.

Examples of semantic effects include:

- local mutation;
- shared-state mutation;
- suspension;
- filesystem access;
- network access;
- database access;
- time;
- randomness;
- durable command submission;
- process spawning;
- unsafe/native operations.

The Core IR must know these effects even if the source usually omits them.

This allows the compiler to enforce rules such as:

> Code executing inside an STM transaction cannot directly perform a nontransactional external effect.

Explicit effect annotations may be available for APIs, interfaces, contracts, or documentation, but source-level effect verbosity should be kept low.

---

## 6. Shared state and STM

Shared mutable state is based around **software transactional memory (STM)** rather than arbitrary locking as the primary abstraction.

Illustrative declarations:

```y
shared inventory: inventory
```

Transactional mutation:

```y
atomic:
    let item = inventory[item_id]

    if item.available == 0:
        retry

    item.available -= 1
```

### 6.1 `atomic`

An `atomic` block behaves as one logical transaction over transactional state.

The runtime/compiler is responsible for:

- observing transactional reads;
- tracking writes;
- detecting conflicts;
- retrying safely;
- committing atomically.

### 6.2 `retry`

`retry` aborts the current attempt and suspends until data relevant to the transaction changes or until its enclosing deadline/cancellation condition fires.

This is semantic retry, not a manually written polling loop.

### 6.3 `or_else`

Alternative transactions can be composed.

Illustrative syntax:

```y
atomic:
    take(primary_queue)
or_else:
    take(secondary_queue)
```

The exact syntax is not frozen, but transactional alternatives are part of the intended model.

---

## 7. Hard rule: no direct external effects inside STM

This is a core semantic rule.

Transactional code **must not directly interact with external nontransactional systems**.

For example, this is invalid in principle:

```y
atomic:
    account.balance -= amount
    payment_provider.charge(card, amount) -- forbidden
```

An STM transaction may be retried.

If the external charge cannot be rolled back with the transaction, retrying the transaction could charge the customer multiple times.

Y therefore separates:

1. transactional state changes;
2. durable intent to perform an external action;
3. the eventual external effect;
4. durable recording of the result.

---

## 8. Durable commands and the outbox model

External effects requested from transactional code are represented as durable commands.

Illustrative direction:

```y
atomic:
    order.status = order_status.payment_pending

    enqueue charge_card(order_id: order.id, amount: order.total, payment_method: order.payment_method)
```

The command becomes durable as part of the same transaction.

A worker executes it outside the transaction.

The command system should strongly encourage or require:

- durable persistence;
- stable command identity;
- idempotence where possible;
- retry policies;
- recorded outcomes;
- deduplication;
- correlation;
- explicit timeout/deadline behavior.

This is effectively an outbox pattern elevated into the language/runtime semantics rather than repeatedly reinvented in application code.

---

## 9. `afterCommit`

There are cases where code conceptually wants to say:

> Do this only if the transaction really commits.

The language should support a post-commit mechanism.

Illustrative syntax:

```y
atomic:
    order.status = order_status.confirmed

    afterCommit:
        enqueue send_confirmation(order.id)
```

The important semantic property is that the action is not performed during speculative transactional execution.

Where reliability matters, `afterCommit` should lower to a durable mechanism rather than an in-memory callback that can be lost in a crash.

---

## 10. External queries from transactional logic

A transaction must also not synchronously query an arbitrary external system and then assume the answer participates in STM consistency.

When external information is required, the intended pattern is:

1. persist a request;
2. execute it outside STM;
3. persist the result;
4. resume/retry logic using transactional state containing that result.

This makes crash/retry behavior explicit and reconstructable.

The language/runtime should make this pattern substantially easier than manually building request tables, workers, correlation IDs, callbacks, and resumption logic.

---

## 11. Suspension without `async` coloring

Y intends to remove the source-level distinction between "normal functions" and "async functions" wherever practical.

Potentially suspending operations are still semantically known.

Example:

```y
let response = http.get(url) within 2s
```

The caller need not necessarily be declared `async`, and the source should ideally not require a mandatory `await` keyword.

The compiler/runtime transforms suspension into the appropriate continuation/state-machine representation.

### 11.1 Suspension must be bounded

A potentially suspending operation must not accidentally wait forever.

Every suspension must have a finite bound through one or both of:

- an explicit deadline/timeout;
- inherited structured cancellation with a finite enclosing deadline.

Illustrative syntax:

```y
let result = queue.receive() within 5s
```

or:

```y
within 30s:
    let user = load_user(id)
    let orders = load_orders(user.id)
    render(user, orders)
```

Exact syntax remains open.

---

## 12. Structured concurrency and cancellation

Concurrency should be hierarchical.

Child work belongs to an enclosing task scope.

If the parent is cancelled, fails, or exceeds its deadline, child work is cancelled unless explicitly detached through a deliberate mechanism.

The language should avoid orphan tasks as the default.

Illustrative direction:

```y
within 10s:
    task user = load_user(id)
    task orders = load_orders(id)

    render(user, orders)
```

The compiler/runtime knows the lifetime and cancellation relationship of these tasks.

---

## 13. Durable workflows

Long-running operations should be expressible as workflows rather than ad hoc combinations of database flags, queues, timers, and worker code.

Illustrative syntax:

```y
workflow fulfill(order_id):
    let order = load_order(order_id)

    let payment = submit charge_card(order.payment)
    let shipment = submit create_shipment(order)

    complete(order, payment, shipment)
```

A workflow may survive:

- process restart;
- machine restart;
- worker reassignment;
- temporary dependency failure.

Its progress and required inputs/results must therefore be reconstructable from durable state.

Workflows are conceptually distinct from short-lived in-memory tasks.

---

## 14. Contracts and refinement

Y should support machine-checkable preconditions, postconditions, and invariants.

Illustrative syntax:

```y
function withdraw(mutable account, amount) returns debit fails withdraw_error
        requires positive = (amount > 0)
        requires covered = (account.balance >= amount)
        ensures debited = (account.balance == old(account.balance) - amount):
    ...
```

Contracts serve several purposes:

- runtime checks where appropriate;
- static verification where possible;
- property-test generation;
- oracle input;
- documentation;
- stronger guidance for coding agents.

The compiler may gradually support richer refinement reasoning without requiring full theorem proving for ordinary programs.

---

## 15. Units of measure

Physical and business units should be representable in the type system.

Examples:

```y
let timeout: seconds = 5s
let distance: kilometers = 12.4km
let amount: isk = 1500kr
```

This prevents entire classes of category errors and gives AI-generated code more semantic structure to work with.

Exact unit syntax is not frozen.

---

## 16. Capabilities and authority

Authority should be explicit.

A function should not automatically have access to every ambient resource in the process.

Capabilities may represent rights such as:

- read a filesystem subtree;
- send a specific class of command;
- access a database;
- call a network service;
- read a secret;
- mutate a particular shared resource.

This makes security properties inspectable and constrains AI-generated code.

The language should prefer explicit authority flow over global ambient access.

---

## 17. State machines and typestate

Protocol state should be representable in types rather than only in runtime comments and Boolean fields.

Example direction:

```y
state order:
    case created
    case payment_pending
    case paid
    case cancelled
    case fulfilled
```

Transitions can be restricted:

```y
transition pay(order) returns pending
```

The exact syntax is open.

The intended property is that illegal state transitions can often become compile-time errors.

---

## 18. Schemas and migrations

Persistent schemas should be first-class enough that the compiler/toolchain can reason about:

- schema versions;
- compatibility;
- serialization;
- migrations;
- additive versus breaking change;
- generated codecs;
- database projections.

Schema evolution should not be an unrelated string-based side system.

Illustrative direction:

```y
schema customer v3:
    id: customer_id
    name: string
    email: email?
```

Migration syntax remains open.

---

## 19. Declarative and derived computation

Y may include a declarative subset for facts, rules, derived values, and incremental computation.

Examples:

```y
derived active_user_count =
    users.where(user => user.active).count()
```

and:

```y
rule high_risk_customer(customer) =
    (customer.failed_payments > 3) and (customer.outstanding_balance > 100_000kr)
```

The goal is not to turn the entire language into Datalog.

The goal is to allow computations that are naturally relational or incremental to be expressed declaratively so the runtime/compiler can choose efficient evaluation strategies.

---

## 20. Collections and pipelines

Collection operations should be concise and composable.

Illustrative style:

```y
let names =
    users
        .where(user => user.active)
        .map(user => user.name)
        .sorted()
```

Pipeline syntax inspired by F#/Elixir may also be considered if it materially improves readability.

The semantic representation should make allocation, laziness/eagerness, parallelism, and ownership effects explicit even if source syntax remains compact.

---

## 21. Resilience and supervision

Y should borrow heavily from Erlang/OTP's treatment of failure.

Long-lived concurrent components should be able to declare:

- restart policy;
- supervision relationships;
- escalation behavior;
- failure isolation.

Failures should not automatically corrupt unrelated state or require hand-written restart scaffolding.

The exact source model is still open.

---

## 22. Deterministic testing of concurrency

Concurrency primitives should be designed together with deterministic testing support.

The runtime/toolchain should be able to:

- control scheduling;
- explore relevant interleavings;
- inject cancellation;
- inject timeouts;
- force transaction conflicts;
- simulate crashes around durable-effect boundaries;
- reproduce discovered schedules.

A concurrency failure found once should be reproducible.

This is especially important when AI agents are generating highly concurrent code.

---

## 23. The two-layer language model

Y intentionally separates pleasant source syntax from a more explicit deterministic semantic layer.

### Layer 1: Source Y

Optimized for:

- humans;
- coding agents;
- compactness;
- maintainability;
- domain expression.

### Layer 2: Semantic/code-behind representation

Optimized for:

- determinism;
- compiler analysis;
- verification;
- transformation;
- backend generation;
- oracle projections.

An AI may transform source into a different valid implementation, but it does not get to reinterpret semantics.

This separation is central to the language.

---

## 24. Compiler architecture

The current intended pipeline is:

```text
Y source
   |
   v
parser / AST
   |
   v
HIR
   |
   v
semantic elaboration
   |
   v
backend-neutral Core IR
   |
   +--> analyses and optimization
   |
   +--> oracle projections
   |
   +--> backend lowering
```

### 24.1 HIR

The high-level IR preserves enough source structure for:

- useful diagnostics;
- type inference;
- effect inference;
- pattern checking;
- ownership elaboration;
- contract association;
- state-machine analysis.

### 24.2 Semantic elaboration

This phase makes implicit source behavior explicit.

Examples include:

- inferred ownership;
- implicit borrows;
- effect sets;
- suspension points;
- cancellation scopes;
- STM boundaries;
- durable commands;
- typed failure propagation;
- pattern exhaustiveness.

### 24.3 Core IR

Core IR is the semantic center of Y.

It must explicitly represent at least:

- values and types;
- ownership and borrows;
- moves;
- control flow;
- typed errors;
- effects;
- suspension;
- deadlines and cancellation;
- tasks/task scopes;
- STM transactions;
- transactional reads and writes;
- retry;
- post-commit actions;
- durable commands;
- workflows;
- state transitions;
- contracts/invariants.

Backends should not have to rediscover these semantics from surface syntax.

---

## 25. Backends

Y should not permanently depend on a single implementation language.

Likely backend progression:

1. **Rust backend/reference projection**
2. **Zig and/or C backend**
3. **direct native backend**, potentially using a framework such as Cranelift

Rust is attractive early because its compiler can validate ownership and borrowing decisions.

However, using Rust as an intermediate form does not mean Rust must remain the final architecture.

The Core IR is the stable center.

---

## 26. Verification by independent oracles

One unusual part of Y is the deliberate use of other mature tools as **semantic or implementation oracles**.

The compiler may generate equivalent projections and ask independent tools whether Y's result is valid.

Candidate oracle families include:

### Ownership and undefined behavior

- `rustc`
- Miri

### C-level memory and control-flow verification

- CBMC
- Frama-C

### Contracts and functional correctness

- Why3
- Dafny
- SPARK-related verification approaches where useful

### State machines and distributed/concurrent protocols

- TLA+
- Apalache

### Concurrent execution

- Loom
- Shuttle
- Y's own deterministic scheduler/explorer

### Optimization correctness

- Alive2, where LLVM-like transformations are involved

### Backend equivalence

Run the same program through multiple backends and compare:

- results;
- failures;
- observable effects;
- state transitions;
- serialized output.

### High-assurance backend experiments

CompCert may be useful for selected C compilation paths where its guarantees justify the constraints.

---

## 27. Oracle disagreement is a bug signal

Oracle results are not decided by majority vote.

If:

- Y accepts a program;
- the Rust ownership projection rejects it;
- a C verifier finds an invalid memory path;
- or two backends produce different observable behavior;

the compiler/toolchain should treat the disagreement as something to investigate.

Ideally it should automatically minimize the failing program into a small reproducer.

This turns the surrounding compiler ecosystem into a differential-testing framework for Y itself.

---

## 28. AI's role in compilation and development

AI is useful in Y, but it is deliberately outside the trusted semantic core.

AI may:

- generate Y source;
- refactor code;
- propose optimizations;
- synthesize contracts;
- generate tests;
- generate verification projections;
- suggest ownership transformations;
- choose among equivalent backend strategies;
- minimize failures;
- explain diagnostics;
- search for alternative implementations.

AI may not:

- redefine language semantics;
- waive failed compiler checks;
- silently reinterpret unsafe code;
- make correctness depend on nondeterministic model output.

The rule is:

> AI proposes. Deterministic tooling verifies.

---

## 29. Why Y is AI-oriented

Y is not AI-oriented because it contains an LLM API in the standard library.

It is AI-oriented because its semantics are structured to make automated code generation and automated review unusually reliable.

Features that matter especially for AI-written code include:

- explicit typed failure;
- ownership safety;
- effect inference;
- no unrestricted external effects from STM;
- durable effect semantics;
- bounded suspension;
- structured cancellation;
- contracts;
- units;
- state machines;
- explicit authority;
- deterministic concurrency testing;
- machine-readable schemas;
- a canonical Core IR;
- independent semantic oracles.

These reduce the amount of implicit human knowledge required to determine whether generated code is safe.

---

## 30. Current settled design decisions

The following should currently be treated as core design commitments.

1. **Y has deterministic language semantics.**
2. **AI is not part of the trusted semantic core.**
3. **There is a backend-neutral Core IR.**
4. **Ownership/borrowing is represented semantically.**
5. **Rust/rustc is useful initially as an ownership oracle/reference backend, not as Y's definition.**
6. **Shared mutable state has first-class STM support.**
7. **STM code cannot directly perform nontransactional external side effects.**
8. **External effects requested from transactional logic go through durable request/command mechanisms.**
9. **Post-commit actions have explicit semantics.**
10. **Potential suspension should not require pervasive source-level `async` coloring.**
11. **Potentially suspending work must be bounded by deadline/timeout and structured cancellation.**
12. **Durable workflows are first-class enough to survive process failure.**
13. **Typed failures are part of normal function signatures.**
14. **The compiler/toolchain should use independent verification/oracle projections where useful.**
15. **Disagreement between independent implementations/oracles should be investigated and minimized, not majority-voted away.**

---

## 31. Strongly favored but not yet fully frozen

The following are part of the current direction but still have meaningful design freedom.

- exact surface syntax;
- effect annotation syntax;
- degree of ownership inference;
- exact optional/result syntax;
- precise task syntax;
- exact `within`/deadline syntax;
- `afterCommit` syntax and lowering rules;
- workflow syntax;
- capability syntax;
- contract/refinement syntax;
- schema/migration syntax;
- state-machine/typestate syntax;
- declarative `rule`/`derived` facilities;
- direct native backend architecture;
- garbage collection versus region/ownership strategies for selected types;
- macro/metaprogramming model;
- exact ABI and foreign-function interface.

---

## 32. Design principle for unresolved questions

When choosing between language features, Y should prefer the option that:

1. gives the compiler more semantic information;
2. removes opportunities for invalid states;
3. remains deterministic;
4. is easy for an AI to generate correctly;
5. is easy for a human to review;
6. can be projected into independent verification tools;
7. does not impose unnecessary ceremony on ordinary source code.

This is the central tension Y is trying to resolve:

> **Make the source simpler while making the semantics more explicit.**

That is the language's current architectural direction.
