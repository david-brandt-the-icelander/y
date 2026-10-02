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
  C#, Java, Kotlin, Swift, C++, Go, PHP and TypeScript. A file bound to nothing does not build.
- **One expression language, defined once.** Two-valued logic with explicit absence, exact arithmetic, defined
  comparison of text and floats, total evaluation and a read set known before evaluation. Each constraint is compiled
  into every place that enforces it, and the generated checks are compared on the recorded corpora so that no two places
  disagree.
- **Clauses in families.** Values and shapes; calls (authority, preconditions, postconditions, failures, laws); frames
  (what a call must not do); state (invariants, keys, references, conservation); protocols and state machines; order in
  time and statistics; budgets; concurrency; information flow; compatibility; and memory: lifetime, mutability and
  aliasing.
- **Memory, written once for every model.** Lifetime, mutability and aliasing are stated over a core that belongs to no
  host: entities with lifetimes; references that own, share, borrow, observe or only identify what they point to; and
  paths that carry access rights and guarantees about every other path. A memory model per host gives the core a precise
  meaning: ownership (Rust), tracing collection (Java, C#, Go, JavaScript), reference counting (Swift, CPython, PHP,
  `shared_ptr`, `Rc`), tracked references (generational handles), regions, manual management (C), and rows in a
  database.
- **Status and evidence, always.** Every constraint is observed, lifted, watched, enforced, proven or assumed with a
  stated reason, and carries the evidence for that status. It is checked by the cheapest means the host allows: the
  host's own compiler, a static analysis, a runtime check, or watching on replicas, test clusters and in CI.
- **A fixed grammar, kinds enabled one at a time.** The grammar below is the whole language, so that early files never
  need rewriting. A kind is enabled when something consumes it, with its miner and its compilation into every place that
  enforces it; a file that uses a kind not yet enabled does not build. The first release enables argument types and
  nothing else.

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
  do. A frame such as `writes only wallet.Balances` is a limit; the manifest is the evidence checked against it.
- **Not a specification of formulas.** Damage after resistances, stacking penalties, movement and market matching are
  functions, not predicates. Their specification is a reference function, compared with its replacement by output. The
  language may call such a function by name; it never restates one.
- **Not a replacement for a type system.** Where a host's type system already proves a constraint, the constraint is
  `proven` by it and costs nothing at run time. The language states it anyway, so that hosts without that type system
  enforce it too, and so that it survives translation into a host that would otherwise lose it.

The last point is why the language covers lifetime, mutability and aliasing. When a Python manager becomes a Rust
procedure, or a C# service is rewritten, what it promised about which objects it keeps, which it changes and which it
shares must carry across. Only a statement that belongs to no host can carry it.

---

## 2. Principles

1. **It states, and never does.** A constraint is a named predicate. Evaluating it changes nothing.
2. **One meaning everywhere.** Absence, arithmetic, comparison, text, time and identity are defined once, in the
   language (§ 4). The code generated for each place is compared on the recorded corpora and on generated edge cases, so
   that the proxy, the Python servers, T-SQL and the engine cannot disagree.
3. **Total and bounded.** Every constraint evaluates, on every input, to true or false, in bounded work, with a read set
   known before it runs. No partial operation is evaluated unguarded, and no quantifier in an enforced constraint ranges
   over something unbounded.
4. **Beside the code, bound by name.** A file binds to its subjects by stable name. The build refuses a file bound to
   nothing and a subject that does not resolve, and every change to a constraint is shown in review.
5. **The boundary, never the body.** A constraint may refer to a subject's inputs, results and failures; its session,
   actor and origin; the world as the catalog describes it; named pure functions; and what happens to the entities that
   cross the boundary: whether they are written, retained, released or shared. It may not refer to a local variable or
   an intermediate value. A check over an intermediate value becomes a named pure function that both can use, or stays
   in the code.
6. **Order is evaluation order.** Within a subject, clauses run in the order written. The first precondition that fails
   decides the failure the caller sees, so lifting checks out of code keeps their order. Order also narrows: after
   `require found : exists m`, later clauses may use `m` as present.
7. **Status and evidence for every constraint.** Each has a status and the evidence for it (§ 10), and every change of
   status is recorded with its reason.
8. **One kind at a time, each in a paragraph.** A kind of constraint is added when something consumes it, and only if
   its meaning can be stated in a paragraph. Sections 6, 7 and 8 open each kind with that paragraph.
9. **People decide what inference must not.** Authorisation, visibility, conservation policy, the game's formulas and
   every boundary with the outside are decided by people. Traffic may tighten them, never define them.

---

## 3. Files, subjects and hosts

### 3.1 Files

A constraint file is UTF-8 text named `*.constraints`, structured by indentation, with comments from `--` to the end of
the line. It opens with an optional `module` line and any `use` lines, then holds subject blocks in any order. Inside a
subject, the order of clauses is the order of evaluation. There is one canonical form, produced by the formatter, so
that diffs show only meaning.

```text
-- space/modules.constraints
module space
use space.predicates          -- named predicates: flies, in_space, docked
use economy                   -- quantities: Isk, ItemQuantity
```

### 3.2 Subjects

| Subject | Binds to | Example |
| --- | --- | --- |
| `procedure` | A stored procedure in the catalog, in Rust or T-SQL | `procedure space.ActivateModule(moduleID: u64, targetID: u64)` |
| `method` | An RPC method, by service and method | `method inventory.MoveItems(itemIDs: list<u64>, toLocationID: u64)` |
| `function` | A function or method in a host language, by its path | `function python 'eve.inventory.InventoryMgr.MoveItems'` |
| `type` | A type of a host language, or of the language itself | `type csharp 'Studio.Ui.InventoryWindow'` |
| `table` | A table in the catalog | `table space.Ships` |
| `resource` | A host type that holds something outside memory: a file, socket, lock, connection or GPU buffer | `resource java 'java.sql.Connection'` |
| `interface` | A trait, interface, protocol or abstract class; the clauses bind every implementation | `interface rust 'edb::Procedure'` |
| `quantity` | A conserved or dimensioned quantity | `quantity Isk : decimal(19, 2)` |
| `failure` | A named failure and its class | `failure NotYours : user` |
| `predicate` | A named, reusable predicate | `predicate flies(s: space.Ships) = s.pilotID == actor` |
| `label` | An information-flow label | `label pii` |
| `scope` | A named span of time that lifetimes can be stated against | `scope fight : from begin(Fight) to end(Fight)` |

A host subject names its host and its path: `function rust 'eve_space::fit::apply'`, `type python
'eve.inventory.LocationCache'`, `function csharp 'Studio.Shop.Checkout.Pay(System.Int64)'`. A renamed symbol keeps its
constraints and their evidence through `was 'old.path'`, which the build checks against the history of the code.

### 3.3 Host profiles

Each host language has a profile, owned by the language's team, that says how the host's types map onto the language's
types (§ 3.4), which memory model it follows (§ 7.4), how its references map onto access and aliasing (§ 8.5), which
kinds its compiler or analyses prove, where a runtime check can be inserted, and how a constraint is watched.

