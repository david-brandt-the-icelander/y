# Examples

Six short pieces of real, permissively licensed Rust (MIT, or MIT OR Apache-2.0), each pinned to a commit, with the stripped bare file and constraint file the tool produces for it. Each folder is a standalone crate (`Cargo.toml`, `src/lib.rs`), `bare/src/lib.brs` and `bare/src/lib.constraints.json` (and `bare/refusals.json`, which is empty), the licence texts, and a README with the source, what was trimmed and what is interesting in the split.

| Folder | Source (commit) | Features shown | Lines: original / bare / constraints | Host text | Constraints |
|---|---|---|---|---|---|
| `01-lazy-buffer` | itertools `src/lazy_buffer.rs` (`1681e07`) | generic container, three impls with different bounds, generic `Index` | 91 / 82 / 180 | 1 of 32 type slots; 8 bounds | 71 slots on 16 subjects |
| `02-intersperse` | itertools `src/intersperse.rs` (`1681e07`) | trait with several impls, blanket impl for closures, aliases, `Iterator` | 168 / 153 / 199 | 1 of 41 type slots; 11 bounds | 90 slots on 21 subjects |
| `03-peeking-take-while` | itertools `src/peeking_take_while.rs` (`1681e07`) | iterator adapter, lifetimes, closure bounds, trait on `&mut I` | 139 / 113 / 188 | 1 of 30 type slots; 16 bounds | 79 slots on 16 subjects; 8 macro items kept as host text |
| `04-human-size` | ripgrep `crates/cli/src/human.rs` (`3fce3b5`) | error enum, `Display`, `From`, string parsing | 149 / 149 / 105 | 0 of 20 type slots; 0 bounds | 34 slots on 19 subjects |
| `05-interpolate` | ripgrep `crates/matcher/src/interpolate.rs` (`3fce3b5`) | string processing, callbacks, lifetimes | 142 / 140 / 123 | 1 of 22 type slots; 2 bounds | 43 slots on 9 subjects |
| `06-sources` | itertools `src/sources.rs` (`1681e07`) | closures stored in structs, `Iterator`, `Debug` | 95 / 81 / 177 | 0 of 28 type slots; 6 bounds | 68 slots on 15 subjects |

The counts come from `cargo run -q -- stats examples/<name>/bare`. "Host text" is the number of type slots the tool left as host text (`types.host_text` of that slot total) and the number of generic or `where` bounds kept as host text (`bounds (host text)`); function bodies are always host text and are counted in the bare file's lines. "Constraints" is the total of `slots_filled` and the number of subjects (functions, impls, types, traits, aliases) they are attached to.

## Checks

For every example: `strip` succeeds with no refusals; `weave` of the bare and constraint files followed by `cargo check` compiles (0 errors, 0 holes undetermined); `cargo run -q -- check examples/<name> --check-only` passes checks 1 to 4 (check 5 is not applicable, there is no library file). `tests/examples.rs` strips every example, compares the output with the committed `bare/` files, and runs checks 1 and 2 without cargo on them.

Refresh the committed files after a change to the tool with `cargo run -q -- strip examples/<name> --out examples/<name>/bare`.

## Tool problems met while choosing snippets

All four problems met while choosing the snippets are fixed, and the pieces they forced out are back in the examples:

- Const generics (`get_array<const K: usize>` in `01-lazy-buffer`) are carried as a generic named `const K` with the host-text bound `usize`.
- A named lifetime inside a trait argument (`impl<'a> From<&'a str> for Ref<'a>` in `05-interpolate`) is kept: the whole trait path is a `host` entry (form `ref_trait`) in the constraints. The one host-text type slot of `05-interpolate` is that entry.
- Item-level macro invocations (`peeking_next_by_clone! { ... }` in `03-peeking-take-while`, with its `macro_rules!` definition) stay verbatim in the bare file, with no constraints extracted; `stats` counts them as "macro items kept as host text" (8 in that example). The impls the macro expands to are therefore not seen by the tool.
- `weave <crate-dir>` for a crate whose stripped files sit in `<crate-dir>/bare/` now weaves them at their own paths (`src/lib.rs`), instead of reporting "1 files woven, 1 not woven".

Each `Cargo.toml` has an empty `[workspace]` table so the examples do not join another workspace, and `examples/.gitignore` ignores `*/target/` and `*/Cargo.lock`. `bare-rust`'s own `Cargo.toml` sets `autoexamples = false`.
