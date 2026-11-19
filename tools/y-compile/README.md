# y-compile

The Y compiler. It reads an example's Y source (`y/src/*.y`: the module, its tests and `main`) and writes what the
examples' `bare/` folders hold: the bare Rust bodies, the constraint files and `y-facts.json`. `bare-rust weave` makes
Rust of them, which builds and passes the example's lowered tests. Besides the seven examples, the Y test programs in
[`tests/programs`](tests/programs/) prove the forms no example uses, through the same chain.

```text
cargo run --release -- ../../examples/nim --out <dir>            # <dir>/src/*.brs, *.constraints.json, <dir>/y-facts.json
cargo run --release -- ../../examples/nim --rust <dir>           # the typed Rust it strips, for reading
cargo test --release --test examples                             # each example and program compiled, woven, built, tested and traced
cargo test --release --test errors                               # Y that must not compile, with the error it gives
cargo test --release --test library                              # the lowering's runtime operations, checked against the crates
```

A program in `tests/programs/<name>` is laid out as an example is: `Cargo.toml`, and `y/src/<name>.y`, `<name>.test.y`
and `main.y`. Its manifest names the runtime crates as `../y-runtime`, as an example's does, which resolves where its
weave is made, beside the examples. Each program's tests claim every path of its module, as `trace.py` requires.

## How it works

1. **Parse.** Each Y file is parsed by [`../y-parse`](../y-parse/).
2. **Resolve** ([`src/model.rs`](src/model.rs)). The module's facets, structures, types, constant groups, functions and
   rules are collected with the types their documentation gives (`game: The game, as game.`). A constant group whose
   name is used as a type (`player`, `move_error`) is a closed type; any other is a module of constants. Each
   parameter is passed as the lowering rules say: `mutable` by `&mut`, a capability by `&mut` (or `&` for `forever`
   and `system`), a number, a truth value or a closed type by value, a parameter the function keeps whole (bound to a
   `mutable` name) by value, and any other by `&`.
3. **Lower** ([`src/lower.rs`](src/lower.rs), [`src/emit.rs`](src/emit.rs)). Statements and expressions become typed
   Rust, by the tables in the examples' READMEs, with each named decision marked `// y: #name` and each `or fail` a `?`
   at the end of its statement, the forms `tools/paths/trace.py` instruments. The lowering infers the type of each
   expression as it goes, to choose the borrow, the conversion (`YString::new`, `.as_str()`, `List::from`) and the
   runtime operation (`list::at_number`) each use needs. A method the lowering has no rule for is the runtime crate's
   method of that name, as [`src/library.rs`](src/library.rs) reads it from the crate (below). What the source asks
   for that the lowering cannot give is a compile error, `file:line: message`, never Rust that fails to build.
4. **Strip** ([`src/bare.rs`](src/bare.rs)). The typed Rust is stripped by `bare-rust`'s own strip, in memory, which
   writes the bare bodies and the constraint files. The lowering then adds what strip cannot see, as
   `tools/paths/regen.py` adds it to a strip of hand-written Rust: each `injected` parameter's mark, each module's
   level (from `tools/bare-rust/tests/y_levels`, the inference the example checks use), and each function's clauses,
   which are made from the Y source rather than carried over from earlier files.
5. **Evaluate** ([`src/evaluate.rs`](src/evaluate.rs)). Each `compile f(…)` is evaluated by a build step, and the
   package lowered again with the values (below).
6. **Facts** ([`src/facts.rs`](src/facts.rs)). `y-facts.json` is written from the Y source and the bare bodies: the
   facets, the capabilities, the claims, the decisions with the bare line of each branch (found as
   `tools/paths/pathfacts.py` finds them), the paths of the module from the enumerator
   `tools/bare-rust/tests/y_paths`, and what each test covers.

The Rust a weave gives is the typed Rust the lowering wrote, apart from the `mut` that weave derives.

## Clauses

| Y | Clause |
|---|---|
| `where name = p` | `require name : p'`, at the `where` line, status `lifted` by `y_runtime::require` |
| `ensures name = p` | `ensure name : p'`, status `lifted` by `y_runtime::ensure` |
| No `injected` and no `mutable` parameter | `effects pure` |
| A `mutable` parameter and no `injected` one | `no io`, `no clock`, `no randomness`, and `mutates only` the members of the parameter the function changes, itself or through the functions it passes them to |
| `injected` parameters | `effects only` the capabilities other than `forever` and `system`; `no io` unless one of them is a device (the console, a window, a site); `no clock` and `no randomness` unless a clock or a random source is among them |
| In a module whose capabilities may suspend, a function that does not | `no suspension` |
| A `mutable` parameter, with or without `injected` ones | also `mutates only` |