| Host | Memory model | Proven by the host | Runtime checks inserted at | Watched by |
| --- | --- | --- | --- | --- |
| Rust | Ownership, with counting inside `Rc` and `Arc` | Types, borrows, exclusivity, `Send` and `Sync`, no use after end in safe code | The procedure's head in the engine; generated wrappers elsewhere | Replay; Miri and sanitizers in CI |
| T-SQL and SQL | Rows | `NOT NULL`, `CHECK`, `UNIQUE` and foreign keys where declared | Generated T-SQL at a procedure's head; the engine at each write | Replicas |
| Python (CPython) | Counting, with a cycle collector | Little; type hints where a checker runs | Decorators at entry and exit; read-only proxies | Effect manifests; heap snapshots and reference probes on test clusters |
| C#, Java, Kotlin, Go, JavaScript, TypeScript | Tracing | Types; nullability where enabled; `readonly`, `final` and `val`, shallowly; escape analysis where the compiler reports it | Entry and exit wrappers; analysers at build | Heap snapshots; weak-reference probes |
| Swift, Objective-C | Counting (ARC) | Types; `Sendable` and actor isolation; non-escaping closures | Entry and exit wrappers | Memory graph snapshots |
| C++ | Manual, with RAII; counting through `shared_ptr` | Types; `const` as a view | Generated wrappers | Sanitizers in CI |
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
  c}`; named refinements: `type Quantity = i64 where value >= 0`.
- **Units and quantities:** `unit m`, `unit s`; `quantity Distance : decimal in m`; arithmetic checks dimensions, and
  conversion between units is an explicit named function.

A host type maps onto these through its profile: T-SQL `INT` to `i32`, `NVARCHAR(50)` to `text where length(value) <=
50`, C# `int?` to `i32?`, Rust `Option<u64>` to `u64?`, Python `int` to `int`.

---

## 4. Expressions, defined once

- **Absence is a value, and logic has two values.** Absence is `none`, a value of every optional type. `exists x` and `x
  is none` test for it; `x?.f` is `none` when `x` is. Comparing an optional value with anything but `none` is a type
  error until it has been narrowed. SQL's `NULL` is `none`, and the T-SQL generated from a constraint uses `IS NULL`
  explicitly, so three-valued logic never decides a constraint.
- **Narrowing follows order.** A clause that has held narrows what follows it: after `require found : exists m`, `m` is
  present in every later clause of the subject; after `require is_ship : x is Ship(s)`, `s` is bound.
- **Arithmetic is exact.** Integers in constraints are unbounded and decimals exact, so a constraint never overflows.
  `fits T` asks whether a value is representable in a host type. Overflow in the host is caught by stating the exact
  result: `ensure total : result == old(a) + old(b)` fails if the host wrapped. Division and modulo must be guarded by
  an earlier conjunct or clause that the divisor is not zero. Decimal rounding is half-even unless a named function says
  otherwise.
- **Floats are IEEE 754, totally ordered.** `f32` and `f64` compare by the standard's total order, so `NaN` compares the
  same way everywhere. `finite x` excludes `NaN` and the infinities, and is the usual refinement.
- **Text is a sequence of Unicode scalar values.** `==` compares scalars and `length` counts them. Normalisation and
  collation are named functions, `normalized(x, NFC)` and `collate(x, 'Latin1_General_CI_AS')`, so that a
  case-insensitive comparison inherited from T-SQL is written as one. `x matches /re/` uses linear-time regular
  expressions only.
- **Time:** `now` is the time of the request at its place in history and `tick` its tick; durations carry units: `50us`,
  `5 ticks`, `30 s`, `2 frames`.
- **Equality and identity:** `==` is structural on values; `same(a, b)` is identity of host references; rows compare by
  key.
- **Collections:** `size`, `in`, `subset`, `distinct`, `sorted by`, `all x in c : p`, `any x in c : p`, `count`, `sum`,
  `min`, `max`. In a `law` only, a quantifier may range over a type, `all x : T : p`, because laws are checked on
  generated values and never at run time.
- **Rows:** `T[key]` looks a row up and is optional; `T where p` selects rows. A selection in an enforced constraint
  must be served by an index, or the clause must be an invariant checked row by row as versions are written; the
  compiler states each clause's read set.
- **Context:** `self`, the row or entity a table's or type's clause is about, whose fields are also in scope by name;
  `actor`, `session`, `origin`, `request`, `now`, `tick`; in postconditions also `result` and `old(e)`, the value of `e`
  at entry.
- **Connectives:** `not`, `and`, `or`, `implies`, `iff`. `and` and `implies` guard their right side, so `b != 0 and a /
  b > 1` is well formed. `if c then a else b` is an expression with a value.
- **Functions:** a function used in a constraint is named in the catalog or declared with a reference implementation in
  a host. It must be total, deterministic and free of effects, which is itself checked as a frame (§ 6.3); its generated
  forms are compared like every other check.
- **`let`:** binds a name to a value for the later clauses of its subject. A name is bound once.
- **Cost:** every clause has a static cost: rows read by key, index ranges, and collection sizes bounded by argument
  shapes. Placement uses it (§ 10.3).

---

## 5. The clauses at a glance

| Family | Clauses | Says | Section |
| --- | --- | --- | --- |
| Values and shapes | parameter types, `returns`, `type … where`, `quantity`, `unit` | What values may cross a boundary | 6.1 |
| Calls | `from`, `require … else`, `ensure`, `fails … when`, `fails only`, `never fails runtime`, `result must be used`, `law` | Who may call, what must hold before and after, how a call may fail | 6.2 |
| Frames | `reads only`, `writes only`, `emits only`, `sends only`, `calls only`, `effects none`, `no io`, `no clock`, `no suspension`, `no allocation`, `deterministic`, `idempotent by`, `commutes with`, `depends only on`, `terminates` | What a call must not do | 6.3 |
| State | `invariant`, `key`, `unique`, `reference … on end`, `acyclic`, `placement follows`, `conserve`, `faucet`, `sink` | What must hold of stored state | 6.4 |
| Protocols | `states`, `protocol` | Which changes and calls may follow which | 6.5 |
| Time and statistics | `always`, `never`, `after … within`, `rate`, `statistic` | What must hold over history | 6.6 |
| Budgets | `budget` | How much time, work, memory and traffic | 6.7 |
| Concurrency | `confined to`, `sendable`, `shareable`, `guarded by`, `lock order`, `atomic`, `no suspension while holding`, `no blocking in` | Who may touch what, from where | 6.8 |
| Information | `label`, `flows only to`, `never flows … to`, `declassify … when … because`, `untrusted`, `sanitised by` | Where information may go | 6.9 |
| Compatibility | `version`, `compatible with`, `replaces`, `was`, `deprecated`, `stable` | What may change between versions | 6.10 |
| Lifetime | `hold`, `lifetime …`, `instances …`, `no strong cycle`, `model` | How long entities live and who keeps them | 7 |
| Mutability and aliasing | `access`, `readonly`, `immutable`, `frozen after`, `write once`, `append only`, `monotonic`, `mutates only`, `interior`, `unchanged`, `exclusive`, `unique`, `disjoint` | Who may change what, and through how many paths | 8 |
| Status | `assume … because`, `pin … because` | What is taken on trust, and what a person fixed | 10 |

---

## 6. The kinds of constraint

Each kind opens with the paragraph that is its meaning, then its syntax, how it fails, how it is checked and how it is
harvested.

### 6.1 Values and shapes

**Meaning.** A value that crosses a boundary has a type, a shape and a range: its width and signedness, whether it may
be absent, the set of values it may take, the size and order of a collection, the format and length of a text, the
relations between the fields of a record, and the unit of a quantity. A value outside its type is rejected at the
boundary before the subject runs.

```text
method inventory.MoveItems(itemIDs: list<u64> where size(value) in 1..1000 and distinct(value),
                           toLocationID: u64,
                           flag: u8 in {4, 5, 62, 133},
                           quantity: i32? where value > 0)
    returns list<{ itemID: u64, quantity: i32 where value > 0 }>

