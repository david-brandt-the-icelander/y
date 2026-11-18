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
| `injected` parameters | `effects only` the capabilities other than `forever` and `system`, then `no clock` and `no randomness` unless a clock or a random source is among them |

`p'` is the Y condition in the constraint language: `f(x) == result.ok` is `f(x) is Ok(_)`, `x == t.case` is `x is
Case` for a closed type, `x.count()` is `count(x)`, and a constant `group.name` is `NAME`.

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
- **Imports** are the ones the code uses, found in the code; the order of the paths in `y-facts.json` is the
  enumerator's sorted order.
- **Documentation** is made from the Y documentation mechanically: the prose, then the roles, each without its type.
  The hand lowering sometimes reworded it.
- **The file's status** is `assumed` with the reason `lowered from y/src by y-compile`.

## What it does not handle yet

It handles what the examples compiled so far use; the checklist in the pull request says which those are.
