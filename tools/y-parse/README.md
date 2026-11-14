# y-parse

A parser for Y ([`proposal/Y-proposal.md`](../../proposal/Y-proposal.md)), as far as the repository's examples use it.
It parses every `.y` file of the seven examples under [`../../examples`](../../examples/) (each module, its tests and
its `main`) into a tree whose every node carries the line it starts on. It checks the form of the source, not its
meaning: it resolves no names and infers no types.

```text
cargo run --release -- ../../examples/nim/y/src/nim.y          # the tree, as JSON
cargo run --release -- --check ../../examples/*/y/src/*.y      # only report errors
cargo test --release                                           # the parser's tests
```

The command prints one JSON object that maps each file, as given, to its tree. A file that does not parse is reported
as `file:line: message`, and the exit status is 1.

## Why Rust

The tools that read Y are of two kinds: the checks in [`../bare-rust/tests`](../bare-rust/tests/), which are Rust, and
the scripts in [`../paths`](../paths/), which are Python. A Rust library serves the first directly, as a
dev-dependency of `bare-rust`, and the second through the command's JSON, which [`../paths/ytree.py`](../paths/ytree.py)
reads. A Python parser would serve only the scripts, and the checks would go on reading Y their own way. The crate has
no dependencies, so it builds in a second, and every tool that needs it can build it with cargo, which they already
need.

## The tree

Every node is an object with a `kind`, a `line` (from 1), and the fields of its kind. A node that takes several lines
has an `end`, its last line. A declaration also has its `visibility` (`public`, `private` or null), its `doc` (the
documentation block above it, with the `blank` lines above that) and its `comment` (the `--` lines right above it). A
declaration and a statement have their `blank`, the blank lines right above their first line, and their `gap`, the
blank lines above the comment lines right above them (their `blank` when there are none), for the layout rules of
§ 3.1. In Rust a node is a [`Node`](src/tree.rs), whose `str`, `flag`, `node`, `nodes`, `walk` and `find` read it; in
Python a node is a dict, and `ytree.walk` and `ytree.nodes` read it.

