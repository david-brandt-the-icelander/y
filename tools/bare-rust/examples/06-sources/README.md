# Unfold, Iterate: iterator sources built from closures

Features: structs holding closures `F: FnMut(&mut St) -> Option<A>` and `F: FnMut(&St) -> St`, constructor functions, `Iterator` impls, `fmt::Debug` impls.

- Source: <https://github.com/rust-itertools/itertools/blob/1681e073235694595912283f65fd1d9cfe3492d5/src/sources.rs> (raw: <https://raw.githubusercontent.com/rust-itertools/itertools/1681e073235694595912283f65fd1d9cfe3492d5/src/sources.rs>), repository `rust-itertools/itertools` at commit `1681e073235694595912283f65fd1d9cfe3492d5`.
- Licence: MIT OR Apache-2.0 (itertools `Cargo.toml`). The copies in this folder are `LICENSE-MIT` and `LICENSE-APACHE`, fetched from the same commit. The MIT file reads "Copyright (c) 2015" and does not name a holder; the crate's `Cargo.toml` lists the author `bluss`, and the code is by the itertools contributors. The MIT notice and permission text are kept in `LICENSE-MIT`, as the licence requires; `src/lib.rs` is a modified copy, so this README also states the changes.

## What was changed from the original

- Removed `#![allow(deprecated)]`, the `#[deprecated(...)]` attributes on `unfold`/`Unfold`, the long doc examples (they call `itertools::...`), and the `debug_fmt_fields!` macro use: the two `Debug` impls are written out by hand with `f.debug_struct(...).field("state", &self.state).finish()`.

The result compiles on its own (`cargo check` in this folder) and is `src/lib.rs`.

## Files

- `Cargo.toml`, `src/lib.rs`: the original as a standalone crate.
- `bare/src/lib.brs`, `bare/src/lib.constraints.json`, `bare/refusals.json`: produced by `cargo run -q -- strip examples/06-sources --out examples/06-sources/bare` (run in `tools/bare-rust`). `tests/examples.rs` keeps them current.
- `LICENSE-*`: the licence texts from the source commit.

## What is interesting in the bare/constraint split

- The closure type parameter `F` and its bound `FnMut(&mut St) -> Option<A>` live in constraints (`generics.type`, 18 slots); the struct body in the bare file is `struct Unfold { f, state }`.
- `fn unfold(initial_state, f) -> ...`: the bare signature has parameter names only; the return type `Unfold<St, F>` and the bounds are constraints.
- `next` bodies, `(self.f)(&mut self.state)` and `mem::replace(&mut self.state, next_state)`, are host text; `&mut` there is an ordinary expression, not a type, so it stays.
- No type slot of this example is host text, and no hole hints were needed.
