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
  visibility; and memory: lifetime, mutability and aliasing.
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
| `method` | An RPC method, by service and method | `method inventory.MoveItems(itemIDs: list<u64>, toLocationID: u64)` |
| `function` | A function or method in a host language | `function python 'eve.inventory.InventoryMgr.MoveItems'` |
| `type` | A type of a host language, or of the language itself | `type csharp 'Studio.Ui.InventoryWindow'` |
| `table` | A table in the catalog | `table space.Ships` |
| `resource` | A host type that holds something outside memory: a file, socket, lock, connection or GPU buffer | `resource java 'java.sql.Connection'` |
| `interface` | A trait, interface, protocol or abstract class; the clauses bind every implementation | `interface rust 'edb::Procedure'` |
| `quantity` | A conserved or dimensioned quantity | `quantity Isk : decimal(19, 2)` |
| `conserve` | A conservation law over columns | `conserve Isk across wallet.Balances.balance, …` |
| `failure` | A named failure and its class | `failure NotYours : user` |
| `predicate` | A named, reusable predicate | `predicate flies(s: space.Ships) = s.pilotID == actor` |
| `block` | A named sequence of preconditions | `block in_space(s: space.Ships)` |
| `label` | An information-flow label | `label pii` |
| `scope` | A named span of time that lifetimes can be stated against | `scope fight : from begin(Fight) to end(Fight)` |

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
  of it; `decimal(p, s)`; `f32` and `f64`; `text`; `bytes`; `time`, simulation time at a place in history; `instant`,
  wall-clock time, allowed only in watched constraints; `duration`; `id<T>`, the identity of a row or entity of `T`.
- **Composite:** records `{ name: T, ... }`; tuples; variants `A | B(T)`, closed unless marked `open`; optional `T?`;
  `list<T>`, `set<T>` and `map<K, V>`; `ref<T>`, a reference to a host entity, used by the memory clauses.
- **Refinements:** `T where p`, in which `value` names the value being refined; shorthands `T in a..b` and `T in {a, b,
  c}`; named refinements: `type Quantity = i64 where value >= 0`. A refinement on an optional type applies to the value
  when it is present. `where` always filters: the values of a type here, the rows of a table in an expression (§ 4).
- **Units and quantities:** `unit m`, `unit s`; `quantity Distance : decimal in m`; arithmetic checks dimensions, and
  conversion between units is an explicit named function.
- **Generics:** `for T where T: Comparable` binds a type parameter and its bounds; bounds name interfaces, so they are
  checked as behavioural subtyping (§ 6.10).
- **Representation.** A type in the language says which values are allowed. When a host needs a representation the
  profile does not choose by default, `as` names it: `map<Key, Entry> as 'std::collections::BTreeMap'`.

A host type maps onto these through its profile: T-SQL `INT` to `i32`, `NVARCHAR(50)` to `text where utf16_length(value)
<= 50`, C# `int?` to `i32?`, Rust `Option<u64>` to `u64?`, Python `int` to `int`.

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
  the exact result: `ensure total : result == old(a) + old(b)` fails if the host wrapped. Integer division truncates
  toward zero and `%` takes the sign of the dividend, as in Rust, C#, Java and T-SQL; `floor_div` and `floor_mod` give
  Python's flooring, and a check lifted from Python uses them. Division and modulo must be guarded by an earlier
  conjunct or clause that the divisor is not zero. Decimals round half to even unless a named function says otherwise;
  `round_half_away` matches T-SQL's `ROUND`, and a check lifted from T-SQL uses it.
- **Floats compare as every host compares them.** `==` and the orderings follow IEEE 754 comparison: `-0.0 == 0.0`, and
  every comparison involving `NaN` is false except `!=`. `total_order(a, b)` exists for sorting. Arithmetic is IEEE 754
  binary32 or binary64, rounding to nearest with ties to even, with no fused multiply-add and no reassociation.
  Transcendental functions are named functions with one pinned implementation. `finite x` excludes `NaN` and the
  infinities, and is the usual refinement. `approx(a, b, abs: e, rel: r)` states a tolerance where the code being
  compared did not pin its arithmetic.
