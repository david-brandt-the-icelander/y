# y-compile

The Y compiler, as far as the repository's examples need it. It reads an example's Y source (`y/src/*.y`: the module,
its tests and `main`) and writes what the examples' `bare/` folders hold: the bare Rust bodies, the constraint files
and `y-facts.json`. `bare-rust weave` makes Rust of them, which builds and passes the example's lowered tests.

```text
cargo run --release -- ../../examples/nim --out <dir>            # <dir>/src/*.brs, *.constraints.json, <dir>/y-facts.json
cargo run --release -- ../../examples/nim --rust <dir>           # the typed Rust it strips, for reading
cargo test --release --test examples                             # each example compiled, woven, built, tested and traced
```

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
   runtime operation (`list::at_number`) each use needs.
4. **Strip** ([`src/bare.rs`](src/bare.rs)). The typed Rust is stripped by `bare-rust`'s own strip, in memory, which
   writes the bare bodies and the constraint files. The lowering then adds what strip cannot see, as
   `tools/paths/regen.py` adds it to a strip of hand-written Rust: each `injected` parameter's mark, each module's
   level (from `tools/bare-rust/tests/y_levels`, the inference the example checks use), and each function's clauses,
   which are made from the Y source rather than carried over from earlier files.
5. **Facts** ([`src/facts.rs`](src/facts.rs)). `y-facts.json` is written from the Y source and the bare bodies: the
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

## Decisions the proposal leaves open

- **`compile f(…)` is not evaluated while building.** The proposal (§ 3.17) has the compiler evaluate the call and
  write its value into the program. y-compile has no evaluator of Y, so it lowers `words = compile
  parse_word_list(game_rules.word_list)` to the call itself, made where the entry point starts, with `expect`: a
  failure is a fault when the program starts rather than an error when it is built. `y-facts.json` `compiled` says so.
  The hand lowering wrote the ten words down as a constant array and checked them against the call in a test.
- **A newtype's `matches(pattern"…")`** is lowered for a pattern of one character class repeated, `^[a-z]*$`, as a
  test of each rune; the runtime has no regular expressions. Its clause keeps the pattern: `self.0 matches /^[a-z]*$/`.

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
- **The entry point's `compile`** is the call, made when the program starts (see above).
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
| Tic-tac-toe | 53 of 891 | 92 of 92 |
| Hangman | 92 of 990 | 90 of 95 |
| Snake | 55 of 953 | 100 of 100 |
| Memory | 167 of 1224 | 134 of 134 |
| Checkers | 356 of 2882 | 311 of 311 |
| Warehouse | 174 of 824 | 88 of 100 |

Hangman's missing subjects are the hand lowering's compiled word list and its check, and its `Display` impl named
`fmt::Display`; Warehouse's are its type aliases (`Price` and the others), its `rendered` (here at module level), and
the helper functions its hand-written tests and `main` used to build lines (`sku`, `line`, `order`, `shelf`), which
the compiler writes out in place.

## What it does not handle yet

It handles the forms the seven examples use, and no others. In particular:

- **The forms the parser does not read** ([`../y-parse`](../y-parse/README.md)): `enumeration`, `state`,
  `transition`, `trait`, `implementation`, `unit`, `command`, `handler`, `workflow`, `lambda`, `old`, `assert`,
  `native`, `assume`, `allow`, `reveal`, `log`, `metric`, `policy`, `compensate`, a test's `faults`, and the rest of
  that list.
- **`compile f(…)` is evaluated when the program starts, not while it is built** (see above).
- **Library operations**: only those the examples call, each with the lowering its README gives (the methods of
  lists, sets, bags, queues, sorted maps, strings, runes, decimals and integers, and of the console, window, clock,
  random source, site, stores and shared state). Any other method is lowered as a plain method call, which may not
  build.
- **Narrowing** by `x == optional.none` (in an `if`, in a `conditional`, and through `or`) and by `x !=
  optional.none`; not through `and`, and not of a result by an `if`.
- **`ensures`** checks the value of the function's last statement; a `return` before it is not checked.
- **Regions**: the combinations of constraints the examples use: `forever`; `within` a bound around waits on a site;
  `within` with `supervise` around `task`s; `atomic`, with `isolation`, `waits until` and `orelse`; `uncancellable,
  within`; and `detach, within`, whose `or fail`s are taken to fail as the console does.
- **A newtype's `matches(pattern"…")`** for one character class repeated.
- **Decimals** of the arithmetic Warehouse uses: sums, differences and products, with integers and with each other,
  rationals divided, and rounding to a scale.