type CycleWindow = { started: time, ends: time } where started <= ends
type ShipName    = text where length(value) in 1..32 and value matches /^[\p{L}\p{N} ._'-]+$/
quantity Distance : decimal in m
```

- **Fails** as a runtime error at the boundary, with the parameter, the clause and the offending value, redacted by its
  labels (§ 6.9).
- **Checked** at the proxy, from which the RPC schema (protobuf or a format like it) is generated with encoders and
  decoders for both ends; by the host's types where they suffice; by generated wrappers elsewhere.
- **Harvested** from the effect manifests at entry: the types, shapes, ranges and sets of values each method was called
  with.

Overflow in the host is stated, not assumed away: `require volume_fits : quantity * type.volume fits i64 else TooLarge`.

### 6.2 Calls: authority, preconditions, postconditions, failures and laws

**Meaning.** A call may be made only by the origins it names. Before its body runs, its preconditions hold, in order, or
it fails with the failure of the first that does not, having changed nothing. After it returns, its postconditions hold
of its result, its inputs and the world, or the subject has a bug. It fails only in the ways its failures name, each in
one of two classes: user failures, shown to the player, and runtime failures, which clients suppress.

```text
failure Gone        : user
failure NotYours    : user
failure OutOfRange  : user { distance: Distance, range: Distance }
failure Overloaded  : runtime
failure NotDocked   : user
failure InsufficientFunds : user

procedure market.PlaceOrder(typeID: u32, price: decimal(19, 2) where value > 0, quantity: i32 where value > 0)
    -> id<market.Orders>
    from Player
    require trades_here : exists session.stationID                         else NotDocked
    let w = wallet.Balances[actor]
    require can_escrow  : w.balance >= price * quantity                     else InsufficientFunds
    ensure escrowed     : wallet.Balances[actor].balance == old(w.balance) - price * quantity
    ensure listed       : market.Orders[result].quantity == quantity
    fails only NotDocked, InsufficientFunds, Overloaded
    result must be used

function rust 'eve_net::frame::encode'(f: Frame) -> bytes
    effects none
function rust 'eve_net::frame::decode'(b: bytes) -> Frame?
    effects none
    law round_trip : all f : Frame : decode(encode(f)) == f
```

- `from` names origins: `Player`, `Gm`, `Timer`, `Reaction`, `Shard`, `Sql`, `External`, `Contract`, or a host's roles.
- `require name : p else F` rejects with `F`; without `else`, the failure is a runtime failure named after the clause.
- `ensure name : p` may use `result` and `old(e)`. A false postcondition is a bug in the subject: the call fails as a
  runtime error and its transaction rolls back, and the clause is watched before it is enforced.
- `fails F when p` says the call fails with `F` whenever `p` held at entry. `fails only` makes the list exhaustive.
  `never fails runtime` is the strongest form, and `never panics` its name for hosts with panics.
- `result must be used` binds the callers: a call whose result is discarded is a build error where the host can see it,
  and a lint elsewhere.
- `law` states an algebraic property over one or more subjects (round trips, inverses, idempotence of a pure function,
  associativity of a merge). It quantifies over a type, so it is checked in CI on generated values and on the recorded
  corpora, never at run time.
- **Checked:** preconditions at the proxy when they need only the call and the session, which the server may then
  assume; otherwise in the engine before the body, as predicate reads; in the Python servers by decorators; at the head
  of a T-SQL procedure while it still runs on SQL Server. Postconditions in the engine at commit, and on replicas when
  watched.
- **Harvested:** preconditions are lifted from guard clauses and `IF … THROW`; failures from manifests at exit;
  postconditions from relations between inputs, outputs and row changes in the recordings.

### 6.3 Frames: what a call must not do

**Meaning.** A frame bounds a subject's effects. It never lists what the subject does; it says what the subject may not
do beyond a bound: the rows it may read or write, the events it may emit, the calls and messages it may send, whether it
may perform input and output, read a clock or a random source, suspend, allocate, or depend on anything but the inputs
it names. A subject that exceeds its frame has a bug, whether or not anything else went wrong.

```text
procedure space.ActivateModule(moduleID: u64, targetID: u64)
    reads only space.Modules, space.Ships, movement.*
    emits only CapacitorConsumed, ModuleActivated
    sends only space.CompleteCycle
    no io
    no clock                     -- `now` comes from the request, never from a clock
    deterministic

function rust 'eve_dogma::stacking::penalised'(values: list<f64>) -> f64
    effects none                 -- pure: no reads, writes, io, clock, randomness or allocation that escapes
    depends only on values
    terminates

procedure wallet.Credit(requestID: id<Requests>, amount: decimal(19, 2))
    idempotent by requestID      -- the same request twice changes the world once
```

- `effects none` is purity. `effects only tx` says every effect goes through the named path, as the engine's sealed
  interface requires of a Rust procedure.
- `deterministic` says the same inputs and the same snapshot give the same result, writes and failures. `no randomness
  except rolls` permits the engine's keyed rolls, which are deterministic given the log.
- `no suspension` forbids `await`, `yield` and tasklet switches between entry and exit; a check after a suspension is
  where races live, so this is often the first frame lifted from Python.
- `no allocation` and `allocates at most` are for hot paths; they are budgets in effect, and are checked as budgets
  (§ 6.7).
- `commutes with` says two subjects' effects may be applied in either order with the same outcome, which is what lets
  the engine treat them as reductions.
- `depends only on` is the information-flow form of a frame (as SPARK's `Depends`): the result is a function of the
  named inputs alone.
- `terminates` says every call returns or fails. It is proven where the host can prove it and otherwise checked as a
  budget.
- **Checked:** statically where the bound is reachable: the DAL procedures and MachoNet calls a Python method can reach,
  the sealed interface of a Rust procedure, a pure function's call graph; at run time at the exits the manifests already
  hook; and by comparing every manifest with its frame.
- **Harvested:** from the manifests, as the union of what was seen, which a person or a soak window turns into a bound.

### 6.4 State: invariants, keys, references and conservation

**Meaning.** Stored state obeys rules whenever anyone can see it: a row's invariants hold of every version written; keys
are unique; a reference names a row that exists, or the rule for when its target ends says otherwise; containment has no
cycles; a row lives in the shard its rule says; and a conserved quantity is neither created nor destroyed except by a
declared faucet or sink. A transaction that would break any of them fails before anyone reads its result.

```text
table space.Ships
    invariant destroyed_means_no_structure : not defenses.destroyed or defenses.structure == 0
    invariant capacitor_non_negative       : capacitor.capacitor >= 0
    invariant capacitor_within_capacity    : capacitor.capacitor <= dogma.capacity(self)

table inv.Items
    key itemID
    unique singleton_blueprint : (typeID, locationID) where singleton and flag == Blueprint
    reference locationID -> inv.Locations on end restrict
    reference ownerID    -> chr.Characters on end dangle
    acyclic containment : via locationID
    placement follows outermost locationID

type kotlin 'studio.match.Score'
    invariant ordered : home >= 0 and away >= 0 holds between calls   -- never broken at a public boundary

-- economy.constraints
conserve Isk across wallet.Balances.balance, market.Orders.escrow, contract.Contracts.collateral
conserve ItemQuantity across inv.Items.quantity

procedure bounty.Pay(characterID: u64, amount: decimal(19, 2))
    faucet Isk at most bounty.cap(characterID, now)
```

- `invariant` on a table holds at every version written, reductions included. On a host type it holds `between calls`
  (at every public boundary, as Eiffel's class invariants) by default, or `always` where the host can check field
  writes.
- `reference f -> T on end r` states what happens when the target ends: `cascade` ends the referrer with it, `restrict`
  refuses to end the target while referred to, `nullify` makes the reference absent, and `dangle` keeps an identity that
  may no longer resolve. These are the row model's ways of holding (§ 7.2).
- `acyclic` and `placement follows` are the containment rules harvested from backups: no item contains itself, and an
  item's shard follows its outermost container.
- `conserve`, `faucet` and `sink` are as in the proposal (§ 6.9): in every transaction the deltas of a conserved
  quantity's columns, plus what transfers and requests carry, sum to zero, unless the procedure declares itself a faucet
  or a sink, with an optional bound.
- **Checked:** by the engine at each write and at commit; by SQL Server for the constraints it already declares; on
  replicas, as watched constraints, while a table is still on SQL Server.
- **Harvested:** row invariants from historical backups from the first day; conservation and each procedure's role as
  faucet or sink from the manifests and the log.

### 6.5 Protocols and state machines

**Meaning.** Some state changes only along declared paths: a module's cycle goes from idle to active and back, never
from cooling to active. Some objects accept calls only in a declared order: a stream is opened, used, then closed or
reset, and nothing follows the close. A state machine constrains the values a field takes over time; a protocol
constrains the order of calls on an entity. A change or call off the declared path is rejected, or reported when
watched.

```text
table space.Modules
    states cycle : Idle -> Active, Active -> Cooling | Idle, Cooling -> Idle
    initial cycle : Idle

table chr.Characters
    states location : Docked -> Undocking, Undocking -> InSpace, InSpace -> Warping | Jumping | Docking,
                      Warping -> InSpace, Jumping -> InSpace, Docking -> Docked

resource rust 'eve_net::QuicStream'
    protocol open (send | receive)* (close | reset)
```

- A `states` clause is checked at each write of the field, against the version it replaces.
- A `protocol` is a regular expression over calls on one entity, checked per entity at each call (typestate). In Rust it
  can often be proven by the typestate pattern, one type per state; elsewhere it is a small automaton per entity,
  inserted by wrappers or watched from the manifests.
- **Harvested:** state machines per entity from the manifests and the log, in simulation time, separating the game's
  hidden states from the old mechanism's (a ship in neither system was the mechanism's).

### 6.6 Order in time, and statistics

**Meaning.** Some rules are about history rather than any one state: a value never decreases, a ship in transit is in a
system again within a few ticks, a session sends no more than so many calls a second, a loot table pays out at its
declared rate. A temporal constraint is a predicate over the log, bounded in time; a statistic is a predicate over many
samples, and is only ever watched.

```text
table chr.Skills
    never points_decrease : points < old(points)

table space.Transits
    after begin(self) : exists space.Ships[shipID].solarSystemID within 3 ticks

method space.CmdWarpTo(targetID: u64)
    rate per session <= 10 per 1 s

procedure loot.Roll(tableID: u32)
    statistic drop_rate : rate(result.dropped) within 0.95..1.05 of loot.declared(tableID) over 100000 samples
```

- `always p` and `never p` are invariants over successive versions; `old(…)` in them means the previous version. Because
  a violation is caused by a write, they are enforced at that write like any invariant.
- `after e : p within d` requires `p` to hold at some point within `d` after event `e`; `before`, `since` and `until`
  are available with the same bounds. An unbounded `eventually` may be written only as watched.
- `rate` bounds how often something happens per scope. A learned limit is priced by cost and enforced at the proxy.
- `statistic` compares an estimator over samples with an interval and alarms when it leaves it; it never rejects
  anything.
- **Checked:** on replicas and the temporal tier, at no cost to the authority; a `within` constraint becomes enforceable
  only where a timer can act at its deadline, which is an open question (§ 17).
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
forbidden. A breach is a race, and a race is a bug whether or not it was observed.

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

function python 'eve.market.MarketMgr.PlaceOrder'
    atomic                         -- no other request observes it half done
    no blocking in tasklet
```

- In the engine, requests run in a fixed order and procedures cannot share mutable state, so most of these are proven by
  construction for procedures; they matter for the Python servers while they remain, for clients, and for services in
  other hosts.
- `confined to` takes a context: `thread`, `task`, `actor`, `shard`, `process`, or a named one such as the UI thread.
- `sendable` and `shareable` correspond to Rust's `Send` and `Sync` and Swift's `Sendable`, and are defined by access
  (§ 8): a shareable entity is immutable, or every write to it is guarded.
- `atomic` says no other execution observes an intermediate state of the subject; `linearizable` is its watched form for
  concurrent data structures.
- **Checked:** by host compilers where they can (Rust, Swift, Clang's thread-safety analysis); by analysers and runtime
  lock-order checkers in CI; watched in production from manifests.

### 6.9 Information flow

**Meaning.** Information carries labels, and a label says where it may go: what is personal stays off published ledgers
until encrypted, a server's secrets never reach a player or a chain, what a pilot may not see never reaches that pilot's
client, input from a client is untrusted until checked. A flow from a labelled source to a sink the label does not
allow, directly or through a result, a failure, a timing or a log line, is a breach.

```text
label pii
label secret(server)
label hidden(owner)
label untrusted

table accounts.Users
    label email, birthDate, lastAddress : pii
    never flows pii to ledger, chain, analytics unless encrypted
    lifetime rows with pii at most 30 days after closedAt

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
  its labels allow, and the compiler checks this; a `require` that reveals whether a cloaked ship is in range is a
  breach even if the body never runs.
- Labels also decide placement: a constraint may run only where everything it reads may flow (§ 10.3). A constraint that
  reads another pilot's hidden state can never run on a client.
- `declassify` is the one way a label is relaxed, and it always carries a condition and a reason.
- **Checked:** statically through the read sets of constraints and the frames of subjects; at the proxies' filters,
  which apply visibility as policy; at the stream boundaries where server secrets are stripped; watched in CI by tracing
  labelled values through replays.

### 6.10 Compatibility and versions

**Meaning.** A subject's constraints are part of its contract, so changing them can break callers, dependants and stored
data. A constraint change that would break any of them is either refused, or shipped in two releases (expand, then
contract), or carried by a new version that runs beside the old one.

```text
procedure space.ActivateModule(moduleID: u64, targetID: u64)
    version 148
    compatible with 147
    replaces python 'space.modules.ActivateModule'
    was 'dogma.ActivateModule'

type rust 'eve_net::Message'
    stable                       -- its encoded form may not change
variant market.OrderKind = Buy | Sell | Contract open
```

| Change | Breaks | Allowed as |
| --- | --- | --- |
| Strengthen a precondition, or add a failure | Callers | A new version, or a failure set declared `open` |
| Weaken a postcondition, a frame or a lifetime guarantee | Dependants | A new version |
| Strengthen an invariant | Stored rows | Watch, backfill, then enforce: expand, then contract |
| Narrow a parameter's type | Callers | A new version |
| Widen a result's type, or add a variant to a closed variant | Dependants | A new version, unless the variant is `open` |
| Return a borrow that lives shorter, or drop `readonly` from a parameter | Dependants and callers | A new version |
| Weaken a precondition, strengthen a postcondition or a frame | Nobody | Freely |

An implementation of an `interface` obeys behavioural subtyping: its preconditions are no stronger than the interface's,
and its postconditions, frames, lifetimes and access no weaker. `edb review` classifies every constraint change in a
pull request by this table.

---

## 7. Lifetime

Lifetime constraints say how long entities live, what keeps them alive, what may refer to them, and what must happen
when they end. They are written once, over a core that belongs to no host, and each host's memory model gives the core
its precise meaning.

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
| `owns` | Yes, alone: the holder is responsible for ending it | No; it ends with or before the holder, unless moved | Nothing | A Rust value or `Box`; a `unique_ptr`; a sole strong reference; a foreign key `on end cascade` |
| `shares` | Yes, jointly: it ends when every sharer lets go | No | Nothing | `Rc`, `Arc`, `shared_ptr`, a Swift strong reference, a CPython reference, any reference under tracing; a foreign key `on end restrict` |
| `borrows` | No | Never: the reference ends before the target, and within its scope | Nothing | Rust `&` and `&mut`; a non-escaping closure or parameter; Swift `unowned` |
| `observes` | No | Yes | A check that the target is still alive | Rust `Weak`, Java `WeakReference`, Swift `weak`, `weak_ptr`; a foreign key `on end nullify`; a reference across shards |
| `identifies` | No | Yes | Lookup: it carries identity, not access | A row id, an entity handle, Pony's `tag`; a foreign key `on end dangle` |

```text
type swift 'Scene.Node'
    hold children : owns
    hold parent   : observes
    hold delegate : observes

function rust 'eve_cache::Cache::get'(self: ref<Cache>, key: ref<Key>) -> ref<Entry>?
    hold result : borrows from self
```

### 7.3 Lifetime predicates

Each predicate is defined over the core; § 7.5 gives its meaning under each model.

- **`lifetime a outlives b`:** `end(a)` is never before `end(b)`.
- **`lifetime e within s`:** `e` begins and ends inside scope `s`.
- **`lifetime e ends with h`:** `e` ends when `h` does, and not before: the usual consequence of `owns`.
- **`lifetime e does not escape [s]`:** after `s` ends (by default, the call), no reference to `e` that `shares`, `owns`
  or `borrows` it is reachable from anywhere outside `s`. `escapes only to p` permits the named paths.
- **`lifetime e ends within d after x`:** `e` has ended within duration `d` after event or entity `x` ends. Under
  tracing this is the leak constraint: after a window closes, nothing keeps its listeners reachable for more than two
  frames.
- **`lifetime e at most d [after x]`:** `e` lives no longer than `d`, which is how data retention is written for rows.
- **`lifetime e released by f exactly once`:** release happens through `f`, once, on every path. `released before end`
  forbids leaving release to finalisation. `ends by f exactly once` is the linear form: the entity is consumed exactly
  once.
- **`lifetime e no use after end`:** no reference is used to reach `e` after `end(e)`; for resources, `no use after
  release`.
- **`lifetime e reachable only from p, q`:** every strong path to `e` from a root passes through one of the named
  holders. It forbids leaks into global caches, event buses and static collections.
- **`no strong cycle [through f, g]`:** no cycle of `owns` and `shares` references through the named fields, which under
  counting is a leak.
- **`instances [per s] at most n`** and **`instances attributed to s are none within d after end(s)`:** bounds on how
  many entities of a type are alive, in total or per scope. The second is the practical leak check for a session, a
  fight or a scene.
- **`lifetime e stable address`:** `e` does not move while referred to by the named paths, as Rust's `Pin` or a pinned
  handle in a moving collector; needed at boundaries with foreign code.
- **`lifetime every ref<T> [obtained from p] within s`:** the same predicate over every reference of a type, or every
  reference obtained through a path; this is how a rule for a whole interface is written.

### 7.4 Memory models

A host profile declares its model, and a type may override it where the host mixes models (`Rc` inside Rust,
`shared_ptr` inside C++, an arena inside anything). The model is stated with `model` in a subject when it is not the
profile's default.

- **Ownership (Rust; C++ with `unique_ptr` and values).** Every entity has exactly one owner at a time; ownership moves;
  the end is when the owner drops it; borrows are scoped and checked by the compiler. Most lifetime constraints on safe
  Rust are proven by the compiler from signatures, and the file states them so that they survive translation. What the
  compiler does not prove, the file adds: that an `Rc` graph has no strong cycle, that a handle is dropped before the
  tick ends, that no clone of an `Arc` escapes its task.
- **Tracing (Java, C#, Kotlin, Go, JavaScript and TypeScript).** An entity lives while it is reachable from a root by
  strong references; its logical end is when it becomes unreachable, which the collector notices later and at no defined
  moment; finalisation is not guaranteed and may resurrect. So under tracing, end constraints are reachability
  constraints, release is always explicit, and `owns` means the holder dominates the entity in the heap graph: every
  strong path to it passes through the holder.
- **Counting (Swift and Objective-C under ARC, CPython, PHP, `shared_ptr`, `Rc` and `Arc`).** An entity lives while its
  strong count is above zero and ends, deterministically, when the count reaches zero. Weak references do not count and
  are cleared or fail on use; `unowned` references do not count and must not outlive. A strong cycle never reaches zero,
  so it leaks, unless a cycle collector (CPython, PHP) runs, in which case it ends later and at no defined moment, as
  under tracing.
- **Tracked references (generational handles, slot maps, entity systems; Vale's generational references).** The runtime
  knows every reference to an entity, or can tell for each use whether its target is still the one it was taken for. An
  entity is ended explicitly; ending it either fails while tracked references remain (`on end refuse`) or invalidates
  them (`on end invalidate`), and a use after end is detected deterministically at the use. Game engines use this model
  for entities, and the rows of a database are a form of it.
- **Regions (arenas, frame allocators, pools; Cyclone's regions).** Entities in a region end together when the region is
  reset; a reference into a region from outside it must be `observes` or `identifies`, or end before the reset.
- **Manual (C, and C++ outside RAII).** An entity ends when it is freed; nothing in the host checks anything.
  Constraints are checked by sanitizers and leak checkers in CI and by wrappers at boundaries.
- **Rows.** A row begins at insert and ends at delete; references are foreign keys, and their rule on end (`cascade`,
  `restrict`, `nullify`, `dangle`) is their hold. A reference across shards can only observe or identify, since no shard
  can keep another's row alive. The engine checks lifetime constraints on rows at each write, as it checks invariants.

### 7.5 Meaning under each model

| Predicate | Ownership | Tracing | Counting | Tracked | Rows |
| --- | --- | --- | --- | --- | --- |
| `end(e)` | Owner drops it | Last strong path from a root removed | Strong count reaches zero | Explicit end | Delete |
| `owns` | The owner | The holder dominates `e` | The only strong reference | The owning handle | `on end cascade` |
| `shares` | `Rc`, `Arc` | Any strong reference | A strong reference | A counted handle | `on end restrict` |
| `borrows` | `&`, `&mut`, checked | A reference not retained after its scope | `unowned`, or a reference not retained | A handle used only within its scope | Read within the transaction |
| `observes` | `Weak` | `WeakReference` | `weak` | A handle checked on use | `on end nullify`, or across shards |
| `does not escape` | Proven from signatures | Escape analysis, or reachability at scope end | Count back to its value at scope begin | No tracked reference outside the scope | No row outside the transaction refers to it |
| `ends within d after x` | Proven where `x` owns `e` | Unreachable within `d`, by weak probe or snapshot | Count zero within `d` | Ended within `d` | Deleted within `d` |
| `released by f exactly once` | `Drop` runs once; `f` is `drop` or a consuming method | `f` called once on every path; never left to the finaliser | `deinit` runs once, or `f` called once | `f` called once | — |
| `no use after end` | Proven in safe code | Use after release, for resources | Proven for `weak`; checked for `unowned` | Detected at use | A lookup of a deleted row fails |
| `reachable only from` | Proven for owned data | Dominator query on snapshots | Referrer query on snapshots | Referrer set | Every referring foreign key is listed |
| `no strong cycle` | Graph walk over `Rc` in CI | Not a leak; still checkable | Graph walk; a cycle is a leak | Graph walk | `acyclic` |

How each is checked: under ownership, by the host compiler, then Miri and leak checks in CI for the rest; under tracing,
by escape analysis and analysers at build, weak-reference probes and heap snapshots on replays and test clusters, and
dominator queries in CI; under counting, by reference probes (`sys.getrefcount`, `use_count`, weak observers), graph
walks on test clusters and the memory graph; under tracked references and rows, deterministically at run time; under
regions, by the region's reset and a check of references into it; under manual management, by sanitizers in CI.

### 7.6 Resources

A `resource` is a type that holds something outside memory. Its constraints combine a protocol (§ 6.5) with lifetime:
how it is released, within what scope, and by whom.

```text
resource java 'java.sql.Connection'
    protocol open (statement | commit | rollback)* close
    lifetime self released by close exactly once
    lifetime self released before end            -- never left to the finaliser
    lifetime self within request                 -- never kept across requests
    confined to thread

resource csharp 'Studio.Audio.Voice'
    lifetime self released by Dispose exactly once
    lifetime self ends within 1 s after end(owner)
```

Under ownership and counting, release is the end and is usually proven; under tracing, `released before end` is the
constraint that forbids relying on finalisation, and it is checked by analysers that follow every path from acquisition
to its `using`, `try`-with-resources or `with`.

### 7.7 Harvesting lifetimes

The proposal's manifests already record the lifetimes and owners of the Python objects in the hot stores (§ 6.13 of the
proposal): which execution created them, which held them, when they were last used and when they ended. That evidence
becomes lifetime constraints on the Python types, and those constraints decide where each piece of state goes: a value
with no row; a table that need not be durable, where it can be rebuilt; a durable row in the right shard, with its holds
becoming foreign keys and their rules on end; or nothing, deleted with its cache. A Python object that `owns` another
becomes a row that cascades to it; one that `observes` becomes a reference that nullifies or an identity that dangles.
Elsewhere, lifetimes are harvested from heap snapshots on test clusters replaying recorded traffic: dominators give
`owns`, referrer sets give `reachable only from`, and populations by scope give `instances`.

---

## 8. Mutability and aliasing

Mutability and aliasing are one subject: whether something may change depends both on what this path may do and on what
every other path may do. The language states both, for any host, with one model.

### 8.1 Access: rights and guarantees

Every path to an entity has, for some interval, two properties:

- **Rights through this path:** `identify` (identity only), `read`, or `write` (which includes read).
- **The guarantee about every other path:** `exclusive` (no other path may read or write), `readers` (others may read;
  no path may write, this one included unless it has `write`), or `any` (no guarantee).

Six combinations matter, and they cover the hosts in use; Pony's reference capabilities are the closest existing system,
and their names are given for reference.

| Access | Means | Pony | Rust | C++ | Java, C# | Swift | Python |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `write exclusive` | Only this path reads or writes | `iso` | `&mut T`; an owned value | — | — | `inout` | — |
| `write readers` | This path writes; others may only read through views | `trn` | — | — | — | — | — |
| `write any` | This path writes; others may too | `ref` | `&RefCell<T>`, `&Cell<T>` | `T&` | Any reference | A class reference | Any reference |
| `read readers` | Nobody writes while this holds: the entity is immutable for the interval | `val` | `&T` without interior mutability | — | A reference to an immutable object | A value type | A reference to a frozen object |
| `read any` | This path cannot write; others may | `box` | — | `const T&` | A read-only view or interface | — | A read-only proxy |
| `identify any` | Identity only | `tag` | An id or handle | An opaque handle | An id | An id | An id |

The distinction between `read readers` and `read any` is the one most languages blur: `const` in C++ and a read-only
interface in Java promise only that this path does not write, not that the entity does not change.

### 8.2 Mutability predicates

- **`access p : rights guarantee`** is the general form, for a parameter, result, field or path: `access cache : read
  readers`.
- **`readonly p`** is `access p : read any`: the subject does not write through `p`, deeply by default. On a parameter,
  it says the caller's value is unchanged by the call.
- **`immutable e [deep | shallow] [after x]`:** no path writes `e` for its whole lifetime, or after event `x` (`frozen
  after construct`, `frozen after publish`). `deep` follows `owns` and `shares` references, not `observes` or
  `identifies`.
- **`write once f`:** `f` is assigned exactly once, at or after construction and before any read: Kotlin's `lateinit
  val`, C#'s `init`, a column set at insert and never updated.
- **`append only t`:** a table or collection gains entries and never changes or loses one: logs, ledgers, event streams.
- **`monotonic f increasing | non-decreasing | decreasing | non-increasing`:** versions, sequences, skill points, closed
  ticks.
- **`mutates only p, q`:** the in-memory counterpart of `writes only`: the subject writes nothing reachable from its
  inputs except through the named paths.
- **`interior p`** names hidden mutable state inside an otherwise immutable entity (a cache, a memo, a lazily computed
  field), and **`unchanged view(e)`** states what must stay the same despite it: the entity is immutable as seen through
  the named pure projection. This is how Rust's `Cell` and `OnceCell` and C++'s `mutable` are stated honestly.

### 8.3 Aliasing predicates

- **`exclusive p`** is `access p : write exclusive`: nothing else can see the entity while this path holds it. C's
  `restrict`, Rust's `&mut`, Pony's `iso`, Clean's unique types.
- **`unique e`:** at most one strong reference to `e` exists: the holder may hand it on, but never copy it.
- **`disjoint a, b`:** no entity is reachable, by `owns` or `shares`, from both; two parameters may be written in
  parallel without one seeing the other's writes.
- **`shared readonly e`:** many paths, none writing; the precondition of `shareable` (§ 6.8).

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

Deep is the default, because that is what callers mean: a `readonly` list of ships means neither the list nor the ships
change. Depth follows `owns` and `shares` and stops at `observes` and `identifies`, which is why holding is stated first
(§ 7.2): an observer's target is someone else's to change. `shallow` limits a predicate to the entity's own fields,
which is what Java's `final` and C#'s `readonly` fields give by themselves.

### 8.5 Per host

| Host | Proven by the host | Enforced at run time | Watched |
| --- | --- | --- | --- |
| Rust | Access through `&` and `&mut`, deep in safe code apart from interior mutability; `Send` and `Sync` | — | `interior` against `unchanged`, by comparing projections on replays |
| T-SQL and the engine | — | `write once`, `append only`, `monotonic` and `immutable` columns, at each write | Replicas, while a table is on SQL Server |
| Python | — | Read-only proxies and frozen objects at boundaries | Deep hashes of parameters before and after calls, on replays |
| C#, Java, Kotlin | `readonly`, `final`, `val` and `init`, shallowly; records and immutable collections | Wrappers that hand out read-only views | Deep comparisons on replays; analysers for escaping `this` before construction ends |
| Swift | Value types; `let`; `Sendable` | — | Memory graph comparisons |
| C++ | `const` as `read any` | — | Deep comparisons in CI |
| JavaScript, TypeScript | `readonly` at compile time only | `Object.freeze`, deeply | Deep comparisons on replays |

### 8.6 Harvesting mutability and aliasing

The manifests and replays show, per call, which parameters were changed, which were never changed, and which overlapped.
A parameter never changed across the recordings becomes a candidate `readonly`; a field assigned once in every recorded
lifetime becomes `write once`; a value seen only to grow becomes `monotonic`; two parameters never reaching a common
entity become `disjoint`. Each is watched first, like every harvested constraint, and checked against the code for a
path that breaks it before it is enforced.

---

## 9. Composition

- **Named predicates.** `predicate flies(s: space.Ships) = s.pilotID == actor` is defined once and used anywhere. A
  check repeated across hundreds of methods becomes one.
- **Blocks of clauses.** `block in_space(s: space.Ships)` names a sequence of `require` clauses with their failures;
  `use in_space(m.ship)` expands it in place, in order. This is how "the actor flies this ship, is in space, is not in
  warp" is written once and kept in its original order everywhere.
- **Refinement types.** `type Quantity = i64 where value >= 0` and `type ShipName = text where …` are named and reused
  in every signature.
- **Generics.** `for T where T: Comparable` binds a type parameter; `type list<T> where sorted by key` can be stated
  once for every `T`.
- **Interfaces.** Clauses on an `interface` bind every implementation, under the rules of behavioural subtyping
  (§ 6.10). "Every implementation of `Repository.save` is idempotent by its id" is one clause.
- **Modules and `use`.** Names are qualified by module; `use` imports predicates, blocks, failures, quantities, labels
  and scopes.

```text
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
| `lifted` | Moved out of code that already enforced it; enforced from its first day |
| `watched` | Checked live; a violation is logged and ranked, and nothing is rejected |
| `enforced` | A violation rejects the request, fails the transaction or fails the build |
| `proven` | A compiler or analysis shows it cannot fail, so its check is left out; the reference executor still checks it on the corpora |
| `assumed` | Taken on trust at a boundary, with a stated reason |

Status lives in the catalog, keyed by the constraint's stable name (its subject and clause name), with its evidence:
what supports it, what has violated it, which clusters of violations were explained and by whom. `edb constraints` shows
it. A file may fix a status with `pin name enforced because "…"`, which only a person may write.

### 10.2 Assumptions

`assume name : p because "…"` states something the subject relies on and does not check, at a boundary where another
place checks it or where it cannot be checked at all: what the proxy enforced, what a foreign library promises, what an
`unowned` reference relies on. An assumption is shown in review like any exception, and every assumption is listed with
what would break if it were false.

### 10.3 Where a constraint runs

Whether a constraint is enforced, watched or assumed is a statement about the program. Where it runs is a choice of
mechanism, made by the compiler from each clause's cost, read set and labels, among the places the host profiles allow:
the client (for the interface and prediction), the proxy, the Python servers, the head of a T-SQL procedure, the engine,
a generated wrapper in a host, the host's own compiler, replicas and the temporal tier, test clusters, and CI. A clause
may exclude a place (`not at client`) but never require one: the same constraint moves from the Python server to the
engine when its subject does, without being rewritten.

---

## 11. Compilation

The constraint compiler parses a file, binds its subjects through the host profiles, checks types, dimensions, labels
and well-formedness (every partial operation guarded, every narrowing justified by order), checks that each kind used is
enabled, computes each clause's read set and cost, and then generates for each place:

- the RPC schema and its encoders and decoders, from argument and result types;
- checks at the proxy, in the Python servers, at the head of T-SQL procedures and in the engine, from preconditions,
  frames and access;
- the engine's write-time checks, from invariants, references, state machines, mutability of columns and conservation;
- for Rust, static assertions, typestate scaffolding where requested, and debug assertions for what the compiler cannot
  prove;
- for C#, Java, Kotlin, Swift, PHP, TypeScript and Go, analysers at build and wrappers at boundaries;
- queries for replicas, the temporal tier and test clusters, from temporal, statistical, lifetime and mutability clauses
  that are watched;
- property tests in CI, from laws and postconditions;
- the catalog entries for status and evidence.

**The same meaning everywhere** is tested, not trusted. Every generated check is run against the reference evaluator on
the recorded corpora and on generated edge cases (absent values, extremes of every integer type, `NaN` and signed zeros,
empty and maximal texts, combining characters, empty collections), and any disagreement fails the build.

**Proof.** A constraint is `proven` when a host compiler proves it (a borrow in Rust, a non-escaping closure in Swift),
when a static analysis does, or when the compiler discharges it from earlier clauses and declared types (a range already
implied by a parameter's type). The reason is recorded.

**Diagnostics** name the constraint, its subject and version, its status, the place it ran, and the inputs that broke
it, redacted by their labels.

---

## 12. How the language grows

The grammar is complete from the start; kinds are enabled in an order set by their consumers. Each arrives with its
paragraph of meaning, its miner and its compilation into every place that enforces it.

| Order | Kind | First consumer | Miner | First places |
| --- | --- | --- | --- | --- |
| 1 | Argument types and shapes | The RPC schema at the proxy | Manifests at entry | Proxy; schema generation |
| 2 | Ranges, sets of values, absence | The proxy; the Python servers | Manifests at entry | Proxy; Python |
| 3 | Results and failures | Lifting checks out of Python and T-SQL | Manifests at exit; raises and `THROW`s | Python; T-SQL; engine |
| 4 | Preconditions on the session | The proxy | Manifests | Proxy |
| 5 | Preconditions on the world | The engine; proxies with replicas | Lifted checks | Engine; replicas |
| 6 | Invariants on rows, keys and references | Tables moving into the engine | Historical backups | Engine; replicas |
| 7 | Conservation | The economy | Manifests and the log | Engine |
| 8 | Order in time and state machines | Replicas and the temporal tier | The log, in simulation time | Replicas |
| 9 | Budgets | CI and the scheduler | Timings and manifests | CI; production metrics |
| 10 | Frames | Review of translated procedures | Manifests and static bounds | Engine; CI |
| 11 | Lifetime and mutability | Moving Python state into rows; the client; other studios' hosts | Manifests at the hot stores; heap snapshots; replays | Rows in the engine; CI; test clusters |
| 12 | Aliasing and concurrency | The Python servers while they remain; clients | Replays; lock traces | CI; analysers |
| 13 | Information flow | Ledgers, chains, visibility | Visibility filters and existing rules | Proxies; stream boundaries; CI |
| 14 | Compatibility | Releases | The catalog's history | `edb review` |
| 15 | Laws and statistics | CI; economy and loot | — | CI; replicas |

The list of kinds is reviewed as a language, yearly. A kind is never added for control flow, which belongs to the host.

---

## 13. Coverage: constraints elsewhere, and how they are written here

The language must be able to state any reasonable constraint that a programming language, a contract system, a database
or a verification tool can state. The table maps the mechanisms in use onto the clauses here.

| Mechanism | Found in | Written here as |
| --- | --- | --- |
| Integer widths, signedness | C, C++, Rust, Java, C#, SQL | `u8` … `i128` as refinements of `int`; `fits T` |
| Range and subrange types | Ada, Pascal | `T in a..b`; `T where …` |
| Enumerations, sum types, sealed hierarchies | Rust, Swift, Kotlin, Java, C# | Variants; `in {…}`; `open` for non-exhaustive |
| Nullability | Kotlin, C#, TypeScript, Swift, SQL `NOT NULL`, Java `@NonNull` | `T?`; `exists`; non-optional by default |
| Refinement and branded types | Liquid Haskell, F*, TypeScript brands | `T where p`; named refinements |
| Units of measure | F#, units libraries | `unit`; `quantity … in` |
| Message and document schemas | protobuf, JSON Schema, OpenAPI, XSD | Parameter and result types on `method` |
| String length and format | SQL `NVARCHAR(n)`, validators | `text where length(value) <= n and value matches /…/` |
| Float finiteness | Game engines, numeric code | `finite` |
| Overflow behaviour | Rust checked arithmetic, C# `checked` | Exact arithmetic; `fits`; `ensure` on exact results |
| Preconditions, postconditions, `old` | Eiffel, JML, SPARK, Dafny, C++ contracts | `require … else`, `ensure`, `old(…)` |
| Class and type invariants | Eiffel, SPARK `Type_Invariant`, Dafny | `invariant … holds between calls` |
| Checked exceptions, typed errors, `noexcept` | Java, Swift `throws`, Rust `Result`, C++ | `failure`; `fails only`; `never fails runtime`; `never panics` |
| `[[nodiscard]]`, `#[must_use]` | C++, Rust | `result must be used` |
| Algebraic laws, property tests | Haskell type-class laws, QuickCheck | `law` |
| Access control on calls | `@RolesAllowed`, capability checks | `from` |
| Frames | Dafny `reads` and `modifies`, SPARK `Global` | `reads only`, `writes only`, `mutates only` |
| Data dependencies | SPARK `Depends` | `depends only on` |
| Purity and effects | Haskell `IO`, Koka effects, D `pure`, C++ `constexpr` | `effects none`, `effects only`, `no io`, `no clock` |
| Determinism | SQL deterministic functions; replay systems | `deterministic` |
| Idempotence | HTTP `PUT`, message handlers | `idempotent by` |
| Termination and variants | Dafny `decreases`, total languages | `terminates`; budgets |
| Complexity and performance budgets | Big-O annotations, frame budgets | `budget work O(…)`, `budget p99 …`, `budget allocates …` |
| Keys, uniqueness, foreign keys, `CHECK` | SQL | `key`, `unique`, `reference … on end`, `invariant` |
| Containment without cycles | Databases, scene graphs | `acyclic … via` |
| Conservation | Ledgers, double-entry bookkeeping | `conserve`, `faucet`, `sink` |
| State machines | Triggers, workflow engines | `states` |
| Typestate, session types | Plaid, Rust's typestate pattern, session-typed languages | `protocol` |
| Temporal logic | TLA+, LTL, runtime verification | `always`, `never`, `after … within`, `since`, `until` |
| Rate limits | API gateways | `rate` |
| Statistical properties | Monitoring, fairness audits | `statistic` (watched only) |
| Ownership and moves | Rust, C++ `unique_ptr` | `hold … : owns`; ownership model |
| Borrows and lifetime parameters | Rust `&`, `&mut`, `'a: 'b`; returned borrows | `hold … : borrows from`; `outlives`; `access` |
| Shared ownership | `Rc`, `Arc`, `shared_ptr`, Swift and CPython strong references | `hold … : shares`; counting model |
| Weak references | Rust `Weak`, Java `WeakReference`, Swift `weak`, `weak_ptr` | `hold … : observes` |
| `unowned` references | Swift | `hold … : borrows`, or `assume … because` for `unowned(unsafe)` |
| Ids, handles, `tag` | Entity systems, rows, Pony | `hold … : identifies` |
| Generational references, slot maps | Vale, game engines | Tracked model; `no use after end`; `on end refuse | invalidate` |
| Non-escaping parameters and closures | Swift, Kotlin `inline`, escape analysis | `lifetime … does not escape` |
| Deterministic release | RAII, `Drop`, `deinit`, destructors | Ownership and counting models: release is the end |
| Explicit release | `using`, `IDisposable`, try-with-resources, Python `with`, Go `defer` | `released by … exactly once`; `released before end` |
| Finalisers | Java, C# | Forbidden as a means of release by `released before end` |
| Linear and affine types | Linear Haskell, Rust moves | `ends by … exactly once`; `unique` |
| Regions, arenas, frame allocators | Cyclone, arenas in games | Region model; `within frame` |
| Pinning | Rust `Pin`, C# `fixed` and pinned handles | `stable address` |
| Leak detection | Heap snapshots, LeakCanary, leak sanitizers | `ends within … after`; `instances … are none within`; `reachable only from` |
| Reference cycles | Swift, CPython, PHP | `no strong cycle` |
| Read-only views | C++ `const`, TypeScript `readonly`, C# `in`, Java read-only interfaces | `readonly`; `access … : read any` |
| Immutability | Rust by default, Java immutable classes, Kotlin `val`, C# records, `Object.freeze`, frozen dataclasses | `immutable deep | shallow`; `frozen after` |
| Assign-once fields | C# `init`, Kotlin `lateinit`, columns set at insert | `write once` |
| Append-only storage | Logs, ledgers, event sourcing | `append only` |
| Monotonic values | Versions, sequences, clocks | `monotonic` |
| Interior mutability, logical const | Rust `Cell`, `RefCell`, `OnceCell`; C++ `mutable` | `interior` with `unchanged view(…)` |
| Exclusive and unique references | C `restrict`, Rust `&mut`, Pony `iso`, Clean unique types | `exclusive`; `unique` |
| Non-overlapping arguments | Fortran, `restrict`, separation logic | `disjoint` |
| Reference capabilities | Pony `iso`, `trn`, `ref`, `val`, `box`, `tag` | `access` with rights and guarantees |
| Thread safety markers | Rust `Send` and `Sync`, Swift `Sendable` | `sendable`, `shareable` |
| Lock discipline | Java `@GuardedBy`, Clang thread-safety analysis | `guarded by`, `lock order`, `no suspension while holding` |
| Confinement and actor isolation | UI threads, Swift actors, Erlang processes | `confined to` |
| Atomicity and linearisability | Transactions, concurrent structures | `atomic`, `linearizable` |
| Information flow and taint | Jif, FlowCaml, taint modes, secrecy wrappers | `label`, `flows only to`, `never flows … to`, `declassify`, `untrusted`, `sanitised by` |
| Data retention | Privacy law, retention policies | `lifetime rows with … at most … after …` |
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
    require found    : exists m                                              else Gone
    require flies_it : m.ship.pilotID == actor                               else NotYours
    require online   : m.isOnline                                            else Offline
    require idle     : m.cycle == Idle                                       else AlreadyActive
    require in_range : movement.distance(m.ship, targetID, now) <= m.range  else OutOfRange
    require can_pay  : m.ship.capacitor >= m.activationCost                  else NoCapacitor
    ensure  cycling  : space.Modules[moduleID].cycle == Active
    ensure  ends     : result.firstCycleEnds == now + m.cycleTime
    fails only Gone, NotYours, Offline, AlreadyActive, OutOfRange, NoCapacitor
    reads only space.Modules, space.Ships, movement.*
    emits only CapacitorConsumed, ModuleActivated
    sends only space.CompleteCycle
    deterministic
    budget p99 <= 50us on SolarSystem.hot
```

### 14.2 A Python manager's state, on its way into rows

```text
-- inventory/python.constraints: harvested from the manifests at the hot stores, watched
type python 'eve.inventory.LocationCache'
    hold entries : owns
    hold manager : observes
    lifetime self within session
    instances attributed to session are none within 5 s after end(session)
    reachable only from 'eve.inventory.InventoryMgr.caches'

function python 'eve.inventory.InventoryMgr.MoveItems'(self, itemIDs: list<u64>, toLocationID: u64)
    from Player
    readonly itemIDs
    hold itemIDs : borrows
    writes only inv.Items, inv.Locations
    no suspension                         -- no recorded call yielded between its first read and its last write
```

These say that the cache is per session, owned by its manager's table of caches and gone with the session: so it is a
table that need not be durable, keyed by session, or nothing at all once the queries it served are fast enough. `no
suspension` says the move can be one transaction without changing what any player saw.

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

The last two clauses on the window are the leak most common in garbage-collected interfaces: a closed window kept
reachable by an event subscription. They are checked by a weak-reference probe taken at `Close` on replays of recorded
sessions.

### 14.5 A scene graph, under counting

```text
type swift 'Scene.Node'
    hold children : owns
    hold parent   : observes
    hold delegate : observes
    no strong cycle through children, parent, delegate
    lifetime self ends with parent
    confined to actor SceneActor
```

### 14.6 Rows with lifetimes

```text
table inv.Items
    reference locationID -> inv.Locations on end restrict    -- shares: a location cannot end while it holds items
    reference ownerID    -> chr.Characters on end dangle     -- identifies: history survives the character
    acyclic containment : via locationID
    placement follows outermost locationID

table inv.Containers
    reference itemID -> inv.Items on end cascade             -- owns: the container ends with its item

table chr.Notifications
    lifetime rows at most 90 days after createdAt
```

### 14.7 A resource, with a protocol

```text
resource java 'java.sql.Connection'
    protocol open (statement | commit | rollback)* close
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

---

## 15. Grammar

The grammar is given in EBNF. Indentation opens and closes blocks (`INDENT`, `DEDENT`); `NAME` is an identifier, `QNAME`
a dotted name, `HOSTPATH` a quoted path in a host language.

```text
file          = [ "module" QNAME NL ] { "use" QNAME NL } { declaration } ;
declaration   = subject | failure | predicate | block | label | scope | quantity | unit | typedef | variantdef ;

subject       = subject_head NL INDENT { clause NL } DEDENT ;
subject_head  = ( "procedure" | "method" ) QNAME params [ "->" type ]
              | ( "function" | "type" | "resource" | "interface" ) HOST HOSTPATH [ params ] [ "->" type ]
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

clause        = "let" NAME "=" expr
              | "from" NAME { "," NAME }
              | "require" [ NAME ":" ] expr [ "else" NAME ]
              | "ensure" [ NAME ":" ] expr
              | "fails" NAME "when" expr | "fails only" NAME { "," NAME } | "never fails runtime" | "never panics"
              | "result must be used"
              | "law" NAME ":" expr
              | frame | state | protocol | temporal | budget | concurrency | flow | compat | memory | access
              | "use" NAME "(" [ expr { "," expr } ] ")"
              | "assume" NAME ":" expr "because" STRING
              | "pin" NAME status "because" STRING
              | "not at" place { "," place }
              | "model" MODEL ;

frame         = ( "reads only" | "writes only" | "emits only" | "sends only" | "calls only" | "effects only" )
                  path { "," path }
              | "effects none" | "no io" | "no clock" | "no randomness" [ "except rolls" ] | "no suspension"
              | "no allocation" | "deterministic" | "idempotent by" path | "commutes with" QNAME
              | "depends only on" path { "," path } | "terminates" ;
state         = "invariant" NAME ":" expr [ "holds" ( "between calls" | "always" ) ]
              | "key" path_list | "unique" NAME ":" path_list [ "where" expr ]
              | "reference" path "->" QNAME "on end" ( "cascade" | "restrict" | "nullify" | "dangle" )
              | "acyclic" NAME ":" "via" path | "placement follows" [ "outermost" ] path
              | "conserve" NAME "across" path { "," path } | ( "faucet" | "sink" ) NAME [ "at most" expr ]
              | "states" path ":" transition { "," transition } | "initial" path ":" NAME ;
transition    = NAME "->" NAME { "|" NAME } ;
protocol      = "protocol" regex_over_calls ;
temporal      = ( "always" | "never" ) NAME ":" expr
              | "after" event ":" expr "within" duration
              | ( "before" | "since" | "until" ) event ":" expr [ "within" duration ]
              | "rate" [ "per" scope_ref ] "<=" NUMBER "per" duration
              | "statistic" NAME ":" expr "within" range "of" expr "over" NUMBER "samples" ;
budget        = "budget" ( percentile "<=" duration [ "on" QNAME ] | "work" complexity
              | "round_trips" "<=" NUMBER | "bytes sent" "<=" expr | "allocates at most" size [ "per" scope_ref ]
              | "retains at most" size [ "per" ( "instance" | scope_ref ) ] ) ;
concurrency   = "confined to" scope_ref | "sendable" | "shareable" | "guarded by" path ":" path_list
              | "lock order" path "before" path | "atomic" | "linearizable"
              | "no suspension while holding" path | "no blocking in" scope_ref ;
flow          = "label" path_list ":" NAME [ "(" path ")" ] [ "when" expr ]
              | "flows" NAME [ "(" NAME ")" ] "only to" sink { "," sink }
              | "never flows" NAME "to" sink { "," sink } [ "unless encrypted" ]
              | "declassify" path "to" sink { "," sink } "when" expr "because" STRING
              | "sanitised by" QNAME "before" sink { "," sink } ;
compat        = "version" NUMBER | "compatible with" NUMBER { "," NUMBER } | "replaces" HOST HOSTPATH
              | "was" HOSTPATH | "deprecated" [ "since" NUMBER ] | "stable" ;
memory        = "hold" path ":" ( "owns" | "shares" | "borrows" [ "from" path ] | "observes" | "identifies" )
              | "lifetime" target lifetime_pred
              | "instances" [ "per" scope_ref ] "at most" NUMBER
              | "instances attributed to" scope_ref "are none within" duration "after" event
              | "no strong cycle" [ "through" path_list ] ;
target        = path | "every ref<" type ">" [ "obtained from" path ] | "rows" [ "with" NAME ] ;
lifetime_pred = "outlives" path | "within" scope_ref | "ends with" path | "does not escape" [ scope_ref ]
              | "escapes only to" path_list | "ends within" duration "after" event
              | "at most" duration [ "after" event ]
              | "released by" NAME "exactly once" | "released before end" | "ends by" NAME "exactly once"
              | "no use after" ( "end" | "release" ) | "reachable only from" path_list | "stable address"
              | "on end" ( "refuse" | "invalidate" ) ;
access        = "access" path ":" ( "identify" | "read" | "write" ) ( "exclusive" | "readers" | "any" )
              | "readonly" path [ "shallow" ] | "immutable" path [ "deep" | "shallow" ] [ "after" event ]
              | "frozen after" event | "write once" path | "append only"
              | "monotonic" path ( "increasing" | "non-decreasing" | "decreasing" | "non-increasing" )
              | "mutates only" path_list | "interior" path | "unchanged" "view(" path ")" [ "=" expr ]
              | "exclusive" path | "unique" path | "disjoint" path "," path { "," path } | "shared readonly" path ;

path          = NAME { "." NAME | "[" expr "]" } [ ".*" ] | HOSTPATH ;
path_list     = path { "," path } ;
event         = ( "begin" | "end" | "released" ) "(" path ")" | "call" QNAME | "return" QNAME
              | ( "insert" | "update" | "delete" ) QNAME | "emit" NAME | path ;      (* a path names a time *)
scope_ref     = "call" | "transaction" | "tick" | "request" | "session" | "frame" | "task" | "thread" | "process"
              | "actor" NAME | "shard" | NAME ;
sink          = "client" | "player" [ "(" path ")" ] | "server" | "ledger" | "chain" | "log" | "analytics"
              | "external" | "shard" [ "(" path ")" ] ;
place         = "client" | "proxy" | "python" | "tsql" | "engine" | "host" | "replica" | "ci" ;
status        = "observed" | "lifted" | "watched" | "enforced" | "proven" | "assumed" ;
MODEL         = "ownership" | "tracing" | "counting" | "tracked" | "region" | "manual" | "rows" ;
(* type, record_type, generics, duration, size, percentile, complexity, range, unit_expr and regex_over_calls
   are written as in the examples: "u64?", "{ a: T }", "for T where T: C", "50us", "2 MB", "p99", "O(size(x))",
   "0.95..1.05", "m / s", "open (send | receive)* close" *)

expr          = (* literals, paths, T[key], T where p, calls of named pure functions, not/and/or/implies/iff,
                   comparisons, arithmetic, if-then-else, exists, is, in, matches, fits, finite, all/any/count/sum/
                   min/max over collections, old(e), result, same(a, b); defined in § 4 *) ;
```

---

## 16. What the language cannot say

- **What a function computes,** in general. Formulas are reference functions compared by output; laws and postconditions
  can pin properties of them, never the whole.
- **Anything about a body's intermediate values.** A rule over one becomes a named pure function, or stays in the code.
- **How anything is done.** No control flow, no assignment, no effects.
- **Unbounded quantification where it is enforced.** A constraint over every row of a table is an invariant checked row
  by row, or watched on a replica.
- **Unbounded liveness, as enforcement.** "Eventually" without a bound may only be watched.
- **The future.** A temporal constraint is decided over history; a deadline can raise an alarm when it passes, not
  prevent it from passing.
- **Style, naming and taste.** Those belong to linters and review.
- **Failure of a request for cost.** Budgets fail builds and raise alarms; they never fail a player's request.

---

## 17. Open questions

1. **Selections at enforcement time.** A selection in an enforced constraint must use an index; should the compiler add
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
   automatically when its state moves into a table? Leaning: proposed automatically, decided by a person, since what is
   lost on deletion is the game's decision.
7. **Deadlines that can act.** Which `after … within` constraints can be enforced by a timer that fires at the deadline,
   rather than only watched? Leaning: those whose remedy is itself a declared procedure, such as returning a ship in
   transit to its origin.
8. **Flows through failures and timing.** Failures are covered; timing is not. Leaning: state timing as out of scope for
   enforcement, and watch the few places where it matters (scans, cloaks) on replicas.
9. **Statistics: when to alarm.** Thresholds for `statistic` clauses trade false alarms for missed drift. Leaning:
   sequential tests with a declared false-alarm rate per clause.
10. **Syntax for agents and people.** Indentation reads well and diffs cleanly, but braces survive more tools. Leaning:
    indentation, with one canonical form from the formatter, and a JSON form of the parsed file for tools.
11. **Laws in the language, or in tests.** A `law` is checked only in CI, like a property test. Leaning: keep it here,
    because it binds every implementation of an interface across hosts, which a test in one host does not.
12. **Profiles for other studios' hosts.** C#, Java, Kotlin and PHP profiles are needed before the language is useful to
    the games in the Stillfront adaptation. Leaning: each profile is owned by the team that adopts the host, and is
    tested against the same edge cases as the first three.