- **Text is a sequence of Unicode scalar values.** `==` compares scalars and `length` counts them; `utf16_length` and
  `byte_length(x, utf8)` count code units, which is what `NVARCHAR(n)` and many host limits count. Normalisation and
  collation are named functions: `normalized(x, NFC)`; `collate(x, 'Latin1_General_CI_AS')`, which gives a key that
  compares as the collation does; and `sql_equals(a, b, collation)`, which also ignores trailing spaces as T-SQL's `=`
  does. A comparison lifted from T-SQL is written with them. `x matches /re/` uses linear-time regular expressions only.
- **Time:** `now` is the time of the request at its place in history and `tick` its tick; durations carry units: `50us`,
  `5 ticks`, `30 s`, `2 frames`.
- **Equality and identity:** `==` is structural on values; `same(a, b)` is identity of host references; rows compare by
  key.
- **Collections:** `size`, `in`, `subset`, `distinct`, `sorted by`, `all x in c : p`, `any x in c : p`, `count`, `sum`.
  `min` and `max` return an optional value, absent for an empty collection. In a `law` only, a quantifier may range over
  a type, `all x : T : p`, because laws are checked on generated values and never at run time.
- **Rows:** `T[key]` looks a row up and is optional; `T where p` selects rows. A selection in an enforced predicate must
  be served by an index, or the clause must be an invariant checked row by row as versions are written; the compiler
  states each clause's read set.
- **Context:** `self`, the row or entity a table's or type's clause is about, whose fields are also in scope by name;
  `actor`, the identity on whose behalf a request runs; `session`, `origin`, `request`, `now`, `tick`; in postconditions
  also `result` and `old(e)`, the value of `e` at entry. In temporal clauses `old(e)` is the previous version, and is
  absent at a row's first version.
- **Connectives:** `not`, `and`, `or`, `implies`, `iff`. `and`, `or` and `implies` guard their right side, and narrow it
  as an earlier clause would, so `b != 0 and a / b > 1` is well formed. `if c then a else b` is an expression with a
  value.
- **Functions:** a function used in a constraint is named in the catalog or declared with a reference implementation in
  a host. It must be total, deterministic and free of effects, which is itself checked as a bound (§ 6.3); its generated
  forms are compared like every other check.
- **`let`:** binds a name to a value for the later clauses of its subject. A name is bound once.
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
| Calls | `from`, `require … else`, `ensure`, `on failure`, `fails … when`, `fails only`, `never fails runtime`, `result must be used`, `callback`, `law` | Who may call, what must hold before and after, how a call may fail | Predicate; laws are properties | 6.2 |
| Bounds on effects | `reads only`, `writes only`, `emits only`, `sends only`, `calls only`, `effects only`, `effects none`, `no io`, `no clock`, `no randomness`, `no suspension`, `deterministic`, `idempotent by`, `commutes with`, `depends only on`, `terminates` | What a call must not do | Property | 6.3 |
| State | `invariant`, `unique`, `reference … on end`, `acyclic`, `conserve`, `faucet`, `sink` | What must hold of stored state | Predicate | 6.4 |
| Protocols | `states`, `initial`, `protocol` | Which changes and calls may follow which | Predicate for `states`; property for `protocol` | 6.5 |
| Time and statistics | `always`, `never`, `after … within`, `before`, `since`, `until`, `eventually`, `rate`, `statistic` | What must hold over history | Predicate for `always`, `never` and `rate`; property otherwise | 6.6 |
| Budgets | `budget` | How much time, work, memory and traffic | Property | 6.7 |
| Concurrency | `confined to`, `sendable`, `shareable`, `guarded by`, `lock order`, `atomic`, `linearizable`, `no suspension while holding`, `no blocking in`, `not reentrant`, `cancellation safe`, `publishes` | Who may touch what, from where, in what order | Property | 6.8 |
| Information | `label`, `flows only to`, `never flows … to`, `declassify … when … because`, `sanitised by` | Where information may go | Property | 6.9 |
| Compatibility and visibility | `version`, `compatible with`, `replaces`, `was`, `deprecated`, `stable`, `visible to` | What may change between versions, and who may refer to what | Checked at build | 6.10 |
| Lifetime | `hold`, `lifetime …`, `instances …`, `no strong cycle`, `model` | How long entities live and who keeps them | Property | 7 |
| Mutability and aliasing | `access`, `readonly`, `immutable`, `frozen after`, `write once`, `append only`, `monotonic`, `mutates only`, `interior`, `unchanged`, `exclusive`, `sole`, `disjoint`, `shared readonly` | Who may change what, and through how many paths | Property; predicate for columns | 8 |
| Derived languages | `local` | A type a woven host needs inside a body | Checked at weave | 11.2 |
| Status and placement | `assume … because`, `pin … because`, `not at` | What is taken on trust, what a person fixed, where a clause may not run | — | 10 |

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

