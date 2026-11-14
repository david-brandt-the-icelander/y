# The constraint language

**Status:** Design
**Date:** 2026-10-02
**Written for:** the team that owns the constraint language, and everyone who writes, harvests, reviews or enforces
constraints: the engine, game-logic, networking and security teams, and studios applying the plan to games written in
other languages.
**Companion documents:** [`database-proposal.md`](database-proposal.md) introduces the language (§ 6.7), its
conservation laws (§ 6.9), the harvesting that fills it (§ 6.13) and its first consumer, the RPC schema at the proxy
(§ 12.2); this document designs the whole language those sections point at.
[`adaptations/stillfront.md`](adaptations/stillfront.md) is why it must constrain C#, Java and PHP as well as Rust,
Python and T-SQL.

---

## 0. The design on one page

The constraint language states what must hold of a program and never says how anything is done. It has no control flow,
no assignment and no loops. It is the one language the plan owns, and it is designed to outlive every implementation it
constrains: a rule stated in it survives the translation of a Python method into a Rust procedure, a T-SQL procedure
into the engine, or a C# service into anything else, because it is stated about the boundary of the code and not in the
code.

- **One kind of file, beside the code.** Files named `.constraints` bind by stable name to subjects in any host
  language: stored procedures, RPC methods, functions, types, tables, resources and interfaces, in Rust, T-SQL, Python,
  C#, Java, Kotlin, Swift, C++, Go, PHP and TypeScript.
- **Every host gets a bare form.** Everything in a host language that only restricts which programs it accepts (its
  types, nullability, lifetimes, mutability and access markers, visibility, generic bounds, declared exceptions) can
  move into the constraint file, and a construct that both restricts and acts is split, its act staying and its
  restriction moving. What is left is a bare form of the language that says what the program does, together with the
  choices its file makes. `strip` turns ordinary source into bare source and a constraint file; `weave` turns them back
  into ordinary source of the same host, which its own toolchain compiles (§ 11.2). The constraint language does not
  replace a host's type system, and yet every host gets a derived language whose type system is the constraint file.
- **One expression language, defined once.** Two-valued logic with explicit absence, exact integer and decimal
  arithmetic, floats as every host compares them, explicit collation, total evaluation and a read set known before
  evaluation. Each constraint is compiled into every place that enforces it, and the generated checks are compared on
  the recorded corpora so that no two places disagree.
- **Clauses in families.** Values and shapes; calls (authority, preconditions, postconditions, failures, callbacks,
  laws); bounds on effects (what a call must not do); state (invariants, uniqueness, references, conservation);
  protocols and state machines; order in time and statistics; budgets; concurrency; information flow; compatibility and
  visibility; memory: lifetime, mutability and aliasing; independence, the facts that let work run in parallel or in
  another order; freedoms, what may vary; named algebraic properties; facts about a collection's contents; and
  expectations, what usually happens, which never decide correctness; and the exits of every callable, each way a
  call can end. Beside them, a platform states what every
  subject of a build may assume (§ 3.6), a profile what the build's traffic was measured to do (§ 3.7), and the facts
  that hold at each point of a body are derived, never written (§ 11.1.1).
- **Memory, written once for every model.** Lifetime, mutability and aliasing are stated over a core that belongs to no
  host: entities with lifetimes; references that own, share, borrow, observe or only identify what they point to; and
  paths that carry access rights and a guarantee about every other path. A memory model per host gives the core a
  precise meaning: ownership (Rust), tracing collection (Java, C#, Go, JavaScript), reference counting (Swift, CPython,
  PHP, `shared_ptr`, `Rc`), tracked references (generational handles), regions, manual management (C), and rows in a
  database.
- **Status and evidence, always.** Every constraint is observed, lifted, watched, enforced, proven or assumed with a
  stated reason, and carries the evidence for that status. It is checked by the cheapest means the host allows: the
  host's own compiler, a static analysis, a runtime check, or watching on replicas, test clusters and in CI.
- **A grammar designed whole, kinds enabled one at a time.** The grammar below is designed as a whole, so that early
  files never need rewriting. A kind is enabled when something consumes it, with its miner and its compilation into
  every place that enforces it; a file that uses a kind not yet enabled does not build. The first release enables the
  types of RPC signatures and nothing else. The list of kinds is reviewed as a language every year, and growth only ever
  adds to the grammar.

---

## 1. What the language is for

A program's rules live in three places today: in its types, in checks scattered through its code, and in what it
reliably does without anyone having written it down. The language gives them one place, outside the code, in a form that
a person can review, an agent can write, a miner can propose and every runtime can enforce identically. Constraints are
written by people, lifted out of code and harvested from running systems (§ 6.13 of the proposal); once in the language
they are enforced where they are cheapest and mean the same wherever they run.

What it is not:

- **Not a programming language.** There is no control flow, no assignment, no loop and no recursion. `let` names a
  value; a conditional is an expression with a value, never a branch that does something.
- **Not a description of what code does.** A constraint may say what code must not do, never what it does. The events a
  procedure emits, the rows it writes and the requests it sends are its effect manifest, a record of what it was seen to
  do, and the manifest stays a record. A bound such as `writes only wallet.Balances` is a limit set as a rule; the
  manifest is evidence checked against the limit, never copied into one.
- **Not a specification of formulas.** Damage after resistances, stacking penalties, movement and market matching are
  functions, not predicates. Their specification is a reference function, compared with its replacement by output. The
  language may call such a function by name; it never restates one.

And one thing it is and is not at once:

- **The type system, moved out.** The language does not replace a host's type system: where the host's compiler proves a
  constraint, the constraint is `proven` by it and costs nothing at run time. But it can hold everything the host's type
  system says, and more, so a host's code can be written without any of it: a bare form of the language, with the types,
  lifetimes and mutability in the constraint file, woven back into the host's ordinary source before its compiler sees
  it (§ 11.2). The host's type system then checks what the file says, and the file says the rest. Hosts without a
  mechanism gain it as generated checks; hosts with it keep using it.

This is why the language covers lifetime, mutability and aliasing. When a Python manager becomes a Rust procedure, or a
C# service is rewritten, what it promised about which objects it keeps, which it changes and which it shares must carry
across, and only a statement that belongs to no host can carry it. With bare languages, translating between hosts
becomes translating a body: the constraint file stays.

---

## 2. Principles

1. **It states, and never does.** A constraint is a named statement about a program. Checking it changes nothing.
2. **One meaning everywhere.** Absence, arithmetic, comparison, text, time and identity are defined once, in the
   language (§ 4). The code generated for each place is compared on the recorded corpora and on generated edge cases, so
   that the proxy, the Python servers, T-SQL, the engine and every woven host cannot disagree.
3. **Predicates are total and bounded.** Every clause that is checked as a predicate on a request or a write evaluates,
   on every input, to true or false, in bounded work, with a read set known before it runs. Clauses about executions and
   the heap are checked another way, and never as predicates on the request path (§ 4.1).
4. **Beside the code, bound by name.** A file binds to its subjects by stable name. A subject that resolves to nothing
   fails the build, and so does a declaration that nothing uses. Every change to a constraint is shown in review.
5. **The boundary, never the body.** A constraint may refer to a subject's inputs, results and failures; its session,
   actor and origin; the world as the catalog describes it; named pure functions; and what happens to the entities that
   cross the boundary: whether they are written, retained, released or shared. It may not refer to a local variable or
   an intermediate value. A check over an intermediate value becomes a named pure function that both can use, or stays
   in the code. The one exception is a type that a woven host needs for a local and cannot infer (`local`, § 11.2): it
   states a type, never a condition.
6. **Order is evaluation order.** Within a subject, predicates run in the order written. The first precondition that
   fails decides the failure the caller sees, so lifting checks out of code keeps their order, and no place may run a
   later clause before an earlier one (§ 10.3). Order also narrows: after `require found : exists m`, later clauses may
   use `m` as present.
7. **Status and evidence for every constraint.** Each has a status and the evidence for it (§ 10), and every change of
   status is recorded with its reason.
8. **One kind at a time, each in a paragraph.** A kind of constraint is added when something consumes it, and only if
   its meaning can be stated in a paragraph. Sections 6, 7, 8 and 11.2 open each kind with that paragraph.
9. **People decide what inference must not.** Authorisation, visibility, conservation laws and the policy on faucets and
   sinks, the game's formulas and every boundary with the outside are decided by people. Miners may propose candidates
   for any of them; traffic may tighten them, never define them.

---

## 3. Files, subjects and hosts

### 3.1 Files

A constraint file is UTF-8 text named `*.constraints`, structured by indentation, with comments from `--` to the end of
the line. A clause continues on following lines indented further than its first. A file opens with an optional `module`
line and any `use` lines, then holds declarations and subject blocks in any order. Inside a subject, the order of
clauses is the order of evaluation. There is one canonical form, produced by the formatter, so that diffs show only
meaning.

```text
-- space/modules.constraints
module space
use space.predicates          -- the predicate flies; the block in_space
use economy                   -- quantities: Isk, ItemQuantity
```

### 3.2 Subjects

| Subject | Binds to | Example |
| --- | --- | --- |
| `procedure` | A stored procedure in the catalog, in Rust or T-SQL | `procedure space.ActivateModule(moduleID: u64, targetID: u64)` |
| `method` | An RPC method, by service and method | `method inventory.MoveItems(itemIDs: list<element_type=u64>, toLocationID: u64)` |
| `function` | A function or method in a host language | `function python 'eve.inventory.InventoryMgr.MoveItems'` |
| `type` | A type of a host language, or of the language itself | `type csharp 'Studio.Ui.InventoryWindow'` |
| `table` | A table in the catalog | `table space.Ships` |
| `resource` | A host type that holds something outside memory: a file, socket, lock, connection or GPU buffer | `resource java 'java.sql.Connection'` |
| `interface` | A trait, interface, protocol or abstract class; the clauses bind every implementation | `interface rust 'edb::Procedure'` |
| `impl` | One implementation of an interface for a type; its clauses add to the interface's (§ 6.11) | `impl rust 'std::iter::Iterator' for 'std::str::Chars'` |
| `quantity` | A conserved or dimensioned quantity | `quantity Isk : decimal<precision=19, scale=2>` |
| `conserve` | A conservation law over columns | `conserve Isk across wallet.Balances.balance, …` |
| `failure` | A named failure and its class | `failure NotYours : user` |
| `predicate` | A named, reusable predicate | `predicate flies(s: space.Ships) = s.pilotID == actor` |
| `block` | A named sequence of preconditions | `block in_space(s: space.Ships)` |
| `label` | An information-flow label | `label pii` |
| `scope` | A named span of time that lifetimes can be stated against | `scope fight : from begin(Fight) to end(Fight)` |
| `region` | A callable, or a named part of one, and how it may execute (§ 6.23) | `region 'shop.checkout.hold_and_charge#hold_one_unit'` |

**Stable names.** A host subject binds by the stable name its code gives it, through the host's own attribute,
annotation or decorator (`#[stable("eve.cache.get")]` in Rust, `[Stable("…")]` in C#, `@stable("…")` in Python), or,
until it has one, by its path, with overloads told apart by their parameter types: `function csharp
'Studio.Shop.Checkout.Pay(System.Int64)'`. A stable name belongs to no host, so a subject named without a host
(`function 'eve.cache.get'`) binds whichever host implements it: one file constrains the Python method and the Rust
procedure that replaces it. A renamed symbol keeps its constraints and their evidence through `was 'old.path'`, which
the build checks against the history of the code. A lambda or closure has no name; it is constrained through the
parameter that receives it (`callback`, § 6.2).

### 3.3 Host profiles

Each host language has a profile, owned by the team that adopts the host, that says how the host's types map onto the
language's types (§ 3.4), which memory model it follows (§ 7.4), how its references map onto access and aliasing
(§ 8.5), which kinds its compiler or analyses prove, where a runtime check can be inserted, how a constraint is watched,
and how `strip` and `weave` treat its source (§ 11.2).

| Host | Memory model | Proven by the host | Runtime checks inserted at | Watched by |
| --- | --- | --- | --- | --- |
| Rust | Ownership, with counting inside `Rc` and `Arc` | Types, borrows, exclusivity, `Send` and `Sync`, no use after end in safe code | The procedure's head in the engine; generated wrappers elsewhere | Replay; Miri, leak checks and sanitizers in CI |
| T-SQL and SQL | Rows | `NOT NULL`, `CHECK`, `UNIQUE` and foreign keys where declared | Generated T-SQL at a procedure's head; the engine at each write | Replicas |
| Python (CPython) | Counting, with a cycle collector | Little; type hints where a checker runs | Decorators at entry and exit; read-only proxies | Effect manifests; tracing at the hot stores' interfaces and heap snapshots on test clusters |
| C#, Java, Kotlin, Go, JavaScript, TypeScript | Tracing | Types; nullability where enabled; `readonly`, `final` and `val`, shallowly; escape analysis where the compiler reports it | Entry and exit wrappers; analysers at build | Heap snapshots; weak-reference probes |
| Swift, Objective-C | Counting (ARC) | Types; `Sendable` and actor isolation; non-escaping closures | Entry and exit wrappers | Memory graph snapshots |
| C++ | Manual, with RAII; counting through `shared_ptr` | Types; `const`, shallowly | Generated wrappers | Sanitizers in CI |
| PHP | Counting, with a cycle collector | Declared types | Entry and exit wrappers | Heap snapshots on test servers |
| C | Manual | Types | Generated wrappers | Sanitizers in CI |

A profile is a contract too: the compiler for a host is tested against its profile on the corpora, and a kind a profile
cannot check is reported as such for every subject of that host, never silently skipped.

### 3.4 Types

- **Primitive:** `bool`; `int`, the mathematical integers, with `u8` to `u128` and `i8` to `i128` as named refinements
  of it; `decimal`, the exact decimals, constrained as in Y by `precision` and `scale` (`decimal<precision=19,
  scale=2>`; `decimal16<scale=s>` to `decimal256<scale=s>` name ClickHouse's precisions, Y § 3.3); `rational`, the
  exact fractions, constrained by `precision`; `f32` and `f64`; `string`; `bytes`; `time`, simulation time at a place in history; `instant`,
  wall-clock time, allowed only in watched constraints; `duration`; `id<entity_type=T>`, the identity of a row or entity of `T`.
- **Composite:** records `{ name: T, ... }`; tuples; variants `A | B(T)`, closed unless marked `open`; optional `T?`;
  collections and mappings (below); `ref<target_type=T>`, a reference to a host entity, used by the memory clauses;
  `ptr<target_type=T>`, a host pointer: the address of a `T`, with no guarantee that it is valid or about any other
  path (§ 8.1). The kinds of pointer between the two (valid, a cursor, an element, a C string, code) are refinements
  of `ptr<target_type=T>`, and `ref<target_type=T>` is the most refined of them (§ 7.8).
- **Collections and mappings.** A collection is one-dimensional and a mapping two-dimensional, as in Y (Y § 3.13).
  Lists, arrays, sets, bags, stacks, queues, linked lists and iterators are collections, and maps of every order are
  mappings, each with the constraints that say what it holds and how it may be used: `element_type`, or `key_type` and
  `value_type`; `ordered by position | insertion | value | key`, `unordered`, `unique`, `count fixed`, `count at most
  n`, `offers …`, `costs …`, `values many`. `list<element_type=T>`, `set<element_type=T>` and `map<key_type=K, value
  type=V>` are three of them. A host type maps onto one through its profile, by its constraints: Rust's `Vec<T>` onto
  `list<element_type=T>`, `BTreeSet<T>` onto `sorted_set<element_type=T>`, `HashMap<K, V>` onto `unordered_map<key
  type=K, value_type=V>`.
- **Type arguments are named,** as in Y (Y § 3.1): `ref<target_type=T>`, `list<element_type=T>`, `set<element_type=T,
  hasher_type=S>`, `map<key_type=K, value_type=V>`, and `hasher_type` where a host's map has one. A positional
  argument is refused at build. The names follow Y's convention: one that takes a type ends in `_type`, one that takes
  a value does not. The arguments of a host's own type (`Node<'a>`) are the host's, and stay positional.
- **Refinements:** `T where p`, in which `value` names the value being refined; shorthands `T in a..b` and `T in {a, b,
  c}`; named refinements: `type Quantity = i64 where value >= 0`. A refinement on an optional type applies to the value
  when it is present. `where` always filters: the values of a type here, the rows of a table in an expression (§ 4).
- **Units and quantities:** `unit m`, `unit s`; `quantity Distance : decimal in m`; arithmetic checks dimensions, and
  conversion between units is an explicit named function.
- **Generics:** a subject with type parameters is a template (§ 3.5). `for T where T: Comparable` binds a type
  parameter and its bounds; bounds name interfaces, so they are checked as behavioural subtyping (§ 6.10). `for 'a` binds a lifetime, and a host subject of any kind may bind both
  (§ 6.11). **Existential types:** `some I` is a value of one type, fixed by the subject, that implements `I`; `any I` is a
  value of any type that implements `I` (§ 6.11).
- **Representation.** A type in the language says which values are allowed. When a host needs a representation the
  profile does not choose by default, `as` names it: `map<key_type=Key, value_type=Entry> as 'std::collections::BTreeMap'`.

A host type maps onto these through its profile: T-SQL `INT` to `i32`, `NVARCHAR(50)` to `string where count(code_units(value, utf16))
<= 50`, C# `int?` to `i32?`, Rust `Option<u64>` to `u64?`, Python `int` to `int`.

### 3.5 Templates and realizations

A subject with type parameters is a **template**, not a type or a function. Writing it with arguments makes a
**realization**: `Vec<u8>` is a *realized type*, `parse<u32>` a *realized function*. A realization is a subject of its
own, made by putting each argument in place of its parameter everywhere in the template: its fields, its signature and
its clauses. It exists as soon as something uses it; nothing declares it. Two uses with the same arguments are the
same realization.

- **A realized type** is a type like any other: `Vec<u8>` has the fields and methods of `Vec` with `u8` for `T`.
- **A realized function** has the template's signature with the arguments in place: `parse<u32>` is
  `(self: ref<target_type=str>) -> Result<u32, ParseIntError>`. A method of a realized type is realized with it: `Vec<u8>::push`.
- **Bounds** say which arguments a template accepts: `T: Ord` is a condition for the realization to exist. A use that
  breaks it does not realize anything; it is an error.
- **Clauses.** A clause of the template holds of every realization, with the arguments in place. A clause can also be
  stated of one realization alone, where it promises more than the template does.
- **Names.** A realization can be named, and is named where a body uses it with arguments written out (§ 11.2):
  `type Bytes = Vec<u8>`, `realize parse_port = 'std::primitive::str::parse'<u16>`. A use whose arguments the host
  infers states no type, and needs no name. Since two uses with the same arguments are one realization, a name belongs
  to the program unit that holds them all (a Rust crate), not to the function that happens to use it.
- **Hosts.** How a host makes realizations is a fact of the host, not of the meaning. Rust and C++ generate code for each
  one when they build. C# makes them at run time. Java and TypeScript check each one and then erase them to one type.
  The meaning is the same in each: every realization is a subject.

A trait object or interface value (`any I`, § 6.11) is not a template: it is one value whose type is chosen at run
time, and nothing is realized for it.

### 3.6 Platforms: what every subject of a build may assume

**Meaning.** A platform is what a build fixes before any subject is compiled: the target's pointer width and byte
order, the target features the code may use, the widest atomic operation, the panic strategy, whether arithmetic checks
for overflow, whether debug assertions run, and the features the build turns on. Choosing a platform is not a
constraint. A `#[cfg(..)]` chooses which code exists, which is behaviour, and it stays in the body. What the choice
leaves is a set of facts that hold for every subject compiled under it. They are added to each subject's own
constraints, and every consumer may use them: a checker discharges checks with them, an optimizer removes the checks
they make redundant (the range check of `usize as u64` on a 64-bit target, a byte swap on a little-endian one, a lock
around an atomic the target has), and a translator chooses the target's representations from them.

```text
platform rust 'x86_64-unknown-linux-gnu'
    pointer width 64
    endian little
    target features fxsr, sse, sse2
    atomics up to 64
    code read only
    calls only code
    panic unwind
    build overflow_checks on, debug_assertions on
    features std, alloc

platform rust 'wasm32-unknown-unknown+simd128'
    pointer width 32
    endian little
    target features simd128
    atomics up to 64
    panic abort
    features alloc
```

- **A declaration, not a clause.** A platform names no subject: every subject built under it is under it, additively.
  A subject cannot weaken a platform fact, and a platform cannot add a requirement to one subject alone.
- **Facts come from the toolchain, not from people.** For Rust they are what `rustc --print cfg` and cargo's feature
  resolution give for the build. The file states them so that a reviewer sees what each build assumes, and the
  compiler compares the declaration with the toolchain at build: a fact the toolchain does not give fails the build. A
  wrong fact would let an optimizer remove a check that is needed, so a platform fact is never `assumed`.
- **A promise without condition holds on every platform** the subject is built for. A promise that holds only on some
  says so with `given build` (§ 6.12), which names a platform fact: `given build pointer width 64`, `given build
  overflow_checks on`.
- **Gated code holds under more.** Code that a `#[cfg(..)]` guards runs under the platform's facts and the guard's
  condition, which the build has decided: under `#[cfg(target_endian = "little")]` the code may rely on little endian
  on every platform that compiles it (§ 11.1.1).
- **A crate's platforms are its builds.** The builds a crate is checked in (bare Rust's `builds.json`: the default
  build, and those that compile its gated code) are its platforms, and every check runs under each of them.
- **Per host.** Each host profile (§ 3.3) lists the facts its builds fix. The JVM and .NET fix no pointer width and
  little else; C and C++ fix the most, through the target and the compiler's flags; T-SQL's facts are the server's
  version and its settings (`ANSI_NULLS`, the collation).

### 3.7 Profiles: what a build's traffic was measured to do

**Meaning.** A profile is to expectations (§ 6.20) what a platform is to facts: a declaration the toolchain adds to a
build, from measurement, that every subject compiled in it is under. It holds what recorded traffic showed: how often
each subject was called, how often each branch of a body went each way, the sizes collections had, which failures
occurred and how often, which calls were hot. The toolchain makes it from replays, test clusters or production samples
and hands it to the compiler as it hands it the target, so that one build of the code can be laid out, inlined and
specialised for the traffic it will see. A profile states nothing that must hold: a profile that is wrong, stale or
missing makes a build slower, never wrong.

```text
profile 'tranquility-2026-10' from replay 'tq-2026-10-14' on platform 'x86_64-unknown-linux-gnu'
    calls 'eve::dogma::Fitting::recalc_cpu'      per tick 1840
    branch 'eve::dogma::Fitting::recalc_cpu' @ 3  taken 0.02          -- the third branch of the body, by its stable position
    size 'eve::market::Book::orders'             p50 12, p99 210
    fails 'wallet.Credit' with InsufficientFunds 0.004
    access 'eve::market::Book::orders'          by orderID 0.92, by range price 0.07, scan 0.01
    index 'eve::dogma::Fitting::slots'          p50 3, p99 15, sequential 0.80
    lookup 'eve::cache::TypeCache::get'         hit 0.97
```

- **Access to collections** is measured as well: by which keys or fields each collection was looked up, how often by a
  range and how often scanned whole (`access`); the indices used and whether they ran in order (`index`); how often a
  lookup found what it looked for (`lookup … hit`). It is what decides a representation (a map, a sorted array, an
  array indexed by a dense key) and, for a table, which index a selection should use (§ 17, question 1).
- **Points in a body** are named by their stable position in the bare body (the subject and the ordinal of the branch,
  call or loop), the same positions the facts of § 11.1.1 are computed at. A position that no longer exists after an
  edit is dropped, not guessed.
- **A profile names its source and its platform**, and is never `enforced`. Its entries are measurements, and a person
  does not write them. A written expectation (§ 6.20) is compared with the profile of each build, and a profile far from
  what a subject expects is reported.
- **Per host.** Each host's profile-guided build takes it in its own form (LLVM's profile data for Rust and C++, the
  JIT's own counters for the JVM and .NET, which a profile can seed); a host with no such build ignores it.

---

## 4. Expressions, defined once

- **Absence is a value, and logic has two values.** Absence is `none`, a value of every optional type. `exists x` and `x
  is none` test for it; `x?.f` is `none` when `x` is. `x == v`, with `x` optional and `v` not, is false when `x` is
  absent. Every other operation on an optional value, ordering included, is a type error until it has been narrowed.
  SQL's `NULL` is `none`, and the T-SQL generated from a constraint tests `IS NULL` explicitly, so three-valued logic
  never decides a constraint.
- **Narrowing follows order.** A clause that has held narrows what follows it: after `require found : exists m`, `m` is
  present in every later clause of the subject; after `require is_ship : x is Ship(s)`, `s` is bound.
- **Integers and decimals are exact.** Integers in constraints are unbounded and decimals exact, so a constraint never
  overflows. `fits T` asks whether a value is representable in a host type. Overflow in the host is caught by stating
  the exact result: `ensure total : result == old(a) + old(b)` fails if the host wrapped. Division and remainder use
  Y's names (Y § 3.1), and `/` and `%` do not exist on integers or decimals. `divide_truncated(a, b)` and
  `remainder_truncated(a, b)` are what `/` and `%` do in Rust, C#, Java and T-SQL; `divide_floor` and `remainder_floor`
  are Python's `//` and `%`, and a check lifted from Python uses them; `divide_euclidean`, `remainder_euclidean` and
  `divide_exact` are also Y's. `/` divides rationals and floats. A division or remainder must be guarded by an earlier
  conjunct or clause that the divisor is not zero. Nothing rounds without naming one of Y's modes (Y § 3.3):
  `to_decimal_rounded(x, scale: 2, rounding.half_away_from_zero)` is T-SQL's `ROUND`, and a check lifted from T-SQL
  uses it; `divide_rounded(a, b, rounding.half_even)` divides decimals.
- **Floats compare as every host compares them.** `==` and the orderings follow IEEE 754 comparison: `-0.0 == 0.0`, and
  every comparison involving `NaN` is false except `!=`. `total_order(a, b)` exists for sorting. Arithmetic is IEEE 754
  binary32 or binary64, rounding to nearest with ties to even, with no fused multiply-add and no reassociation.
  Transcendental functions are named functions with one pinned implementation. `finite x` excludes `NaN` and the
  infinities, and is the usual refinement. `approx(a, b, abs: e, rel: r)` states a tolerance where the code being
  compared did not pin its arithmetic.
- **Strings are y-string's (Y § 3.3).** A `string` is a sequence of runes, each one extended grapheme cluster, which
  is what a person sees as one character; `count(s)` counts them. Its Unicode scalar values are `scalars(s)`, and a
  host's single character (Rust's `char`, a Java `int` code point) is a `scalar`. `count(code_units(s, utf16))` and
  `count(code_units(s, utf8))` count an encoding's code units, which is what `NVARCHAR(n)` and many host limits count.
  `==` compares scalars: a host's string is not normalised unless its profile says so, so `string<normalization=nfc>`
  states that it is, as every string Y makes is. `string<character_set=ascii>` holds ASCII only, so a rune is one
  scalar and one byte, and the three counts agree. Collation is a named function: `collate(x,
  'Latin1_General_CI_AS')`, which gives a key that compares as the collation does; and `sql_equals(a, b, collation)`,
  which also ignores trailing spaces as T-SQL's `=` does. A comparison lifted from T-SQL is written with them. `x
  matches /re/` uses linear-time regular expressions only.
- **Time:** `now` is the time of the request at its place in history and `tick` its tick; durations carry units: `50us`,
  `5 ticks`, `30 s`, `2 frames`.
- **Equality and identity:** `==` is structural on values; `same(a, b)` is identity of host references; rows compare by
  key; `p moved from q` is identity of a value that moved (§ 6.14.3).
- **Handles:** a handle is a value that stands for a place in a collection, not for what is stored there: an entry, a
  cursor. `current(h)` is the value at the place, read where the expression is evaluated (inside `old(…)`, at entry),
  so a handle that has been consumed still names its place; `same(current(h), v)` asks whether the place holds the very
  value `v`. A type is a handle when it borrows a collection (`hold : borrows`, § 7.2) and stands for one place of it; the views
  its subject declares read the place (§ 3.2): `current`, the value there, and `key`, its key. `owner(h)` is the collection the handle borrows, which its `hold : borrows` clause (§ 7.2) already ties it to;
  it has no host function, because the handle hides it, and the wrapper that made the handle checks it, since it knows
  the collection it made the handle from. `owner(h)` is also the root of a path: `owner(h)[k]` is the element at key
  `k`, and `mutates only owner(h)[k]` is the element frame of § 6.14.8. Both are refused at build for a value that is
  not a handle.
- **The key a map holds:** `m[k]` finds a value by `==` on keys, so it cannot tell which key entity the map holds.
  `stored_key(m, k)` is the key the map holds for the key equal to `k`, or `none`; `same(stored_key(m, k), x)` asks
  whether `x` is that very entity. It is pure and total, a view of the map and the key (Rust's `get_key_value`).
- **Tuples:** `e.0`, `e.1` are the parts of a tuple, and a pattern `(a, b)` names them, in `let` and after `is` (§ 6.14.7).
- **Collections:** `size`, `in`, `subset`, `distinct`, `sorted by`, `all x in c : p`, `any x in c : p`, `count`, `sum`.
  `min` and `max` return an optional value, absent for an empty collection. A `list<element_type=T>` has an order: `xs[i]`,
  `present(xs, i)`, `xs[i..j]`, `xs ++ ys`, `reverse`, `take`, `skip`, `zip`, `enumerate`, `select`, `while_true`,
  `flatten`, and `map` and `filter` by a named function; and `yields(it)` is the sequence an iterator gives (§ 6.15). In a `law` only, a quantifier may range over
  a type, `all x : T : p`, because laws are checked on generated values and never at run time.
- **Rows:** `T[key]` looks a row up and is optional; `T where p` selects rows. A selection in an enforced predicate must
  be served by an index, or the clause must be an invariant checked row by row as versions are written; the compiler
  states each clause's read set.
- **Context:** `self`, the row or entity a table's or type's clause is about, whose fields are also in scope by name;
  `actor`, the identity on whose behalf a request runs; `session`, `origin`, `request`, `now`, `tick`; in postconditions
  also `result` and `old(e)`, the value of `e` at entry. In temporal clauses `old(e)` is the previous version, and is
  absent at a row's first version.
- **Connectives:** `not`, `and`, `or`, `implies`, `iff`. `and`, `or` and `implies` guard their right side, and narrow it
  as an earlier clause would, so `b != 0 and divide_truncated(a, b) > 1` is well formed. `if c then a else b` is an expression with a
  value.
- **Functions:** a function used in a constraint is named in the catalog or declared with a reference implementation in
  a host. It must be total, deterministic and free of effects, which is itself checked as a bound (§ 6.3); its generated
  forms are compared like every other check.
- **`let`:** binds a name, or a tuple pattern `(a, b)`, to a value for the later clauses of its subject. A name is bound once.
- **Cost:** every predicate has a static cost: rows read by key, index ranges, and collection sizes bounded by argument
  shapes. Placement uses it (§ 10.3).

### 4.1 Predicates and properties

Clauses are of two sorts, and the difference decides how they are checked.

- **Predicates** are evaluated on a request, a write or a commit: types and shapes, preconditions, postconditions,
  invariants, uniqueness, references, state transitions, conservation, `always` and `never`, and rates. They obey
  principle 3: total, bounded, with a known read set. They can be enforced on the request path.
- **Properties** are statements about executions or the heap: bounds on effects, protocols, budgets, concurrency,
  information flow, lifetime, mutability, aliasing, laws, statistics and unbounded temporal clauses. Their quantifiers
  range over every execution or every reachable entity, so they are never evaluated as predicates on a request. They are
  proven by a host compiler or an analysis; checked by instrumentation on replays, test clusters and in CI; watched in
  production; or enforced at run time only where a host profile provides a cheap mechanism that decides them without
  searching (the engine's checks on column writes, a per-entity automaton for a protocol, a read-only proxy in Python, a
  generational handle).

§ 5 gives each family's sort.

---

## 5. The clauses at a glance

| Family | Clauses | Says | Sort | Section |
| --- | --- | --- | --- | --- |
| Values and shapes | parameter and result types, `field`, `type … where`, `quantity`, `unit`, `layout` | What values may cross a boundary, and how they are laid out | Predicate | 6.1 |
| Calls | `from`, `require … else`, `ensure`, `on failure`, `on cancel`, `fails … when`, `fails only`, `never fails runtime`, `exits only`, `result must be used`, `callback`, `law` | Who may call, what must hold before and after, how a call may fail | Predicate; laws are properties | 6.2 |
| Bounds on effects | `reads only`, `writes only`, `emits only`, `sends only`, `calls only`, `effects only`, `effects none`, `pure`, `no io`, `no device`, `devices only`, `no clock`, `no randomness`, `no suspension`, `deterministic`, `idempotent by`, `depends only on`, `terminates`, and `when` on any of them | What a call must not do | Property | 6.3 |
| State | `invariant`, `unique`, `reference … on end`, `acyclic`, `conserve`, `faucet`, `sink` | What must hold of stored state | Predicate | 6.4 |
| Protocols | `states`, `initial`, `protocol` | Which changes and calls may follow which | Predicate for `states`; property for `protocol` | 6.5 |
| Time and statistics | `always`, `never`, `after … within`, `before`, `since`, `until`, `eventually`, `rate`, `statistic` | What must hold over history | Predicate for `always`, `never` and `rate`; property otherwise | 6.6 |
| Budgets | `budget` | How much time, work, memory and traffic | Property | 6.7 |
| Concurrency | `confined to`, `guarded by`, `lock order`, `atomic`, `linearizable`, `no suspension while holding`, `no blocking in`, `not reentrant`, `cancellation safe`, `publishes` | Who may touch what, from where, in what order: rules a correct program keeps | Property | 6.8 |
| Information | `label`, `flows only to`, `never flows … to`, `declassify … when … because`, `sanitised by`, `time independent of` | Where information may go | Property | 6.9 |
| Compatibility and visibility | `version`, `compatible with`, `replaces`, `was`, `deprecated`, `stable`, `visible to` | What may change between versions, and who may refer to what | Checked at build | 6.10 |
| Interfaces and implementations | `implements`, `assoc`, `for`, `some`, `any`, `hold … borrows for` | Which interfaces a type implements, what an implementation adds, and what a host subject is parameterised by | Checked at build | 6.11 |
| Conditional promises | `given` | What a promise relies on: a bound's implementations, the default body, a build setting | Checked at build, as the promise is | 6.12 |
| Callbacks and calls | `callback … when / with / returning`, `callback … called by`, `fails … in`, `final(e)`, `protocol` over calls | When a function argument is called, with what, what it returned, in what order, and by the calls a result makes later | Checked by wrappers or in CI | 6.13 |
| Independence | `pure`, read sets (`reads only`), write sets (`mutates only`, `writes only`), `disjoint`, `commutes with`, `sendable`, `shareable` | What may run in parallel or in another order: facts an optimizer, scheduler or translator may use | Property | 6.16 |
| Sequences | `yields(it)`, `xs[i]`, `present`, `xs[i..j]`, `++`, `reverse`, `take`, `skip`, `zip`, `enumerate`, `select`, `while_true`, `flatten`, `map`, `filter` | What an iterator yields, and the order of a list's elements | Expressions: predicates when finite, properties for infinite sequences | 6.15 |
| Small additions | `evaluable at compile time`, `must be used`, `moved from`, `ends in call`, `survive call`, `except exhausted`, `expected`, `amortised`, `result.0`, `mutates only p[e]`, `obliges caller` | Compile-time evaluation, must-use types, moves, drops, exhaustion, expected cost, tuple parts, an element frame, what a caller must establish | As the section each extends | 6.14 |
| Lifetime | `hold`, `lifetime …` (including `cleared on end`, `usable after construct`, `reclaimed only after released`), pointer kinds (`valid`, `nonnull`, `cursor`, `element`, `aligned`, `terminated`, `data`, `code`), `instances …`, `no strong cycle`, `model` | How long entities live and who keeps them | Property | 7 |
| Mutability and aliasing | `access`, `readonly`, `immutable`, `frozen after`, `write once`, `append only`, `monotonic`, `mutates only`, `interior`, `unchanged`, `exclusive`, `sole`, `disjoint`, `shared readonly` | Who may change what, and through how many paths | Property; predicate for columns | 8 |
| Derived languages | `local` | A type a woven host needs inside a body | Checked at weave | 11.2 |
| Status and placement | `assume … because`, `pin … because`, `not at` | What is taken on trust, what a person fixed, where a clause may not run | — | 10 |
| Freedoms | `order unspecified`, `may reassociate within`, `result any of`, `may batch`, `may reorder` | What may vary between runs, builds and hosts, and so is no one's to rely on | Checked by running callers under each choice | 6.17 |
| Algebraic properties | `associative`, `commutative`, `identity`, `absorbing`, `idempotent`, `involutive`, `inverse of`, `distributes over`, `monotone`, `injective`, `equivalent to` | Laws with fixed names, which tools recognise | Property, checked as laws are | 6.18 |
| Collection contents | `sorted`, `distinct`, `dense`, `heap ordered`, `nonempty` | What holds of a collection's elements | Predicates, in any expression | 6.19 |
| Expectations | `expect` | What usually happens: sizes, frequencies, likelihoods, rare failures; never correctness | Watched only | 6.20 |
| Platforms | `platform`, with `pointer width`, `endian`, `target features`, `atomics up to`, `panic`, `build`, `features` | What every subject of a build may assume | Checked at build, against the toolchain | 3.6 |
| Levels | `level` on a module: `data`, `calculations`, `structured`, `shared`, `effects` | How much of its language a module may use | Checked at build | 6.22 |
| Regions | `region`, with `within`, `forever because`, `uncancellable`, `atomic`, `isolation`, `may run again`, `waits until`, `on failure unchanged` | How a callable, or a named part of one, may execute | Property | 6.23 |
| Profiles | `profile`, with `calls`, `branch`, `size`, `fails`, `access`, `index`, `lookup` | What a build's traffic was measured to do | Measured; never enforced | 3.7 |

---

## 6. The kinds of constraint

Each kind opens with the paragraph that is its meaning, then its syntax, how it fails, how it is checked and how it is
harvested.

### 6.1 Values and shapes

**Meaning.** A value that crosses a boundary has a type, a shape and a range: its width and signedness, whether it may
be absent, the set of values it may take, the size and order of a collection, the format and length of a text, the
relations between the fields of a record, and the unit of a quantity. A value outside its type is rejected at the
boundary before the subject runs. A type's fields have types of their own, and where a boundary with foreign code needs
it, a layout.

```text
failure TooLarge : user