`p'` is the Y condition in the constraint language: `f(x) == result.ok` is `f(x) is Ok(_)`, `x == t.case` is `x is
Case` for a closed type, `x.count()` is `count(x)`, and a constant `group.name` is `NAME`.

## Suspension and regions

A function that waits for a capability that may suspend (`window.next_frame()`, `site.get(…)`), or calls one that
does, is an `async fn`, and each such point is awaited; a test runs a call of one to its end with
`y_window::block_on`. An entry point that opens a window (`system.window(title: …, width: …, height: …)` in an
`inject`) is lowered as Snake's hand lowering did it: `main` calls `y_window::run` with those constants, and the Y
`main` becomes `async fn entry`. `y-facts.json` `suspensions` lists each point.

A region `within` a bound (`region fetch is within game_rules.fetch_time or fail:`) is a block of its own that makes
the deadline, `let deadline = Deadline::after(…);`, and gives it to each wait in it (`site.get(…, &deadline)`), as
Memory's hand lowering did. A failure of such a wait is a `Within<HttpError>`, which a type with a case for each of
`http_error` and `expired` converts by an `impl From` of its own.

A region, `region name is c₁, c₂:`, is marked where it starts, `// y: region name is c₁, c₂`: on the line of its one
statement, or on a line of its own above several, with `// y: end region name` after them when statements follow it
in its block. Its constraints become the clauses of its region subject as `regen.py` makes them.

## Structures in constants

