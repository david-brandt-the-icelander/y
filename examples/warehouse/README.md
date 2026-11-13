# Warehouse, from Y to Rust

One day of a small warehouse:
- it reads its catalogue from a store, migrating old records as it reads them;
- it packs the day's orders with two tasks that share the stock;
- it reports what each order costs and how its bill is split between the people who pay it;
- it files the receipts in a second store, and prints them once the report is written.

The example is written in Y, lowered to bare Rust plus its constraint files, and woven into Rust that builds, runs and
passes its tests.

It is the example for what the games do not need:
- atomic regions, `waits until` and `orelse` over shared state;
- supervised tasks, detached work and an `uncancellable` region;
- decimals, rationals and rounding modes;
- pipelines over lists;
- collection kinds whose constraints are enforced;
- an arena with typed indexes;
- a resource released exactly once;
- schemas, with a migration.

The pipeline and the lowering rules are the ones [Hangman](../hangman/README.md) sets out. This README states only what
Warehouse adds. As there, the lowering from Y to bare Rust is done by hand, as a compiler would do it, and the other steps
are the real tools.

## Running it

```
cargo run          # in a folder with an items.store; see below
cargo test         # the lowered Y tests
```

The catalogue is the file `items.store`, one record a line: `item v<version> <sku>|<fields>`. A version 1 record has a
name and a price. A version 2 record adds the VAT rate. Version 1 records are migrated as they are read:

```
item v1 cup-3|Cup;12.00
item v2 map-7|Map;9.99;0.0000
item v2 tea-1|Tea;4.50;0.2400
```

The program writes `receipts.store` beside it.

## Files

| Stage | Files | Lines | Written by |
|---|---|---|---|
| Y source | `y/src/warehouse.y`, `y/src/warehouse.test.y`, `y/src/main.y` | 287, 316, 63 | hand |
| Bare bodies | `bare/src/warehouse.brs`, `bare/src/warehouse/tests.brs`, `bare/src/main.brs`, `bare/src/lib.brs` | 419, 255, 121, 3 | `bare-rust strip` of the Rust below, which was written as the Y compiler would lower the source |
| Constraint files | `bare/src/*.constraints.json` (97 subjects, 16 clauses) | 927 | `strip`, plus the clauses, regions, status and module levels the Y lowering adds, then `bare-rust libfmt` |
| Y facts | `bare/y-facts.json` | 614 | the same, from the Y source and the bare bodies |
| Rust | `src/warehouse.rs`, `src/warehouse/tests.rs`, `src/main.rs`, `src/lib.rs` | 418, 255, 121, 3 | `bare-rust weave` |

## What Warehouse adds to Hangman's lowering