method inventory.MoveItems(itemIDs: list<u64> where size(value) in 1..1000 and distinct(value),
                           toLocationID: u64,
                           flag: u8 in {4, 5, 62, 133},
                           quantity: i32? where value > 0)
    -> list<{ itemID: u64, quantity: i32 where value > 0 }>
    require volume_fits : exists quantity implies quantity * inv.unitVolume(itemIDs) fits i64 else TooLarge

type CycleWindow = { started: time, ends: time } where started <= ends
type ShipName    = text where length(value) in 1..32 and value matches /^[\p{L}\p{N} ._'-]+$/
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

procedure market.PlaceOrder(typeID: u32, price: decimal(19, 2) where value > 0, quantity: i32 where value > 0)
    -> id<market.Orders>
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

function kotlin 'studio.ui.withLock'(lock: ref<Lock>, body: ref<Function0>)
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
  hosts with panics is `never panics`.
- `result must be used` binds the callers: a call whose result is discarded is a build error where the host can see it,
  and a lint elsewhere.
- `callback c : called exactly once | at most once | at least once [before return]` states how a function argument is
  called, as Kotlin's `callsInPlace` does; `callback c : conforms to interface rust 'eve_ui::Handler'` applies an
  interface's clauses to whatever is passed.
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

function 'eve.dogma.stacking.penalised'(values: list<f64>) -> f64
    effects none
    depends only on values
    terminates

procedure wallet.Credit(requestID: id<Requests>, amount: decimal(19, 2))
    idempotent by requestID        -- the same request twice changes the world once

-- economy/bounds.constraints: a rule, not a description
interface rust 'eve_market::Procedure'
    writes only market.*, wallet.Escrow
```

- `effects none` is purity: no reads of mutable state, no writes, input and output, clock or randomness. Allocating what
  it returns is not an effect.
- `effects only p` says every effect goes through the named path, as the engine's sealed interface requires of a Rust
  procedure.
- `deterministic` says the same inputs and the same snapshot give the same result, writes and failures. `no randomness
  except rolls` permits the engine's keyed rolls, which are deterministic given the log.
- `no suspension` forbids `await`, `yield` and tasklet switches between entry and exit.
- `commutes with` says two subjects' effects may be applied in either order with the same outcome, which is what lets
  the engine treat them as reductions.
- `depends only on` is the information-flow form of a bound (as SPARK's `Depends`): the result is a function of the
  named inputs alone.
- `terminates` says every call returns or fails. It is proven where the host can prove it and otherwise checked as a
  budget.
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

procedure bounty.Pay(characterID: u64, amount: decimal(19, 2))
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

method inventory.MoveItems(itemIDs: list<u64>, toLocationID: u64)
    budget work O(size(itemIDs))
    budget bytes sent <= 256 * size(itemIDs)

type csharp 'Studio.Ui.InventoryWindow'
    budget retains at most 2 MB per instance
    budget allocates at most 0 B per frame        -- after the first frame
```

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
```

- In the engine, requests run in a fixed order and procedures cannot share mutable state, so most of these are proven by
  construction for procedures; they matter for the Python servers while they remain, for clients, and for services in
  other hosts.
- `confined to` takes a context: `thread [name]`, `task`, `isolation name` (a Swift actor, an Erlang process, any unit
  that owns its state alone), `shard` or `process`.
- `sendable` and `shareable` correspond to Rust's `Send` and `Sync` and Swift's `Sendable`, and are defined by access
  (§ 8): a shareable entity is immutable while shared, or every write to it is guarded.
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

method chat.Send(channelID: u64, message: text)
    label message : untrusted
    sanitised by chat.escape before log, client
```

- Sinks are `client`, `player(x)`, `server`, `ledger`, `chain`, `log`, `analytics`, `external` and `shard(x)`.
- A failure is a flow. A precondition that reads hidden information may fail only with a failure no more revealing than
  its labels allow, and the compiler checks this: a `require` that reveals whether a cloaked ship is in range is a
  breach even if the body never runs. Timing is a flow too, and is out of scope for enforcement (§ 17).
- Labels also decide placement: a constraint may run only where everything it reads may flow (§ 10.3). A constraint that
  reads another pilot's hidden state can never run on a client.
- `declassify` is the one way a label is relaxed, and it always carries a condition and a reason.
- **Checked:** statically through the read sets of predicates and the bounds of subjects; at the proxies' filters, which
  apply visibility as policy; at the stream boundaries where server secrets are stripped; watched in CI by tracing
  labelled values through replays.

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
bounds, lifetimes and access no weaker. `visible to public | crate | module | m` states who may refer to a subject, as
access modifiers do. `edb review` classifies every constraint change in a pull request by this table.

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

function rust 'eve_cache::Cache::get'(self: ref<Cache>, key: ref<Key>) -> ref<Entry>?
    hold result : borrows from self
```

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
- **`lifetime e no use after end`:** no reference is used to reach `e` after `end(e)`; for resources, `no use after
  release`.
- **`lifetime e reachable only from p, q`:** every strong path to `e` from a root passes through one of the named
  holders. It forbids leaks into global caches, event buses and static collections.
- **`lifetime e on end refuse | invalidate`:** under tracked references, ending `e` while tracked references remain
  either fails or invalidates them.
- **`lifetime e stable address [while held by p]`:** `e` does not move while the named paths refer to it, as Rust's
  `Pin` or a pinned handle in a moving collector; needed at boundaries with foreign code.
- **`lifetime every ref<T> [obtained from p] within s`:** the same predicate over every reference of a type, or every
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
| `write any` | This path writes; others may too | `ref`, with confinement | `&RefCell<T>`, `&Cell<T>` | `T&` | Any reference | A class reference | Any reference |
| `read readers` | Nobody writes while this holds: the entity is immutable for the interval | `val` | `&T` without interior mutability | A `const` object, shallowly | A reference to an immutable object | A value type | A reference to a frozen object |
| `read any` | This path cannot write; others may | `box`, with confinement | — | `const T&`, shallowly | A read-only view or interface | — | A read-only proxy |
| `identify any` | Identity only | `tag` | An id or handle | An opaque handle | An id | An id | An id |

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
  inputs except through the named paths.
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
function python 'eve.fitting.FittingSvc.ApplyFit'(self, ship: ref<Ship>, fit: ref<Fit>)
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
- **Refinement types.** `type Quantity = i64 where value >= 0` and `type ShipName = text where …` are named and reused
  in every signature.
- **Generics.** `for T where T: Comparable` binds a type parameter; a clause stated for every `T` applies to every
  instance.
- **Interfaces.** Clauses on an `interface` bind every implementation, under the rules of behavioural subtyping
  (§ 6.10). "Every implementation of `Repository.save` is idempotent by its id" is one clause.
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

procedure space.LaunchDrones(shipID: u64, droneIDs: list<u64>)
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
listed with what would break if it were false.

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
implied by a parameter's type). The reason is recorded.

**Diagnostics** name the constraint, its subject and version, its status, the place it ran, and the inputs that broke
it, redacted by their labels.

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

| Host | Moves to the constraint file | Stays in the bare body | Woven into |
| --- | --- | --- | --- |
| Rust | Types of parameters, results and fields; lifetimes; `mut`, `&` and `&mut`; `pub`; generic bounds and `where` clauses; `Send` and `Sync` bounds; `Option` around a value; error types, as failures; `#[must_use]`, `#[non_exhaustive]`; the types of casts and type arguments | Expressions, control flow, `?`, `match`, `impl` blocks, macros, `unsafe` blocks, constructors and conversions (`as _`), the collection a `collect` builds | Rust with types, lifetimes, `mut` and visibility written, and wrappers for the rest |
| T-SQL | Parameter and column types; `NULL` and `NOT NULL`; `CHECK`, `UNIQUE` and `FOREIGN KEY` | Statements; defaults; triggers' bodies | DDL and procedures, with generated checks at procedures' heads |
| C# | Types of members and parameters; nullable annotations; `readonly`, `init` and `in`; access modifiers; `sealed`; generic constraints; the access `ref` and `out` grant | Bodies, passing by `ref` and `out`, `using`, LINQ, lambdas | C# with types and modifiers written, and analyser rules |
| Java, Kotlin | Types; `final` and `val`; nullability; access modifiers; `throws`; generic bounds | Bodies, `try`-with-resources, lambdas | Java or Kotlin with types and modifiers written |
| Python | Type hints; `Final`; `frozen=True`; `@final` | Everything else | Python with hints written and decorators that check at run time |
| TypeScript | Types; `readonly`; optional markers; access modifiers; `as`, `satisfies` and `!` | JavaScript | TypeScript |

**Inside bodies.** Hosts infer most local types, so a bare body rarely needs one. Where a woven host needs the type of a
local and cannot infer it, the file gives it: `local total : decimal(19, 2)` under the function's subject, binding the
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
| String length and format | SQL `NVARCHAR(n)`, validators | `text where utf16_length(value) <= n and value matches /…/` |
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

function python 'eve.inventory.InventoryMgr.MoveItems'(self, itemIDs: list<u64>, toLocationID: u64)
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
    lifetime every ref<edb::Row> obtained from tx within transaction
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
    field entries : map<Key, Entry> as 'std::collections::HashMap'
    hold entries : owns

function rust 'eve_cache::Cache::get'(self: ref<Cache>, key: ref<Key>) -> ref<Entry>?
    visible to public
    access self : read readers
    access key : read readers
    hold result : borrows from self

function rust 'eve_cache::Cache::insert'(self: ref<Cache>, key: Key, entry: Entry)
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
              | conserve ;

subject       = subject_head NL INDENT { clause NL } DEDENT ;
subject_head  = ( "procedure" | "method" ) QNAME params [ "->" type ]
              | ( "function" | "type" | "resource" | "interface" ) [ HOST ] STABLE [ params ] [ "->" type ]
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

clause        = "let" NAME "=" expr
              | "from" NAME { "," NAME }
              | "require" [ NAME ":" ] expr [ "else" NAME [ payload ] ]
              | "ensure" [ NAME ":" ] expr
              | "on failure" ( "unchanged" | NAME "ensure" NAME ":" expr )
              | "fails" NAME "when" expr | "fails only" NAME { "," NAME } [ "open" ]
              | "never fails runtime" | "never panics" | "result must be used"
              | "callback" NAME ":" ( "called" ( "exactly once" | "at most once" | "at least once" )
                                      [ "before return" ]
                                    | "conforms to" "interface" [ HOST ] STABLE )
              | "law" NAME ":" expr
              | "field" NAME ":" type
              | "layout" ( "c" | "packed" | "transparent" ) | "size" NUMBER | "align" NUMBER
              | "local" NAME ":" type
              | bound | state | protocol | temporal | budget | concurrency | flow | compat | memory | access
              | "use" NAME "(" [ expr { "," expr } ] ")"
              | "assume" NAME ":" expr "because" STRING
              | "pin" NAME status "because" STRING
              | "not at" place { "," place }
              | "model" MODEL ;
payload       = "{" NAME ":" expr { "," NAME ":" expr } "}" ;

bound         = ( "reads only" | "writes only" | "emits only" | "sends only" | "calls only" | "effects only" )
                  path_list
              | "effects none" | "no io" | "no clock" | "no randomness" [ "except rolls" ] | "no suspension"
              | "deterministic" | "idempotent by" path | "commutes with" QNAME
              | "depends only on" path_list | "terminates" ;
state         = "invariant" NAME ":" expr [ "holds" ( "at write" | "at commit" | "between calls" | "always" ) ]
              | "unique" NAME ":" "(" path_list ")" [ "where" expr ] [ "per shard" | "global" ]
              | "reference" path "->" [ "one of" ] QNAME { "," QNAME }
                  "on end" ( "cascade" | "restrict" | "nullify" | "dangle" )
              | "acyclic" NAME ":" "via" path
              | ( "faucet" | "sink" ) NAME [ "at most" expr ]
              | "states" path ":" transition { "," transition } | "initial" path ":" NAME ;
transition    = NAME "->" NAME { "|" NAME } [ "when" expr ] ;
protocol      = "protocol" regex_over_calls ;
temporal      = ( "always" | "never" | "eventually" ) NAME ":" expr
              | "after" event ":" expr "within" duration
              | ( "before" | "since" | "until" ) event ":" expr [ "within" duration ]
              | "rate" [ "per" scope_ref ] "<=" NUMBER "per" duration
              | "statistic" NAME ":" expr "within" expr ".." expr "over" NUMBER "samples" ;
budget        = "budget" ( percentile "<=" duration [ "on" QNAME ] | "work" complexity
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
              | "sanitised by" QNAME "before" sink { "," sink } ;
compat        = "version" NUMBER | "compatible with" NUMBER { "," NUMBER } | "replaces" HOST STABLE
              | "was" STABLE | "deprecated" [ "since" NUMBER ] | "stable"
              | "visible to" ( "public" | "crate" | "module" | QNAME ) ;
memory        = "hold" path ":" ( "owns" | "shares" | "borrows" [ "from" path ] | "observes" [ "softly" ]
                                | "identifies" )
              | "lifetime" target lifetime_pred
              | "instances" [ "per" scope_ref ] "at most" NUMBER
              | "instances attributed to" scope_ref "are none within" duration "after" event
              | "no strong cycle" [ "through" path_list ] ;
target        = path | "every ref<" type ">" [ "obtained from" path ] | "rows" ;
lifetime_pred = "outlives" path | "within" scope_ref | "ends with" path | "does not escape" [ scope_ref ]
              | "escapes only to" path_list | "ends within" duration "after" event
              | "at most" duration [ "after" event ]
              | "released by" NAME "exactly once" | "released before end" | "ends by" NAME "exactly once"
              | "no use after" ( "end" | "release" ) | "reachable only from" path_list
              | "stable address" [ "while held by" path_list ] | "on end" ( "refuse" | "invalidate" ) ;
access        = "access" path ":" ( "identify" | "read" | "write" ) ( "exclusive" | "readers" | "any" )
              | "readonly" path [ "shallow" | "transitively" ]
              | "immutable" path [ "deep" | "shallow" ] [ "after" event ] | "frozen after" event
              | "write once" path | "append only" [ path ]
              | "monotonic" path ( "increasing" | "non-decreasing" | "decreasing" | "non-increasing" )
              | "mutates only" path_list | "interior" path | "unchanged" "view(" path ")" [ "=" expr ]
              | "exclusive" path | "sole" path | "disjoint" path "," path { "," path } | "shared readonly" path ;

path          = NAME { "." NAME | "[" expr "]" } [ ".*" ] | STABLE ;
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
(* type, record_type, generics, duration, size, percentile, complexity, unit_expr and regex_over_calls are
   written as in the examples: "u64?", "map<K, V> as 'std::collections::BTreeMap'", "{ a: T }",
   "for T where T: C", "50us", "2 MB", "p99", "O(size(x))", "m / s", "begin (send | receive)* finish" *)

expr          = (* literals, paths, T[key], T where p, calls of named pure functions, not/and/or/implies/iff,
                   comparisons, arithmetic, if-then-else, exists, is, in, matches, fits, finite, approx,
                   all/any/count/sum/min/max over collections, old(e), result, same(a, b), view(e);
                   defined in § 4 *) ;
```

---

## 16. What the language cannot say

- **What a function computes,** in general. Formulas are reference functions compared by output; laws and postconditions
  can pin properties of them, never the whole.
- **Anything about a body's intermediate values.** A rule over one becomes a named pure function, or stays in the code;
  `local` gives a woven host a type, never a condition.
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
8. **Flows through timing.** Failures are covered; timing is not. Leaning: keep timing out of scope for enforcement, and
   watch the few places where it matters (scans, cloaks) on replicas.
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