A structure whose values are made in a constant group (Checkers' `level`, in `levels.all`) keeps its `string` fields
as `&'static str`, as the hand lowering did: a constant cannot make a `YString`.

## Warehouse

[`src/ware.rs`](src/ware.rs) lowers what Warehouse adds, by its README's table:

- **Schemas.** A schema's latest version is the structure, and `impl Schema` reads every version: its key, then its
  other fields separated by `;`, each parsed by its type, and a field an earlier version lacks from that version's
  `migrate` (`vat_rate := tax.standard_rate`). Text is stored with its line endings escaped.
- **Decimals and bounded integers** are `Decimal<D, P, S>` and `Integer<D, MIN, MAX>`, the digits `D` the smallest
  that holds the precision or the range. Y's derived results follow the rule the hand lowering used: a sum or a
  difference of two decimals is at the wider type, a product with an integer at precision 38 and the decimal's scale,
  a product of two decimals at precision 38 and the sum of the scales; `to_decimal_rounded(scale: s, …)` gives the
  type wanted where the context has one, and otherwise the source's precision with scale `s`. A number as written is
  `Decimal::<…>::literal("4.50")` of the type its place has, or `of_units` in a constant.
- **Shared state.** An `atomic` region is a closure over a transaction, run by `atomic`, `atomic_with` (for an
  isolation level) or `orelse` (for `orelse region`s), each shared name read once (`let held_holdings =
  tx.get(holdings);`) and written back when the region changes it; `waits until name = (p)` returns `Step::Retry`
  while `p` does not hold. The region's value is the function's when it returns one.
- **Tasks.** A region with `supervise(…)` runs its `task` statements as `TaskBody` closures under `supervised`, and
  checks its `within` deadline when they have ended, failing with the case that carries an `expired`.
- **Cancellation and detachment.** `uncancellable, within d` runs the statements in `Cancellation::held`; `detach
  because "…", within d` runs them in `detach` and waits for them before the function ends.
- **Arenas and resources.** `arena name:` makes `Arena::new("name")`; `filed.batch()` is held (`Held::new`) and its
  release, `batch.end()`, is the function's result.
- **Constants a Rust constant cannot hold** (Warehouse's `day.orders`, a list of structures that hold lists) are
  functions, `day::orders()`, as the hand lowering wrote them.
- **The fields of a structure `main` imports are public**, as `main` makes and reads them; any other structure's are
  `pub(super)`.

## What it added to the runtime and the examples

- `y_runtime::list::sorted`, for `list.sorted()` in Memory's tests: a sorted copy as a `Vec`. The hand lowering sorted
  a clone in place, which needs a `mut` local inside an expression.
- `Integer::to_u32`, for a bounded integer where a `uint32` is wanted (`list.repeated(each, times: payers)`). The
  hand lowering cast with `as u32`.
- `examples/warehouse/tests/optimistic.rs`, which is not lowered from Y, named the hand lowering's type aliases
  (`Price`, `Rate`, `Quantity`), which no Y name gives. It now makes its line with `Decimal::literal` and
  `Integer::from_i128`, whose types the fields give, and passes against both the hand-written and the compiled Rust.
- `y_runtime::pattern`, the matcher for `matches(pattern"…")`, with `check`, which y-compile calls at compile time.
- `y_runtime::narrowed`, `narrowed_ok` and `narrowed_err`, which read a name a condition narrowed.
- `Forever::wait_on`, which runs what a region that is `forever` holds and gives its waits the capability.
- The stand-ins `y_runtime::command`, `faults`, `observe` and `claims`, and `y_runtime::assertion`, for the
  declarations no example uses (below). Atomic regions consult `faults` and buffer `observe`'s entries per run.
- Their levels in `tools/bare-rust/libraries/levels.json`: `Submit` is at `effects` (it reads the clock and waits
  between attempts), `faults` and the run buffering of `observe` at `shared`, and `log` and `metric` at
  `calculations`, trusted to buffer through thread-local state, since observability needs no capability (Y § 3.7).
- In the tools: the enumerators of paths (`tools/paths/yp.py`, `tools/bare-rust/tests/y_paths`) take every callable,
  `transition`, `handler`, `workflow`, a lambda and a nested function as well as a `function`, and an enumeration's
  members; `tools/paths/ytrace.rs` records no path for a call that faults; `trace.py` copies a package's `native/`; and
  bare-rust's region check lets an atomic region keep a `Deadline` it is `within`, a `Forever` it is `forever` with,
  and a `Cancellation` it is `uncancellable` with.

## `compile`: evaluation while building

`compile f(…)` is evaluated while the program is built (Y § 3.17), by a build step of y-compile
([`src/evaluate.rs`](src/evaluate.rs)):

1. The package is lowered with each `compile` as the call itself, and each is listed with its function, the Rust of its
   call, its value's type and the name it binds.
2. A crate of that Rust, its `let`s made mutable (weave has not derived its `mut`s), is written to the system's
   temporary folder (`y-compile-eval/<crate>`) with an entry point that makes each call in order and prints its value as
   Rust data. `cargo run` builds and runs it.
3. The package is lowered again, each `compile` now its value: a constant of the data (`const COMPILED_WORDS: &[&str] =
   &[…]`), and, where the value is not a Rust constant (a list, a string, a refined type), the value built from the data.
   In `main` it is built once, on first use, in a `static … : LazyLock<…>`, as the proposal's lowering says. In a library
   module it is built where it is read, by a function of its own (`fn square_of_table() -> List<u32>`): a static built
   on first use is shared state, which bare-rust's levels count as `shared`, and a module whose level is below it may
   not hold one (Y § 3.19).
4. A test is generated beside it that makes the same call at run time and compares the two (§ 3.17, "A generated test
   makes the same call at run time").

The value is the one the call gives at run time, because it is the same Rust. What is checked is the proposal's rules:
the operand is one call of a function with no `injected` parameter; each argument is a literal, a list of them, a
constant, or a name another `compile` bound; a call that fails is a compile error that names the call, the failure's
case and the case's `fails` role; and a call that faults is a compile error with the fault. The values written as data
are numbers, truth values, strings, refined types over them, closed types without payloads, decimals, and lists of
these; a `compile` of any other type is a compile error that names it. `y-facts.json` `compiled` says where each is.

## Narrowing

A condition narrows (Y § 3.3): where it holds, or where it does not, it shows that an optional is there or that a
result succeeded or failed, and the name is the value there. `narrowings` in [`src/lower.rs`](src/lower.rs) reads
what a condition shows: `x != optional.none` and `x == result.ok` where they hold, `x == optional.none` and `x ==
result.err` where they do not; `a and b` where it holds, both operands' facts; `a or b` where it does not, both
operands'; and `not`. It applies:

- **Within a condition:** the right operand of `and` is evaluated where the left held, and of `or` where it did not, so
  `(word != optional.none) and (word.count() > 0)` reads the word in its second operand.
- **In the block a condition guards:** an `if`'s block and its `else`'s, each `condition` of a `conditional` and its
  `else`, and a `while`'s block. The name is bound at the block's start, `let word = narrowed(word);`, through the
  runtime's `narrowed`, `narrowed_ok` and `narrowed_err`, which fault if the value is not what the condition showed (a
  bug of the lowering). A copied value is copied out (`*narrowed(&n)`).
- **After conditions that did not hold:** in a `conditional`, each condition is tested, and its block runs, where the
  ones before it did not hold, with what that shows.
- **An `if` on a result** (`if read = (parsed == result.ok):`) is a `match`, with the result's value in one arm and
  its failure in the other.
- **A case with its payload** (`(looked == result.err(lookup_error.missing))`, `(measured == shape.circle)`) is an arm
  whose pattern binds the fields its block reads, `Err(LookupError::Missing { key: looked_key, .. })`, and
  `looked.failure.key` and `measured.radius` read them.

The forms the examples use keep the lowering they had (a let-else for a condition that fails, a `match` on an
optional). The fact ends where the value may change: a name a block assigns, or lends `mutable`, is not narrowed in
it. Where a narrowed optional is compared with another optional (Tic-tac-toe's `(second == first)`), the value is
compared as an optional, `second == Some(*narrowed(&first))`.

## `ensures` on every way out

What a function ensures is checked on every way it returns (Y § 3.11). The function's `ensures` clauses are kept in
the lowering's context, and each `return` binds the value to the result's name, checks each clause, and gives it; a
function with no result checks them at each `return` and at its end. Where a function's last statement gives its
value, as the examples' do, that value is bound and checked once after it, as before, and only the `return`s before it
check for themselves. A function that may fail checks them only where it succeeds.

## Regions

A region takes any combination of constraints that constraint language § 6.23 allows, and one it does not is a compile
error: a constraint said twice; `isolation` or `waits until` without `atomic`; `uncancellable` or `detach` without
`within`; `within` with `forever`. Each lowering keeps its constraints as bare-rust's region check reads them:

| Region | Lowered to |
|---|---|
| `atomic` (with `isolation`, `waits until`, `orelse`) | a transaction, `atomic`, `atomic_with` or `orelse`, as before |
| … `, within d` | `let deadline = Deadline::after(d);`, given to the transaction (`Some(&deadline)`); with `or fail`, its expiry, `Aborted::Expired`, is the function's failure through the case that carries an `expired` |
| … `, uncancellable, within d` | the transaction run inside `Cancellation::new().held(…, d, \|\| …)` |
| … `, forever because "…"` | the transaction run inside `Forever::wait_on(forever, \|\| …)`, which gives the wait its capability |
| `within d`, `supervise(…)` | the tasks under `supervised`, as before; the deadline checked when they end |
| … `, uncancellable` / `forever` | `supervised` run inside `held` / `wait_on` |
| … `, detach because "…"` | the tasks built and supervised inside `detach(…, move \|\| …)`, waited for before the function ends |
| `detach because "…", within d` | the statements in `detach`, as before |
| … `, uncancellable` | the statements inside `held`, inside `detach` |
| `uncancellable, within d` | the statements in `Cancellation::held`, as before |

Detached work runs on a thread of its own, so it takes its own handle to each `shared` state it uses (`let tally =
tally.clone();`), the ones its statements name and the ones injected into the functions it calls. A detached region in a
function that cannot fail runs work that cannot fail, and the function waits for it without a result to pass on.

bare-rust's region check (`tools/bare-rust/src/region.rs`) counted what keeps an atomic region's own constraints as
effects of its body: the deadline a `within` starts, the `Forever` a `forever` gives its wait, and the hold an
`uncancellable` puts on cancellation. § 6.23 allows each with `atomic`, so the check now leaves out exactly those: a
`y_runtime::Deadline` call in a region that is `within`, the `Forever` parameter and its `wait_on` in one that is
`forever`, and `y_runtime::task::Cancellation` in one that is `uncancellable`.

## `matches(pattern"…")`

A pattern is matched by a small matcher in the runtime, [`y_runtime::pattern`](../../examples/y-runtime/src/pattern.rs),
and checked at compile time against the subset it takes, by the same parser: a pattern outside it is a compile error.
The proposal says a `pattern"…"` literal is a regular expression checked at compile time (§ 3.1), and fixes no dialect,
so the subset is this one: a character, which `\` escapes; `.`; classes `[a-z0-9_]` and `[^…]`; `\d`, `\w`, `\s`;
the anchors `^` and `$`; `*`, `+` and `?`, greedy; alternation `|`; and groups `(…)`. A pattern matches when it matches
some part of the text, as a regular expression's search does. The literal's text is taken as written, without a
string's escapes. It is used in a refined type's `where` and as `text.matches(pattern"…")`; Hangman's `^[a-z]*$`
keeps its lowering, a test of each rune.

## Decimals, rationals and integers

Y's derived constraints (§ 3.3) decide each result's type, and each operand is converted to it first, so that the
arithmetic never overflows:

| Operation | Result |
|---|---|
| `a + b`, `a - b` | scale `max(s₁, s₂)`; precision `max(p₁ − s₁, p₂ − s₂) + max(s₁, s₂) + 1` |
| `a * b` | scale `s₁ + s₂`; precision `p₁ + p₂` (`multiply`) |
| `a * n`, `n` an integer | the decimal's scale; precision `p` plus the digits of `n`'s range (`times`) |
| `a.divide_rounded(b, rounding.m)` | the dividend's scale, the larger precision, rounded as named |
| `a.divide_exact(b)` | the same, or the failure `inexact` |
| `a.to_integer_rounded(rounding.m)` | an `integer` of the range the decimal's whole part has |
| `a.to_float64_nearest()` | `float64` |
| `a.to_rational()`, `rational.new(numerator: n, denominator: d)` | `rational` |
| `r₁ + r₂`, `-`, `*`, `/` | `rational`, exact |
| `r.to_decimal_rounded(scale: s, rounding: …)`, `r.to_decimal_exact(scale: s)` | a decimal of scale `s` (the one wanted where the context names one), or `inexact` |
| `i₁ + i₂`, `-`, `*` on `integer<minimum, maximum>` | an integer whose range is the extremes of the operands' |
| `a < b`, `==`, … | exact across scales and precisions: both in one type that holds each |

A precision past 76 is unbounded, `Decimal<Big, UNBOUNDED, s>`; one from 39 to 76 is stored in the runtime's `I256`. A
decimal without a precision is taken as precision 38, as before. A value meets its declared type where it is bound, given or
returned, converted there; one whose scale would fall is a compile error that names `to_decimal_rounded` (§ 3.3, "A
scale never falls silently"), and `/` on a decimal or an integer is a compile error that names the division to use.

## Library methods

[`src/library.rs`](src/library.rs) reads what y-collections, y-string, y-runtime, y-window and y-http offer from the
crates themselves: bare-rust's strip of each crate's sources gives its constraint data, whose public `function` subjects
are each type's methods and each module's functions, with their parameters and results. A Y method the lowering has
no rule for is lowered as the method of the same name on the receiver's Rust type, its arguments borrowed or given as
the method's parameters take them, and its result's type read from the method's (an element borrowed and perhaps
absent is copied out, `items.front().cloned()`). One the type does not offer is a compile error that names the method
and lists what the type offers, and so is a method a structure or closed type of the module does not have. The runtime
operations the lowering's own rules call are listed in `LOWERED`, and `tests/library.rs` checks each against the
crates.

## The declarations and statements no example uses

The test programs `orders`, `shapes` and `claims` prove each of these through the whole chain. Where the proposal
leaves a meaning or a spelling open, the decision is recorded here, and the proposal's own text is settled only where
it said nothing (the pull request lists each edit).

| Y | Lowered to |
|---|---|
| `enumeration e [is t]:` | A Rust `enum`, its members in the order written (so `<` follows the order), `E::ALL`, the members in order, for `for x in e`, and `value()`, each member's value when the enumeration names a type, its position from 0 otherwise. `Display` writes the value |
| `state s:` and its `case`s | `struct S<St = SState>`, the shared fields and `state: St`; a module `s` with a structure per case, its own fields; `SState`, the enum of the cases, into which each converts. `s.c(…)` makes a value in a state, of type `S<s::C>`; a case's field is read through `.state` |
| `transition t(from, …) returns to` | A function that takes the value it starts from by value, so the name passed to it cannot be used afterwards: a later read is a compile error. `from.becomes(s.c(…))` keeps the shared fields and replaces the state, and is a compile error where no transition of the module returns `s.c` |
| A value in a state where the state is not known (`stored: …, as s`) | `S<SState>`; a value in a known state is lent there as a copy with its state forgotten (`forget()`). `conditional x = v:` with `condition x.c = (x == s.c)` matches `known()`, each arm the value in its state |
| `trait t:` | A Rust trait of its methods' signatures; `me` is `&self`, or `&mut self` in a `mutable function`. A method in a trait has no body |
| `implementation t for x:` | `impl T for X`. A method the trait does not declare, and one the trait declares that is not given, are compile errors. A method's paths are named by the method's name, like a function's |
| `unit u symbol s is t` and `unit u symbol s is n other` | Erased: a quantity is its base type. Literals take the unit by its symbol (`5m`, `3km`). `+`, `-` and comparison need one unit on both sides; `*` takes a plain number; any other mix is a compile error naming both units. `x.to_other()` converts a unit to the one it is made of, multiplying by the factor |
| `structure` with a `transient` field | A written-out `PartialEq` that compares every other field, and no `Hash`, which would disagree with it |
| `old(e)` in an `ensures` | `let old_1 = e;` where the function starts, read by the check. Anywhere else, a compile error |
| `assert name = (p)` | `y_runtime::assertion(p, "m::f#name")`: when `p` does not hold, a fault, as `fail not_reachable` is |
| `assume name = p because "…"`, `allow(code) because "…"` | A comment that keeps the claim, `// y: assume name because "…"`. Neither has a run-time meaning: the first feeds the enumerator of paths (below), the second a lint |
| `log level "text {x}"` | `y_runtime::observe::log`, with the construct's stable identity. The levels are `debug`, `info`, `warning` and `error` |
| `metric name = value` | `y_runtime::observe::metric`, with the identity |
| `name = lambda name(…) returns r captures c:` and `nested function name(…)` | A private function written after the one it is in, its parameters its own and then what it captures, in the order listed; and, where it captures something or is bound as a value, a closure of the same name that passes the captures. A capture not listed, and a listed name not in scope, are compile errors. Its paths are named by its own name (`covers is_larger > bigger`) |
| `command c:` with `policy:` | A structure of its fields and `impl y_runtime::command::Command`: its `Policy` and the key duplicates collapse on (`deduplicate_by`). Each policy field is required; a missing one is a compile error |
| `enqueue c(…)` | Added to the `enqueue<command_type=c>` in scope, an `Outbox` held in `shared`, inside the atomic region it is in. Outside one, a compile error |
| `handler c(command, …)` | A function |
| `workflow w(…)` | A function. A workflow that holds a capability other than `store`, `shared`, `submit` and `enqueue` is a compile error |
| `submit c(…)` | `charges.submit(c)` on the `submit<command_type=c>` in scope, whose result is `result<value_type=r, failure_type=undelivered<failure_type=e>>` from the handler's `returns r fails e`. `submit.to(c)` makes one, the handler's capabilities taken from where it is made, and the compensation `on_abandon: compensate d(…)` names, delivered to `d`'s handler |
| `function f(…) native because "…"` | A call of `crate::<module>::native::f`, in `native/rust/<module>.rs` of the package, which the module includes by `#[path]`. The claim is kept as a comment |
| `type t native because "…"` | `pub use` of the native implementation's type of the same name |
| `sensitive<value_type=t>` and `reveal(x) because "…"` | `y_runtime::claims::Sensitive<T>`, which writes as `<sensitive>`; `reveal` is a capability, `Reveal`, and `reveal(x)` borrows the value through it |
| `type t is collection:` with `ordered by`, `unique`, `unordered`, `values many`, `offers` | The y-collections kind those constraints describe (Y § 3.13's table): `ordered by position` with `offers back_add, front_take` is `Queue`, `unique` and `ordered by value` is `SortedSet`, and so on. Its methods are the crate's |
| A test's `faults:` | `y_runtime::faults::force` before the test's `when`: `conflict at f#txn, times: n` and `expire at f#txn` |

**Decisions where the proposal is open.**

- **A unit's declaration** is `unit name symbol s is type` for a base unit and `unit name symbol s is n other` for one
  made of another; `x.to_other()` is its one conversion. The proposal says only that the factor is declared once on
  the unit.
- **A trait** is `trait name:` and its methods' signatures, each documented; **an implementation** is `implementation
  trait for type:` and its methods. Neither is spelled in the proposal.
- **`log` takes a level and a string**, `log warning "…"`, from `debug`, `info`, `warning` and `error`; **`metric`** is
  `metric name = value`. The proposal names both statements without their form.
- **`enqueue.new()`** makes an empty outbox where a test has no `fake enqueue<command_type=c>`; both are the same.
  **`submit.to(handler)`** makes the capability that delivers to a handler; the proposal names the type
  `submit<command_type=c>` and not how one is made.
- **A command's handler is the handler of the command's name**, as `handler charge_card(command, …)` in the proposal is
  `charge_card`'s.
- **Workflows and handlers run as ordinary functions** in this runtime: delivery is immediate and in process (below).
- **`allow` takes a lint's code**, `allow(Y2031)`; y-compile has no lints, so it keeps the claim and checks nothing.

**What the runtime stand-ins keep and do not keep.** Each is in `examples/y-runtime` and documented there as a stand-in:

- `command`: `Command`, `Policy`, `Outbox`, `Submit`, `Undelivered`. A submitted command is delivered at once, in the
  process; a handler that faults (panics) is retried with the policy's backoff while it is idempotent and within its
  deadline, then abandoned and compensated or parked. Nothing is durable: commands and outcomes do not outlive the
  process, and a workflow does not resume after a crash.
- `faults`: `conflict at` and `expire at` an atomic region. `crash after commit` and `cancel at` are not kept; a test
  that forces them is a compile error naming the fault.
- `observe`: `log` and `metric` write one line each to standard error, buffered inside an atomic run and written only if
  it commits.
- `claims`: `Sensitive` and `Reveal`.

The tracer (`tools/paths/trace.py`) copies a package's `native/` folder with its sources, and a call that faults (a
handler forced to panic) takes no path, so a test of a retried handler claims only the attempts that returned.

## Where its output differs from the hand lowering

Each kind of difference below keeps the woven Rust building and its tests passing.

- **An `if` whose branch fails or returns** is lowered `if p { … } else { … }`, with the rest of the function in the
  `else`. The hand lowering often wrote a guard, `if p { // y: #a, else #b` with the code after it as the `else`. Both
  are forms `trace.py` reads, and the paths are the same.
- **A `for` over a list of numbers or other copied values** binds the element by value, `for &heap in &game.heaps`;
  the hand lowering bound a reference and dereferenced it where needed.
- **A `conditional` over a result** binds the value in each arm to the label's name, `Ok(attempted)` and
  `Err(attempted)`; the hand lowering chose a name for each (`Ok(chosen)`, `Err(error)`).
- **A string returned from a closed choice** is made in each arm (`YString::new("…")`), not once around the `match`.
- **A list of strings is joined** with the runtime's `YString::joined`, not through a `Vec<&str>`, so the bare body
  has no typed locals.
- **A test's checks** read the console's lines in each assertion; the hand lowering sometimes bound them to a local
  first (`let written = console.written();`). A check that a string is empty compares it with `""`.
- **A value chosen by a `conditional` and checked by `ensures`** is bound to the result's name before the check, as
  the hand lowering did, but the check is the Y condition as written (`value >= -10 && value <= 10`), not a range.
- **A count compared with a number** is the list's `count()` (`open.count() == 0`) where the hand lowering sometimes
  used `list::count`, which gives a `u32`.
- **`mutates only`** names each member of a `mutable` parameter that changes, or the parameter itself when every
  member does; the hand lowering sometimes named the parameter when only some members change.
- **A test file's own functions** (Tic-tac-toe's `marked`) bind each element of a list of numbers by value.
- **A precondition** is the Y condition as written: `not words.is_empty()` where the hand lowering wrote
  `count(words) > 0`.
- **A `conditional` with a condition that narrows** among others (Hangman's `guess`) is lowered as the hand lowering
  did it: the conditions before it as guards, it as a let-else, and the rest as a chain. A guard has no `else` marker.
- **A newtype's `Display`** is written `std::fmt::Display`, and its constraint checks keep the Y brackets
  (`if !(…)`).
- **The console's text, once narrowed,** is made a `YString` at once (`let typed = YString::from(typed);`), in every
  example; the hand lowering did so only where the text was used twice.
- **Effects clauses** follow one rule (the table above). The hand lowering was not always consistent: Snake's `play`
  says `no io` though it draws in a window, and Checkers' `play` leaves out `no clock`.
- **What a failing function ensures** is checked on the value before it is given (`let started = …; ensure(…);
  Ok(started)`), as the hand lowering did; its clause keeps the Y condition (`result is Ok(started) implies
  started.turned.is_empty()` where the hand lowering wrote `count(started.turned) == 0`).
- **An element of a list that is not copied** (a picture) is borrowed with `list::at_number_ref`, and given to the
  window by reference.
- **A structure is passed by reference** unless the function keeps it, in every example. Checkers' hand lowering
  passed its small copied structures (`square`, `piece`, a `move` it applies) by value, where Nim's passed `move` by
  reference; y-compile keeps Nim's rule. Of a list parameter it keeps, it takes the value (`chains_after`'s `path`).
- **`count()` takes the number type its context wants**: compared with or passed as a `uint32` it is the runtime's
  `list::count` (a `u32`), and otherwise the list's own `count()` (a `usize`), where the hand lowering chose case by
  case.
- **A `conditional` whose conditions test the value's members** (`conditional counted = placed.piece:` with
  `condition counted.other = (counted.owner != owner):`) names the value, `let counted = placed.piece;`, and is a
  chain; the hand lowering wrote the members out in full. One whose conditions are a case and a further test
  (`(shown == outcome.playing) and …`) is a `match` arm with a guard, as the hand lowering wrote it.
- **Warehouse's types** are written out (`Decimal<i64, 12, 2>`) where the hand lowering named them with aliases of
  its own (`Price`, `Rate`, `Net`, `Vat`, `Quantity`, `Payers`); only `Sku`, a Y name, is an alias. Its decimal
  conversions are turbofish calls (`to_decimal::<i128, 38, 2>()`), which strip makes realizations of.
- **A copy of a borrowed value** (`order.lines` of `order: &Order`, `wanted` given to a queue) is marked `// y: a
  copy` and listed in `copies` in every example, with the reason `… is borrowed, and a value is kept`; the hand
  lowering marked some and not others.
- **A function lent shared state or a store** gets no effects clauses, as in the hand lowering: its regions' clauses
  say what it may do.
- **`rendered()` on an arena** is a function the lowering writes beside the module, as the hand lowering did: the
  entries one per line, an entry `under` another indented by two spaces. The proposal does not define `rendered`.
- **A test's helpers.** Warehouse's hand-written tests built their lines with helper functions of their own (`sku`,
  `line`, `order`, `shelf`); the compiler writes each value out where the Y source makes it.
- **The entry point's `compile`** is its value, written as data by the build step, as the hand lowering wrote it;
  the static is named after the name it binds (`WORDS`), and the generated test after the static.
- **Imports** are the ones the code uses, found in the code; the order of the paths in `y-facts.json` is the
  enumerator's sorted order.
- **Documentation** is made from the Y documentation mechanically: the prose, then the roles, each without its type.
  The hand lowering sometimes reworded it.
- **The file's status** is `assumed` with the reason `lowered from y/src by y-compile`.

## Where the hand lowering is inconsistent

The compiler follows one rule where the hand lowering varied, and does not copy the variation:

- **Effects clauses.** Snake's `play` says `no io` though it draws in a window; Checkers' `play` leaves out `no
  clock`, which every other function without a clock says.
- **Passing structures.** Nim passes its copied `move` by reference; Checkers passes its copied `square`, `piece` and
  `move` by value. The compiler passes a structure by reference unless the function keeps it.
- **`mutates only`.** Snake's `advance` and Tic-tac-toe's `place` say `mutates only game` though each leaves some of
  the game's fields alone; the compiler names the fields that change (the whole parameter only when all do).
- **Copies.** The hand lowering marked some copies of borrowed values `// y: a copy` and listed them in `copies`, and
  left others (Hangman's `mark.clone()`, Warehouse's `wanted.sku.clone()`) unmarked; the compiler marks and lists each.
- **A string from a closed choice** was made around the whole `match` in Nim and Tic-tac-toe, and in each arm in
  Snake and Checkers; the compiler makes it in each arm.

## How close it comes

With its output in place of the hand lowering (`tools/paths/rebare.sh --from-y <example>`), every example passes its
checks in `tools/bare-rust/tests/y_examples.rs`, and `cargo test --release --test examples` here weaves each, builds
it, runs its tests and `trace.py`. Against the committed `bare/` files:

| Example | Bare lines that differ | Subjects with the same names |
|---|---|---|
| Nim | 62 of 838 | 87 of 87 |
| Tic-tac-toe | 55 of 891 | 92 of 92 |
| Hangman | 98 of 990 | 91 of 95 |
| Snake | 55 of 953 | 100 of 100 |
| Memory | 167 of 1224 | 134 of 134 |
| Checkers | 355 of 2882 | 311 of 311 |
| Warehouse | 176 of 824 | 88 of 100 |

A bare line differs when the compiler writes it and the committed file does not have it. Hangman's missing subjects are
the hand lowering's names for its compiled word list and its check (`COMPILED_WORD_LIST`, `compiled_word_list_matches_
run_time`; the compiler names them after the name the `compile` binds), and its `Display` impl named `fmt::Display`;
Warehouse's are its type aliases (`Price` and the others), its `rendered` (here at module level), and the helper
functions its hand-written tests and `main` used to build lines (`sku`, `line`, `order`, `shelf`), which the compiler
writes out in place.

## Paths: decimals, assumptions and a returned failure

The enumerators of paths (`tools/bare-rust/tests/y_paths`, `tools/paths/yp.py`, which give the same paths) know three
more things:

- **Decimals are ordered.** A comparison of a `decimal<…>` has the sub-outcomes an integer's has: `not (price < limit)`
  is `'eq` or `'gt`. A decimal has values between any two whole numbers, so what a comparison shows of it is kept with
  its ends included (`x < 3` shows `x <= 3`), and a decimal literal is a known value only when it is whole (`0.00`).
  `integer<minimum=a, maximum=b>` is ordered too, and bounded by its range.
- **`assume name = p because "…"` holds from where it is**, without proof (Y § 3.15): the prover takes `p` as a fact.
  An assumption that cannot hold where it is is a problem, as a loop that may never run is. Warehouse's `split` assumes
  `left >= 0.00`, since every share is rounded down, and so `else uneven` is one path, not `'lt` and `'gt`.
- **`return x = f() or fail` is a way out**, `x'fails`, as `x = f() or fail` is. y-compile lowers it
  `let x = f(…)?;` and `Ok(x)`, where a trace sees the `?`. Nim's `read_move` gains `read_move > chosen'fails`, which
  its test of a move the game refuses now covers; its hand lowering is written the same way.

## What it does not handle yet

It handles the forms the seven examples and the test programs use. In particular, it does not handle:

- **Durability**: commands, outcomes and workflows live only as long as the process (above). Y's durable command log,
  its workers and a workflow's resumption are a runtime, not a lowering, and this one is a stand-in.
- **The faults `crash after commit` and `cancel at`**, which need a runtime that can crash and resume, or cancel a task
  at a named point. A test that forces them is a compile error.
- **Generic declarations** of the module's own: a trait's methods take no type parameters, and an implementation is for
  a named type.
- **A lint for `allow` to suppress**: the claim is kept as a comment.
- **A `compile` of a value y-compile cannot write as data**: a structure, a closed type with payloads, a set or a map.
  It is a compile error.