| Y | Rust | Runtime |
|---|---|---|
| `shared<value_type=holdings>`, and a region that is `atomic` | `Shared<Holdings>`, and a closure over a `Tx` that reads with `tx.get` and writes with `tx.set` | `y_runtime::shared`: every atomic region takes one global lock, so regions are serializable by construction and none is ever re-run. Writes are buffered and applied when the region ends, so a failure leaves no trace |
| `waits until enough = (…)` on the region `take_from_shelf` | the closure returns `Step::Retry` when the condition does not hold, marked `// y: waits until enough` | the region waits on one condition variable, which every commit notifies |
| `region take_from_shelf is …:` `orelse region put_on_backorder is …:` | one closure per alternative, each marked `// y: #name`, passed in order to `orelse` | the alternatives run left to right under one hold of the lock; the first that does not retry commits. The path tools count the alternatives as a decision whose outcomes are the regions' names, and `trace.py` records an alternative where it commits |
| `region packing is within 10s or fail, supervise(restart: restart_policy.each_failed, at_most: 2, per: 1min):`, with `task first_half = …` and `task second_half = …` | a `Deadline`, and two `TaskBody` closures run by `supervised` | `y_runtime::task::supervised`: a thread per task; the region ends when all have; a failed task is restarted as the policy allows |
| `region filing is uncancellable, within 50ms:` | `Cancellation::held(…, 50ms, …)` | cancellation is held off while the region runs, and a hold past its bound is a fault |
| `region printing is detach because "…", within 1min:` in `main` | `detach(60s, …)`, waited for before the program ends | `y_runtime::task::detach`: a thread that may outlive its scope, waited for no longer than its bound |
| `decimal<precision=12, scale=2>`, `decimal<precision=5, scale=4>`, and the results the compiler derives, `decimal<scale=2>` and `decimal<scale=6>` | `Decimal<i64, 12, 2>`, `Decimal<i32, 5, 4>`, `Decimal<i128, 38, 2>`, `Decimal<i128, 38, 6>` | `y_runtime::decimal`: the backend picks the size from the constraints |
| `integer<minimum=1, maximum=1000>` | `Integer<i16, 1, 1000>` | checked where a value meets the constraint |
| `(total.to_rational() / payers.to_rational()).to_decimal_rounded(scale: 2, rounding: rounding.toward_negative)` | `Rational<Big, UNBOUNDED>`, divided exactly, then rounded once | `split` gives the first payer the cents left over |
| `….to_decimal_rounded(scale: 2, rounding: rounding.half_away_from_zero)` in `order_total` | `to_decimal_rounded(Rounding::HalfAwayFromZero)` | the VAT stays exact until the total is rounded |
| `order.lines.map(net_of).sum()`, `order.lines.filter(taxed).map(vat_of).sum()`, `orders.map(lines_of).flatten()` | iterator chains: `iter().map(net_of).fold(…)`, `filter(\|line\| taxed(line))`, `flat_map(lines_of)` | — |
| `bag<element_type=sku>`, `queue<element_type=line>`, `sorted_map<key_type=sku, value_type=item>`, `type sku is string<character_set=ascii>` | `Bag<Sku>`, `Queue<Line>`, `SortedMap<Sku, Item>`, `AsciiString` | `y-collections` and `y-string`: each kind offers only what its constraints allow, so a misuse does not compile (the `compile_fail` tests in `y-collections` show it) |
| `arena receipt_scratch:`, `receipt_scratch.allocate(entry(…))`, `index<unit_type=entry>` | `Arena::new("receipt_scratch")`, `allocate`, `Index<Entry>` | `y_runtime::arena`: released at once when the arena ends; an index from another arena is a fault |
| `batch = filed.batch()`, a resource whose release writes the batch and can fail with `store_error` | `Held::new(filed.batch())`, ended with `batch.end()`, whose failure is the function's | `y_runtime::arena::Held`: released exactly once, on a normal exit by `end` and on a failure when it is dropped |
| `schema item v1:`, `schema item v2:`, `migrate item v1 to v2: vat_rate := tax.standard_rate` | `impl Schema for Item`, whose `decode` reads both versions and migrates version 1 | `y_runtime::store::Store`: records with their versions, read one by one, so a record that fails fails alone, against its key |
| `store<schema_type=item>`, `system.store(path"items.store")` | `Store<Item>`, `system.store("items.store")`; `Store::fake(lines)` in the tests | — |
| each `region name is …:` | a `region` subject in the constraint file, `<function>#<name>`, with one clause for each constraint: `atomic`, `waits until enough : …`, `within 10s`, `uncancellable`, `detach because "…"` (constraint language § 6.23). The function marks where the region starts, `// y: region name is …` or `// y: #name` | weave refuses a region its function does not mark, or whose marker states other constraints |
| the facets' levels, which the Y compiler infers (Y § 3.19) | `"level"` on each facet's module subject: `pricing` at `structured`, `stock` at `shared`, `catalogue::tax` at `data`, the others at `effects` | weave, and `bare-rust levels`, refuse a body that uses more than its module's level: `pricing` at `calculations` fails on `split`'s `&mut` |
| `constants day:` with lists of structures | functions that make them, since a Rust `const` cannot hold a `List` | — |

## Checks

- `cargo test`: the 25 lowered Y tests, one for each of the module's 25 paths.
- `tools/paths/rebare.sh warehouse`: 10 decisions, 25 paths, 25 tests, `uncovered: [] unknown: []`, and the example's
  checks in `tools/bare-rust/tests/y_examples.rs`, which pass.
- `python3 tools/paths/trace.py examples/warehouse warehouse`: every path is taken by the test that claims it, the two
  alternatives of `reserve`'s `orelse` among them.
- The run above, on the three records shown, packs both orders and reports `Order 1001: 26.04, paid as [8.68, 8.68,
  8.68]`. That is 21.00 net and 5.04 VAT, split three ways.

## What had to be assumed

- **Stand-ins for the runtime.** Where Y's runtime is described but not built, the stand-ins are simpler:
  - Tasks are threads, and cancellation reaches a task only where it checks.
  - The store is a text file of lines.
  - A failing release on a failing exit is written to the error stream, beside the failure that is returned.
- **Syntax the proposal has not fixed.** The forms are the simplest that fit the rest of Y:
  - Payloads are read as `stored.value` and `stored.failure.key`.
  - Allocation in an arena is `arena.allocate(…)`, and its lines are read with `rendered()`.
  - A resource is opened with `filed.batch()`.
  - A bag is changed with `add(x, times: n)` and `remove(x, times: n)`.
