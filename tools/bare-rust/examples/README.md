# Examples

Six short pieces of real, permissively licensed Rust (MIT, or MIT OR Apache-2.0), each pinned to a commit, with the stripped bare file and constraint file the tool produces for it. Each folder is a standalone crate (`Cargo.toml`, `src/lib.rs`), `bare/src/lib.brs` and `bare/src/lib.constraints.json` (and `bare/refusals.json`, which is empty), the licence texts, and a README with the source, what was trimmed and what is interesting in the split.

| Folder | Source (commit) | Features shown | Lines: original / bare / constraints | Host text | Constraints |
|---|---|---|---|---|---|
| `01-lazy-buffer` | itertools `src/lazy_buffer.rs` (`1681e07`) | generic container, three impls with different bounds, generic `Index` | 87 / 78 / 162 | 1 of 29 type slots; 7 bounds | 64 slots on 15 subjects |
| `02-intersperse` | itertools `src/intersperse.rs` (`1681e07`) | trait with several impls, blanket impl for closures, aliases, `Iterator` | 168 / 153 / 199 | 1 of 41 type slots; 11 bounds | 90 slots on 21 subjects |
| `03-peeking-take-while` | itertools `src/peeking_take_while.rs` (`1681e07`) | iterator adapter, lifetimes, closure bounds, trait on `&mut I` | 123 / 95 / 208 | 1 of 34 type slots; 17 bounds | 88 slots on 18 subjects |
| `04-human-size` | ripgrep `crates/cli/src/human.rs` (`3fce3b5`) | error enum, `Display`, `From`, string parsing | 149 / 149 / 105 | 0 of 20 type slots; 0 bounds | 34 slots on 19 subjects |
| `05-interpolate` | ripgrep `crates/matcher/src/interpolate.rs` (`3fce3b5`) | string processing, callbacks, lifetimes | 128 / 126 / 87 | 0 of 22 type slots; 2 bounds | 32 slots on 5 subjects |
| `06-sources` | itertools `src/sources.rs` (`1681e07`) | closures stored in structs, `Iterator`, `Debug` | 95 / 81 / 177 | 0 of 28 type slots; 6 bounds | 68 slots on 15 subjects |

The counts come from `cargo run -q -- stats examples/<name>/bare`. "Host text" is the number of type slots the tool left as host text (`types.host_text` of that slot total) and the number of generic or `where` bounds kept as host text (`bounds (host text)`); function bodies are always host text and are counted in the bare file's lines. "Constraints" is the total of `slots_filled` and the number of subjects (functions, impls, types, traits, aliases) they are attached to.

## Checks

For every example: `strip` succeeds with no refusals; `weave` of the bare and constraint files followed by `cargo check` compiles (0 errors, 0 holes undetermined); `cargo run -q -- check examples/<name> --check-only` passes checks 1 to 4 (check 5 is not applicable, there is no library file). `tests/examples.rs` strips every example, compares the output with the committed `bare/` files, and runs checks 1 and 2 without cargo on them.

Refresh the committed files after a change to the tool with `cargo run -q -- strip examples/<name> --out examples/<name>/bare`.

## Tool problems met while choosing snippets

- Const generics are refused by `strip` (`const-generics: const K: usize`); `lazy_buffer.rs` had `get_array<const K: usize>`, which was removed from the example.
- Weave drops a named lifetime inside a trait argument: `impl<'a> From<&'a str> for Ref<'a>` is woven as `impl<'a> From<&str> for Ref<'a>`, and the woven code fails `cargo check` with E0308 (method not compatible with trait); checks 2, 3 and 4 fail. The impl, which was only used by tests, was removed from `05-interpolate`.
- Macro-defined impls are not handled, so `peeking_next_by_clone!` was replaced by its expansion for one type.
- `weave <dir>` prints "1 files woven, 1 not woven" for a single-file crate; the file does weave and compile, so this looks like a counting message only.

Each `Cargo.toml` has an empty `[workspace]` table so the examples do not join another workspace, and `examples/.gitignore` ignores `*/target/` and `*/Cargo.lock`. `bare-rust`'s own `Cargo.toml` sets `autoexamples = false`.
