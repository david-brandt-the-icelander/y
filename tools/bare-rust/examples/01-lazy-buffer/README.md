# LazyBuffer: a generic container over an iterator

Features: generic struct with associated-type fields, three impl blocks with different bounds, a generic `Index` impl.

- Source: <https://github.com/rust-itertools/itertools/blob/1681e073235694595912283f65fd1d9cfe3492d5/src/lazy_buffer.rs> (raw: <https://raw.githubusercontent.com/rust-itertools/itertools/1681e073235694595912283f65fd1d9cfe3492d5/src/lazy_buffer.rs>), repository `rust-itertools/itertools` at commit `1681e073235694595912283f65fd1d9cfe3492d5`.
- Licence: MIT OR Apache-2.0 (itertools `Cargo.toml`). The copies in this folder are `LICENSE-MIT` and `LICENSE-APACHE`, fetched from the same commit. The MIT file reads "Copyright (c) 2015" and does not name a holder; the crate's `Cargo.toml` lists the author `bluss`, and the code is by the itertools contributors. The MIT notice and permission text are kept in `LICENSE-MIT`, as the licence requires; `src/lib.rs` is a modified copy, so this README also states the changes.

## What was changed from the original

- Removed `use alloc::vec::Vec` (std's prelude has `Vec`) and `use crate::size_hint::{self, SizeHint}`; the one helper used (`size_hint::add_scalar` and the `SizeHint` alias) is copied from `src/size_hint.rs` at the same commit into an inline `mod size_hint`.
- `get_array<const K: usize>` is kept (const generics are carried as host text).

The result compiles on its own (`cargo check` in this folder) and is `src/lib.rs`.

## Files

- `Cargo.toml`, `src/lib.rs`: the original as a standalone crate.
- `bare/src/lib.brs`, `bare/src/lib.constraints.json`, `bare/refusals.json`: produced by `cargo run -q -- strip examples/01-lazy-buffer --out examples/01-lazy-buffer/bare` (run in `tools/bare-rust`). `tests/examples.rs` keeps them current.
- `LICENSE-*`: the licence texts from the source commit.

## What is interesting in the bare/constraint split

- `LazyBuffer<I: Iterator>` has a field of type `Vec<I::Item>`. In the bare file the struct keeps its field names and the types go to constraints (`fields.type`), so the bare text is `struct LazyBuffer { it, buffer }`.
- Generics and their bounds (`I: Iterator`, `I::Item: Clone`, the `where` clause of the `Index` impl) are constraints; the `impl` headers in the bare file are `impl LazyBuffer`, `impl Index for LazyBuffer`.
- The associated-type projections `I::Item` and `<Vec<I::Item> as Index<J>>::Output` are the only type slots left as host text (1 of 29).
- Parameter and result types of every method (`&mut self`, `usize`, `Vec<I::Item>`) moved to constraints, so method bodies are the host text that stays: `self.buffer.push(x)`, `self.it.by_ref().take(delta)`.
