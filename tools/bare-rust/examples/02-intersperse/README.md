# IntersperseWith: a trait with several implementations

Features: a public trait with a blanket impl for closures, a struct impl, type aliases, an `Iterator` impl with provided-method overrides.

- Source: <https://github.com/rust-itertools/itertools/blob/1681e073235694595912283f65fd1d9cfe3492d5/src/intersperse.rs> (raw: <https://raw.githubusercontent.com/rust-itertools/itertools/1681e073235694595912283f65fd1d9cfe3492d5/src/intersperse.rs>), repository `rust-itertools/itertools` at commit `1681e073235694595912283f65fd1d9cfe3492d5`.
- Licence: MIT OR Apache-2.0 (itertools `Cargo.toml`). The copies in this folder are `LICENSE-MIT` and `LICENSE-APACHE`, fetched from the same commit. The MIT file reads "Copyright (c) 2015" and does not name a holder; the crate's `Cargo.toml` lists the author `bluss`, and the code is by the itertools contributors. The MIT notice and permission text are kept in `LICENSE-MIT`, as the licence requires; `src/lib.rs` is a modified copy, so this README also states the changes.

## What was changed from the original

- Replaced `use super::size_hint` by an inline `mod size_hint` holding `add`, `add_scalar` and `sub_scalar` (bodies copied from `src/size_hint.rs` at the same commit, doc comments dropped).
- Doc comments contain intra-doc links to `crate::Itertools`, which does not exist here; they were left as text because they do not affect compilation.

The result compiles on its own (`cargo check` in this folder) and is `src/lib.rs`.

## Files

- `Cargo.toml`, `src/lib.rs`: the original as a standalone crate.
- `bare/src/lib.brs`, `bare/src/lib.constraints.json`, `bare/refusals.json`: produced by `cargo run -q -- strip examples/02-intersperse --out examples/02-intersperse/bare` (run in `tools/bare-rust`). `tests/examples.rs` keeps them current.
- `LICENSE-*`: the licence texts from the source commit.

## What is interesting in the bare/constraint split

- `trait IntersperseElement<Item>` and its two impls (`IntersperseElementSimple<Item>` and the blanket `impl<Item, F: FnMut() -> Item> ... for F`) show how the same trait is bound twice: the bare file has `impl IntersperseElement for F` and the bound `F: FnMut() -> Item` is in constraints.
- The type aliases `Intersperse<I>` and `IntersperseElementSimple` keep only their names in the bare file; their right-hand sides are constraints.
- `fold`, `size_hint` and `next` keep their bodies, including the destructuring of `Self { element, iter, peek }`, as host text; their `where` clauses (`F: FnMut(B, Self::Item) -> B`) moved out.
- The empty `FusedIterator` impl with its bounds shows an impl whose whole content is constraints.