method inventory.MoveItems(itemIDs: list<element_type=u64> where count(value) in 1..1000 and distinct(value),
                           toLocationID: u64,
                           flag: u8 in {4, 5, 62, 133},
                           quantity: i32? where value > 0)
    -> list<{ itemID: u64, quantity: i32 where value > 0 }>
    require volume_fits : exists quantity implies quantity * inv.unitVolume(itemIDs) fits i64 else TooLarge

type CycleWindow = { started: time, ends: time } where started <= ends
type ShipName    = string where count(scalars(value)) in 1..32 and value matches /^[\p{L}\p{N} ._'-]+$/
quantity Distance : decimal in m

type rust 'eve_net::wire::Header'
    field magic   : u32
    field length  : u32 where value <= 65535
    layout c
```

- **Fails** as a runtime failure at the boundary, with the parameter, the clause and the offending value, redacted by
  its labels (§ 6.9).
- **Checked** at the proxy, from which the RPC schema (protobuf or a format like it) is generated with encoders and
  decoders for both ends; by the host's types where they suffice, which in a woven host they do by construction
  (§ 11.2); by generated wrappers elsewhere.
- **Harvested** from the effect manifests at entry and exit: the types, shapes, ranges and sets of values each method
  was called with and returned.
- `layout c`, `layout packed`, `layout transparent`, `size n` and `align n` state what an ABI needs, and are proven by
  the host's compiler where it can check them.

### 6.2 Calls: authority, preconditions, postconditions, failures, callbacks and laws

**Meaning.** A call may be made only by the origins it names. Before its body runs, its preconditions hold, in order, or
it fails with the failure of the first that does not, having changed nothing. After it returns, its postconditions hold
of its result, its inputs and the world, or the subject has a bug; after it fails, what it promised on failure holds. It
fails in the ways its failures name, each in one of two classes: user failures, shown to the player, and runtime
failures, which clients suppress. A function it receives as an argument is called as it says, and satisfies what it
says.

```text
failure NotDocked         : user
failure InsufficientFunds : user
failure OutOfRange        : user { distance: Distance, range: Distance }

procedure market.PlaceOrder(typeID: u32, price: decimal<precision=19, scale=2> where value > 0, quantity: i32 where value > 0)
    -> id<entity_type=market.Orders>
    from Player
    require docked      : exists session.stationID                          else NotDocked
    let w = wallet.Balances[actor]
    require has_wallet  : exists w
    require can_escrow  : w.balance >= price * quantity                     else InsufficientFunds
    ensure  escrowed    : wallet.Balances[actor].balance == old(w.balance) - price * quantity
    ensure  listed      : market.Orders[result].quantity == quantity
    on failure unchanged
    fails only NotDocked, InsufficientFunds
    result must be used

function kotlin 'studio.ui.withLock'(lock: ref<target_type=Lock>, body: ref<target_type=Function0>)
    callback body : called exactly once before return

function rust 'eve_net::frame::encode'(f: Frame) -> bytes
    effects none
function rust 'eve_net::frame::decode'(b: bytes) -> Frame?
    effects none
    law round_trip : all f : Frame : decode(encode(f)) == f
```

- `from` names origins: `Player`, `Gm`, `Timer`, `Reaction`, `Shard`, `Sql`, `External`, `Contract`, or a host's roles.
- `require name : p else F { field: e, … }` rejects with `F`, filling its payload from expressions the clause can see.
  Without `else`, the failure is a runtime failure named after the clause.
- `ensure name : p` may use `result` and `old(e)`. A false postcondition is a bug in the subject: the call fails as a
  runtime failure and its transaction rolls back, and a harvested postcondition is watched before it is enforced.
- `on failure unchanged` is the strong guarantee: a call that fails has changed nothing a caller can see. Procedures in
  the engine have it by construction; host functions state it. `on failure F ensure name : p` states what holds after
  failing with `F`, as JML's exceptional postconditions do.
- `fails F when p` says the call fails with `F` whenever `p` held at entry. `fails only` lists the user failures a call
  may raise; marked `open`, the list may grow without a new version. Runtime failures are always possible, because an
  enforced constraint may reject, unless the subject says `never fails runtime`, the strongest form, whose name for
  hosts with panics is `never panics`. `never panics except exhausted` leaves open the one failure of running out of what the platform gives
  (§ 6.14.5). A promise that holds only given what an implementation or the build does is qualified by
  `given` (§ 6.12).
- Together these state a part of the subject's **exits** (§ 6.21), which every callable has, written or not.
- `result must be used` binds the callers: a call whose result is discarded is a build error where the host can see it,
  and a lint elsewhere. A type can say `must be used` for every subject that returns it (§ 6.14.2).
- `callback c : called exactly once | at most once | at least once [before return]` states how a function argument is
  called, as Kotlin's `callsInPlace` does; `callback c : conforms to interface rust 'eve_ui::Handler'` applies an
  interface's clauses to whatever is passed. A callback may carry a condition, name its arguments and bind its result
  for the later clauses (§ 6.13).
- `law` states an algebraic property over one or more subjects: round trips, inverses, idempotence of a pure function,
  associativity of a merge. It quantifies over a type, so it is checked in CI on generated values and on the recorded
  corpora, never at run time.
- **Checked:** the leading preconditions that need only the call and the session at the proxy, which the server may then
  assume; the rest in the engine before the body, as predicate reads; in the Python servers by decorators; at the head
  of a T-SQL procedure while it still runs on SQL Server. Postconditions in the engine at commit, and on replicas when
  watched.
- **Harvested:** preconditions are lifted from guard clauses and `IF … THROW`; failures from manifests at exit;
  postconditions from relations between inputs, outputs and row changes in the recordings.

### 6.3 Bounds on effects: what a call must not do

**Meaning.** A bound limits a subject's effects. It never lists what the subject does: that is the effect manifest's
job, and the manifest stays a record. A bound is a rule about what the subject may not do beyond a limit: the rows it
may read or write, the events it may emit, the calls and messages it may send, whether it may perform input and output,
read a clock or a random source, suspend, or depend on anything but the inputs it names. A bound is worth writing when
it is a rule someone would defend ("nothing outside the economy writes wallet balances"), not when it restates what one
body happens to do today. A subject that exceeds its bound has a bug, whether or not anything else went wrong.

```text
interface rust 'edb::Procedure'
    effects only tx                -- everything goes through the transaction it was given
    no io
    no clock                       -- `now` comes from the request, never from a clock
    no randomness except rolls
    deterministic

function 'eve.dogma.stacking.penalised'(values: list<element_type=f64>) -> f64
    effects none
    depends only on values
    terminates

procedure wallet.Credit(requestID: id<entity_type=Requests>, amount: decimal<precision=19, scale=2>)
    idempotent by requestID        -- the same request twice changes the world once

-- economy/bounds.constraints: a rule, not a description
interface rust 'eve_market::Procedure'
    writes only market.*, wallet.Escrow

function rust 'eve::fitting::apply'(ship: ref<target_type=Ship>, fit: ref<target_type=Fit>)
    reads only ship.modules, ship.cargo, fit
    mutates only ship.modules              when not docked(ship)
    mutates only ship.modules, ship.cargo  when docked(ship)    -- docked, the unfitted modules go to the cargo

function rust 'eve::dogma::falloff'(range: f64, optimal: f64, falloff: f64) -> f64
    pure
```

- `effects none` is purity: no reads of mutable state, no writes, input and output, clock or randomness. Allocating what
  it returns is not an effect.
- **A device access is an effect.** A read or write of a device's memory (a memory-mapped register, a buffer a device
  reads or writes by DMA) or of an I/O port is an effect of its own kind, `device`, as input and output are. Each one
  happens as the body writes it: none is removed, merged, repeated or moved past another device access, and a read is
  never taken to give back what was last written. `devices only p` bounds which devices a subject touches, and `no
  device` forbids them. In Rust these are `read_volatile` and `write_volatile` on a device's address; in C and C++, an
  access through a `volatile` lvalue to such an address. A `volatile` used for something else (a variable a signal
  handler shares) is concurrency (§ 6.8), not a device.
- `pure` is `effects none`, `depends only on` every parameter, and `deterministic`, in one word: the subject reads only
  its inputs, writes none of them, has no other effect, and gives the same result for the same inputs. It adds no
  meaning, but it is the word optimizers and translators look for: a pure call may be computed once, moved, left out
  when its result is unused, and run in parallel with anything (§ 6.16).
- `reads only p` and `writes only p` take rows of tables (`market.*`, `wallet.Escrow`) or paths in memory reachable
  from the inputs (`ship.fit`, `self.items[k]`). With `mutates only` (§ 8.2), the in-memory form of `writes only`, they
  are the subject's **read set** and **write set**: everything it may read or write beyond its own locals and what it
  allocates. A subject whose sets are stated says nothing of any other path, which is what makes the sets usable by
  someone who cannot see the body (§ 6.16).
- **`when c`** makes any bound conditional on its inputs: `mutates only ship.cargo when docked(ship)`. The bound holds
  in every call in which `c` held at entry. `c` is an expression over the inputs, as a precondition is, never over a
  local (principle 5). Several bounds hold together, conditional or not, so a bound for one case is written with the
  condition of that case, as below, and an unconditional bound limits every case. A call in which no condition holds is
  bounded by the unconditional bounds alone. A branch in the body on the inputs is what makes such a bound true:
  `when` is that branch seen from the boundary, and it is how a subject that writes different things in different
  cases states a write set for each.
- `effects only p` says every effect goes through the named path, as the engine's sealed interface requires of a Rust
  procedure.
- `deterministic` says the same inputs and the same snapshot give the same result, writes and failures. `no randomness
  except rolls` permits the engine's keyed rolls, which are deterministic given the log.
- `no suspension` forbids `await`, `yield` and tasklet switches between entry and exit.
- `commutes with` says two subjects' effects may be applied in either order with the same outcome, which is what lets
  the engine treat them as reductions. It is one of the facts of independence (§ 6.16).
- `depends only on` is the information-flow form of a bound (as SPARK's `Depends`): the result is a function of the
  named inputs alone.
- `evaluable at compile time` says the body may run in a constant context (§ 6.14.1).
- `terminates` says every call returns or fails. It is proven where the host can prove it and otherwise checked as a
  budget. A bound that holds only given what an implementation does carries a `given` (§ 6.12).
- **Checked:** statically where the bound is reachable: the DAL procedures and MachoNet calls a Python method can reach,
  the sealed interface of a Rust procedure, a pure function's call graph; on replays and in production, by comparing
  each manifest with the bounds that apply to it. Where no bound has been written, nothing is compared: a manifest alone
  constrains nothing.
- **Harvested:** candidates come from the manifests, as the union of what was seen; a person, or a soak window for a
  bound a person has asked for, turns a candidate into a rule.

### 6.4 State: invariants, uniqueness, references and conservation

**Meaning.** Stored state obeys rules whenever anyone can see it: a row's invariants hold of every version written, or
of every version committed where a transaction may pass through a state that breaks them; uniqueness holds within a
shard or across the universe; a reference names a row that exists, in one of the tables it may name, or its rule for
when the target ends says otherwise; containment has no cycles; and a conserved quantity is neither created nor
destroyed except by a declared faucet or sink. A transaction that would break any of them fails before anyone reads its
result. A table's key and its placement in shards are declared with the table in Rust and are not restated here.

```text
table space.Ships
    invariant destroyed_means_no_structure : not defenses.destroyed or defenses.structure == 0
    invariant capacitor_non_negative       : capacitor.capacitor >= 0
    invariant capacitor_within_capacity    : capacitor.capacitor <= dogma.capacity(self)

table inv.Items
    unique singleton_blueprint : (typeID, locationID) where singleton and flag == Blueprint per shard
    reference locationID -> one of inv.Locations, space.Ships, chr.Characters on end restrict
    reference ownerID    -> chr.Characters on end dangle
    acyclic containment : via locationID
    invariant same_system : solarSystemID == inv.outermost(self).solarSystemID holds at commit

type kotlin 'studio.match.Score'
    invariant ordered : home >= 0 and away >= 0 holds between calls

-- economy.constraints
conserve Isk across wallet.Balances.balance, market.Orders.escrow, contract.Contracts.collateral, chain.Escrow.balance
conserve ItemQuantity across inv.Items.quantity

procedure bounty.Pay(characterID: u64, amount: decimal<precision=19, scale=2>)
    faucet Isk at most bounty.cap(characterID, now)
```

- `invariant` on a table holds `at write` by default: at every version written, reductions included. `holds at commit`
  defers it to commit, as SQL's deferrable constraints do, for a transaction that must pass through a state that breaks
  it. On a host type it holds `between calls` (at every public boundary, as Eiffel's class invariants do) by default, or
  `always` where the host can check field writes.
- `unique … per shard` is the default and is enforced by the shard; `unique … global` needs a registry, and is watched
  until one exists.
- `reference f -> T on end r` states what happens to this row when the row it names ends: `cascade` ends this row with
  it, `restrict` refuses to end the target while this row names it, `nullify` makes the reference absent, and `dangle`
  keeps an identity that may no longer resolve. `one of T1, T2, …` names a reference that may point into any of several
  tables, as EVE's `locationID` does. These are the row model's ways of holding (§ 7.2), seen from the referring row.
- `acyclic` is the containment rule harvested from backups: no item contains itself.
- `conserve`, `faucet` and `sink` are as in the proposal (§ 6.9): in every transaction the deltas of a conserved
  quantity's columns, plus what transfers and requests carry, sum to zero, unless the procedure declares itself a faucet
  or a sink, with an optional bound.
- **Checked:** by the engine at each write and at commit; by SQL Server for the constraints it already declares; on
  replicas, as watched constraints, while a table is still on SQL Server.
- **Harvested:** row invariants from historical backups from the first day; candidate conservation laws and each
  procedure's role as faucet or sink from the manifests and the log, for people to decide (principle 9).

### 6.5 Protocols and state machines

**Meaning.** Some state changes only along declared paths, some of them only when a condition holds: a module's cycle
goes from idle to active only while the ship can pay, and never from cooling to active. Some objects accept calls only
in a declared order: a stream is begun, used, then finished or reset, and nothing follows. A state machine constrains
the values a field takes over time; a protocol constrains the order of calls on an entity. A change or call off the
declared path is rejected, or reported when watched.

```text
table space.Modules
    initial cycle : Idle
    states cycle : Idle -> Active when ship.capacitor >= activationCost,
                   Active -> Cooling | Idle,
                   Cooling -> Idle

table chr.Characters
    states location : Docked -> Undocking, Undocking -> InSpace, InSpace -> Warping | Jumping | Docking,
                      Warping -> InSpace, Jumping -> InSpace, Docking -> Docked

resource rust 'eve_net::QuicStream'
    protocol begin (send | receive)* (finish | reset)
```

- A `states` clause is a predicate, checked at each write of the field against the version it replaces; a transition's
  `when` is evaluated on the version being written.
- A `protocol` is a regular expression over the methods it names, plus `begin` and `end` of the entity; calls it does
  not name are unconstrained. It is checked per entity, by a small automaton inserted by wrappers, or proven in Rust by
  the typestate pattern, one type per state, or watched from the manifests. A protocol whose next step depends on data,
  beyond what `when` on a state machine can say, is stated as a state machine over a field that records the state.
  A subject may also state a protocol over its own calls, to its callbacks and to methods of its parameters (§ 6.13).
- **Harvested:** state machines per entity from the manifests and the log, in simulation time, separating the game's
  hidden states from the old mechanism's (a ship in neither system was the mechanism's).

### 6.6 Order in time, and statistics

**Meaning.** Some rules are about history rather than any one state: a value never decreases, a ship in transit is in a
system again within a few ticks, a session sends no more than so many calls a second, a loot table pays out at its
declared rate. A temporal constraint is a statement over the log, bounded in time; a statistic is a statement over many
samples, and is only ever watched.

```text
table chr.Skills
    never points_decrease : exists old(points) and points < old(points)

table space.Transits
    after begin(self) : exists space.Ships[shipID].solarSystemID within 3 ticks

method space.CmdWarpTo(targetID: u64)
    rate per session <= 10 per 1 s

procedure loot.Roll(tableID: u32)
    let d = loot.declared(tableID)
    statistic drop_rate : rate(result.dropped) within 0.95 * d .. 1.05 * d over 100000 samples
```

- `always p` and `never p` are predicates over successive versions. Because a violation is caused by a write, they are
  enforced at that write like any invariant.
- `after e : p within d` requires `p` to hold at some point within `d` after event `e`; `before`, `since` and `until`
  take the same bounds. `eventually name : p`, without a bound, may only be watched.
- `rate` bounds how often something happens per scope. A learned limit is priced by cost and enforced at the proxy.
- `statistic` compares an estimator (`rate`, `mean`, `quantile(q, …)`) over samples with an interval and raises an alarm
  when it leaves it; it never rejects anything.
- **Checked:** `always`, `never` and `rate` as predicates; the rest on replicas and the temporal tier, at no cost to the
  authority. A `within` constraint becomes enforceable only where a timer can act at its deadline (§ 17).
- **Harvested:** from the log in simulation time. Order seen in one Python stream is mostly accident, so an order
  constraint is a candidate only when a read of what the other wrote, or a person, explains it.

### 6.7 Budgets

**Meaning.** A budget bounds what a subject costs: time per call at a percentile, work and how it grows with input,
round trips, bytes sent, memory allocated or retained, share of a tick. Budgets are stated as "no worse than" per shard
class or host class. A budget never fails a request; it fails a build, raises an alarm or moves the scheduler.

```text
procedure space.ActivateModule(moduleID: u64, targetID: u64)
    budget p99 <= 50us on SolarSystem.hot
    budget work O(1)
    budget round_trips <= 0

method inventory.MoveItems(itemIDs: list<element_type=u64>, toLocationID: u64)
    budget work O(count(itemIDs))
    budget bytes sent <= 256 * count(itemIDs)

type csharp 'Studio.Ui.InventoryWindow'
    budget retains at most 2 MB per instance
    budget allocates at most 0 B per frame        -- after the first frame
```

- `budget work expected O(f)` bounds the mean over the subject's random choices, and `amortised` the average over a
  sequence of calls (§ 6.14.6).
- **Checked:** on the recorded corpora and fights in CI, against the previous release; in production from the router's
  timings and the manifests.
- **Harvested:** from timings and manifests, with how cost grows with input fitted from the recordings.

### 6.8 Concurrency

**Meaning.** State touched by more than one thread, task or process obeys declared rules: an entity is confined to one
execution context, or it may be moved between them, or shared among them; a field is touched only while its lock is
held; locks are taken in one order; a sequence of steps appears indivisible; nothing blocks or suspends where that is
forbidden; a function is not re-entered while it runs, or survives being cancelled at any suspension; and what one
thread publishes, another sees whole. A breach is a race, and a race is a bug whether or not it was observed.

```text
type csharp 'Studio.Ui.InventoryWindow'
    confined to thread ui

type rust 'eve_client::scene::Mesh'
    sendable                       -- may move to another thread
    shareable                      -- may be read from several at once

type java 'studio.match.Lobby'
    guarded by self.lock : members, state
    lock order Lobby.lock before Match.lock
    no suspension while holding self.lock
    publishes ready                -- writes made before `ready` is set are visible to whoever reads it set

function python 'eve.market.MarketMgr.PlaceOrder'
    atomic                         -- no other request observes it half done
    no blocking in tasklet
    not reentrant

function rust 'eve_client::net::Session::reconnect'
    cancellation safe              -- dropping the future at any await leaves the session usable
                                   -- (a cancellation is one of the call's exits, § 6.21)
```

- In the engine, requests run in a fixed order and procedures cannot share mutable state, so most of these are proven by
  construction for procedures; they matter for the Python servers while they remain, for clients, and for services in
  other hosts.
- `confined to` takes a context: `thread [name]`, `task`, `isolation name` (a Swift actor, an Erlang process, any unit
  that owns its state alone), `shard` or `process`.
- `sendable` and `shareable` correspond to Rust's `Send` and `Sync` and Swift's `Sendable`, and are defined by access
  (§ 8): a shareable entity is immutable while shared, or every write to it is guarded. They are facts of independence
  and are listed there (§ 6.16); the clauses of this section are rules that a correct program keeps whether or not
  anything runs in parallel, and a breach of one is a bug.
- `atomic` says no other execution observes an intermediate state of the subject; `linearizable` is its watched form for
  concurrent data structures.
- `publishes f` is the release-and-acquire rule that `volatile`, atomics with ordering and safe publication express in
  hosts: everything written before `f` is set is visible to any thread that observes `f` set.
- **Checked:** by host compilers where they can (Rust, Swift, Clang's thread-safety analysis); by analysers, runtime
  lock-order checkers and race detectors in CI; watched in production from the manifests.

### 6.9 Information flow

**Meaning.** Information carries labels, and a label says where it may go: what is personal stays off published ledgers
until encrypted, a server's secrets never reach a player or a chain, what a pilot may not see never reaches that pilot's
client, input from a client is untrusted until checked. A flow from a labelled source to a sink the label does not
allow, directly or through a result, a failure or a log line, is a breach.

```text
label pii
label secret(server)
label hidden(owner)
label untrusted
label key

table accounts.Users
    label email, birthDate, lastAddress : pii
    never flows pii to ledger, chain, analytics unless encrypted
    lifetime rows at most 30 days after closedAt

table rng.PeriodSecrets
    label secret : secret(server)
    flows secret(server) only to server
    declassify secret to player, chain when now >= periodEnd because "rolls are verified after the period ends"

table space.Ships
    label cloaked : hidden(ownerID)
    label position : hidden(ownerID) when cloaked

method chat.Send(channelID: u64, message: string)
    label message : untrusted
    sanitised by chat.escape before log, client

function rust 'eve_net::auth::tags_equal'(a: bytes, b: bytes) -> bool
    label a, b : key
    time independent of a, b
```

- Sinks are `client`, `player(x)`, `server`, `ledger`, `chain`, `log`, `analytics`, `external` and `shard(x)`.
- A failure is a flow. A precondition that reads hidden information may fail only with a failure no more revealing than
  its labels allow, and the compiler checks this: a `require` that reveals whether a cloaked ship is in range is a
  breach even if the body never runs.
- **Timing is a flow, where a subject says so.** `time independent of p, q` says that how long the subject takes, and
  which memory addresses it touches in what order, do not depend on the values of the labelled inputs `p` and `q`
  (their sizes may matter). It is how a comparison of a secret, a cipher, or a check that must not reveal how far it
  got before it failed is written. It is stated only of subjects whose inputs carry a label, and only where someone
  defends it: everything else may take as long as it takes. An optimizer that keeps it does not add a branch on, an
  early exit at, or a table lookup indexed by those values.
- Labels also decide placement: a constraint may run only where everything it reads may flow (§ 10.3). A constraint that
  reads another pilot's hidden state can never run on a client.
- `declassify` is the one way a label is relaxed, and it always carries a condition and a reason.
- **Checked:** statically through the read sets of predicates and the bounds of subjects; at the proxies' filters, which
  apply visibility as policy; at the stream boundaries where server secrets are stripped; watched in CI by tracing
  labelled values through replays. `time independent of` is checked on the compiled code, for the platforms of the
  build (§ 3.6), by a constant-time analysis where the host has one, and by timing the subject on generated inputs that
  differ only in the labelled values; it is `enforced` only where the analysis runs, and watched elsewhere.

### 6.10 Compatibility and visibility

**Meaning.** A subject's constraints are part of its contract, so changing them can break callers, dependants and stored
data. A constraint change that would break any of them is either refused, or shipped in two releases (expand, then
contract), or carried by a new version that runs beside the old one. Visibility says who may refer to a subject at all.

```text
procedure space.ActivateModule(moduleID: u64, targetID: u64)
    version 148
    compatible with 147
    replaces python 'space.modules.ActivateModule'
    was 'dogma.ActivateModule'

type rust 'eve_net::Message'
    stable                       -- its encoded form may not change
    visible to crate

variant market.OrderKind = Buy | Sell | Contract open
```

| Change | Breaks | Allowed as |
| --- | --- | --- |
| Strengthen a precondition with a user failure, or add a user failure | Callers | A new version, or a failure list marked `open` |
| Enforce a watched constraint, rejecting as a runtime failure | Nobody, unless the subject says `never fails runtime` | Freely, after its soak window |
| Weaken a postcondition, a bound, a lifetime or an access guarantee | Dependants | A new version |
| Strengthen an invariant | Stored rows | Watch, backfill, then enforce: expand, then contract |
| Narrow a parameter's type, or narrow visibility | Callers | A new version |
| Widen a result's type, or add a variant to a closed variant | Dependants | A new version, unless the variant is `open` |
| Return a borrow that lives shorter, or drop `readonly` from a parameter | Dependants and callers | A new version |
| Weaken a precondition, strengthen a postcondition or a bound, widen visibility | Nobody | Freely |

Runtime failures are open by design: any enforced constraint may reject. So moving a harvested constraint from watched
to enforced changes no contract and needs no person, as the proposal's soak window requires. An implementation of an
`interface` obeys behavioural subtyping: its preconditions are no stronger than the interface's, and its postconditions,
bounds, lifetimes and access no weaker (§ 6.11 says how an implementation is stated). `visible to public | crate | module | m` states who may refer to a subject, as
access modifiers do. `edb review` classifies every constraint change in a pull request by this table.

### 6.11 Interfaces and their implementations

**Meaning.** The clauses of an `interface` bind every implementation (§ 9), and an implementation may promise more than
the interface does (§ 6.10). Four things are needed to say that, and the language so far has none of them. A type says
which interfaces it implements, and what each associated type is in it. An implementation is a subject, whose clauses
add to the interface's. A value can be of a type that is only known by its interface: one type that the subject fixes
(`some I`, Rust's `impl Trait`), or any type that implements it (`any I`, Rust's `dyn Trait`). And a host subject can
take parameters: lifetimes, which a type that holds a borrow cannot do without, and types with bounds, which are
interfaces.

```text
type rust 'std::str::Chars'
    for 'a                                          -- a lifetime parameter, as `for T where T: Bound` is a type one
    hold : borrows for 'a                           -- the type holds a borrow; a use says from what
    implements 'std::iter::Iterator' { Item = scalar as 'char' }, 'std::clone::Clone'

impl rust 'std::iter::Iterator' for 'std::str::Chars'
    for 'a
    assoc Item = scalar as 'char'
    function next(self: ref<target_type=Chars<'a>>) -> (scalar as 'char')?
        access self : write exclusive
    effects none                                    -- beyond what Iterator says of every implementation

impl rust 'std::convert::From<ref<target_type=string>>' for 'std::string::String'    -- the interface's arguments choose the impl
    function from(value: ref<target_type=string>) -> string as 'std::string::String'

function rust 'eve_cache::Cache::keys'(self: ref<target_type=Cache>) -> some 'std::iter::Iterator' { Item = ref<target_type=Key> }
    hold result : borrows from self

function rust 'eve_cache::Cache::extend'(self: ref<target_type=Cache>, items: some 'std::iter::IntoIterator' { Item = T })
    for T where T: Clone
    access self : write exclusive
```

- **`implements I { Assoc = T }`** is a clause of a `type`: it lists the interfaces the type implements, each with its
  arguments and the associated types it fixes. An associated type is a name an interface leaves open and every
  implementation fills; it is used in the interface's own signatures as `Self::Item`, and in a bound as `T::Item`.
  `implements` lists the implementations that exist; it does not say what they promise, which is the impl's.
- **`impl I for T`** is a subject: one implementation of one interface for one type. It carries `for` parameters and
  `where` bounds (the same as the type's, or fewer, as in `impl<'a, T: Clone> Clone for Wrap<'a, T>`), the associated types
  `assoc Name = type`, the methods it implements, and clauses. A call that resolves to the implementation has the
  interface's clauses and then the impl's, and a call through a bound or an `any` has the interface's alone. An impl's
  clauses obey behavioural subtyping (§ 6.10): preconditions no stronger than the interface's, postconditions, bounds,
  lifetimes and access no weaker. A method is named after its impl, `Chars as Iterator::next`. A type's `implements`
  entry may name the impl subject that carries it (`impl`), and must when the implementation has clauses.
- **`some I { Assoc = T } + J`** is the type of a value of one type that the subject fixes and the caller cannot name, and
  that implements `I` and `J`. As a parameter it is a type the caller picks (a generic that is not named); as a result it
  is a type the callee picks, and a caller may rely on its interfaces and on nothing more. `Fn(A) -> R` is an interface
  with call syntax. A reference inside the bounds is shared and unnamed, and a result that has one borrows, so that
  `hold result : borrows from self` says from what.
- **`any I`** is the type of a value of any type that implements `I`, with dispatch through the interface: it is the
  referent, and is reached through a reference or a host pointer, as an unsized list is (§ 3.4): `ref<target_type=any I>`, `any I as
  'std::boxed::Box'`. Only the interface's clauses hold of it, never an impl's.
- **`for 'a`, `for T where T: Bound`** bind parameters on any host subject: a type, a function, an impl. Bounds are
  interfaces, so they are checked as behavioural subtyping (§ 6.10). A host type is applied to lifetimes as it is to types:
  `Chars<'a>`, `Node<'_>`. **`hold : borrows for 'a`** is the clause of a type that holds a borrow, one for each lifetime it
  holds; it is the lifetime counterpart of `hold f : borrows from p` on a parameter (§ 7.2), with no path because the
  value is the holder.
- **Fails:** as a build, not at run time. A type that does not implement what it says, an associated type that is not
  the real one, an impl whose clauses break behavioural subtyping, a use of an `any` that relies on what only an impl says,
  and a bound that an argument does not meet are refused at compile or review time, and `edb review` sorts the changes:
  adding an `implements` entry or an impl is free; removing one, or changing an associated type, breaks every dependant
  and needs a new version (§ 6.10).
- **Checked:** by the host's compiler after weaving, which holds the bounds, the impls, the associated types and the
  lifetimes (for Rust, `rustc`); for a library file, by compiling each entry against the real item: an impl as `X:
  Trait<Assoc = T>`, a method as `<X as Trait>::name` with the impl's parameters, a `some` parameter as a generic with its
  bound, a `some` result as a bound that the real result type must meet. The compiler of the language checks an impl's
  clauses against the interface's wherever both are visible (§ 11.1).
- **Harvested:** from the host's source and its compiler. `impl I for T` blocks give impl subjects and, for a type of the
  same file, its `implements` entry with the associated types of the block's `type Name = …;` items; the signatures give
  `impl Trait`, `dyn Trait`, lifetime arguments and generics; the compiler's intermediate form says which impl each call
  resolves to. For a library, from its documentation's list of implementors and the source of its impls.

What stays host text: a lifetime or `mut` inside the bounds of a `some` or `any`, a higher-ranked `for<'a>` bound, `?Sized`
and other relaxed bounds, const generics, negative impls and fully qualified paths (`<T as I>::X`). The bounds of a
parameter are carried as the host writes them until the language has a form for them.

### 6.12 Promises that hold unless an implementation does otherwise

**Meaning.** `never panics`, `terminates`, the bounds on effects (§ 6.3), `ensure` and `fails` (§ 6.2) promise without
condition, and little of what a library says is unconditional. `HashMap::get` never panics unless the `Hash` or `Eq` of
its key panics, a default method may be overridden by an implementation that does not keep what the default does, and
an arithmetic result is exact only where the build checks for overflow. A clause may therefore be qualified by `given`,
which names what it relies on. The promise holds whenever everything it names holds, and says nothing otherwise: a
conditional promise is weaker than an unconditional one, never stronger, and it is not a failed one. What it names is
one of three things. **The implementations of a bound**, by parameter and method: `Q.hash` is the `hash` that `Q`'s
`Hash` bound brings, whichever type the caller chose, and the promise is made about its behaviour (`never panic`,
`terminate`, `effects none`). **The default body**: `given default` says the method is not overridden, so the promise is
about the default's body and an implementation that overrides it must state its own (§ 6.11). **A host build
setting**: `given build overflow_checks` names a setting the host profile lists (§ 3.3), which no subject chooses and
every build fixes.

```text
function rust 'std::collections::HashMap::get'(self: ref<target_type=HashMap<K, V>>, k: ref<target_type=Q>) -> ref<target_type=V>?
    for K, V, Q where K: 'std::borrow::Borrow<Q>', Q: 'std::hash::Hash' + 'std::cmp::Eq'
    never panics   given Q.hash, Q.eq never panic
    effects none   given Q.hash, Q.eq effects none
    law borrow_agrees : all k : K : hash(k) == hash(borrow(k))     -- the caller's choice of type must satisfy it

function rust 'std::iter::Iterator::count'(self: Self) -> u64 as 'usize'
    ensure counted : old(count(self)) <= 18446744073709551615 implies result == old(count(self))
    fails panic when old(count(self)) > 18446744073709551615   given build overflow_checks

function rust 'std::cmp::Ord::max'(self: Self, other: Self) -> Self
    never panics   given default and Self.cmp never panic
```

- **`P.m, P.n never panic | terminate | effects none`** is a condition on the implementations of one or more methods
  of the bounds of a generic parameter `P` of the subject. A subject that has no such parameter has no such condition: an
  implementation fixed by the subject is checked as the subject's own body. A condition is a promise of the same
  family as the clauses it qualifies, so that it can be assumed by a checker and is asked of the caller.
- **`given default`** may qualify the clause of a method of an interface (or an impl, § 6.11); it names no method,
  because the method is the subject. **`given build S`** names one fact of the platform (§ 3.6): for Rust,
  `overflow_checks`, `debug_assertions`, `panic=unwind` and `panic=abort` (the forms this section first had), and any
  platform fact, `pointer width 64`, `endian little`, `target features avx2`, `features std`. The kinds of fact are
  fixed by the host profile, and a name outside them is an error rather than a new setting.
- **Several conditions** are joined by `and`, and hold together. Two clauses of one kind with different conditions are
  two promises, and the caller may rely on whichever condition it has.
- **A law over a generic parameter** (§ 6.2) is a requirement, not a promise: it says what a type must satisfy to be
  a sound choice for the parameter, as `Borrow` requires of `Hash` and `Eq` above. It is stated for the parameter and
  not for one type, binds the caller, and does not take a `given`. It is checked for each type the code instantiates the
  parameter with, in CI, as laws are for a function (§ 6.2), and it is what the `given` conditions of the subject lean
  on: a type that breaks its law voids what is conditional on it.
- **Fails:** as the promise it qualifies fails, and only when the condition holds. If `Q.hash` panics, `HashMap::get`
  may panic and nothing has been broken; if the conditions hold and the promise does not, the subject has a bug. A
  condition that names something that does not exist (a parameter the subject lacks, a method its bound lacks, a setting the
  profile does not list, `given default` on a function that is not a method of an interface) is refused at build. A
  caller that discharges a generated check with a conditional promise (trial § 7.3) must also discharge its condition,
  so a condition no one can discharge leaves the check open instead of closing it.
- **Checked:** the condition names are checked against the subject, its bounds and the profile, as above. What a
  conditional promise says is checked as the unconditional one is (§ 6.2, § 6.3), by the host's compiler, by tests or by
  replay, in the builds where the condition holds. A condition on an implementation is discharged at the call site, from the
  type chosen, by that type's own constraints: the implementation of `Hash` for `u64` states `never panics` without
  condition, and so the call is unconditional there. A build setting is discharged from the build.
- **Harvested:** from documentation that says "unless", "provided that", "if `T`'s … panics" or "in debug builds", from the
  attributes and bodies that show a call to a bound's method inside a subject that otherwise cannot panic (the MIR of a call
  through a bound names it), from `#[cfg(debug_assertions)]` and the overflow checks of arithmetic in a body, and, for
  the default body, from the provided methods of a trait. For a library, from its documentation.

What stays prose: a condition on a value rather than an implementation ("unless the iterator is infinite"), on a
combination of conditions that `and` cannot say (`or`, `not`), on a feature of the host's toolchain other than the
profile's, and on an unlisted build setting; a condition on a closure argument is a `callback` (§ 6.13).

### 6.13 Callbacks: their conditions, arguments, results and order

**Meaning.** `callback c : called exactly once` (§ 6.2) says how often a function argument is called, and nothing else. A
library says more: `Option::map_or_else` calls its `default` when the option is none and its `f` with the value when
there is one, and its result is what the one it called returned. A callback clause may therefore state **when** the
argument is called (`when p`, a condition on the call's entry; where it does not hold the argument is not called),
**with what** (`with`, the call's arguments), and **what it returned** (`returning r`). The result is a value that
crossed the boundary (principle 5), not a call the language makes: a clause never calls a function argument, which need
not be pure, it only names what the one call gave. The names `with` and `returning` give are in scope in the subject's
other clauses, so that a postcondition can say what the result is in terms of them. A **protocol** over the subject's own
calls states their order: the same regular expression as § 6.5, whose events are here the subject's callbacks, its
parameters, and the methods of a parameter's bounds (`self.next`), with `begin` and `end` of the call itself.

```text
function rust 'std::option::Option::map_or_else'(self: T?, default: D, f: F) -> U
    callback default : called exactly once when self is none, returning d
    callback f       : called exactly once when exists self, with self, returning r
    ensure from_callback : result == (if exists self then r else d)

function rust 'std::collections::hash_map::Entry::and_modify'(self: Entry<'a, K, V>, f: F) -> Entry<'a, K, V>
    callback f : called exactly once when self is Occupied(e), with v
    ensure in_place : result is Occupied(r) implies same(current(r), v)

function rust 'std::iter::Iterator::count'(self: Self) -> u64 as 'usize'
    protocol self.next+                  -- next is called, and at least once, even if there are no items

macro rust 'std::assert_eq!'(left: ref<target_type=T>, right: ref<target_type=U>)
    protocol left right                  -- left is evaluated, then right, once each
```

- **`callback c : called … [before return] [when p] [, with a, b] [, returning r]`.** `when` is an expression over the
  subject's names, `self`, the parameters and the generics, never over what a callback returns. Each `with` item is an
  expression over the same names, which the argument equals (by identity, for a reference), or a single new name, which
  names that argument for the clauses that follow (`with v` above is the `&mut V` that `f` receives). `returning r`
  binds one name and ends the clause. `with` and `returning` need `exactly once` or `at most once`, because they name
  the values of one call. `r` and `v` exist only where the callback is called, so a clause that uses them says where
  (`if exists self then r else d`, `result is Occupied(r) implies …`). `current(h)` is the value the handle `h` (an entry) stands for (§ 4).
- **`protocol` over calls.** An event is a callback or a parameter of the subject (for a macro, a parameter is the
  evaluation of its argument, and for a function it is the passing of it), or `p.m`, the method `m` of the bounds of
  parameter `p` (`self.next` when `Self: Iterator`). `*`, `+`, `?`, `|` and brackets are as in § 6.5, and calls the
  protocol does not name are unconstrained. A protocol over one call of the subject begins at its entry and ends at its
  return; it does not reach into a result's later calls, which `called by` and `fails … in` state (§ 6.13.1).
- **Fails:** a callback called when its condition does not hold, not called when it does, or called other than the clause
  counts, is a bug in the subject: the call is the subject's, and so is the promise. A postcondition that uses a name from
  a callback that was not called is a bug in the clause. A protocol that the calls do not follow is reported, or
  rejected where a wrapper inserted it. Names that do not exist (a `when` over a name the subject lacks, a protocol event
  that is neither a callback nor a parameter, a method its bound lacks, `returning` after a count that is not one call)
  are refused at build.
- **Checked:** the names, as above, at build. The calls are checked by a generated wrapper around the argument where the
  host allows one (it counts and records each call's arguments and result, and compares them with the clause's
  condition and with the postcondition at return), and otherwise in CI by tests and replay. A protocol is a small
  automaton over the wrapper's events, or the host's typestate where it has one. A clause that relies on a callback
  result is checked after return, when the wrapper's record is complete.
- **Harvested:** the count, condition and result of a callback from the body of the subject: the arm of a `match` or
  `if` that calls the argument, and what the arm returns; a protocol from the sequence and loops of calls in a body, and
  from documentation that says "calls … repeatedly", "at least once" or "left then right". For a library, from its
  documentation and source.

#### 6.13.1 Calls a result makes later

**Meaning.** `Iterator::map` returns an adapter that holds the closure and calls it each time the adapter is advanced, so
no call of `map` calls it. What those later calls do is still a promise of `map`, about its result, and a clause of the
subject may state it. The later calls are the **run** of a method `m` of a path `P` (`result`, or a parameter): the
calls of `P.m` made one after another from the subject's return until the first that returns `none`, which is the run
that `yields(P)` describes (§ 6.15). Three forms speak of a run, and one expression reads its end.

```text
function rust 'std::iter::Iterator::map'(self: Self, f: F) -> 'std::iter::Map'
    callback f : called by result.next, at most once per call, with x, returning y
    ensure items     : yields(result) == y
    ensure arguments : x == old(yields(self))

function rust 'std::iter::Iterator::take_while'(self: Self, predicate: P) -> 'std::iter::TakeWhile'
    callback predicate  : called by result.next, at most once per call, with x, returning keep
    ensure items        : yields(result) == while_true(keep, old(yields(self)))
    ensure consumed     : x == take(count(keep), old(yields(self)))
    ensure not_put_back : final(yields(self)) == skip(count(keep), old(yields(self)))

function rust 'std::iter::Iterator::enumerate'(self: Self) -> 'std::iter::Enumerate'
    fails panic in result.next when old(count(self)) > 18446744073709551615   given build overflow_checks
```

- **`callback c : called by P.m [, exactly once per call | at most once per call] [, with a, …] [, returning r]`.** The
  callback is called by the calls of `P.m` in the run, and by nothing else: not by the subject, and not by a method that
  does not advance the result. The count per call of `P.m` is `exactly once`, `at most once` or, when it is not stated,
  any number including none (`filter`'s predicate is called until an item passes). How many calls there are in all is
  not stated here: it follows from the sequence equations, as `x == old(yields(self))` says that every item is passed.
- **The logs.** In a `called by` clause `with a` and `returning r` bind lists, not values: `a` is the list of the
  argument over the calls of the run, in order, and `r` the list of the results, so that `a[k]` and `r[k]` belong to
  the `k`-th call and `count(r)` is the number of calls. `with` takes names only, because an expression has no value at
  a call that has not happened; and the clause has no `when`, because a condition on the subject's entry says nothing
  about calls made later (the equations state which calls there are). The names are in scope in the subject's other
  clauses, which are about the run and not about the call of the subject, so `ensure items : yields(result) == y`
  relates the result to whatever the callback returned.
- **A value that crossed the boundary.** As in the first form of this section (principle 5) the clause never calls the
  callback and does not fix what it returns. `yields(result) == y` holds for every behaviour the closure has, which is
  how it can be stated for a closure that has no name, is not pure and is called later; the closure's effects happen
  once per call of the run and only its results enter the clause.
- **`yields` of an adapter.** § 6.15 gives `yields` to a type whose `next` is deterministic given its state. An
  adapter over a closure has one as well, when the subject that makes it names the closure's logs: in that subject's
  clauses `yields(result)` is the sequence of the run, a function of the logs (`yields(result) == y`). For the type
  elsewhere it has none.
- **`fails F in P.m when p`.** A call of the run fails with `F` when `p` holds; `p` is over the subject's entry (the
  parameters' values at entry, as in `fails F when p`, and `old` is implicit), and the call of the subject does not fail.
  `given` qualifies it as every failure clause (`given build overflow_checks`).
- **`final(e)`.** The value `e` has at the end of the run; `old(e)` is its value at entry. It is for what a parameter
  still holds: `final(yields(self))` is what is left of the iterator `self` once the run is over, read through the
  result that holds it, or through the caller's own place when `self` is a `&mut I`, as it is in `iter.by_ref()
  .take_while(…)`. A subject with no `called by` and no `fails … in` has no run, and `final` there is refused.
- **Fails:** a run whose calls do not follow the clause (a callback called outside the run, more than once per call
  where the clause says once, or a log that disagrees with an equation) is a bug in the subject. Refused at build: `P`
  that is neither `result` nor a parameter, a method its bounds lack (known when `P`'s bounds are), `with` of an
  expression, `when` in a `called by` clause, a count other than the two, and `final` without a run.
- **Checked:** by a generated wrapper around the closure that records each call's arguments and result, and a copy of
  the result (a `Clone`, or the same call run again where the host cannot copy) driven with `next` until `None` or a
  bound of items (1,024 by default). The logs are the wrapper's record; each equation is compared on them; a run that
  hits the bound is compared by prefix and the clause's status is `observed`, never `proven`. The count per call is
  checked on the same record.
- **Harvested:** from documentation that says "calls this closure on each element", "while it returns `true`" or "once
  for each item", and from the body of the adapter's `next` (map: `self.iter.next().map(&mut self.f)`, which is
  at most once per call, with the item taken and what `f` returned).

What stays outside: a protocol whose next step depends on a value the calls returned (`next` is called until it returns
none), an order among calls of different arguments of several subjects, and the calls a result makes from a method that
does not advance it (a type that runs its closure from `size_hint` would be stated with that method as `P.m`).

### 6.14 Nine small additions

Each is a clause or an expression that extends a section already written, and is small enough to state on its own
(language-gaps § 4.4). They are collected here because they share a form: one sentence of meaning, one line of syntax,
one way to fail. The section each extends is named in its heading.

#### 6.14.1 `const`: evaluable at compile time (extends § 6.3)

**Meaning.** `evaluable at compile time` says the body of a subject may run in a constant context: the host compiler
may call it while it builds. It is a bound on effects, as trial § 12 says of `const fn`. It forbids input and output, a
clock, randomness, suspension and any call to a subject that is not itself evaluable at compile time, and it requires
that the same inputs give the same result. Unlike `effects none` it permits writes through the subject's own `&mut`
arguments, which is what `mem::swap` does. It is a promise only where the host promises it to later versions; a body that
is evaluable under an unstable host feature makes none, and states none.

```text
function rust 'std::primitive::slice::len'(self: ref<target_type=list<element_type=T> as 'std::primitive::slice'>) -> u64 as 'usize'
    evaluable at compile time
function rust 'std::mem::swap'(x: ref<target_type=T>, y: ref<target_type=T>)
    evaluable at compile time          -- and the writes through x and y are allowed
```

- **Fails:** a call in a constant context of a subject that is not, a build error at the call. A body that breaks the
  claim is a bug in the subject: the host compiler refuses it, where it can see.
- **Checked:** by the host compiler where the host has the notion (`const fn`, `constexpr`): `weave` writes `const`
  from the clause and `strip` reads it back. A library's claim is checked by calling the item in a constant context.
- **Harvested:** from `const fn` and `constexpr`, and, for a library, from the stability attribute (`rustc_const_stable`
  is a promise, `rustc_const_unstable` is not).

#### 6.14.2 Must-use on a type (extends § 6.2)

**Meaning.** `result must be used` (§ 6.2) attaches to a subject. `must be used` attaches to a type, or to an interface:
a value of it that is dropped without being used is a bug in the caller, wherever it was made. Every subject whose result
is the type, or `some` of the interface, then has `result must be used` without writing it; in the Rust host a `fails E`
result is a `Result`, which is must-use, so a subject that fails has it too. A subject may add its own clause for a reason
of its own (`mem::replace`: "you can just assign the new value").

```text
type rust 'std::iter::Chain'
    must be used
type rust 'std::result::Result'
    must be used
```

- **Fails:** a discarded value is a build error where the host can see it, and a lint elsewhere.
- **Checked:** by the host's own lint (`unused_must_use`, `[[nodiscard]]`); `weave` writes `#[must_use]` for the plain
  attribute. A message the host carries stays in `source`.
- **Harvested:** from `#[must_use]` on a type or a trait, `[[nodiscard]]` on a class.

#### 6.14.3 Moves (extends § 7.2)

**Meaning.** A value that moves keeps its identity (§ 7.1: an entity is anything with identity). `==` cannot show it,
because it is structural: two values that compare equal are not told apart, and `max` of two of them returns one. `same`
is the identity of host references, not of a value that moves. `p moved from q` is true when the entity at `p` after the
call is the entity `q` named at entry: moved, not copied or cloned. It is an expression, usable under `implies` and
`when`; `hold p : moved from q` is its unconditional form, beside `owns`, `shares` and `borrows`. A copy (`Copy`,
`clone`) is a new entity and is never moved from.

```text
function rust 'std::cmp::max'(v1: T, v2: T) -> T
    ensure tie_second : (v1 <= v2 and v1 >= v2) implies result moved from v2
function rust 'std::mem::replace'(dest: ref<target_type=T>, src: T) -> T
    hold dest   : moved from src
    hold result : moved from dest
```

- **Fails:** a value that is not the one named is a bug in the subject.
- **Checked:** statically, under ownership, where a move is visible in the body; otherwise in tests, by values that
  compare equal and carry a tag. Under counting and tracing the identity is the host reference, and `same` applies.
- **Harvested:** from documentation that says which argument is returned or kept ("returns the second argument"), and
  from the arm of a body that returns a parameter.

#### 6.14.4 Ends and drops (extends § 7.3)

**Meaning.** `lifetime v ends in call [when p]` says the entity `v` had at entry ends before the call returns: it is
dropped, and its destructor runs, unless it was moved out; with `when`, only where `p` holds. `lifetime a, b survive
call` says none of the named entities ends in the call: no destructor runs, none is freed or deinitialised, and a move
to a result or to a holder is a survival. For a reference parameter the entity is its referent. Both are over the scope
`call` (§ 7.1). A by-value argument that neither clause names is left unstated.

```text
function rust 'std::mem::swap'(x: ref<target_type=T>, y: ref<target_type=T>)
    lifetime x, y survive call
function rust 'std::mem::drop'(v: T)
    lifetime v ends in call              -- "disposes of a value"; with `when p` only where p holds
```

- **Fails:** a drop that happens where `survive` says none, or does not happen where `ends` says it does, is a bug in
  the subject.
- **Checked:** statically under ownership (drop elaboration shows which values are dropped on which path); in tests by
  a value whose destructor counts. Under tracing a drop is release (§ 7.1), and `ends` is read as `released`.
- **Harvested:** from documentation that says "dropped" or "without deinitializing", and from the arm of a body that does
  not move a parameter.

#### 6.14.5 Resource exhaustion (extends § 6.2)

**Meaning.** `never panics` (§ 6.2) is false of every call that may need more memory than the platform gives.
`never panics except exhausted` keeps the rest of it: the one failure left open is exhaustion, that the platform refuses
what the call asks for (an allocation, or a size that overflows the host's index type while the call works out how much
to ask for). What the host does then is the host's (a capacity-overflow panic, an abort on allocation failure); no other
panic and no other runtime failure is possible. It may carry `given` (§ 6.12), and `never panics` alone says exhaustion
cannot happen, which is the stronger promise.

```text
function rust 'std::collections::HashMap::entry'(self: ref<target_type=map<key_type=K, value_type=V, hasher_type=S>>, key: K) -> Entry<'_, K, V>
    never panics except exhausted given K.hash, K.eq, S.hash_one never panic
```

- **Fails:** a panic that is not exhaustion is a bug in the subject; exhaustion is not.
- **Checked:** as `never panics`, in tests under an allocator that cannot fail, where a panic is a bug; and under one
  that can, where only an abort or the host's capacity panic is allowed.
- **Harvested:** from documentation that says "panics if the new capacity exceeds" and "aborts on allocation failure",
  and from a body that reserves.

#### 6.14.6 Expected and amortised cost (extends § 6.7)

**Meaning.** `budget work O(f)` bounds every call. `budget work expected O(f)` bounds the mean over the subject's own
random choices (a hash seed, a pivot): one call may cost more, and nothing bounds the worst. `budget work amortised
O(f)` bounds the total of any sequence of n calls on one instance, started from empty, by n times `f`: one call may
cost more, the average per call does not. Both words together say both. A budget never fails a request (§ 6.7).

```text
function rust 'std::collections::HashMap::get'(self: ref<target_type=map<key_type=K, value_type=V, hasher_type=S>>, k: ref<target_type=Q>) -> ref<target_type=V>?
    budget work expected O(1)
function rust 'std::collections::HashMap::insert'(self: ref<target_type=map<key_type=K, value_type=V, hasher_type=S>>, k: K, v: V) -> V?
    budget work expected amortised O(1)
```

- **Checked:** in CI on a work counter (probes, comparisons) over many seeds, or over generated sequences of calls,
  against the previous release.
- **Harvested:** from documentation that says "expected" or "amortized"; std's table of collection costs marks them `~`
  and `*`.

#### 6.14.7 Tuple parts (extends § 4)

**Meaning.** `e.0`, `e.1` are the components of a tuple, by position. A pattern `(a, b)`, with `_` for a part that is not
wanted, binds a name to each part: `let (a, b) = e` for the later clauses of the subject, and `e is (a, b)` as `is`
binds the names of a variant. Both are for a call that returns a tuple.

```text
function rust 'std::primitive::slice::split_at_mut'(self: ref<target_type=list<element_type=T> as 'std::primitive::slice'>, mid: u64 as 'usize')
        -> (ref<target_type=list<element_type=T> as 'std::primitive::slice'>, ref<target_type=list<element_type=T> as 'std::primitive::slice'>)
    ensure sizes : count(result.0) == mid and count(result.1) == old(count(self)) - mid
    disjoint result.0, result.1
    ensure same_sizes : result is (head, tail) implies count(head) + count(tail) == old(count(self))

function rust 'std::primitive::slice::split_at_mut'(self: ref<target_type=list<element_type=T> as 'std::primitive::slice'>, mid: u64 as 'usize')
        -> (ref<target_type=list<element_type=T> as 'std::primitive::slice'>, ref<target_type=list<element_type=T> as 'std::primitive::slice'>)
    let (head, tail) = result
    ensure sizes : count(head) == mid and count(tail) == old(count(self)) - mid
```

A `let` clause binds for the clauses after it, once: a name that is already bound (a parameter, an earlier `let`, a
generic) is refused, `_` binds nothing, and the expression sees only the names bound before the clause. In a library file
`let` is a clause kind of its own; a tool that reads the file binds the names as it reads the clauses in order.

- **Fails:** a component the tuple lacks (`.2` of a pair) and a pattern of the wrong length are refused at build.
- **Checked:** at build, by the type of `e`. **Harvested:** from the documentation of the parts of a pair.

#### 6.14.8 Element frame condition (extends § 8.2)

**Meaning.** `mutates only p[e]` is `mutates only` (§ 8.2) with one element of a collection for its path: the element of
`p` at key `e` may change, appear or vanish, and every other element keeps its value and its place. It says nothing of the
collection's state that is not an element (a capacity, a hasher), which a path of its own bounds. It is the clause for
what `ensure others_kept : all k2 in old(self) : k2 != k implies self[k2] == old(self[k2])` says of one subject.

```text
function rust 'std::collections::HashMap::insert'(self: ref<target_type=map<key_type=K, value_type=V, hasher_type=S>>, k: K, v: V) -> V?
    mutates only self[k]
```

A handle (§ 4) hides the collection it borrows, so the collection is named with `owner`: `mutates only owner(self)[key(self)]`
says that a call that consumes an entry changes the element of the entry's key and no other, and `owner(self)[old(key(self))]`
is that element afterwards. `key` is a view the entry's type declares (`Entry::key`).

```text
function rust 'std::collections::hash_map::Entry::or_insert'(self: Entry<'a, K, V>, default: V) -> ref<target_type=V>
    ensure stored : self is Vacant(e) implies owner(self)[old(key(self))] == default
    mutates only owner(self)[old(key(self))]
```

- **Checked:** as `mutates only` (§ 8.2), by comparing the collection before and after on generated calls; for
  `owner(h)`, by the wrapper that made the handle from a collection it holds.
  **Harvested:** from documentation that says "no other key changes".

#### 6.14.9 Obligations on callers (extends § 10.2)

**Meaning.** `assume name : p because "…"` (§ 10.2) is what a subject relies on and does not check. `obliges caller : p
because "…"` is the same seen from the caller: a precondition the callee does not check, which every call must establish.
It differs from `require`, which is checked at entry and fails: when `p` is false nothing fails, and the host may do
anything (for `unsafe fn` in Rust, undefined behaviour). The `because` says what goes wrong, for the reviewer who reads the
call site.

```text
function rust 'std::primitive::slice::get_unchecked'(self: ref<target_type=list<element_type=T> as 'std::primitive::slice'>, i: u64 as 'usize') -> ref<target_type=T>
    obliges caller : i < count(self) because "the index is not checked; out of bounds is undefined behaviour"
```

- **Checked:** each call site shows the reason, as an `assume` of its own in review; where `p` is a predicate, a debug
  build asserts it. The host's `unsafe` marks the call. **Harvested:** from the `# Safety` section of an `unsafe fn`.

What stays outside: whether a `const` body is evaluated at compile time (the host may, and need not); a drop by a body
the subject cannot name (a value of a type parameter is dropped by its own destructor, whose effects `given` names no
method of); the identity of what a clone shares with its original (`Clone` promises none: each type's `clone` decides, and a clause
cannot state the absence of a promise); and a macro's arm, whose parameters a clause of the macro cannot name.

### 6.15 Sequences: order, positions, and what an iterator yields

**Meaning.** A `list<element_type=T>` has an order: its elements stand at positions 0, 1, 2 and so on, and two lists are equal when
they have the same elements at the same positions. A list may be finite, or infinite: the sequence a counter yields has
no last element. `size`, `reverse` and `in` need the end; everything else below is defined for both. The operators are
pure and deterministic, total on every list for which they have a value (`size` and `reverse` have none for an infinite
one, below), and each is defined here by its elements, so that every host compiles it identically
(principle 2). Writing `xs[k]` for the element at position `k`:

| Operator | Value | Defined by |
| --- | --- | --- |
| `xs[i]` | `T?` | The element at position `i`; `none` when `i < 0` or `i >= count(xs)` (for an infinite `xs`, when `i < 0`). It never fails. For an optional `T` it is flat, below. |
| `present(xs, i)` | `bool` | `i >= 0` and (`xs` is infinite or `i < count(xs)`): position `i` holds an element, whether or not that element is `none`. |
| `take(n, xs)` | `list<element_type=T>` | The first `n` elements, or all of them if there are fewer; empty when `n <= 0`. `take(n, xs)[k] == xs[k]` for `0 <= k < n`. |
| `skip(n, xs)` | `list<element_type=T>` | `xs` without its first `n` elements; `xs` when `n <= 0`, empty when `n >= count(xs)`. `skip(n, xs)[k] == xs[k + n]`. |
| `xs[i..j]`, `xs[i..]`, `xs[..j]` | `list<element_type=T>` | `take(j - max(i, 0), skip(max(i, 0), xs))`, `skip(i, xs)` and `take(j, xs)`, where `skip` and `take` of a negative count are `skip`/`take` of 0. Positions outside the list are not elements, so a slice never fails: `xs[2..9]` of three elements has one. |
| `xs ++ ys` | `list<element_type=T>` | For a finite `xs`, `count(xs) + count(ys)` elements: `xs[k]` for `k < count(xs)`, then `ys[k - count(xs)]`. For an infinite `xs`, `xs`. It is associative, and `[]` is its unit. |
| `reverse(xs)` | `list<element_type=T>` | For a finite `xs`: `reverse(xs)[k] == xs[count(xs) - 1 - k]`. |
| `map(f, xs)` | `list<element_type=U>` | `f` is a named pure function (below). Same size as `xs`; `map(f, xs)[k] == f(xs[k])`. |
| `filter(p, xs)` | `list<element_type=T>` | `p` is a named pure predicate. `filter(p, []) == []`, and `filter(p, [x] ++ r) == (if p(x) then [x] else []) ++ filter(p, r)`: the elements for which `p` holds, in their order. |
| `select(bs, xs)` | `list<element_type=T>` | `bs` is a list of booleans. The elements `xs[k]` at the positions `k` where `bs[k] == true`, in order, for the positions both lists have. `select([b] ++ r, [x] ++ s) == (if b then [x] else []) ++ select(r, s)`, and `filter(p, xs) == select(map(p, xs), xs)`. |
| `while_true(bs, xs)` | `list<element_type=T>` | `take(n, xs)`, with `n` the position of the first `false` in `bs`, or `count(bs)` when there is none (`xs` when `bs` is infinite and has none): the elements before the first verdict that fails. |
| `flatten(xss)` | `list<element_type=T>` | `xss` is a list of lists (or of values that implement `IntoIterator`, read as their `yields`). `flatten([]) == []` and `flatten([xs] ++ r) == xs ++ flatten(r)`: with an infinite `xss[k]`, the elements after it are never reached. |
| `zip(xs, ys)` | `list<element_type=(T, U)>` | `zip(xs, ys)[k] == (xs[k], ys[k])` while both exist: as many pairs as the shorter list has. |
| `enumerate(xs)` | `list<element_type=(int, T)>` | `enumerate(xs)[k] == (k, xs[k])`. |
| `scalars(s)` | `list<element_type=scalar>` | The Unicode scalar values of the string `s`, in order (§ 4): `count(scalars(s))` is how many it has. |
| `deref(r)` | `T` | For a `ref<target_type=T>`, the value it refers to. `map(deref, xs)` reads a list of references as the list of values. |

A list literal `[a, b]` is the list of its elements, and `[]` the empty list. A `ref<target_type=list<element_type=T>>` is read as the list it
refers to wherever a list is expected, as `count(self)` has always been. If the element type is optional, `xs[i]` is flat
like every optional (§ 4: absence is one value): it is `none` both beyond the ends and for an element that is `none`, and
`xs[i] == none` does not tell the two apart. `present(xs, i)` does, and so does the slice `xs[i..i + 1]`, which is `[]`
or a list of one element and cannot be mistaken because a list does not flatten. A host `Option<Option<U>>` read at the
boundary is `U?` and loses the same distinction, so a clause that needs it says `present`. None of the clauses with
`xs[i]` is false under the flat reading: `Iterator::next`'s `head` stays true, and its `advances` is `present(old(yields(self)),
0) implies yields(self) == old(yields(self))[1..]`, which holds and says as much for an iterator of options as for any other,
where `exists result implies …` would say nothing when `next` returned `Some(None)`.

**A named function.** `map` and `filter` take the name of a function, not a value: a function is never an argument,
a variable or a result of an expression, which is what keeps every expression pure and total. The name is a language
function (`scalars`, `deref`, `yields`), a view of the catalog or of the file (§ 4's "Functions": total, deterministic,
free of effects), and no parameter of the subject. `map(yields, xss)` is the list of the sequences of a list of
collections, which `flatten` joins. In particular, a closure the subject receives (`Iterator::map`'s `f`) is not a
named function: it may do anything, it is called later and a number of times the clause does not control, and it has no
name in the catalog. A clause cannot say `yields(result) == map(f, yields(self))` for it. What such an adapter yields is
stated through the closure's logs instead (§ 6.13.1): `callback f : called by result.next, with x, returning y` and
`yields(result) == y`, with `select` and `while_true` taking the verdicts of a predicate (`yields(result) ==
select(keep, old(yields(self)))`), so the closure is never called by the clause and never named as a function.

**What an iterator yields.** `yields(it)` is a view of any value `it` of a type that implements an iterator: the
sequence of items that calling `next` on it would return, in order, from its present state and up to but not including the
first `None`. It is a view of the state (a pure projection, § 4): evaluating it does not advance the iterator, and
`old(yields(it))` is what was left at entry. It is finite when the iterator ends and infinite when it does not. For a
value that is not an iterator but a collection (it implements `IntoIterator`), `yields(c)` is its items in the order its
`IntoIterator` defines; for a list, the list. `count(it)` is `count(yields(it))`, the count the older clauses already use
(§ 4's "Collections"). A double-ended iterator gives one sequence, read from either end: `next_back` returns the last element of `yields` and removes it, so repeated `next_back`
gives `reverse(yields(it))`, and the `DoubleEndedIterator` an `implements` clause states needs no clause of its own about
order. After a `None`, a host iterator that is not fused may resume; `yields` is defined from the state at the time of
the view, so it describes what is left from there and says nothing about the resumption. A type whose `next` is not
deterministic given its state, or has an effect that matters (it reads a clock), has no `yields`; one that runs a closure
has it only in the clauses of a subject that names the closure's logs (§ 6.13.1).

```text
function rust 'std::primitive::str::chars'(self: ref<target_type=string>) -> 'std::str::Chars'
    ensure scalars : yields(result) == scalars(self)

function rust 'std::primitive::slice::split_at_mut'(self: ref<target_type=list<element_type=T> as 'std::primitive::slice'>, mid: u64 as 'usize')
        -> (ref<target_type=list<element_type=T> as 'std::primitive::slice'>, ref<target_type=list<element_type=T> as 'std::primitive::slice'>)
    ensure part_elements : result.0 == old(self)[..mid] and result.1 == old(self)[mid..]

function rust 'std::iter::Iterator::next'(self: ref<target_type=Self>) -> Self::Item?
    ensure head     : result == old(yields(self))[0]
    ensure advances : present(old(yields(self)), 0) implies yields(self) == old(yields(self))[1..]

function rust 'std::iter::Iterator::chain'(self: Self, other: U) -> 'std::iter::Chain'
    ensure items : yields(result) == old(yields(self)) ++ old(yields('<U as std::iter::IntoIterator>::into_iter'(other)))
function rust 'std::iter::Iterator::take'(self: Self, n: u64 as 'usize') -> 'std::iter::Take'
    ensure first_items : yields(result) == take(n, old(yields(self)))
function rust 'std::iter::Iterator::copied'(self: Self) -> 'std::iter::Copied'
    ensure copies : yields(result) == map(deref, old(yields(self)))
```

Equality of infinite sequences is of every finite prefix: `xs == ys` holds when `take(n, xs) == take(n, ys)` for every
`n`. Such a clause is a property (§ 4.1), never a predicate: a check can refute it on a prefix and can never confirm it.
`size`, `count` and `reverse` have no value on an infinite sequence, and a clause that needs one is decided where the
sequence is finite and not decided where it is not (no host reports it as a violation); the older `count(result) ==
…` clauses are read this way. Everything else, the positions, `take`, `skip`, slices, `map`, `zip` and `enumerate`,
says something of the first `n` elements for every `n`, which is how a promise about a counter's prefix is written:
`ensure counts : take(3, yields(result)) == [0, 1, 2]`.

- **Fails:** nothing at run time: every operator is total on the lists it has a value for, and an index or a slice
  beyond the ends gives `none` or fewer elements, never a failure. A promise that an index is in range is a `require` or a
  `fails … when` of its own (`fails panic when mid > count(self)`); Rust's slicing panics there, and a clause that claims
  the same is a `fails`, not the slice. Refused at build: `yields` of a type that has no `next`; `map` or `filter` given a
  parameter, a callback, a generic closure or an expression instead of a named function; a named function that does not
  exist in the catalog or the file; the wrong number of arguments; `select` or `while_true` whose first argument is not a list of booleans, and `flatten` of
something that is not a list of lists or collections; and `size` or `reverse` of a sequence that `yields`
  proves infinite (a type that is `Iterator` and not `ExactSizeIterator` is not proven either way, and the clause is
  decided where it ends).
- **Checked:** every operator by its definition above. A conformance suite holds each equation with generated lists
  (empty, one element, two, and indices `-1`, `0`, `size - 1`, `size`, `size + 1` and very large), and every host's
  compilation of an operator must pass it before it may be used in a generated check (§ 11.1). `yields(it)` is checked by
  running a copy of `it` (a `Clone`, or the same call run again where the host cannot copy) with `next` until it returns
  `None` or until a bound of items (1,024 by default) is reached: the sequence is compared whole when it ends within the
  bound and by prefix when it does not, and the clause's status is `observed`, never `proven`, in the second case. A clause
  with a closure inside the iterator is checked as § 6.13.1 says, with the closure's calls recorded, and has no `yields`
  elsewhere. In the library tools, `libcheck` lints the
  operators (names, arguments, that `map` and `filter` are given a view of the file or `scalars` or `deref`), and `libstats`
  counts them; neither compiles a sequence clause against `std`.
- **Harvested:** `yields` from documentation that says an iterator "yields", "returns the items", "in order" or "from the
  front", and from doc tests of the form `assert_eq!(it.next(), Some(x))` followed by `assert_eq!(it.next(), None)`, which
  are a prefix and the end; positions and slices from `get(i)`, `first`, `last` and `&xs[a..b]`; `++`, `take`, `skip`,
  `zip`, `enumerate`, `reverse` from the adapters and functions of the same names in a body or a documentation line
  ("the first `n` items", "after the first `n`", "in step"); `map` and `filter` from a call of a named function in a body,
  and from a documentation line that names a function. For a library, from its documentation and source.

What stays outside: a range of a generic `Step`
type (`start..end` yields the successor of `start`, which `Step` defines, and `Step` is an unstable trait no bound can
name: the language has no successor of a type parameter, and a range of integers is the refinement `T in a..b`, not a
value of `Range`); and `collect`, whose result is any `FromIterator` and promises nothing of the items by itself (the
impls of `Result` and `String` state theirs, as subjects of their own). That an order is arbitrary, as std's `HashMap`
documents of its iteration, is not stated here, where a clause states what holds: it is a freedom (§ 6.17), `order
unspecified yields(self.iter())`. The lifetime of the elements an iterator has not yet given is stated by `lifetime
every item of yields(it) ends with it` (§ 7.3); the calls an adapter makes of its closure by `called by` (§ 6.13.1).

### 6.16 Independence: what lets work run in parallel or in another order

**Meaning.** Some constraints are rules a correct program keeps: `atomic`, a lock order, a confinement (§ 6.8). Others
are facts about what a subject touches, which a compiler, a scheduler or a translator may use to run work in parallel,
in another order, once instead of many times, or on another core or host. This section gathers the second kind. A
fact of independence is a bound like any other (§ 6.3, § 8): a body that breaks it has a bug. What sets it apart is
its use: it is relied on by someone who does not see the body, across a library boundary, a separately compiled unit,
or a translation into another host, and a later edit that breaks it is caught instead of silently undoing an
optimization.

The facts are:

- **`pure`** (§ 6.3): reads only its inputs, writes none of them, has no other effect, deterministic.
- **The read set**, `reads only p, q`, and **the write set**, `mutates only p, q` in memory and `writes only` for rows
  (§ 6.3, § 8.2), each with `when c` where they differ by case.
- **`disjoint a, b`** (§ 8.3): no entity is reachable from both, so writes through one are not seen through the other.
- **`commutes with S`** (§ 6.3): the two subjects' effects give the same outcome in either order, even where their
  write sets meet (two additions to one balance).
- **`sendable`** and **`shareable`** (§ 6.8): an entity may move to another thread, or be read from several at once.

**Two calls are independent** when neither's write set meets the other's read set or write set (on the entities their
arguments reach, with `disjoint` deciding whether two paths can meet), or when they commute; when neither has another
effect that the other observes (input and output, a clock, an emitted event, a message) unless both are bounded to
none; and when nothing orders them (a `protocol`, § 6.5; a lock order, § 6.8). Independent calls may run in parallel or
in either order, and the outcome is the one of program order.

```text
function rust 'eve::dogma::Fitting::recalc_cpu'(self: ref<target_type=Fitting>)
    reads only self.modules
    mutates only self.cpu

function rust 'eve::dogma::Fitting::recalc_power'(self: ref<target_type=Fitting>)
    reads only self.modules
    mutates only self.power
    -- the two read the same field and write different ones: they may run at the same time
```

- **Composed.** A caller's read set includes the read sets of the calls it makes, with the callee's parameters
  replaced by the caller's arguments; likewise its write set. A call into a subject with no stated sets has the
  largest sets its access allows (everything it can reach through `read` and `write` paths, § 8.1), which is why a
  library's sets are worth stating.
- **Derived where it can be.** A body's read and write sets can be computed from the body, as weave derives `mut`
  through rustc. A computed set is `proven`; a stated set that the computed one exceeds fails the build. Stating a set
  is still worth it at a boundary that others rely on, for the reason in the meaning above.
- **What a consumer may rely on.** An optimization may rely only on a fact that is `proven` or `enforced`, never on one
  that is `observed` or `watched`: a fact that is wrong once makes an optimized program wrong every time.
- **Checked:** by computing the sets from the body where the host allows it (Rust's borrows give most of them: a
  `&T` parameter is in no write set); by comparing what a call touches with its sets on replays, as for any bound;
  `commutes with` by property tests that run both orders.
- **Harvested:** from the manifests and replays (§ 8.6): the paths each call read and wrote, as the union of what was
  seen; `pure` from a body whose computed sets are its parameters' reads alone and that has no other effect.

### 6.17 Freedoms: what may vary

**Meaning.** Everything a program does that someone can observe is kept by a translation, unless something says it may
vary: a translator cannot tell an order nobody relies on from one somebody does. A freedom says it. It widens what the
subject may do to a stated set of outcomes, every one of them correct, and so tells every caller that it may not rely on
any one of them. A freedom is the most useful thing an optimizer or translator can be told, because it is the room it
works in: an unspecified order lets a hash map be replaced and a loop be split across cores, a tolerance lets a sum be
reassociated and vectorised, and a log that may be batched need not be written on the hot path.

```text
function rust 'std::collections::HashMap::iter'(self: ref<target_type=map<key_type=K, value_type=V>>) -> some 'std::iter::Iterator' { Item = (ref<target_type=K>, ref<target_type=V>) }
    order unspecified yields(result)                -- the pairs, in an order that may differ between maps and runs

function rust 'eve::dogma::sum_bonuses'(xs: list<element_type=f64>) -> f64
    may reassociate within abs: 1e-9, rel: 1e-12    -- any order of the additions; the result within that of the written one

procedure 'eve::market::FindSeller'(typeID: u64, qty: u64) -> u64?
    result any of select(o in market.Orders : o.typeID == typeID and o.qty >= qty).sellerID

function rust 'eve::telemetry::record'(e: Event)
    may batch emits                                  -- the events go out later and together, in their order
    may reorder emits                                -- and in any order among themselves
```

- **`order unspecified s`:** the sequence `s` (a result, an iterator's `yields`, the rows of a query) has the elements a
  clause of § 6.15 states, in any order. What its order is not, it says nothing of: `sorted` (§ 6.19) and an
  unspecified order cannot both be stated of one sequence.
- **`may reassociate within abs: e, rel: r`:** the subject's floating-point operations may be regrouped and reordered,
  and the result is within the tolerance of the one the body as written computes, by the `approx` of § 4. Without it,
  a translator keeps every float operation in its order, as the host's own strict mode does.
- **`result any of e`:** the result is one element of the collection `e`, any one, where the body picks one by an order
  nobody relies on (the first match of a scan).
- **`may batch` and `may reorder`** take a kind of effect (`emits`, `sends`, `writes` to a path), and say the effects of
  that kind may be delayed past the subject's return and applied together (`may batch`), or applied in any order among
  themselves (`may reorder`). They never reorder an effect with one of another kind, and never delay one that the
  subject's own result or a later call of the caller reads.
- **What a freedom cannot widen.** The failure a caller sees: the first precondition that fails decides it (principle
  6). A bound, a postcondition or an invariant: every outcome a freedom allows must keep them all.
- **Freedoms are decided by people** (principle 9), and a miner may only propose one: a body's order being arbitrary
  does not mean no caller relies on it.
- **Checked:** the reference evaluator accepts every outcome in the set. Callers are checked for reliance by running them
  in CI under each kind of choice: a shuffled order, a different association of a sum, another element of the set, the
  effects batched. A caller whose result changes relies on what the freedom says varies, and has a bug.
- **Harvested:** from documentation that says "arbitrary order", "unspecified", "any of", "may be reordered" or "is not
  guaranteed to be".

### 6.18 Named algebraic properties

**Meaning.** A `law` (§ 6.2) states any property over a subject. A few properties are wanted by every optimizer and
prover, and a fixed name with a fixed meaning is what lets a tool find them: `associative` with an `identity` turns a
loop into a parallel reduction, `idempotent` makes a retry or a cached call safe, `monotone` lets a search be binary,
and `equivalent to` lets a translator use another implementation and check it against this one. Each name is a law with
the meaning below, checked as a law is, and nothing more.

```text
function rust 'eve::dogma::stack'(a: Modifier, b: Modifier) -> Modifier
    associative
    commutative
    identity Modifier::NONE

function rust 'eve::text::normalise'(s: string) -> string
    idempotent

function rust 'eve::market::price_after_tax'(p: decimal<precision=19, scale=2>, rate: decimal<precision=5, scale=4>) -> decimal<precision=19, scale=2>
    monotone increasing in p

function rust 'eve::dogma::falloff_fast'(range: f64, optimal: f64, falloff: f64) -> f64
    equivalent to 'eve::dogma::falloff' within rel: 1e-12
```

| Name | On | The law |
| --- | --- | --- |
| `associative` | `f(a: T, b: T) -> T` | `f(f(a, b), c) == f(a, f(b, c))` |
| `commutative` | `f(a: T, b: T) -> U` | `f(a, b) == f(b, a)` |
| `identity e` | `f(a: T, b: T) -> T` | `f(e, a) == a and f(a, e) == a` |
| `absorbing z` | `f(a: T, b: T) -> T` | `f(z, a) == z and f(a, z) == z` |
| `idempotent` | `f(a: T) -> T`, or `f(a: T, b: T) -> T` | `f(f(a)) == f(a)`; `f(a, a) == a` |
| `involutive` | `f(a: T) -> T` | `f(f(a)) == a` |
| `inverse of g` | `f(a: U) -> T`, `g(a: T) -> U` | `f(g(a)) == a` |
| `distributes over g` | `f`, `g` of `(T, T) -> T` | `f(a, g(b, c)) == g(f(a, b), f(a, c))` |
| `monotone increasing in p` (`decreasing`, `non-decreasing`, `non-increasing`) | any `f` with a parameter `p` | larger `p`, the other arguments equal, gives a result larger (or not smaller, …) |
| `injective` | any `f` | different arguments give different results |
| `equivalent to S [within …]` | `f` and `S` of one signature | `f` and `S` give the same result, effects and failures for every input; with `within`, a result in that tolerance |

- **Floats.** A float operation is rarely associative exactly: `associative within rel: r` states it in a tolerance,
  which is what `may reassociate` (§ 6.17) needs of the operation it regroups.
- **On a method of an interface**, a name binds every implementation, as any law does (§ 9): `associative` on a `Monoid`'s
  `combine`.
- **Fails:** a name on a subject of the wrong shape (`associative` on a function of one argument) is refused at build.
- **Checked:** as laws are, by property tests in CI over generated values, and for `equivalent to` by running both on
  the corpora; proven where a prover can.
- **Harvested:** from trait implementations that promise them (`Ord`, a monoid), from documentation, and from bodies
  that a prover shows to have them.

### 6.19 Facts about a collection's contents

**Meaning.** Some properties of a collection's elements are what decide how it can be used: a sorted list can be
searched by halves, a list of distinct elements needs no deduplication, keys in a dense range can index an array instead
of a map. They are named predicates, defined once over the sequences of § 6.15, and used in any expression: an
invariant of a type, a postcondition, a precondition. Tools recognise the names.

```text
type rust 'eve::market::Book'
    invariant bids_sorted : sorted(self.bids, by: price, descending)
    invariant one_per_id  : distinct(self.bids, by: orderID)

function rust 'eve::inventory::slots'(ship: ref<target_type=Ship>) -> list<element_type=u16>
    ensure dense(result, 0 .. count(result))          -- the slots are 0, 1, 2, … with no gap, in some order
```

| Predicate | Holds when |
| --- | --- |
| `sorted(xs [, by: k] [, descending] [, strictly])` | every element is not greater (or, `strictly`, less) than the next, comparing `k` of each |
| `distinct(xs [, by: k])` | no two elements (no two `k`s) are equal |
| `dense(xs, a .. b)` | the elements are exactly the integers of `a .. b`, each once |
| `heap ordered(xs [, by: k])` | every element at `i` is not less than those at `2i + 1` and `2i + 2` |
| `nonempty(xs)` | `count(xs) > 0` |

- A property of a structure's own shape (a balanced tree, a hash table's load) is an invariant of that type, stated by
  its own projections (§ 9, `view`), not one of these names.
- **Checked** as the clause they appear in is; in an invariant, where the invariant is.
- **Harvested:** from types that keep them (`BTreeMap`'s keys are `sorted` and `distinct`), from documentation ("in
  ascending order", "without duplicates"), and from a `sort` or `dedup` just before a value is returned.

### 6.20 Expectations: what usually happens

**Meaning.** An expectation states what a subject's use usually looks like: how large its inputs tend to be, how often
it is called, how likely it is to fail, whether it is on a hot path. An optimizer uses it to choose a layout, a
representation, where to inline and what to keep out of the way; a translator, to choose between two implementations
that are both correct. An expectation never decides correctness: a wrong one makes the program slower, never wrong. It
is therefore never `enforced` and nothing may be discharged by it; it is watched, and a subject used far from its
expectation is reported, as a budget is (§ 6.7). Where an expectation is a person's statement of intent (this list is
small by design), a profile (§ 3.7) is the measurement of a build, and the two are compared.

```text
function rust 'eve::dogma::Fitting::modules_of'(self: ref<target_type=Fitting>, group: u32) -> list<element_type=ModuleID>
    expect count(result) <= 8  usually 0.99
    expect hot

procedure 'wallet.Credit'(requestID: id<entity_type=Requests>, amount: decimal<precision=19, scale=2>)
    expect fails InsufficientFunds  rarely

type rust 'eve::market::Book'
    expect access self.orders by orderID  usually 0.9
    expect scans self.orders  rarely

function rust 'eve::admin::rebuild_index'()
    expect cold
```

- **`expect p [usually f]`:** `p` holds in at least the fraction `f` of calls (all calls, without `usually`, which is
  still only expected). `p` is over the inputs and the result, as a postcondition is.
- **`expect fails X rarely | often | f`**, **`expect hot`**, **`expect cold`**, and **`expect called per s about n`**
  name the cases optimizers use most.
- **How a collection is used.** `expect access p by k [usually f]` (it is looked up by `k`; `by range k` for ranges),
  `expect access p sequential | random`, `expect scans p rarely`, `expect index i of p < n [usually f]`, and `expect
  lookup s hits [usually f]`. They are the indexing assumptions a representation is chosen for: a collection expected to
  be looked up by one key and rarely scanned is a map on that key, one expected to be read in order is an array, and a
  table expected to be selected by a column gets an index on it. Each is compared with the profile's `access`, `index`
  and `lookup` entries (§ 3.7).
- **Never a premise.** An expectation is not a fact at any point (§ 11.1.1): a check discharged by an expectation would
  be wrong in the calls it does not cover.
- **Checked:** watched against the profiles of the builds (§ 3.7) and against traffic, with a `statistic` (§ 6.6) where
  the expectation names a fraction.
- **Harvested:** from the profiles: a candidate expectation for each subject whose measured use is narrow, which a person
  turns into a statement of intent, or leaves to the profile.

### 6.21 Exits: every way a call can end

**Meaning.** A call ends in one of a few ways: it returns; it fails with a failure the subject names, as a thrown
exception, an error value (`Err`, Go's `error`, a T-SQL `THROW`) or a rejected request; it panics or raises what its
host does not check (an unwinding panic, an unchecked exception, a Python exception nobody declared); it is cancelled
at a point where it suspends (an `await`, a `yield`, a tasklet switch, a check of a cancellation token, a timeout that
fires there); or it runs out of what the platform gives (§ 6.14.5). These are the subject's **exits**, and every
callable has a set of them. An optimizer, a prover and a translator need it for every call they see: a call that may
exit early is a point where the state the body has built so far becomes visible to whoever handles the exit, so what
may move across it, and what a caller may assume after it, depend on it.

```text
function rust 'eve_net::frame::encode'(f: Frame) -> bytes
    exits only return                              -- never fails, panics or suspends

function rust 'eve::market::Book::place'(self: ref<target_type=Book>, o: Order) -> Result<OrderID, PlaceError>
    exits only return, fails PlaceError
    on failure unchanged

function rust 'eve_client::net::Session::send'(self: ref<target_type=Session>, m: Message) -> Result<(), NetError>
    exits only return, fails NetError, cancelled
    on cancel unchanged                            -- cancelled before the frame is written, or not at all
```

- **The set is never absent.** A subject's exits are what its clauses state: `fails only` and `fails … when` name its
  failures; `never fails runtime` and `never panics` remove panics and runtime failures; `no suspension` (§ 6.3) removes
  cancellation; `never panics except exhausted` leaves exhaustion alone. `exits only` states the whole set at once, in
  the words `return`, `fails F`, `panics`, `cancelled` and `exhausted`. Where nothing is stated, the set is computed from
  the body, as read and write sets are (§ 6.16): the union of the exits of the calls it makes, of the failures it
  raises, and `cancelled` if it suspends. A callable whose body is not seen and whose file says nothing (a foreign
  function, an unannotated library) has every exit its host allows. The set is recorded in the catalog for every
  subject, so that no consumer has to guess.
- **Cancellation is an exit, not a failure the body raises.** It happens only at a suspension point, and only if the
  subject's caller or runtime cancels it there. `on cancel unchanged` and `on cancel ensure name : p` state what holds
  after it, as `on failure` does after a failure; `cancellation safe` (§ 6.8) is the weakest of them: what is left is
  still usable.
- **The platform decides some exits.** Under `panic abort` (§ 3.6) a panic ends the process, so no caller in it sees the
  state the panic left; a host without exceptions has no unchecked exit but its panics and aborts.
- **Exits compose.** A call's exits are its callee's, mapped: a failure the caller handles is no exit of the caller, one
  it passes on (`?`, a rethrow, an unhandled exception) is. A subject that states a narrower set than its body has fails
  the build, as a stated read set does.
- **Fails:** an exit that the set does not hold is a bug in the subject: a call of `encode` that panics breaks `exits
  only return`.
- **Checked:** by computing the set from the body, where the host lets the compiler see every call; Rust's `Result`
  types and `?`, Java's checked exceptions and Swift's `throws` give the failures, and the panics are found as for
  `never panics`; cancellation from the suspension points of the body. Watched on replays for hosts that cannot be
  analysed (Python), from the exits each call was seen to take.
- **Harvested:** from the manifests at exit, as failures are (§ 6.2), and from the bodies.

### 6.22 Levels: how much of its language a module uses

**Meaning.** A module is written at a level: the most of its language that anything in it, or in what it imports, uses.
The levels are Y's (Y § 3.19), after LCD's levels of common denominator, and each contains the ones below it: `data`,
declarations only, which is what this language can say; `calculations`, functions of values, with no mutable name, loop
or recursion; `structured`, with mutable names, loops and recursion; `shared`, with transactional shared state, arenas
and resources; and `effects`, with capabilities, waits and external effects. A `level` on a module is a ceiling
that nothing in the module may rise above, so that the module can be carried by every backend and host that carries
that level.

It is a field of the module's subject in the constraint data, which is JSON (bare Rust § 2.5), and the text form
prints it under the file's `module` line:

```json
{ "kind": "module", "name": "pricing", "visible": "public", "level": "calculations" }
```

- **Subject:** the module.
- **Fails:** a construct above the level, at build, naming the construct, where it is and the level it needs; and an
  import of a module whose level is higher.
- **Checked:** by the Y compiler, which infers every module's level, for Y; for another host, by the analysis its
  profile names (§ 3.3), which looks for mutation, loops and recursion, shared state and effects, or by the bounds on
  effects of § 6.3 that a lower level implies (`pure`, `no io`, `effects none`), which are checked as those are.
- **Harvested:** from the Y compiler's inferred level, and for other hosts from the effect manifests and the bodies:
  the level a module was seen to need is proposed as its ceiling, for a person to pin.
- **For bare Rust:** a module subject's `level` covers the module, the modules inside it (Y's facets) and its files,
  but not its `tests`. `bare-rust` reads what each needs from the bare bodies and the constraint data: a function body
  is `calculations`; a loop, an assignment, `&mut`, a parameter the function changes (write access: Y's `mutable`) or
  recursion, direct or through other functions, is `structured`; a parameter the lowering marks `"injected": true`
  whose type is `Shared<…>` is `shared`; any other injected parameter, which is a capability, an `effects only …`
  clause, `async`, `.await`, a wait (`waits until`), or a region that waits `forever` or is detached is `effects`. A
  call of a function of the crate needs what that function needs. A call into a library needs the level its module
  has in a table, `libraries/levels.json` (`y_runtime::shared` is `shared`, `y_runtime::task` and `std::fs` are
  `effects`); every library is listed, at least by its crate, and a call into one that is not needs `effects`, as
  nothing says it has none. Which function a call calls is read one of two ways. Where the woven crate builds, weave
  with cargo takes it from rustc: the crate's MIR names the item each call resolved to, so a method is its receiver's
  type's whatever names the receiver (a closure's parameter, a chained call), and a library's function is found where
  it is defined, through a re-export, a renamed import or a local that holds it. A trait's method called on a type
  parameter may be any impl of the trait in the crate; a library's trait method needs the level of its type's module
  and of its trait's. Without a build (a crate that does not compile, a weave without cargo, `bare-rust levels`
  without `--crate`), a call is matched through the file's `use` declarations, and a method call takes its receiver's
  type from the constraint data when the receiver is a parameter or a typed local; when the type is unknown, the call,
  like a name defined more than once out of the caller's reach, needs the most that any function it may be needs.
  Weave says which way it read the calls. An import of a module pinned higher needs that module's level. Weave fails
  on anything above its module's level, and `bare-rust levels <bare-dir>` prints the level each module needs and why,
  with the level pinned on it (with `--crate <dir> --out <dir>`, from rustc's calls). `bare-rust levels --libraries
  <crate>…` checks the table against a library crate's MIR: a public function that calls a library at a higher level
  than the table gives the function, or one the table does not list, fails. The table's `trusted` lets a function make
  the calls it lists, with why, and no others; an entry whose calls the function no longer makes fails too. Every module of the examples is pinned at the level it needs, by either reading, and the examples' checks hold
  each facet's pin to a second inference made from the Y source alone.

### 6.23 Regions: how a piece of code may execute

**Meaning.** How code may execute is not what it computes: how long it may take, whether it may wait without end,
whether it may be cancelled, whether anything else may see it half done, whether it may run more than once. These are
execution constraints, stated on a region: a whole callable, or a named part of one. They are Y's regions (Y § 3.8),
and they gather clauses that other sections already have for whole subjects: `atomic` and `cancellation safe` (§ 6.8),
`on failure unchanged` (§ 6.21) and budgets of time (§ 6.7). A constraint says what must hold, never how: a lock, a
transaction, multi-version concurrency control or a proof that nothing is shared may each keep `atomic`, and the host or
the backend chooses.

```text
region 'shop.checkout.hold_and_charge#hold_one_unit'
    atomic                                 -- nothing else observes it half done
    isolation serializable                 -- the default for atomic
    may run again                          -- the runtime may re-run it after a conflict
    within 5 s
    on failure unchanged

region 'memory.setup.new_game#fetch'
    within game_rules.fetch_time           -- every wait inside it shares the deadline
```

- **Subject:** `region 'function#name'`, by the stable name of the callable and the region's name in it. A host with no
  named regions states them on the whole callable, its `function` subject, or names a block by its own means (a label,
  an attribute, a marker comment the profile names).
- **The constraints:** `within d`, a deadline every wait in the region shares; `forever because "…"`, a wait without
  bound, assumed (§ 10.2); `detach because "…"`, work that may outlive the callable that starts it, assumed the same
  way; `uncancellable`, cancellation held until it ends, with `within`; `atomic`; `isolation serializable` or
  `isolation snapshot`, what an atomic region observes, serializable unless it says otherwise; `may run again`, which
  permits the host to run it more than once; `waits until name : p`, it starts only once `p` holds, named as Y names
  it; `on failure unchanged`.
- **What goes with what:** `isolation`, `may run again` and `waits until` are for an atomic region; `uncancellable`
  needs a `within`; a region is `within` a bound or waits `forever`, not both; and a region says each constraint
  once.
- **What follows:** `may run again` requires that the region have no effect but on the state the run's commit makes
  visible, which is `effects only` (§ 6.3) over that state, so a region that may run again and calls the network
  breaks it at build. Y's atomic regions imply it.
- **Fails:** a region that a run shows half done to another, that reads two states a serializable order cannot
  explain, that outlives its deadline, or that is cancelled while uncancellable.
- **In the constraint data:** a subject of kind `region`, named `<function>#<region>`, whose clauses are of kind
  `region`, one constraint each, with `supervise(…)`, the policy for the failures of the region's tasks, among them.
  What a Y atomic region implies, `may run again` and `on failure unchanged`, its lowering states:

  ```json
  { "kind": "region", "name": "warehouse_example::warehouse::stock::units_on_shelf#counted",
    "clauses": [ { "kind": "region", "text": "atomic", "source": "y/src/warehouse.y:188" },
                 { "kind": "region", "text": "isolation snapshot", "source": "y/src/warehouse.y:188" },
                 { "kind": "region", "text": "may run again", "source": "y/src/warehouse.y:188" },
                 { "kind": "region", "text": "on failure unchanged", "source": "y/src/warehouse.y:188" } ] }
  ```

  For bare Rust, `bare-rust` checks the rules above and that the function is a subject of the file, and weave binds
  the region to its body. The function marks where the region starts, `// y: region counted is …` or, for an
  alternative of an `orelse`, `// y: #name`, and a marker that states the constraints states the same ones as the
  clauses. The code the marker starts is what follows it in its block, up to `// y: end region counted` or the block's
  end, or the statement it ends for a marker after code; a region that Y statements follow in its block says where it
  ends. Weave reads that code's syntax and holds it to each constraint. Which function each call calls it takes from
  rustc where the woven crate builds, as § 6.22 does, so `atomic` bound to a local or re-exported is still the
  runtime's transaction; without a build, it resolves the names through the file's `use` declarations and the
  constraint data. An atomic region runs in a transaction of `y_runtime::shared`, and, as it may
  run again, uses no capability but `shared` state, does not wait, calls no function passed in, and calls nothing
  that needs `effects` (§ 6.22); a snapshot one passes `Isolation::Snapshot`; `within` binds a deadline that every
  wait takes, by whatever name it binds it to; `forever` gives the wait a parameter of type `Forever`;
  `detach` and `supervise` call the runtime's `detach` and `supervised`; `uncancellable` holds cancellation with a
  `Cancellation`; `waits until name` marks its condition and retries until it holds. A region that is not in its function's body, that says other constraints than its marker, or whose
  code does not keep them is refused.
- **Checked:** by the Y compiler for Y; by host analyses where they exist (a transaction's isolation level in T-SQL, a
  lock held across the region in Rust); by concurrency explorers and replays in CI, which run the region's interleavings
  and compare them with a serial order (Loom, Shuttle, Y's own explorer); watched in production from conflict metrics
  and deadline expiries.
- **Harvested:** from a host's transactions and their isolation levels, its locks, its timeouts and the blocks it
  shields from cancellation.

---

## 7. Lifetime

**Meaning.** Every entity begins and ends, and something is responsible for its end. Lifetime constraints say how long
entities live, what keeps them alive, what may refer to them and in what way, what must happen when they end, and that
nothing uses them afterwards. They are written once, over a core that belongs to no host, and each host's memory model
gives the core its precise meaning.

### 7.1 Entities, references and three kinds of end

An **entity** is anything with identity and a lifetime: an object in a host's heap, a value on a stack, a closure, a
row, a handle to something outside memory. Its lifetime is an interval from `begin(e)` to `end(e)`. A **reference** is a
path by which one entity, or a scope, reaches another.

There are three ends, and constraints name the one they mean:

- **The logical end, `end(e)`:** the moment after which no correct program uses `e`. Under ownership it is when the
  owner drops it; under counting, when its strong count reaches zero; under tracing, when the last strong path from a
  root is removed; for a row, its deletion.
- **Release, `released(e)`:** the moment a resource gives back what it holds outside memory: a file closed, a lock
  released, a connection returned, a GPU buffer freed. Under ownership and counting it coincides with the logical end,
  through `Drop`, `deinit` or a destructor; under tracing it must be explicit, through `close`, `Dispose` or `with`,
  because finalisation is not guaranteed to happen.
- **Reclamation, `reclaimed(e)`:** the moment its memory is returned. Constraints rarely name it; budgets do.

A beginning has two moments too, for an entity whose host runs code to make it (a constructor, an initialiser, a
factory that fills a struct): **`begin(e)`**, when its memory is there, and **`constructed(e)`**, when that code has
returned successfully. Between them only the constructing code may use it; a construction that fails never reaches
`constructed(e)`, and the entity is never usable.

A **scope** is a span of time that lifetimes are stated against: `call`, `transaction`, `tick`, `request`, `session`,
`frame`, `task`, `thread`, `process`, a named `scope`, or another entity's lifetime.

### 7.2 How a reference holds

Every reference holds its target in one of five ways. The five are the vocabulary of every lifetime constraint.

| Hold | Keeps the target alive | May outlive the target | Use requires | Examples |
| --- | --- | --- | --- | --- |
| `owns` | Yes, alone: the holder is responsible for ending it | No; it ends with or before the holder, unless moved | Nothing | A Rust value or `Box`; a `unique_ptr`; a sole strong reference; a row over the rows that name it with `on end cascade` |
| `shares` | Yes, jointly: it cannot end while any sharer holds it | No | Nothing | `Rc`, `Arc`, `shared_ptr`, a Swift strong reference, a CPython reference, any reference under tracing; a row that names its target with `on end restrict` |
| `borrows` | No | Never: the reference ends before the target, and within its scope | Nothing | Rust `&` and `&mut`; a non-escaping closure or parameter |
| `observes` | No | Yes | A check that the target is still alive | Rust `Weak`, Java `WeakReference`, Swift `weak`, `weak_ptr`; a row that names its target with `on end nullify`; a reference across shards |
| `identifies` | No | Yes | Lookup: it carries identity, not access | A row id, an entity handle, Pony's `tag`; a row that names its target with `on end dangle` |

Some host references are combinations, written as such:

- **Swift's `unowned`** does not keep its target alive and is stored, not scoped, so it is `observes` with `lifetime
  target outlives self` asserted; the runtime traps on a use after the target ends, which makes the assertion checked.
  `unowned(unsafe)` is the same with the assertion as an `assume`.
- **Soft references** (Java's `SoftReference`) are `observes softly`: the collector may clear them under memory pressure
  even while the target is otherwise reachable.
- **Ephemerons** (`WeakMap` in JavaScript, `ConditionalWeakTable` in .NET, `WeakHashMap` in Java) observe their keys and
  keep each value only while its key lives: `hold cache.keys : observes` with `lifetime cache.values ends with key`.
- **Phantom references and cleaners** notify after the end; they are not holds but a form of finalisation, and `released
  before end` forbids relying on them for release.

```text
type swift 'Scene.Node'
    hold children : owns
    hold parent   : observes
    hold delegate : observes
    lifetime delegate outlives self          -- `unowned`, checked by the runtime's trap

function rust 'eve_cache::Cache::get'(self: ref<target_type=Cache>, key: ref<target_type=Key>) -> ref<target_type=Entry>?
    hold result : borrows from self
```

A value that moves is not one of the five: it keeps its identity and its holder changes. `hold result : moved from v` and
the expression `result moved from v` say which value came back where `==` cannot (§ 6.14.3).

### 7.3 Lifetime predicates

Each predicate is defined over the core; § 7.4 and § 7.5 give its meaning under each model.

- **`lifetime a outlives b`:** `end(a)` is never before `end(b)`.
- **`lifetime e within s`:** `e` begins and ends inside scope `s`.
- **`lifetime e ends with h`:** `e` ends when `h` does, and not before: the usual consequence of `owns`.
- **`lifetime e does not escape [s]`:** after `s` ends (by default, the call), no reference to `e` that `owns`, `shares`
  or `borrows` it is reachable from anywhere outside `s`. `escapes only to p` permits the named paths.
- **`lifetime e ends within d after x`:** `e` has ended within duration `d` after event `x`. Under tracing this is the
  leak constraint: after a window closes, nothing keeps its listeners reachable for more than two frames.
- **`lifetime e at most d [after x]`:** `e` lives no longer than `d`, which is how data retention is written for rows.
- **`lifetime e released by f exactly once`:** release happens through `f`, once, on every path. `released before end`
  forbids leaving release to finalisation. `ends by f exactly once` is the linear form: the entity is consumed exactly
  once.
- **`lifetime self released at end`:** said of a type: the end of each value of it is also its release (§ 7.1). It
  gives back something other than memory: a lock is unlocked, a file closed, a buffer flushed, a borrow flag reset, a
  connection closed. The moment a value ends is then part of what the program does, and a rewrite that moves it
  changes the program, where moving the end of any other value only changes when memory is reclaimed. A type has it
  when its host says so through a destructor (Rust's `Drop`, a C++ destructor under RAII, Swift's `deinit`), or when it
  owns a value of a type that has it. `bind` keeps the end of such a value where the source has it (bare-rust
  proposal § 2.2).
- **`lifetime e usable after construct`:** nothing but the code that constructs `e` (its constructor, and what that
  calls with `e`) uses `e` before `constructed(e)`, and nothing uses it at all if construction fails. It is the rule that
  `this` does not escape a constructor in Java, C# and C++, that a virtual call in a C++ constructor does not reach a
  derived class not yet built, and that a Rust value is never seen half made (which safe Rust gives by construction: a
  struct exists only once every field is given). Said of a type, it holds for each value of it.
- **`lifetime e reclaimed only after released`:** the memory of `e` is not returned before its release has run: a
  value whose type has a destructor is not freed without it (C's `free` of a C++ object, a `delete` through a base
  class whose destructor is not virtual, an arena reset over values that own resources). It is the reverse of
  `released before end`, and it is not a promise that the release runs: Rust's `mem::forget` keeps it, because what is
  forgotten is never reclaimed either. Said of a type with `released at end`, it holds for each value of it.
- **`lifetime e cleared on end`:** when `e` ends, no storage the program used for it still holds its contents: the
  entity's own memory is overwritten before anything may reuse it, and so is every copy the program made of it on the
  way (the old buffer of a growing collection, a value moved out of a place that kept its bytes). Memory after an end
  is observable outside the program (a core dump, swapped pages, the next tenant of a reused buffer), and the write
  that clears it is one no later read uses, which an optimizer would otherwise remove. `lifetime every value labelled
  key cleared on end` states it for everything that carries a label (§ 6.9). For rows, the row's pages are overwritten
  or its storage is encrypted with a key that ends with it. Rust's `zeroize` and C's `explicit_bzero` and
  `memset_s` are how hosts do it; under tracing, a host whose strings cannot be overwritten cannot keep it for a
  string, and its profile says so (§ 3.3).
- **`lifetime e no use after end`:** no reference is used to reach `e` after `end(e)`; for resources, `no use after
  release`.
- **`lifetime e reachable only from p, q`:** every strong path to `e` from a root passes through one of the named
  holders. It forbids leaks into global caches, event buses and static collections.
- **`lifetime e ends in call [when p]`:** `e` is dropped before the call returns, where `p` holds, unless it was moved
  out; **`lifetime a, b survive call`:** none of them ends in the call (§ 6.14.4).
- **`lifetime e on end refuse | invalidate`:** under tracked references, ending `e` while tracked references remain
  either fails or invalidates them.
- **`lifetime e stable address [while held by p]`:** `e` does not move while the named paths refer to it, as Rust's
  `Pin` or a pinned handle in a moving collector; needed at boundaries with foreign code.
- **`lifetime every item of e ends with p`:** the same predicate over the many entities a sequence holds, read at the
  moment the predicate's event happens: the items `e` has then, each an entity. `lifetime every item of yields(self)
  ends with self` says that the elements an iterator has not yet given end when the iterator does, which is what
  dropping a `vec::IntoIter` does to them; the elements already given are not in `yields(self)` and moved out.
- **`lifetime every ref<target_type=T> [obtained from p] within s`:** the same predicate over every reference of a type, or every
  reference obtained through a path; this is how a rule for a whole interface is written.
- **`no strong cycle [through f, g]`:** no cycle of `owns` and `shares` references through the named fields, which under
  counting is a leak.
- **`instances [per s] at most n`** and **`instances attributed to s are none within d after e`:** bounds on how many
  entities of a type are alive, in total or per scope. The second is the practical leak check for a session, a fight or
  a scene.

### 7.4 Memory models

A host profile declares its model, and a type may override it where the host mixes models (`Rc` inside Rust,
`shared_ptr` inside C++, an arena inside anything), with `model` in the subject.

- **Ownership (Rust; C++ with `unique_ptr` and values).** Every entity has exactly one owner at a time; ownership moves;
  the end is when the owner drops it; borrows are scoped and checked by the compiler. `Drop` runs at most once, and
  exactly once unless the value is leaked (`mem::forget`, a cycle of `Rc`), which is safe in Rust and so must be ruled
  out by a constraint and checked by leak checks in CI. Borrow constraints on safe Rust are proven by the compiler from
  signatures, and the file states them so that they survive translation and so that `weave` can write them (§ 11.2).
  What the compiler does not prove, the file adds: that an owned, `'static` or `Rc` value does not escape, that an `Rc`
  graph has no strong cycle, that a handle is dropped before the tick ends.
- **Tracing (Java, C#, Kotlin, Go, JavaScript and TypeScript).** An entity lives while it is reachable from a root by
  strong references; its logical end is when it becomes unreachable, which the collector notices later and at no defined
  moment; finalisation is not guaranteed and may resurrect. So under tracing, end constraints are reachability
  constraints, release is always explicit, and `owns` means the holder dominates the entity in the heap graph: every
  strong path to it passes through the holder.
- **Counting (Swift and Objective-C under ARC, CPython, PHP, `shared_ptr`, `Rc` and `Arc`).** An entity lives while its
  strong count is above zero and ends, deterministically, when the count reaches zero. Weak references do not count and
  are cleared or fail on use. A strong cycle never reaches zero, so it leaks, unless a cycle collector (CPython, PHP)
  runs, in which case it ends later and at no defined moment, as under tracing. Counts are evidence, not proof: a count
  that returns to its old value can hide an escape, and immortal objects and deferred counting in recent CPython make
  counts unreliable, so the checks that matter use referrer queries.
- **Tracked references (generational handles, slot maps, entity systems; Vale's generational references).** The runtime
  knows every reference to an entity, or can tell at each use whether its target is still the one it was taken for. An
  entity is ended explicitly; ending it either fails while tracked references remain (`on end refuse`) or invalidates
  them (`on end invalidate`), and a use after end is detected deterministically at the use. Game engines use this model
  for entities.
- **Regions (arenas, frame allocators, pools; Cyclone's regions).** Entities in a region end together when the region is
  reset: `within` the region's scope is proven by construction. A reference into a region from outside it must be
  `observes` or `identifies`, or end before the reset; `does not escape` means no such reference remains at the reset,
  checked by walking the references into the region in CI.
- **Manual (C, and C++ outside RAII).** An entity ends when it is freed; nothing in the host checks anything. Every
  predicate is checked by sanitizers and leak checkers in CI and by wrappers at boundaries, and is otherwise assumed.
- **Rows.** A row begins at insert and ends at delete. A foreign key's rule on end is a hold, seen from the referring
  row: with `cascade`, the named row owns the rows that name it; with `restrict`, the referring row shares its target,
  which cannot end while named; with `nullify`, it observes; with `dangle`, it identifies. A reference across shards can
  only observe or identify, since no shard can keep another's row alive. The engine checks lifetime constraints on rows
  at each write, as it checks invariants.

### 7.5 Meaning under each model

| Predicate | Ownership | Tracing | Counting | Tracked | Rows |
| --- | --- | --- | --- | --- | --- |
| `end(e)` | Owner drops it | Last strong path from a root removed | Strong count reaches zero | Explicit end | Delete |
| `owns` | The owner | The holder dominates `e` | The only strong reference | The owning handle | The named row, over rows that name it `on end cascade` |
| `shares` | `Rc`, `Arc` | Any strong reference | A strong reference | A counted handle | A row that names `e` `on end restrict` |
| `borrows` | `&`, `&mut`, checked | A reference not retained after its scope | A reference not retained after its scope | A handle used only within its scope | Read within the transaction |
| `observes` | `Weak` | `WeakReference` | `weak`; `unowned` with `outlives` | A handle checked on use | `on end nullify`, or across shards |
| `does not escape` | Proven for borrows; analysis or watched for owned, `'static` and `Rc` values | Escape analysis, or reachability at scope end | Referrer query at scope end | No tracked reference outside the scope | No row outside the transaction names it |
| `ends within d after x` | Proven where `x` owns `e`; leak checks otherwise | Unreachable within `d`, by weak probe or snapshot | Count zero within `d`, by weak observer | Ended within `d` | Deleted within `d` |
| `released by f exactly once` | `Drop` runs at most once; exactly once where leaks are ruled out | `f` called once on every path; never left to the finaliser | `deinit` runs once, or `f` called once | `f` called once | — |
| `no use after end` | Proven in safe code | Use after release, for resources | Proven for `weak`; trapped for `unowned` | Detected at use | A lookup of a deleted row is absent |
| `reachable only from` | Proven for owned data | Dominator query on snapshots | Referrer query on snapshots | Referrer set | Every foreign key naming it is listed |
| `no strong cycle` | Graph walk over `Rc` in CI | Not a leak; still checkable | Graph walk; a cycle is a leak | Graph walk | `acyclic` |

Regions and manual management are described in § 7.4, since their meaning does not differ by predicate: under regions
every end is the reset, and under manual management every predicate is checked by sanitizers or assumed.

### 7.6 Resources

A `resource` is a type that holds something outside memory. Its constraints combine a protocol (§ 6.5) with lifetime:
how it is released, within what scope, and by whom.

```text
resource java 'java.sql.Connection'
    protocol begin (createStatement | prepareStatement | commit | rollback)* close
    lifetime self released by close exactly once
    lifetime self released before end            -- never left to the finaliser
    lifetime self within request                 -- never kept across requests
    confined to thread

resource csharp 'Studio.Audio.Voice'
    hold owner : observes
    lifetime self released by Dispose exactly once
    lifetime self ends within 1 s after end(owner)
```

Under ownership and counting, release is the end and is usually proven; under tracing, `released before end` is the
constraint that forbids relying on finalisation, and it is checked by analysers that follow every path from acquisition
to its `using`, `try`-with-resources or `with`.

### 7.7 Harvesting lifetimes

The proposal records the lifetimes and owners of the Python objects in the hot stores by tracing those stores at their
managers' interfaces on test clusters replaying recorded fights, since in-memory state is the manifests' blind spot
(§ 6.13 of the proposal). That evidence becomes lifetime constraints on the Python types, and those constraints inform
where each piece of state goes: a value with no row; a table that need not be durable, where it can be rebuilt; a
durable row in the right shard; or nothing, deleted with its cache. When state moves into rows, a Python object that
owns others becomes a row that the others name with `on end cascade`; one that observes becomes a reference that
nullifies, or an identity that dangles. Elsewhere, lifetimes are harvested from heap snapshots on test clusters
replaying recorded traffic: dominators give `owns`, referrer sets give `reachable only from`, and populations by scope
give `instances`.

### 7.8 What a pointer may point at

**Meaning.** A host pointer, `ptr<target_type=T>` (§ 3.4), is an address and nothing more: it may be anything, and reading through
it is defined only if something else says it is safe. Every safer kind of pointer is the same address with constraints
on what it may point at, and a reference is the most constrained of them. The kinds are refinements of `ptr<target_type=T>`, named
predicates of § 4 written with `where`, so that a parameter, a field or a result says which kind it is and a checker or
an optimizer knows what it may assume of it.

| Kind | Refinement | May point at |
| --- | --- | --- |
| A pointer that may be invalid | `ptr<target_type=T>` | Any address, or none. It may be held, compared and passed on; reading or writing through it is undefined. |
| A valid pointer | `ptr<target_type=T> where valid(value)` | Null, or an address the process can reach inside an allocation that has begun and not ended, at any byte of it, not only at the start of an entity. |
| Not null | `where nonnull(value)` | Anything but null; with `valid`, a live allocation. |
| A cursor over `a` | `where cursor(value, a)` | An element of the array `a`, or one past its last: it may be compared and stepped, and read through only where it is not at the end. |
| An element of `a` | `where element(value, a)` | The start of an element of `a`: `value - base(a)` is a multiple of `size_of(T)`, and less than `count(a) * size_of(T)`. |
| Aligned | `where aligned(value)` | An address that is a multiple of the alignment of `T`. |
| A C string | `ptr<target_type=u8> where terminated(value, 0)` | A valid address from which a `0` byte comes before the end of its allocation; `terminated(value, v)` for any sentinel. |
| Data | `where data(value)` | Memory the program reads and writes: its stacks, heaps and data segments, never its code. |
| Code | `fn(…) -> … where code(value)` | The start of a function in the code the process loaded and may run, never data. |
| A reference | `ref<target_type=T>` | `ptr<target_type=T> where valid(value) and nonnull(value) and aligned(value)`, at the start of a live entity of type `T`, which stays live while the reference is used (§ 7.2), with the access of § 8.1. |

```text
function c 'eve_native::name_of'(p: ptr<target_type=u8> where valid(value) and terminated(value, 0)) -> u64
    ensure len : result == position(value, 0)          -- the length, up to the terminator

function rust 'eve::mesh::Vertices::iter_raw'(self: ref<target_type=Vertices>) -> (ptr<target_type=Vertex>, ptr<target_type=Vertex>)
    ensure first : element(result.0, self.items) or count(self.items) == 0
    ensure end   : cursor(result.1, self.items) and result.1 == base(self.items) + count(self.items)

type c 'eve_native::Callbacks'
    field on_tick : fn(u64) where code(value)
```

- **Each kind is a set of constraints, and they combine:** a valid, aligned element of `a` is all three. A use that
  needs more than its pointer's kind gives (reading through a cursor at its end, calling through a pointer that is not
  `code`) is the defect, found where the use is.
- **For the whole process,** two constraints are worth stating once, in its platform (§ 3.6), since they hold of every
  pointer in it: `code read only`, that no write reaches memory that holds code, and `calls only code`, that every
  indirect call goes to an address that is `code`. They are what W^X pages and control-flow integrity give, and the
  build's flags and the loader's settings are checked against them, as every platform fact is.
- **Provenance.** `valid`, `cursor` and `element` are about the allocation the pointer came from, not only the
  address: a pointer stepped from one array onto the next is not an element of the second, whatever its address. This
  is the rule of C, C++ and Rust (strict provenance), and it is what lets an optimizer assume two pointers from
  different allocations never meet (§ 8.3).
- **Checked:** by the host where it can (a reference in Rust, Swift and safe C#; a non-null pointer type; a `&[T]` is a
  valid element pointer with its size); in CI by Miri for Rust's raw pointers and by the address and undefined-behaviour
  sanitizers for C and C++; by hardware where the platform has it (CHERI capabilities carry bounds, and are `valid` and
  `cursor` in hardware).
- **Harvested:** from the host's types (`&T`, `NonNull<T>`, `*const c_char` with `CStr::from_ptr` beside it,
  `std::span`, `extern "C" fn`), and from the documentation of C APIs ("must be NUL-terminated", "one past the end",
  "may be null").

---

## 8. Mutability and aliasing

**Meaning.** Whether something may change depends both on what this path may do and on what every other path may do, so
mutability and aliasing are stated together. Every path to an entity has rights (to identify, read or write it) and a
guarantee about every other path (none may touch it, others may only read, or no guarantee). From those two, the
language states read-only views and true immutability, values written once, collections that only grow, values that only
move one way, hidden caches that leave what can be seen unchanged, and references that are exclusive, sole or disjoint.

### 8.1 Access: rights and guarantees

Every path to an entity has, for some interval, two properties:

- **Rights through this path:** `identify` (identity only), `read`, or `write` (which includes read).
- **The guarantee about every other path:** `exclusive` (no other path may read or write), `readers` (others may read;
  no path may write, this one included unless it has `write`), or `any` (no guarantee).

Six combinations matter, and they cover the hosts in use. Pony's reference capabilities are the closest existing system,
and are given for reference; Pony's `ref` and `box` also deny every alias in other actors, which here is `confined to
isolation`, so the correspondence for those two holds with that clause added.

| Access | Means | Pony | Rust | C++ | Java, C# | Swift | Python |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `write exclusive` | Only this path reads or writes | `iso` | `&mut T`; an owned value | — | — | `inout` | — |
| `write readers` | This path writes; others may only read through views | `trn` | — | — | — | — | — |
| `write any` | This path writes; others may too | `ref`, with confinement | `&RefCell<T>`, `&Cell<T>`, `*mut T` | `T&`, `T*` | Any reference | A class reference | Any reference |
| `read readers` | Nobody writes while this holds: the entity is immutable for the interval | `val` | `&T` without interior mutability | A `const` object, shallowly | A reference to an immutable object | A value type | A reference to a frozen object |
| `read any` | This path cannot write; others may | `box`, with confinement | `*const T` | `const T&`, `const T*`, shallowly | A read-only view or interface | — | A read-only proxy |
| `identify any` | Identity only | `tag` | An id or handle | An opaque handle | An id | An id | An id |

A host pointer, `ptr<target_type=T>`, has rights and never a guarantee, so its access is `read any` or `write any`, and the file
writes only the rights. `*const T` is `ptr<target_type=T>` with `read`, a constraint. `*mut T` is `ptr<target_type=T>` with no access: `write
any` constrains nothing, so writing through a pointer is the absence of the read-only constraint rather than a
constraint of its own. That is where a pointer and a reference differ: `&mut T` is `write exclusive`, and its
exclusivity is a constraint that an optimiser relies on and `*mut T` does not have. What a reference adds to a pointer
is the guarantee and validity: a reference is not null, is aligned, and its target lives while it is used, where a
pointer may dangle, and whoever reads or writes through it must know otherwise (in Rust, in `unsafe` code). What the two
share is that each designates a place, so what other paths write shows through it, and that neither constructs nor ends
what it reaches: a slot holds a pointer as it holds any value. The kinds of pointer between `ptr<target_type=T>` and `ref<target_type=T>`, and
what each may point at, are in § 7.8.

The distinction between `read readers` and `read any` is the one most languages blur: a `const` reference in C++ and a
read-only interface in Java promise only that this path does not write, not that the entity does not change. And C++'s
`const` stops at pointers, so it can prove a shallow `readonly` but not a deep one.

### 8.2 Mutability predicates

- **`access p : rights guarantee`** is the general form, for a parameter, result, field or path: `access cache : read
  readers`.
- **`readonly p`** is `access p : read any`: the subject does not write through `p`, deeply (§ 8.4). On a parameter, it
  says the caller's value is unchanged by the call.
- **`immutable e [deep | shallow] [after x]`:** no path writes `e` for its whole lifetime, or after event `x`; **`frozen
  after x`** is its form for the subject itself (`frozen after construct`, `frozen after publish`).
- **`write once f`:** `f` is assigned exactly once, before any read: Kotlin's `val` and Java's `final` fields give it,
  shallowly; C#'s `readonly` fields and `init` accessors limit writes to construction, which `write once` tightens to
  one; a column set at insert and never updated is its form for rows.
- **`append only [t]`:** a table or collection gains entries and never changes or loses one: logs, ledgers, event
  streams.
- **`monotonic f increasing | non-decreasing | decreasing | non-increasing`:** versions, sequences, skill points, closed
  ticks.
- **`mutates only p, q`:** the in-memory counterpart of `writes only`: the subject writes nothing reachable from its
  inputs except through the named paths. A path may name one element of a collection, `self[key]`: every other element
  is unchanged (§ 6.14.8). It is the subject's write set (§ 6.16), may be conditional (`when`, § 6.3), and decides
  which facts survive a call to it (§ 11.1.1).
- **`interior p`** names hidden mutable state inside an otherwise immutable entity (a cache, a memo, a lazily computed
  field), and **`unchanged view(e) = …`** states what must stay the same despite it: the entity is immutable as seen
  through the named pure projection. This is how Rust's `Cell` and `OnceCell` and C++'s `mutable` are stated honestly.

### 8.3 Aliasing predicates

- **`exclusive p`** is `access p : write exclusive`: nothing else can see the entity while this path holds it. C's
  `restrict`, Rust's `&mut`, Pony's `iso`, Clean's unique types.
- **`sole e`:** at most one strong reference to `e` exists: the holder may hand it on, but never copy it.
- **`disjoint a, b`:** no entity is reachable, by `owns`, from both; two parameters may be written in parallel without
  one seeing the other's writes.
- **`shared readonly e`:** many paths, none writing; one way to be `shareable` (§ 6.8).

```text
function python 'eve.fitting.FittingSvc.ApplyFit'(self, ship: ref<target_type=Ship>, fit: ref<target_type=Fit>)
    readonly fit                   -- harvested: no recorded call changed it
    mutates only ship.modules, ship.cargo
    disjoint ship, fit

type rust 'eve_client::predict::World'
    interior memo
    unchanged view(self) = (self.rows, self.tick)

table log.Records
    append only
table space.Shards
    monotonic closedTick non-decreasing
```

### 8.4 Depth and interior mutability

Depth follows `owns` by default, because that is what callers mean: a `readonly` ship means neither the ship nor the
modules it owns change. It stops at `shares`, `observes` and `identifies`, because what is shared or observed is someone
else's to change; `transitively` extends a predicate across `shares` as well, where that is what is meant. This is why
holding is stated first (§ 7.2): under tracing, every reference `shares` unless a field is declared to `own`, so a deep
`readonly` on a Java object covers what it owns, not the whole heap reachable from it. `shallow` limits a predicate to
the entity's own fields, which is what Java's `final` and C#'s `readonly` fields give by themselves.

### 8.5 Per host

| Host | Proven by the host | Enforced at run time | Watched |
| --- | --- | --- | --- |
| Rust | Access through `&` and `&mut`, deep in safe code apart from interior mutability; `Send` and `Sync` | — | `interior` against `unchanged`, by comparing projections on replays |
| T-SQL and the engine | — | `write once`, `append only`, `monotonic` and `immutable` columns, at each write | Replicas, while a table is on SQL Server |
| Python | — | Read-only proxies and frozen objects at boundaries | Deep hashes of parameters before and after calls, on replays |
| C#, Java, Kotlin | `readonly`, `final`, `val` and `init`, shallowly; records and immutable collections | Wrappers that hand out read-only views | Deep comparisons on replays; analysers for `this` escaping before construction ends |
| Swift | Value types; `let`; `Sendable` | — | Memory graph comparisons |
| C++ | `const`, shallowly | — | Deep comparisons in CI |
| JavaScript, TypeScript | `readonly` at compile time only | A deep freeze, for values the subject owns: freezing changes the caller's object for good, and a proxy changes its identity | Deep comparisons on replays |

### 8.6 Harvesting mutability and aliasing

The manifests and replays show, per call, which parameters were changed, which were never changed, and which overlapped.
A parameter never changed across the recordings becomes a candidate `readonly`; a field assigned once in every recorded
lifetime becomes `write once`; a value seen only to grow becomes `monotonic`; two parameters never reaching a common
entity become `disjoint`. Each is watched first, like every harvested constraint, and checked against the code for a
path that breaks it before it is enforced.

---

## 9. Composition

- **Named predicates.** `predicate flies(s: space.Ships) = s.pilotID == actor` is defined once and used in any
  expression.
- **Blocks.** `block in_space(s: space.Ships)` names a sequence of `require` clauses with their failures; `use
  in_space(m.ship)` expands it in place, in order. A check repeated across hundreds of methods ("the actor flies this
  ship, is in space, is not in warp") is written once and keeps its original order everywhere.
- **Refinement types.** `type Quantity = i64 where value >= 0` and `type ShipName = string where …` are named and reused
  in every signature.
- **Generics.** `for T where T: Comparable` binds a type parameter; a clause stated for every `T` applies to every
  realization (§ 3.5).
- **Interfaces.** Clauses on an `interface` bind every implementation, under the rules of behavioural subtyping
  (§ 6.10). "Every implementation of `Repository.save` is idempotent by its id" is one clause. What one implementation
  adds is an `impl` subject (§ 6.11).
- **Model state.** The language has no ghost variables. What a specification needs as a model of an entity's state is a
  named pure projection over what exists, `view(e)`, declared in the catalog and usable anywhere an expression is.
- **Modules and `use`.** Names are qualified by module; `use` imports predicates, blocks, failures, quantities, labels
  and scopes.

```text
failure NotYours : user
failure Docked   : user
failure InWarp   : user
failure Gone     : user

block in_space(s: space.Ships)
    require flies_it : flies(s)                     else NotYours
    require undocked : s.location != Docked         else Docked
    require not_warp : s.location != Warping        else InWarp

procedure space.LaunchDrones(shipID: u64, droneIDs: list<element_type=u64>)
    from Player
    let s = space.Ships[shipID]
    require found : exists s                        else Gone
    use in_space(s)
```

---

## 10. Status, evidence, assumptions and placement

### 10.1 Status

| Status | Meaning |
| --- | --- |
| `observed` | Seen to hold, with how often, in which contexts and with which violations |
| `lifted` | Moved out of code that already enforced it, or out of a host's own annotations by `strip`; enforced from its first day |
| `watched` | Checked live; a violation is logged and ranked, and nothing is rejected |
| `enforced` | A violation rejects the request, fails the transaction or fails the build |
| `proven` | A compiler or analysis shows it cannot fail, so its check is left out; the reference executor still checks it on the corpora |
| `assumed` | Taken on trust at a boundary, with a stated reason |

Status lives in the catalog, keyed by the constraint's stable name (its subject and its clause's name, or a name derived
from the clause), with its evidence: what supports it, what has violated it, which clusters of violations were explained
and by whom. `edb constraints` shows it. A file may fix a status with `pin name enforced because "…"`; the review tool
refuses a change to a `pin` that no person has approved.

### 10.2 Assumptions

`assume name : p because "…"` states something the subject relies on and does not check, at a boundary where another
place checks it or where it cannot be checked at all: what the proxy enforced, what a foreign library promises, what an
`unowned(unsafe)` reference relies on. An assumption is shown in review like any exception, and every assumption is
listed with what would break if it were false. `obliges caller : p because "…"` is the same seen from the caller: a
precondition the callee does not check, which each call must establish (§ 6.14.9).

### 10.3 Where a constraint runs

Whether a constraint is enforced, watched or assumed is a statement about the program. Where it runs is a choice of
mechanism, made by the compiler from each clause's sort, cost, read set and labels, among the places the host profiles
allow: the client (for the interface and prediction), the proxy, the Python servers, the head of a T-SQL procedure, the
engine, a generated wrapper in a host, the host's own compiler, replicas and the temporal tier, test clusters, and CI.

Placement never reorders. A place runs a prefix of a subject's predicates, never a later one before an earlier one, so
the failure the caller sees is the one the order decides. The proxy runs the leading predicates that need only the call
and the session; a session predicate written after a world predicate runs where the world predicate runs. A clause may
exclude a place (`not at client`) but never require one: the same constraint moves from the Python server to the engine
when its subject does, without being rewritten.

---

## 11. Compilation, and derived languages

### 11.1 Checks for every place

The constraint compiler parses a file, binds its subjects through the host profiles, checks types, dimensions, labels
and well-formedness (every partial operation guarded, every narrowing justified by order), checks that each kind used is
enabled, computes each predicate's read set and cost, and then generates for each place:

- the RPC schema and its encoders and decoders, from signature types and failures;
- checks at the proxy, in the Python servers, at the head of T-SQL procedures and in the engine, from preconditions,
  bounds and access;
- the engine's write-time checks, from invariants, references, state machines, mutability of columns and conservation;
- for each woven host, the annotations its compiler checks (§ 11.2), and wrappers for what it cannot;
- analysers at build for hosts that are not woven;
- queries for replicas, the temporal tier and test clusters, from properties that are watched;
- property tests in CI, from laws and postconditions;
- the catalog entries for status and evidence.

**The same meaning everywhere** is tested, not trusted. Every generated check is run against the reference evaluator on
the recorded corpora and on generated edge cases (absent values, extremes of every integer type, negative divisors,
`NaN` and signed zeros, empty and maximal texts, trailing spaces, combining characters, empty collections), and any
disagreement fails the build.

**Proof.** A constraint is `proven` when a host compiler proves it (a borrow in Rust, a non-escaping closure in Swift),
when a static analysis does, or when the compiler discharges it from earlier clauses and declared types (a range already
implied by a parameter's type), or from the facts that hold where the check is made (§ 11.1.1). The reason is recorded.

**Optimizers need no barriers.** An optimizer may do anything that keeps every constraint that applies, every effect
the program has, and the order of every effect that no freedom (§ 6.17) lets vary, choosing among the outcomes a
freedom allows. There is no clause that forbids an optimization as such. What a barrier does in a host (a write that
must not be removed, a comparison that must not exit early, a device read that must not be merged) is a constraint
about something someone can observe: memory after an end (`cleared on end`, § 7.3), timing (`time independent of`,
§ 6.9), or a device (an effect, § 6.3). Stated as such, it is kept by every optimizer that keeps constraints, and
checked as constraints are. Exit points (§ 11.1.1) are kept the same way: an effect is not moved across a point where
the call may fail, panic or be cancelled, because the state at that exit is observable, unless what the subject states
for that exit (`on failure unchanged`, `on cancel unchanged`) makes the two orders look the same from there.

**Diagnostics** name the constraint, its subject and version, its status, the place it ran, and the inputs that broke
it, redacted by their labels.

#### 11.1.1 Facts at a point of a body

**Meaning.** A branch does not only choose a path: the code it chooses runs under its condition. In `if i < v.len() {
v[i] }`, the index runs where `i < v.len()` holds, and in `if x > 0 { a(x) }` the call runs where `x > 0` holds. The
facts that hold at a point of a body are defined, so that every consumer uses the same ones. They are derived from the
body and the files, never written: writing them would repeat the body, and could contradict it (principle 5).

At a point of a body, these hold:

1. **The platform's facts** for the build (§ 3.6).
2. **The conditions of every `#[cfg(..)]`** around the point, which the build has decided.
3. **The subject's preconditions,** in order, and the presence that order narrows (principle 6), where they are
   `lifted`, `enforced` or `proven`: a precondition that is only watched does not make anything hold.
4. **The conditions of every enclosing branch:** an `if` condition in its first branch and its negation in the other;
   a match arm's pattern and guard, and the failure of every earlier arm's; a loop's condition in its body and its
   negation after it; a `let … else`, whose pattern holds after it.
5. **The postconditions of the calls made earlier on the path,** with their arguments in place (`ensure`, § 6.2), and
   the fact that an earlier call did not fail, where its failures are stated.
6. **What an earlier `assert!` checked,** after it: a fact the author wrote by hand on an intermediate value, which the
   bare body keeps (bare Rust § 2.1).

**Exit points.** Every call whose callee's exits (§ 6.21) hold more than `return`, and every suspension point, is a
point where the body may stop and the state it has built so far becomes visible to whoever handles the exit: a caller's
handler, a retry, the runtime that cancelled it. After such a point, on the path where the body goes on, the fact that
the call returned (and with what) holds; on the exit path, only what the callee states for that exit (`on failure`,
`on cancel`). A call whose exits are `return` alone is no exit point, which is why the set matters for every call.

**Facts survive what does not write them.** A fact holds until something writes what it reads. A call ends only the
facts that read a path in the call's write set (`mutates only`, `writes only`, § 6.16); a call whose write set is not
known ends every fact about what its arguments reach. This is the frame rule, and it is why write sets belong here as
well as in § 6.16: `i < v.len()` survives `log(x)`, whose write set is empty, and does not survive `v.clear()`.

```text
-- body (bare Rust)                       -- the facts that hold on each line
if x > 0 {                                --   x > 0
    let n = count(&ws);                   --   x > 0; result of count (its ensure)
    a(x, n)                               --   a's `require x > 0` is discharged here
}
```

- **Used by checks.** A generated check (a callee's precondition, an index in range, an arithmetic result that fits,
  § 11.2) is discharged where the facts at its point imply it, and the reason recorded names the facts.
- **Used by optimizers,** which remove the same checks at run time, and by **independence** (§ 6.16): a branch's
  condition is in the read set of the code it guards, and a write set that differs by branch is stated at the
  boundary with `when` (§ 6.3) wherever the condition is on the inputs.
- **Used by translators.** A host with flow typing (TypeScript's narrowing, Kotlin's smart casts, Swift's `if let`)
  carries some of the facts as types; a host without it keeps them as the checks they discharge.
- **Computed** by the constraint compiler from the bare body, the files and the platform, as a set of expressions in
  the language of § 4 at each point where a check or a call is; a fact that is not an expression of the language (a
  property of a closure, a value of a type the language cannot name) is not a fact here, and the check it would
  discharge stays.

### 11.2 Derived languages: the host, bare

**Meaning.** For any host language, removing every mechanism that only restricts which programs it accepts, or checks a
value, leaves a bare form of the language that says only what the program does. Bare source and a constraint file are
woven into ordinary source of the host, which the host's own toolchain compiles; ordinary source is stripped into bare
source and a constraint file. The two directions are inverses, so any codebase can move into the bare form and back
without loss, and a host's type system becomes the checker of the part of the constraint file it can express.

The bare form is the image of erasure, `bare = strip(host)`, and is neither a subset nor a superset of the host: `rustc`
rejects `fn get(self, key)`, and bare Rust rejects `&'a self`. `weave` maps a bare body and a constraint file to host
source, with `strip(weave(b, c)) = b`, so each bare body stands for a family of host programs, one for each file that
completes it. A bare form is defined in three parts:

- **A grammar delta.** The host's grammar with every constraint production made empty: the types of parameters,
  results, fields and locals, lifetimes, access markers, visibility, generic parameters and bounds, `where` clauses.
  Structure stays: a struct keeps its field names and a tuple variant its arity (`A(_)`). Every emptied production is a
  slot that the file fills and a place where `weave` prints. Written as an extension of the host's grammar (for Rust, of
  tree-sitter-rust), the delta gives the bare form's parser, the file's slots and the weaver's insertion points at once.
- **A ban on disguised constraints.** Some constraints are ordinary calls and statements, which no grammar can remove:
  `assert!`, `.unwrap()` and `.expect()`, a leading `if cond { return Err(NotYours) }`. A lint, part of the bare form's
  definition, finds them by position. A check at the head of a subject, on its inputs or the world, belongs in the file
  as a `require … else`, and is woven back as a generated check. A check on an intermediate value may stay in the body
  (principle 5). For now `.unwrap()` and `.expect()` stay in the body wherever they are; where one covers an input, the
  lint marks it as covered by the `require` that states the same presence.
- **A list of mixed constructs**, each split as below, in each host's profile.

**What moves and what stays.** The rule is one sentence: a mechanism that only restricts or checks moves to the file;
anything that decides what the program does stays in the body. Types of parameters, results and fields move; so do
nullability, lifetimes, `mut` and `const` and `final` and `readonly`, `&` and `&mut`, visibility modifiers, generic
bounds, declared exceptions, and attributes such as `#[must_use]` and `#[non_exhaustive]`. Control flow, calls,
constructors, conversions, pattern matching, implementations of traits and interfaces, macros and `unsafe` blocks stay.

A construct that is both behaviour and constraint is split in two: the behaviour stays in the body, and the constraint
moves to the file. A Rust cast `x as u32` keeps the conversion, `x as _`, and moves the target type;
`collect::<Vec<u8>>()` keeps the choice of `Vec`, and moves the element type; `&self` keeps the receiver, and moves its
access; a trait object keeps dispatch through the interface, and moves the set of implementations allowed; C#'s `ref`
keeps passing by reference, and moves the access it grants.

**Holes.** Where the constraint chooses the behaviour, splitting leaves a hole in the body that only the file can fill:
an integer's width, which decides overflow, from its declared type; the target of `as _`; `Rc` or `Arc` from `shares`
and `sendable`; `&` or `&mut` from `access`; a `Box` where an owned recursive type needs one; a lifetime parameter from
`borrows from`; `let mut` from the writes the body makes. So a bare body says what the program does only together with
its file, and two rules hold:

- **`weave` fails if a hole is left undetermined.** A hole is determined when the file states it, or when the host's
  compiler infers it from what the file states; one that is neither is reported at its place in the body, with the
  clause that would fill it.
- **Whoever writes a bare body reads its file,** not only the body, to know what the body does.

`weave` therefore prints and never infers: the file is authoritative, and where it says `access self : write
exclusive`, `weave` writes `&mut self`. Each profile lists its holes, and how each is filled.

**Every written type has a name.** A type that the host's source writes in the middle of an expression has nothing to
attach to there, so the bare form names the value or the realization that carries it: a cast or a literal's type goes
onto a local (`let n = len;` with `n` a cast local to `f64`), a closure's parameter types are keyed by the closure's
name, and a turbofish is a realization the body names (`s.parse_port()` for `s.parse::<u16>()`, § 3.5). The names are
the file's, so the body reads as if the host had no generics at the place of use.

| Host | Moves to the constraint file | Stays in the bare body | Woven into |
| --- | --- | --- | --- |
| Rust | Types of parameters, results and fields; lifetimes; `mut`, `&` and `&mut`; `pub`; generic bounds and `where` clauses; `Send` and `Sync` bounds; `Option` around a value; error types, as failures; `#[must_use]`, `#[non_exhaustive]`; the types of casts and type arguments | Expressions, control flow, `?`, `match`, `impl` blocks, macros, `unsafe` blocks, constructors and conversions (`as _`), the collection a `collect` builds | Rust with types, lifetimes, `mut` and visibility written, and wrappers for the rest |
| T-SQL | Parameter and column types; `NULL` and `NOT NULL`; `CHECK`, `UNIQUE` and `FOREIGN KEY` | Statements; defaults; triggers' bodies | DDL and procedures, with generated checks at procedures' heads |
| C# | Types of members and parameters; nullable annotations; `readonly`, `init` and `in`; access modifiers; `sealed`; generic constraints; the access `ref` and `out` grant | Bodies, passing by `ref` and `out`, `using`, LINQ, lambdas | C# with types and modifiers written, and analyser rules |
| Java, Kotlin | Types; `final` and `val`; nullability; access modifiers; `throws`; generic bounds | Bodies, `try`-with-resources, lambdas | Java or Kotlin with types and modifiers written |
| Python | Type hints; `Final`; `frozen=True`; `@final` | Everything else | Python with hints written and decorators that check at run time |
| TypeScript | Types; `readonly`; optional markers; access modifiers; `as`, `satisfies` and `!` | JavaScript | TypeScript |

**Inside bodies.** Hosts infer most local types, so a bare body rarely needs one. Where a woven host needs the type of a
local and cannot infer it, the file gives it: `local total : decimal<precision=19, scale=2>` under the function's subject, binding the
local by its name in the body. It is the one place a constraint file names something inside a body, and it states a
type, never a condition on a value (principle 5).

**The round trip is a law.** `weave(strip(source)) == source`, up to the formatter's canonical form, and
`strip(weave(bare, file)) == (bare, file)` for every constraint the host can express; what it cannot express is woven as
generated checks, which carry a marker that `strip` recognises and removes again. The canonical form writes what the
host lets it leave out only where leaving it out would change the meaning: Rust lifetimes are elided wherever the
elision rules give the same meaning (§ 14.9). Both directions are checked in CI over the whole codebase, so neither the
bare form nor the ordinary form can drift. Diagnostics from the host's compiler are mapped back, through a map that
`weave` writes, to the line of the bare body or the clause of the file they concern.

**Libraries.** Code that is not woven, such as a host's standard library and a codebase's dependencies, needs a
constraint file but no bare body: an agent writing bare code calls a library by name and must know what it promises,
while the host's compiler checks the calls after weaving. Such code is stable: a library is versioned, and its file is
written once per version, for the items in use, and shared by everyone who uses that version. An agent writes it, and
the host's compiler checks its signatures against the real items; that needs no stripper. Where a library changes too
fast, or is too large, for that, a partial strip writes the file and discards the body. It starts from what the host
already produces for separate compilation (rustdoc's JSON for Rust, `.d.ts` files for
TypeScript, stubs for Python, reference assemblies for C#), and, where the source is available, looks into bodies for
more than the signatures say: guards at a function's head, the failures and panics a body can reach, and the effects of
the calls it makes. Each clause carries the status of its source: signatures are `proven`, since the library's compiler
checked them; guards found in its code are `lifted`, for the version the build locks; what its documentation says is
`assumed … because "documentation"`; what an analysis of its bodies suggests is `observed`. In a file an agent writes,
signatures the host's compiler has checked are `proven`, and the rest is `assumed … because "hand-written"` after a
second agent's review. Generated code relies only on `proven` and `lifted` clauses
and on assumptions a person has accepted, as when a generated check is left out because a library's postcondition
already guarantees it (§ 11.1). A bare module's own file is its public interface in the same
way: callers depend on the file and never on the body, so at the boundary the codebase's own modules and its libraries
are alike.

**What it buys.**

- **Two artefacts, each read for one thing.** A reviewer reads the bare body for what the code does and the constraint
  file for what must hold, and neither hides the other.
- **Translation is of bodies only.** One file binds by stable name across hosts (§ 3.2), so a Python method woven with
  its file gets hints and runtime checks, and the Rust procedure that replaces it, woven with the same file, gets types,
  lifetimes and borrows. The translation changes the body; the constraints stay put, and the replay that compares the
  two runs under the same file.
- **Hosts without a mechanism gain it.** Python gets lifetimes and mutability as generated checks; C# gets borrows as
  analyser rules; T-SQL gets state machines as checks at procedures' heads.
- **Agents write what the code does.** An agent writes a bare body; the file, much of it harvested or stripped from
  existing code, says what must hold, and the host's compiler checks the body against it after weaving. The files of
  its dependencies tell it what they promise, in the same language.
- **Existing code moves in mechanically.** `strip` lifts a host's own annotations into the file with status `lifted`, so
  a codebase enters the bare form without anyone writing a constraint by hand.

**Limits.** What `strip` cannot see it cannot move: types produced by macros or code generators stay where they are
generated; and reflection and runtime patching (Python's monkeypatching) are constrained only at the boundaries the
profile can wrap. Each profile lists what it moves, what it splits, what it leaves and why. The first bare form, Rust,
is planned in [`bare-rust.md`](bare-rust.md).

---

## 12. How the language grows

The grammar is designed whole; kinds are enabled in an order set by their consumers. Each arrives with its paragraph of
meaning, its miner and its compilation into every place that enforces it.

| Order | Kind | First consumer | Miner | First places |
| --- | --- | --- | --- | --- |
| 1 | Signature types: arguments, replies and the failures each method raises | The RPC schema at the proxy | Manifests at entry and exit | Proxy; schema generation |
| 2 | Ranges, sets of values, absence | The proxy; the Python servers | Manifests at entry | Proxy; Python |
| 3 | Failures by condition, and postconditions | Lifting checks out of Python and T-SQL | Raises and `THROW`s; manifests at exit | Python; T-SQL; engine |
| 4 | Preconditions on the session | The proxy | Manifests | Proxy |
| 5 | Preconditions on the world | The engine; proxies with replicas | Lifted checks | Engine; replicas |
| 6 | Invariants on rows, uniqueness and references | Tables moving into the engine | Historical backups | Engine; replicas |
| 7 | Conservation | The economy | Manifests and the log, for people to decide | Engine |
| 8 | Order in time and state machines | Replicas and the temporal tier | The log, in simulation time | Replicas |
| 9 | Budgets | CI and the scheduler | Timings and manifests | CI; production metrics |
| 10 | Bounds on effects | Review of translated procedures | Manifests and static bounds, for people to decide | Engine; CI |
| 11 | Lifetime, mutability and access | Moving Python state into rows; the client; other studios' hosts | Tracing at the hot stores; heap snapshots; replays | Rows in the engine; CI; test clusters |
| 12 | Aliasing and concurrency | The Python servers while they remain; clients | Replays; lock traces | CI; analysers |
| 13 | Information flow | Ledgers, chains, visibility | Visibility filters and existing rules | Proxies; stream boundaries; CI |
| 14 | Compatibility and visibility | Releases | The catalog's history | `edb review` |
| 15 | Laws and statistics | CI; economy and loot | — | CI; replicas |
| 16 | Levels of modules | Y's backends, and hosts that carry only some levels | The Y compiler's inferred levels; effect manifests | Y builds; CI |
| 17 | Regions and execution constraints | Y's regions; transactions and timeouts in the hosts | Isolation levels, locks and timeouts in the hosts | Y builds; CI explorers |

**Derived languages arrive host by host.** A host's bare form is usable once the kinds its own annotations need are
enabled: for Python, kinds 1 to 3; for T-SQL, kinds 1 to 6; for Rust, C#, Java and Kotlin, also kinds 11, 12 and 14,
since their types carry lifetimes, mutability, access and visibility. Until then `strip` leaves in the body what the
file cannot yet hold.

The list of kinds is reviewed as a language, yearly. A kind is never added for control flow, which belongs to the host.

---

## 13. Coverage: constraints elsewhere, and how they are written here

The language must be able to state any reasonable constraint that a programming language, a contract system, a database
or a verification tool can state, since a bare form must be able to hold everything its host's annotations said. The
table maps the mechanisms in use onto the clauses here.

| Mechanism | Found in | Written here as |
| --- | --- | --- |
| Integer widths, signedness | C, C++, Rust, Java, C#, SQL | `u8` … `i128` as refinements of `int`; `fits T` |
| Range and subrange types | Ada, Pascal | `T in a..b`; `T where …` |
| Enumerations, sum types, sealed hierarchies | Rust, Swift, Kotlin, Java, C# | Variants; `in {…}`; `open` for non-exhaustive |
| Nullability | Kotlin, C#, TypeScript, Swift, SQL `NOT NULL`, Java `@NonNull` | `T?`; `exists`; non-optional by default |
| Refinement and branded types | Liquid Haskell, F*, TypeScript brands | `T where p`; named refinements |
| Units of measure | F#, units libraries | `unit`; `quantity … in` |
| Generic bounds, traits, concepts | Rust, C#, Java, Swift, C++ concepts | `for T where T: …` |
| Message and document schemas | protobuf, JSON Schema, OpenAPI, XSD | Parameter and result types on `method` |
| String length and format | SQL `NVARCHAR(n)`, validators | `string where count(code_units(value, utf16)) <= n and value matches /…/` |
| Float finiteness and tolerance | Game engines, numeric code | `finite`; `approx` |
| Overflow behaviour | Rust checked arithmetic, C# `checked` | Exact arithmetic; `fits`; `ensure` on exact results |
| Layout and ABI | Rust `repr(C)`, C# `StructLayout`, C packing | `layout`, `size`, `align` |
| Visibility and access modifiers | Every object-oriented language; Rust `pub` | `visible to` |
| Preconditions, postconditions, `old` | Eiffel, JML, SPARK, Dafny, C++ contracts | `require … else`, `ensure`, `old(…)` |
| Exceptional postconditions, strong exception guarantee | JML `signals`, C++ guarantees | `on failure F ensure`; `on failure unchanged` |
| Class and type invariants | Eiffel, SPARK `Type_Invariant`, Dafny | `invariant … holds between calls` |
| Checked exceptions, typed errors, `noexcept` | Java, Swift `throws`, Rust `Result`, C++ | `failure`; `fails only`; `never fails runtime`; `never panics` |
| `[[nodiscard]]`, `#[must_use]` | C++, Rust | `result must be used` |
| Contracts on function arguments | Kotlin `callsInPlace`, Racket higher-order contracts | `callback … : called …`; `callback … : conforms to` |
| Algebraic laws, property tests | Haskell type-class laws, QuickCheck | `law` |
| Model and ghost state | Dafny ghost fields, JML model fields | Named pure projections, `view(e)` |
| Access control on calls | `@RolesAllowed`, capability checks | `from` |
| Frames | Dafny `reads` and `modifies`, SPARK `Global` | `reads only`, `writes only`, `mutates only` |
| Data dependencies | SPARK `Depends` | `depends only on` |
| Purity and effects | Haskell `IO`, Koka effects, D's strongly `pure` functions | `effects none`, `effects only`, `no io`, `no clock` |
| Determinism | SQL deterministic functions; replay systems | `deterministic` |
| Idempotence | HTTP `PUT`, message handlers | `idempotent by` |
| Termination and variants | Dafny `decreases`, total languages | `terminates`; budgets |
| Complexity and performance budgets | Big-O annotations, frame budgets | `budget work O(…)`, `budget p99 …`, `budget allocates …` |
| Uniqueness, foreign keys, `CHECK` | SQL | `unique … per shard | global`, `reference … on end`, `invariant` |
| Deferred constraints | SQL `DEFERRABLE INITIALLY DEFERRED` | `invariant … holds at commit` |
| Polymorphic references | EVE's `locationID`, polymorphic associations | `reference … -> one of …` |
| Containment without cycles | Databases, scene graphs | `acyclic … via` |
| Conservation | Ledgers, double-entry bookkeeping | `conserve`, `faucet`, `sink` |
| State machines, guarded transitions | Triggers, workflow engines, statecharts | `states … when` |
| Typestate | Plaid, Rust's typestate pattern; one side of a session type | `protocol` |
| Temporal logic | TLA+, LTL, runtime verification | `always`, `never`, `after … within`, `since`, `until`, `eventually` |
| Rate limits | API gateways | `rate` |
| Statistical properties | Monitoring, fairness audits | `statistic` (watched only) |
| Ownership and moves | Rust, C++ `unique_ptr` | `hold … : owns`; ownership model |
| Borrows and lifetime parameters | Rust `&`, `&mut`, `'a: 'b`; returned borrows | `hold … : borrows from`; `outlives`; `access` |
| Shared ownership | `Rc`, `Arc`, `shared_ptr`, Swift and CPython strong references | `hold … : shares`; counting model |
| Weak and soft references, ephemerons | Rust `Weak`, Java `WeakReference` and `SoftReference`, Swift `weak`, `weak_ptr`, `WeakMap` | `hold … : observes [softly]`; `ends with key` |
| `unowned` references | Swift | `hold … : observes` with `outlives`; `assume` for `unowned(unsafe)` |
| Ids, handles, `tag` | Entity systems, rows, Pony | `hold … : identifies` |
| Generational references, slot maps | Vale, game engines | Tracked model; `no use after end`; `on end refuse | invalidate` |
| Non-escaping parameters and closures | Swift, Kotlin `inline`, escape analysis | `lifetime … does not escape` |
| Deterministic release | RAII, `Drop`, `deinit`, destructors | Ownership and counting models: release is the end |
| Explicit release | `using`, `IDisposable`, try-with-resources, Python `with`, Go `defer` | `released by … exactly once`; `released before end` |
| Finalisers, phantom references, cleaners | Java, C# | Forbidden as a means of release by `released before end` |
| Linear and affine types | Linear Haskell, Rust moves | `ends by … exactly once`; `sole` |
| Regions, arenas, frame allocators | Cyclone, arenas in games | Region model; `within frame` |
| Pinning | Rust `Pin`, C# `fixed` and pinned handles | `stable address` |
| Leak detection | Heap snapshots, LeakCanary, leak sanitizers | `ends within … after`; `instances … are none within`; `reachable only from` |
| Reference cycles | Swift, CPython, PHP | `no strong cycle` |
| Read-only views | C++ `const`, TypeScript `readonly`, C# `in`, Java read-only interfaces | `readonly`; `access … : read any` |
| Immutability | Rust by default, Java immutable classes, Kotlin `val`, C# records, `Object.freeze`, frozen dataclasses | `immutable deep | shallow`; `frozen after` |
| Assign-once fields | Kotlin `val`, Java `final`, columns set at insert | `write once` |
| Append-only storage | Logs, ledgers, event sourcing | `append only` |
| Monotonic values | Versions, sequences, clocks | `monotonic` |
| Interior mutability, logical const | Rust `Cell`, `RefCell`, `OnceCell`; C++ `mutable` | `interior` with `unchanged view(…)` |
| Exclusive and sole references | C `restrict`, Rust `&mut`, Pony `iso`, Clean unique types | `exclusive`; `sole` |
| Non-overlapping arguments | Fortran, `restrict`, separation logic | `disjoint` |
| Reference capabilities | Pony `iso`, `trn`, `ref`, `val`, `box`, `tag` | `access` with rights and guarantees, and `confined to isolation` |
| Thread safety markers | Rust `Send` and `Sync`, Swift `Sendable` | `sendable`, `shareable` |
| Lock discipline | Java `@GuardedBy`, Clang thread-safety analysis | `guarded by`, `lock order`, `no suspension while holding` |
| Confinement and actor isolation | UI threads, Swift actors, Erlang processes | `confined to` |
| Memory ordering and publication | `volatile`, ordered atomics, safe publication | `publishes` |
| Reentrancy and cancellation | Non-reentrant locks, async cancellation safety | `not reentrant`, `cancellation safe` |
| Atomicity and linearisability | Transactions, concurrent structures | `atomic`, `linearizable` |
| Information flow and taint | Jif, FlowCaml, taint modes, secrecy wrappers | `label`, `flows only to`, `never flows … to`, `declassify`, `sanitised by` |
| Data retention | Privacy law, retention policies | `lifetime rows at most … after …` |
| API stability and deprecation | Semantic versioning, `#[non_exhaustive]`, `@Deprecated` | `version`, `compatible with`, `stable`, `open`, `deprecated` |
| Behavioural subtyping | Liskov, Eiffel's inherited contracts | `interface` and the rules of § 6.10 |

What none of these can state is what a function computes in general; that stays with reference functions and comparison
by output (§ 16).

---

## 14. Worked examples

### 14.1 Module activation, in the full language

```text
-- space/modules.constraints
module space
use space.predicates

failure Gone          : user
failure NotYours      : user
failure Offline       : user
failure AlreadyActive : user
failure OutOfRange    : user { distance: Distance, range: Distance }
failure NoCapacitor   : user

procedure space.ActivateModule(moduleID: u64, targetID: u64) -> { firstCycleEnds: time }
    version 148
    from Player
    replaces python 'space.modules.ActivateModule'
    let m = space.Modules[moduleID]
    require found    : exists m                                         else Gone
    require flies_it : m.ship.pilotID == actor                          else NotYours
    require online   : m.isOnline                                       else Offline
    require idle     : m.cycle == Idle                                  else AlreadyActive
    let d = movement.distance(m.ship, targetID, now)
    require in_range : d <= m.range                                     else OutOfRange { distance: d, range: m.range }
    require can_pay  : m.ship.capacitor >= m.activationCost             else NoCapacitor
    ensure  cycling  : space.Modules[moduleID].cycle == Active
    ensure  ends     : result.firstCycleEnds == now + m.cycleTime
    fails only Gone, NotYours, Offline, AlreadyActive, OutOfRange, NoCapacitor
    deterministic
    budget p99 <= 50us on SolarSystem.hot
```

### 14.2 A Python manager's state, on its way into rows

```text
-- inventory/python.constraints: harvested by tracing the hot stores on test clusters; watched
type python 'eve.inventory.LocationCache'
    hold entries : owns
    hold manager : observes
    lifetime self within session
    lifetime self reachable only from 'eve.inventory.InventoryMgr.caches'
    instances attributed to session are none within 5 s after end(session)

function python 'eve.inventory.InventoryMgr.MoveItems'(self, itemIDs: list<element_type=u64>, toLocationID: u64)
    from Player
    readonly itemIDs
    hold itemIDs : borrows
    no suspension                         -- no recorded call yielded between its first read and its last write
```

These say the cache is per session, held only by its manager's table of caches and gone with the session: a candidate
for a table that need not be durable, keyed by session, or for nothing at all once the queries it served are fast
enough. `no suspension` says only that no recorded call yielded mid-move; whether the move may become one transaction is
still decided as every order is, by a read that explains it or by a person, since order seen in one stream is mostly
accident.

### 14.3 What every Rust procedure promises

```text
-- edb/kit.constraints: proven by the kit's sealed interface and lints; stated so that every host sees it
interface rust 'edb::Procedure'
    hold tx : borrows
    access tx : write exclusive
    lifetime every ref<target_type=edb::Row> obtained from tx within transaction
    effects only tx
    no io
    no clock
    no randomness except rolls
    deterministic
    confined to shard
```

### 14.4 A window and its listeners, under tracing

```text
-- ui/inventory-window.constraints
type csharp 'Studio.Ui.InventoryWindow'
    confined to thread ui
    hold subscriptions : owns
    lifetime self released by Dispose exactly once
    lifetime self released before end
    lifetime self ends within 2 frames after call Close
    budget allocates at most 0 B per frame

type csharp 'Studio.Events.EventBus'
    hold handlers : observes                 -- the bus never keeps a window alive
```

The last two clauses on the window catch the leak most common in garbage-collected interfaces: a closed window kept
reachable by an event subscription. They are checked by a weak-reference probe taken at `Close` on replays of recorded
sessions, with a collection forced at the probe.

### 14.5 A scene graph, under counting

```text
type swift 'Scene.Node'
    hold children : owns
    hold parent   : observes
    hold delegate : observes
    no strong cycle through children, parent, delegate
    lifetime self ends with parent
    confined to isolation SceneActor
```

### 14.6 Rows with lifetimes

```text
table inv.Items
    reference locationID -> one of inv.Locations, space.Ships, chr.Characters on end restrict
    reference ownerID    -> chr.Characters on end dangle      -- identifies: history survives the character
    acyclic containment : via locationID

table inv.Containers
    reference itemID -> inv.Items on end cascade              -- the item owns its container row

table chr.Notifications
    lifetime rows at most 90 days after createdAt
```

### 14.7 A resource, with a protocol

```text
resource java 'java.sql.Connection'
    protocol begin (createStatement | prepareStatement | commit | rollback)* close
    lifetime self released by close exactly once
    lifetime self released before end
    lifetime self within request
    confined to thread
```

### 14.8 Server secrets and personal data

```text
label pii
label secret(server)

table accounts.Users
    label email, birthDate, lastAddress : pii
    never flows pii to ledger, chain, analytics unless encrypted

table rng.PeriodSecrets
    label secret : secret(server)
    flows secret(server) only to server
    declassify secret to player, chain when now >= periodEnd because "rolls are verified after the period ends"
    write once secret
```

### 14.9 Bare Rust, woven

The bare body says what the code does:

```text
// eve_cache/src/lib.rs, bare
struct Cache { entries }

impl Cache {
    fn get(self, key) { self.entries.get(key) }
    fn insert(self, key, entry) { self.entries.insert(key, entry); }
}
```

The constraint file says what must hold, including everything Rust's own annotations would have said:

```text
-- eve_cache/cache.constraints
type rust 'eve_cache::Cache'
    visible to crate
    field entries : map<key_type=Key, value_type=Entry> as 'std::collections::HashMap'
    hold entries : owns

function rust 'eve_cache::Cache::get'(self: ref<target_type=Cache>, key: ref<target_type=Key>) -> ref<target_type=Entry>?
    visible to public
    access self : read readers
    access key : read readers
    hold result : borrows from self

function rust 'eve_cache::Cache::insert'(self: ref<target_type=Cache>, key: Key, entry: Entry)
    visible to public
    access self : write exclusive
```

`weave` writes the Rust that `rustc` compiles, and that `strip` turns back into the two files above:

```rust
pub(crate) struct Cache { entries: std::collections::HashMap<Key, Entry> }

impl Cache {
    pub fn get(&self, key: &Key) -> Option<&Entry> { self.entries.get(key) }
    pub fn insert(&mut self, key: Key, entry: Entry) { self.entries.insert(key, entry); }
}
```

`hold result : borrows from self` is woven without a named lifetime, because Rust's elision rules give `get` exactly
that meaning; `weave` writes `<'a>` only where elision would say something else.

---

## 15. Grammar

The grammar is given in EBNF. Indentation opens and closes blocks (`INDENT`, `DEDENT`), and a clause continues on lines
indented further than its first; `NAME` is an identifier, `QNAME` a dotted name, `STABLE` a quoted stable name or host
path.

```text
file          = [ "module" QNAME NL ] { "use" QNAME NL } { declaration } ;
declaration   = subject | failure | predicate | block | label | scope | quantity | unit | typedef | variantdef
              | conserve | platform | profile ;
platform      = "platform" [ HOST ] STABLE NL INDENT { platform_fact NL } DEDENT ;
profile       = "profile" STABLE "from" NAME STABLE "on platform" STABLE NL INDENT { measured NL } DEDENT ;
measured      = "calls" STABLE "per" scope_ref NUMBER | "branch" STABLE "@" NUMBER "taken" NUMBER
              | "size" path_or_stable percentile NUMBER { "," percentile NUMBER } | "fails" STABLE "with" NAME NUMBER
              | "access" STABLE use NUMBER { "," use NUMBER }
              | "index" STABLE percentile NUMBER { "," percentile NUMBER } [ "," "sequential" NUMBER ]
              | "lookup" STABLE "hit" NUMBER ;
use           = "by" [ "range" ] NAME | "scan" ;
path_or_stable = STABLE | path ;
platform_fact = "pointer width" NUMBER | "endian" ( "little" | "big" ) | "target features" NAME { "," NAME }
              | "atomics up to" NUMBER | "panic" ( "unwind" | "abort" )
              | "build" NAME ( "on" | "off" ) { "," NAME ( "on" | "off" ) } | "features" NAME { "," NAME }
              | "code read only" | "calls only code" ;
                                                       (* a NAME of a target feature or a feature may hold "-": "bulk-memory" *)

subject       = subject_head NL INDENT { clause NL } DEDENT ;
subject_head  = ( "procedure" | "method" ) QNAME params [ "->" type ]
              | ( "function" | "type" | "resource" | "interface" ) [ HOST ] STABLE [ params ] [ "->" type ]
              | "impl" [ HOST ] STABLE "for" STABLE
              | "table" QNAME ;
HOST          = "rust" | "tsql" | "python" | "csharp" | "java" | "kotlin" | "swift" | "objc" | "cpp" | "c"
              | "go" | "php" | "typescript" | "javascript" ;
params        = "(" [ param { "," param } ] ")" ;
param         = "self" | NAME ":" type ;

failure       = "failure" NAME ":" ( "user" | "runtime" ) [ record_type ] NL ;
predicate     = "predicate" NAME params "=" expr NL ;
block         = "block" NAME params NL INDENT { clause NL } DEDENT ;
label         = "label" NAME [ "(" NAME ")" ] NL ;
scope         = "scope" NAME ":" "from" event "to" event NL ;
quantity      = "quantity" NAME ":" type [ "in" unit_expr ] NL ;
unit          = "unit" NAME NL ;
typedef       = "type" NAME [ generics ] "=" type NL ;
variantdef    = "variant" QNAME "=" NAME [ "(" type ")" ] { "|" NAME [ "(" type ")" ] } [ "open" ] NL ;
conserve      = "conserve" NAME "across" path_list NL ;

clause        = "let" ( NAME | pattern ) "=" expr
              | "from" NAME { "," NAME }
              | "require" [ NAME ":" ] expr [ "else" NAME [ payload ] ]
              | "ensure" [ NAME ":" ] expr
              | "on failure" ( "unchanged" | NAME "ensure" NAME ":" expr )
              | "on cancel" ( "unchanged" | "ensure" NAME ":" expr )
              | "exits only" exit { "," exit }
              | "fails" NAME [ "in" later ] "when" expr | "fails only" NAME { "," NAME } [ "open" ]
              | "never fails runtime" | "never panics" [ "except exhausted" ]
              | "result must be used" | "must be used" | "obliges caller" ":" expr "because" STRING
              | qualifiable "given" condition { "and" condition }
              | "callback" NAME ":" ( "called" ( "exactly once" | "at most once" | "at least once" )
                                      [ "before return" ] [ "when" expr ]
                                      [ "," "with" arg { "," arg } ] [ "," "returning" NAME ]
                                    | "called by" later [ "," ( "exactly once" | "at most once" ) "per call" ]
                                      [ "," "with" NAME { "," NAME } ] [ "," "returning" NAME ]
                                    | "conforms to" "interface" [ HOST ] STABLE )
              | "law" NAME ":" expr
              | "field" NAME ":" type
              | generics
              | "implements" implemented { "," implemented }
              | "assoc" NAME "=" type
              | "layout" ( "c" | "packed" | "transparent" ) | "size" NUMBER | "align" NUMBER
              | "local" NAME ":" type
              | bound [ "when" expr ] | state | protocol | temporal | budget | concurrency | flow | compat | memory
              | access | freedom | algebra | expectation
              | "use" NAME "(" [ expr { "," expr } ] ")"
              | "assume" NAME ":" expr "because" STRING
              | "pin" NAME status "because" STRING
              | "not at" place { "," place }
              | "model" MODEL ;
exit          = "return" | "fails" NAME | "panics" | "cancelled" | "exhausted" ;
qualifiable   = (* a clause of the form "ensure", "fails … when", "never panics", or a bound, including "terminates" *) ;
condition     = "default" | "build" BUILD | NAME "." NAME { "," NAME "." NAME } ( "never panic" | "terminate" | "effects none" ) ;
BUILD         = "overflow_checks" | "debug_assertions" | "panic=unwind" | "panic=abort"
              | platform_fact ;                       (* a fact of the platform, § 3.6, of a kind the host profile lists *)
pattern       = "(" ( NAME | "_" ) { "," ( NAME | "_" ) } ")" ;   (* a tuple's parts, also after "is": e is (a, b) *)
later         = ( "result" | NAME ) "." NAME ;      (* the run of a method of the result or of a parameter, § 6.13.1 *)
arg           = expr | NAME ;                       (* a new NAME names the argument for the clauses that follow; "with" and "returning" need "exactly once" or "at most once"; after "called by" a NAME is a list, one item per call *)
payload       = "{" NAME ":" expr { "," NAME ":" expr } "}" ;
generics      = "for" generic { "," generic } [ "where" bound_pred { "," bound_pred } ] ;
generic       = LIFETIME | NAME [ ":" bounds ] ;
bound_pred    = ( NAME | LIFETIME ) ":" bounds ;
implemented   = STABLE [ "{" NAME "=" type { "," NAME "=" type } "}" ] ;
bounds        = bound { "+" bound } ;
bound         = ( QNAME | STABLE ) [ "<" type_arg { "," type_arg } ">" ]
                  [ "{" NAME "=" type { "," NAME "=" type } "}" | "(" [ type { "," type } ] ")" [ "->" type ] ]
              | LIFETIME ;
type_arg      = type | LIFETIME ;
LIFETIME      = "'" NAME ;

bound         = ( "reads only" | "writes only" | "emits only" | "sends only" | "calls only" | "effects only" )
                  path_list
              | "effects none" | "pure" | "no io" | "no device" | "devices only" path_list | "no clock" | "no randomness" [ "except rolls" ] | "no suspension"
              | "deterministic" | "idempotent by" path | "commutes with" QNAME
              | "depends only on" path_list | "terminates" | "evaluable at compile time" ;
state         = "invariant" NAME ":" expr [ "holds" ( "at write" | "at commit" | "between calls" | "always" ) ]
              | "unique" NAME ":" "(" path_list ")" [ "where" expr ] [ "per shard" | "global" ]
              | "reference" path "->" [ "one of" ] QNAME { "," QNAME }
                  "on end" ( "cascade" | "restrict" | "nullify" | "dangle" )
              | "acyclic" NAME ":" "via" path
              | ( "faucet" | "sink" ) NAME [ "at most" expr ]
              | "states" path ":" transition { "," transition } | "initial" path ":" NAME ;
transition    = NAME "->" NAME { "|" NAME } [ "when" expr ] ;
protocol      = "protocol" regex_over_calls ;   (* an event is a NAME of a callback or parameter of the subject, or NAME "." NAME, a method of a parameter's bound; "begin" and "end" are the call's own, § 6.13 *)
temporal      = ( "always" | "never" | "eventually" ) NAME ":" expr
              | "after" event ":" expr "within" duration
              | ( "before" | "since" | "until" ) event ":" expr [ "within" duration ]
              | "rate" [ "per" scope_ref ] "<=" NUMBER "per" duration
              | "statistic" NAME ":" expr "within" expr ".." expr "over" NUMBER "samples" ;
budget        = "budget" ( percentile "<=" duration [ "on" QNAME ] | "work" [ "expected" ] [ "amortised" ] complexity
              | "round_trips" "<=" NUMBER | "bytes sent" "<=" expr | "allocates at most" size [ "per" scope_ref ]
              | "retains at most" size [ "per" ( "instance" | scope_ref ) ] ) ;
concurrency   = "confined to" scope_ref | "sendable" | "shareable" | "guarded by" path ":" path_list
              | "lock order" path "before" path | "atomic" | "linearizable" | "publishes" path
              | "no suspension while holding" path | "no blocking in" scope_ref
              | "not reentrant" | "cancellation safe" ;
flow          = "label" path_list ":" NAME [ "(" path ")" ] [ "when" expr ]
              | "flows" NAME [ "(" NAME ")" ] "only to" sink { "," sink }
              | "never flows" NAME "to" sink { "," sink } [ "unless encrypted" ]
              | "declassify" path "to" sink { "," sink } "when" expr "because" STRING
              | "sanitised by" QNAME "before" sink { "," sink }
              | "time independent of" path_list ;
compat        = "version" NUMBER | "compatible with" NUMBER { "," NUMBER } | "replaces" HOST STABLE
              | "was" STABLE | "deprecated" [ "since" NUMBER ] | "stable"
              | "visible to" ( "public" | "crate" | "module" | QNAME ) ;
memory        = "hold" [ path ] ":" ( "owns" | "shares" | "borrows" [ "from" path | "for" LIFETIME { "," LIFETIME } ]
                                    | "observes" [ "softly" ] | "identifies" | "moved from" path )
              | "lifetime" target lifetime_pred
              | "lifetime" path_list "survive call"
              | "instances" [ "per" scope_ref ] "at most" NUMBER
              | "instances attributed to" scope_ref "are none within" duration "after" event
              | "no strong cycle" [ "through" path_list ] ;
target        = path | "every ref<target_type=" type ">" [ "obtained from" path ] | "every item of" expr | "rows"
              | "every value labelled" NAME ;
lifetime_pred = "usable after construct" | "reclaimed only after released"
              | "outlives" path | "within" scope_ref | "ends with" path | "ends in call" [ "when" expr ]
              | "does not escape" [ scope_ref ]
              | "escapes only to" path_list | "ends within" duration "after" event
              | "at most" duration [ "after" event ]
              | "released by" NAME "exactly once" | "released before end" | "ends by" NAME "exactly once"
              | "no use after" ( "end" | "release" ) | "reachable only from" path_list
              | "stable address" [ "while held by" path_list ] | "on end" ( "refuse" | "invalidate" )
              | "cleared on end" ;
access        = "access" path ":" ( "identify" | "read" | "write" ) ( "exclusive" | "readers" | "any" )
              | "readonly" path [ "shallow" | "transitively" ]
              | "immutable" path [ "deep" | "shallow" ] [ "after" event ] | "frozen after" event
              | "write once" path | "append only" [ path ]
              | "monotonic" path ( "increasing" | "non-decreasing" | "decreasing" | "non-increasing" )
              | "mutates only" path_list [ "when" expr ] | "interior" path | "unchanged" "view(" path ")" [ "=" expr ]
              | "exclusive" path | "sole" path | "disjoint" path "," path { "," path } | "shared readonly" path ;

freedom       = "order unspecified" expr | "may reassociate" "within" tolerance | "result any of" expr
              | ( "may batch" | "may reorder" ) ( "emits" | "sends" | "writes" path ) ;
algebra       = "associative" [ "within" tolerance ] | "commutative" | "identity" expr | "absorbing" expr
              | "idempotent" | "involutive" | "inverse of" QNAME | "distributes over" QNAME
              | "monotone" ( "increasing" | "decreasing" | "non-decreasing" | "non-increasing" ) "in" NAME
              | "injective" | "equivalent to" ( QNAME | STABLE ) [ "within" tolerance ] ;
tolerance     = ( "abs:" NUMBER [ "," "rel:" NUMBER ] | "rel:" NUMBER ) ;
expectation   = "expect" ( expr [ "usually" NUMBER ] | "fails" NAME ( "rarely" | "often" | NUMBER )
                         | "hot" | "cold" | "called per" scope_ref "about" NUMBER
                         | "access" path ( "by" [ "range" ] NAME | "sequential" | "random" ) [ "usually" NUMBER ]
                         | "scans" path ( "rarely" | "often" ) | "index" NAME "of" path "<" expr [ "usually" NUMBER ]
                         | "lookup" path "hits" [ "usually" NUMBER ] ) ;
                                                       (* sorted, distinct, dense, heap ordered and nonempty are
                                                          predicates of § 6.19, written as calls in an expr *)
path          = ( NAME | "owner" "(" path ")" ) { "." ( NAME | NUMBER ) | "[" expr "]" } [ ".*" ] | STABLE ;
                                                       (* NUMBER: a tuple's part, result.0; "[" expr "]" in "mutates only" is one element;
                                                          owner(h) is the collection a handle borrows, § 4 *)
path_list     = path { "," path } ;
event         = ( "begin" | "end" | "released" ) "(" path ")" | "call" QNAME | "return" QNAME
              | ( "insert" | "update" | "delete" ) QNAME | "emit" NAME | "construct" | "publish"
              | path ;                                              (* a path names a time *)
scope_ref     = "call" | "transaction" | "tick" | "request" | "session" | "frame" | "task" | "thread" [ NAME ]
              | "isolation" NAME | "process" | "shard" | NAME ;
sink          = "client" | "player" [ "(" path ")" ] | "server" | "ledger" | "chain" | "log" | "analytics"
              | "external" | "shard" [ "(" path ")" ] ;
place         = "client" | "proxy" | "python" | "tsql" | "engine" | "host" | "replica" | "ci" ;
status        = "observed" | "lifted" | "watched" | "enforced" | "proven" | "assumed" ;
MODEL         = "ownership" | "tracing" | "counting" | "tracked" | "region" | "manual" | "rows" ;
(* type, record_type, duration, size, percentile, complexity, unit_expr and regex_over_calls are
   written as in the examples: "u64?", "map<key_type=K, value_type=V> as 'std::collections::BTreeMap'", "{ a: T }", "50us", "2 MB",
   "p99", "O(count(x))", "m / s", "begin (send | receive)* finish". A type may also be an existential,
   "some" bounds or "any" bounds, as in "some 'std::iter::Iterator' { Item = u8 } + Clone"; a host type applied to
   lifetimes, "Chars<'a>"; or an associated type, "Self::Item" (§ 6.11). A "for" clause on a subject binds its
   generics: "for 'a", "for T where T: Comparable". *)

expr          = (* literals, paths, T[key], T where p, calls of named pure functions, not/and/or/implies/iff,
                   comparisons, arithmetic, if-then-else, exists, is, in, matches, fits, finite, approx,
                   all/any/count/sum/min/max over collections, old(e), result, same(a, b), p moved from q, view(e), e.0, current(h), owner(h), stored_key(m, k), final(e), sequences (§ 6.15);
                   defined in § 4 *) ;
seq_expr      = "yields" "(" expr ")"                      (* the items an iterator gives from now to its first None *)
              | expr "[" expr "]"                          (* position: the element, or none beyond the ends *)
              | expr "[" [ expr ] ".." [ expr ] "]"        (* slice: take(j - max(i, 0), skip(max(i, 0), xs)); never fails *)
              | expr "++" expr                             (* concatenation; an infinite left operand is the result *)
              | "[" [ expr { "," expr } ] "]"              (* list literal *)
              | ( "reverse" | "enumerate" ) "(" expr ")"
              | ( "take" | "skip" | "zip" | "present" | "select" | "while_true" ) "(" expr "," expr ")"
              | "flatten" "(" expr ")"
              | ( "map" | "filter" ) "(" fname "," expr ")"
              | "scalars" "(" expr ")" | "deref" "(" expr ")" ;
fname         = NAME ;                                     (* a language function (scalars, deref, yields) or a view; never a parameter or a callback, § 6.15 *)
handle_expr   = "current" "(" expr ")"                     (* the value at the place a handle stands for, read where the expression is evaluated, § 4 *)
              | "owner" "(" expr ")"                       (* the collection a handle borrows; also a path root *)
              | "stored_key" "(" expr "," expr ")"         (* the key a map holds for the key equal to the second argument, or none *)
              | "final" "(" expr ")" ;                     (* the value at the end of a result's run; needs a "called by" or "fails … in", § 6.13.1 *)
```

---

## 16. What the language cannot say

- **Closures as values.** A function is named, never passed: `map` and `filter` take the name of a pure function. An
  adapter over a closure is stated by the logs of its calls (`called by`, § 6.13.1), not by calling the closure.
- **What a function computes,** in general. Formulas are reference functions compared by output; laws and postconditions
  can pin properties of them, never the whole.
- **Anything about a body's intermediate values.** A rule over one becomes a named pure function, or stays in the code;
  `local` gives a woven host a type, never a condition. The facts that hold at a point of a body are derived from the
  body (§ 11.1.1), and are not written.
- **How anything is done.** No control flow, no assignment, no effects.
- **Unbounded quantification where it is enforced.** A predicate over every row of a table is an invariant checked row
  by row, or is watched on a replica; properties are never evaluated on the request path.
- **Unbounded liveness, as enforcement.** "Eventually" without a bound may only be watched.
- **The future.** A temporal constraint is decided over history; a deadline can raise an alarm when it passes, not
  prevent it from passing.
- **Style, naming and taste.** Those belong to linters and review.
- **Failure of a request for cost.** Budgets fail builds and raise alarms; they never fail a player's request.

---

## 17. Open questions

1. **Selections at enforcement time.** A selection in an enforced predicate must use an index; should the compiler add
   the index, refuse the constraint, or demote it to watched? Leaning: refuse and say which index would serve it, so
   that the cost is a decision.
2. **How far to invest in proof.** Arithmetic preconditions and ranges can often be discharged by a solver from types
   and earlier clauses. Leaning: discharge what follows from types and order without a solver first, and add a solver
   only where measurement shows checks that cost.
3. **Ends under tracing, precisely enough to enforce.** Weak probes and snapshots depend on when the collector runs.
   Leaning: enforce lifetime constraints under tracing only in CI and on test clusters, where a collection can be forced
   at the probe, and watch them in production.
4. **Deep checks for hosts without immutability.** Deep hashes of parameters before and after a call are exact but
   costly. Leaning: on replays and in CI only; read-only proxies at boundaries in production where a constraint is
   enforced.
5. **Projections for interior mutability.** `unchanged view(e)` needs a projection that is not the body. Leaning: a
   named pure function in the catalog, reviewed with the type.
6. **From Python lifetimes to rows.** Should a harvested `owns` on a Python type become a foreign key `on end cascade`
   automatically when its state moves into a table? Leaning: proposed automatically and watched; enforced only after a
   person has looked, since what is lost on deletion is the game's decision. Until then the default is `restrict`, which
   loses nothing.
7. **Deadlines that can act.** Which `after … within` constraints can be enforced by a timer that fires at the deadline,
   rather than only watched? Leaning: those whose remedy is itself a declared procedure, such as returning a ship in
   transit to its origin.
8. **Flows through timing.** Failures are covered, and so is timing where a subject states `time independent of`
   (§ 6.9). The rest of timing is not. Leaning: enforce `time independent of` only where a constant-time analysis runs
   on the compiled code, watch it elsewhere, and watch the few places where timing matters to the game (scans, cloaks)
   on replicas.
9. **Statistics: when to alarm.** Thresholds for `statistic` clauses trade false alarms for missed drift. Leaning:
   sequential tests with a declared false-alarm rate per clause.
10. **Syntax for agents and people.** Indentation reads well and diffs cleanly, but braces survive more tools. Leaning:
    indentation, with one canonical form from the formatter, and a JSON form of the parsed file for tools.
11. **Laws in the language, or in tests.** A `law` is checked only in CI, like a property test. Leaning: keep it here,
    because it binds every implementation of an interface across hosts, which a test in one host does not.
12. **Profiles for other studios' hosts.** C#, Java, Kotlin and PHP profiles are needed before the language is useful to
    the games in the Stillfront adaptation. Leaning: each profile is owned by the team that adopts the host, and is
    tested against the same edge cases as the first three.
13. **Where the bare form lives.** Does a team keep bare source and constraint files in the repository and weave at
    build, or keep ordinary source and strip for review? Leaning: the bare form is the source of truth for code that
    agents write, the ordinary form for code people still edit by hand, with the round-trip law keeping both honest; a
    module moves to the bare form when its file is complete.
14. **Locals that need types.** How often a woven host needs a `local` that it cannot infer decides whether bodies stay
    bare in practice. Leaning: measure it per host on the existing code by stripping and weaving it, and teach `weave`
    the inference that removes the common cases. Rust is measured first
    ([`bare-rust.md`](bare-rust.md), experiment E2).
15. **Macros and generated code.** Types produced by macros, derives and code generators stay where they are generated.
    Leaning: treat a generator's input as its subject, so its constraints are stated once for everything it generates.
16. **How much of independence is written.** Read and write sets can be computed from most bodies, and stating them
    everywhere would repeat the body. Leaning: compute them always; state them at boundaries that others rely on
    without the body (a library, an interface, a procedure the engine schedules), where the stated set is the promise
    and the computed set checks it.
17. **Platforms as files.** Should a platform be written by hand, or only generated from the toolchain for each build
    and shown in review? Leaning: generated, as `builds.json` is now, with a hand-written platform only for a target a
    build cannot run on here, which the toolchain still describes (`rustc --print cfg --target`).
18. **Facts at a point, and how far to compute them.** Branch conditions and preconditions are cheap; postconditions of
    earlier calls and the frame rule need the callees' files and write sets. Leaning: branches, preconditions, `cfg` and
    the platform first, with the frame rule only where every call on the path has a known write set.
19. **Freedoms the code already takes.** A body that iterates a `HashMap` already varies its order, whether or not its
    file says so, and a caller may already rely on it by accident. Leaning: harvest such bodies as candidates, run their
    callers under a shuffled order in CI before a person states the freedom, and report the callers whose result
    changes.
20. **Profiles for code that changes.** A profile is keyed by positions in bodies, which edits move. Leaning: drop what
    no longer matches and measure again, never carry a measurement across an edit; a profile is cheap to remake on the
    corpora, and a stale one is worse than none.