| Kind | Fields |
|---|---|
| `file` | `decls`; `comments`, every comment of the file as `comment` nodes with their `text`; `alone`, whether no code is before it on its line; `indent`; `blank`, the blank lines right above it; and `after`, what the nearest line above that is not blank is: `comment`, `head` (code that ends in `:`), `doc` (the rule that closes a documentation block), `code`, or null for none |
| `module`, `import` | `name`; `from` and `names` |
| `facet` | `name`, `decls` |
| `constants`, `service` | `name`, `items`: `constant` or `setting` nodes with `name`, `type` (or null) and `value` |
| `type_decl` | `name`, `type` (what it refines, or null), `where` (clauses), `cases` (`case` nodes with `name` and `fields`) |
| `structure`, `schema` | `name`, `members`: `field` (`name`, `key`, `type`), `derived` (`name`, `type`, `value`), `invariant` (`name`, `condition`); a schema has its `version` |
| `migrate` | `name`, `from`, `to`, `body` |
| `function` | `name`, `params` (`param` nodes: `name`, `injected`, `mutable`, `type`), `returns`, `fails` (a list of names), `where` and `ensures` (clauses: `name`, `condition`), `body` |
| `rule` | `name`, `params`, `value` |
| `test` | `name`, `covers` (a `path` node with its `text` and its `steps`, or null), `sections` (`section` nodes: `name` is `given`, `inject`, `when` or `then`, and `body`) |
| `doc` | `text`, its lines without the rules' indentation; `entries`: a line that starts with a name and `: ` (`game: The game, as game.`, `fails guess_error.empty: …`), with the indented lines that continue it, as `doc_entry` nodes with `name`, `text`, `end`, and `type`, the type after its last word `as`, or null when it does not end in `as type.` |
| `let` | `name`, `mutable`, `value`: `a = …`, `mutable a = …`, and a test's `then:` condition |
| `assign`, `expr` | `target` and `value` (`a.b := …`); `value` (a statement that is an expression) |
| `if`, `while` | `name`, `condition`, `body`, `else` |
| `for` | `name`, `iterable`, `body`, `else` |
| `else` | `name`, `body` (empty for a loop's `else name` without a colon) |
| `conditional` | `label`, `subject` (or null), `conditions` (`condition` nodes: `name` such as `judged.hanged`, `test`, `body`), `else` |
| `compare` | `label`, `left`, `right`, `conditions` (each `test` is `equal`, `less` or `greater`), `else` |
| `region` | `name`, `constraints`, `body`, `orelse` (the regions of its `orelse region` lines) |
| `constraint` | `word`, its first word; `text`, as written; and for some words more: `waits` has `name` and `condition`, `within` has `bound` and `or_fail`, `detach` has `because`, `isolation` has `level`, a call such as `supervise(…)` has `args` |
| `arena`, `task`, `inject` | `name`, `body`; `name`, `value`; `bindings` (`binding` nodes: `name`, `value`), `body` |
| `return`, `fail` | `name` and `value`, both optional; `value`, optional |
| `continue`, `break`, `retry` | none |
| `name`, `member`, `call`, `index` | `name`; `object`, `name`; `callee`, `args` (`arg` nodes: `name` or null, `value`); `object`, `index` |
| `binary`, `unary` | `op`, `left`, `right`; `op` (`not` or `-`), `operand` |
| `literal` | `type` (`integer`, `decimal`, `duration`, `string`, `character`, `boolean`, `path`, `pattern`, `url`), `text` as written, and for a string or a character its `value`, the text between the quotes |
| `list` | `items` |
| (any expression) | `parens`, true when it is written in brackets, `(a or b)`; the tree has no node for brackets |
| `or_fail`, `forever` | `value` (`e or fail`); `value`, `because` (`e forever because "…"`) |
| `mutable`, `fake`, `compile` | `value`: an argument `mutable g`, a test's `fake store(…)`, `main`'s `compile f(…)` |
| `type` | `name`, `args` (`type_arg` nodes: `name`, `value`, a type or a literal, and `text`, the value as written), `text` as written |

## How lines become a tree

Lines nest by their indentation into blocks; a line that ends in `:` opens one. A line goes on over the next lines
while a bracket is open, and also while it does not end in `:` and the next line that is not blank or a comment is
indented deeper than its first line. So a function's `where` and `ensures` lines belong to its head, and a condition
broken after `and` belongs to its line. A line of three or more dashes opens a documentation block, and the next such
line closes it; the block belongs to the declaration below it. A `--` outside a string starts a comment.

A statement is known by its first word. An `else` joins the `if`, `for` or `while` before it in the same block, and an
`orelse region` the region before it; a `conditional` or a `compare` holds its own `condition` and `else` lines, each
named after its label. Where a keyword is used as a name (`forever = system.forever()`, `random.index_below(…)`), the
word after it decides: a keyword followed by `=`, `:=`, `.`, `(` or `[` starts no statement, except `inject(…):`.

## Forms the proposal does not fix

The examples write these, and the parser accepts them as written:

- `service openmoji:` and its settings, `address = url"…"` (memory): the proposal has `http<service=…>` and the
  prefix `url"…"`, but no declaration of a service, and `service` is not among its reserved words.
- `else name` without a colon or a block, for a loop that may never run: the loop's outcome that it did not run.
- A test's `covers` path is kept as its text and its steps split at `>`; a step such as `row'many'1lt` is not parsed
  further. The proposal says a test covers at most one path, and a second is an error.
- A string's `value` is its text between the quotes: escapes and `{…}` interpolations are not interpreted.
- A region's constraints are kept as written, besides the parts listed above: `regen.py` makes the region subject's
  clauses from that text.
- A `rule` is one expression after `=`.
- A documentation entry is any line of a documentation block that starts with one name and `: `, or with `fails` and
  one name and `: `; other lines are prose. A prose line that starts that way is taken for an entry.

## What it does not handle yet

The proposal has forms that no example uses, and the parser rejects them, or reads them as ordinary names and calls
(`old(x)` in an `ensures`): `enumeration`, `state` and `transition`; `trait` and `implementation`; `unit`; `command`,
`handler` and `workflow`; `transient` and `me`; `old` and `assert`; `lambda`, `nested` and `captures`; `enqueue` and
`submit`; `native because "…"` in place of a body, and `native` types; `assume`, `allow` and `reveal`; a test's
`faults`; `log` and `metric`; a collection's `ordered by`, `unique` and the other words of its declaration; `policy`
and `compensate`. It does not check that a name is a reserved word used where only that word may stand, nor the layout
and lint rules of § 3.1, § 3.2 and § 3.5, which `tools/bare-rust/tests/y_examples.rs` checks on the tree. It does
refuse two forms the lint rules forbid, with an error of their own: an `else if` ladder, and an `else` without a name.

## Who uses it

- `tools/bare-rust/tests/y_levels/mod.rs`, which infers each facet's level from the Y source to check the pinned
  levels, reads the tree.
- `tools/bare-rust/tests/y_examples.rs` reads the tree for the regions (each has its subject, and the bare body marks
  the end of one that statements follow) and for the decisions a module names.
- `tools/paths/regen.py` reads it, through `tools/paths/ytree.py`, for the functions, rules and derived values and
  their facets, their `where` and `ensures` clauses, invariants, `injected` parameters, and regions.
- `tools/paths/pathfacts.py` reads it for the decisions of each function, the functions with injected capabilities,
  and the tests and the path each covers; `tools/paths/places.py` finds the Y places of the other facts in it (a
  facet, a function's head, a claim, a call that may suspend, a `compile`, a unit's constant or constraint).
- The path enumerator, `tools/paths/yp.py`, and its Rust port `tools/bare-rust/tests/y_paths/mod.rs`, read every
  declaration, statement and expression of a module from the tree. They keep facts on expressions by a text they
  write from the nodes, `game.misses.count()`, and the brackets the source writes (`parens`) decide how `and` and
  `or` split.
- `tools/bare-rust/tests/y_examples.rs` checks the layout rules of § 3.1 on the blank lines and comments the tree
  records, and its lint rules on the tree: the roles of a documentation block grouped by kind, no brackets around a
  single element, a name changed only when it is `mutable`. It reads each test's `when` from the tree, to check that
  the test calls the function whose path it covers, and finds there the node at each place a fact in `y-facts.json`
  gives.
- `tools/paths/trace.py` reads the tests and the path each claims to cover from the test file's tree.
- The lint `type_argument_name`, `tools/paths/ylint.py` and its twin `tools/bare-rust/tests/y_lint/mod.rs`, reads the
  type arguments in the code and in the types documentation entries end with, the declarations, and the comments that
  suppress it.

No tool reads Y as lines any more. Where a fact must be found again by what it said, and no node says it,
`pathfacts.py` falls back to the text the fact's line had in git's `HEAD` (`places.by_text`).
