# PeekingTakeWhile: an iterator adapter with lifetimes

Features: `impl Iterator` for a struct borrowing `&'a mut I`, lifetime parameters, `FnOnce`/`FnMut` closure bounds, a trait implemented for `&mut I`, `Peekable`, `vec::IntoIter`, and `slice::Iter<'a, T>`.

- Source: <https://github.com/rust-itertools/itertools/blob/1681e073235694595912283f65fd1d9cfe3492d5/src/peeking_take_while.rs> (raw: <https://raw.githubusercontent.com/rust-itertools/itertools/1681e073235694595912283f65fd1d9cfe3492d5/src/peeking_take_while.rs>), repository `rust-itertools/itertools` at commit `1681e073235694595912283f65fd1d9cfe3492d5`.
- Licence: MIT OR Apache-2.0 (itertools `Cargo.toml`). The copies in this folder are `LICENSE-MIT` and `LICENSE-APACHE`, fetched from the same commit. The MIT file reads "Copyright (c) 2015" and does not name a holder; the crate's `Cargo.toml` lists the author `bluss`, and the code is by the itertools contributors. The MIT notice and permission text are kept in `LICENSE-MIT`, as the licence requires; `src/lib.rs` is a modified copy, so this README also states the changes.

## What was changed from the original

- Kept `PeekingNext` and its impls for `&mut I`, `Peekable<I>` and `vec::IntoIter<T>`, plus `PeekingTakeWhile` and its `Iterator` and `PeekingNext` impls.
- Removed the impls for `PutBack`, `PutBackN`, `RepeatN`, `Drain`, `array::IntoIter`, the `use_alloc`/`use_std` cfg-gated impls, the `Debug` impl (it used the crate's `debug_fmt_fields!` macro) and the doc intra-links.
- Replaced the `peeking_next_by_clone!` macro and its ~20 invocations by its expansion for `slice::Iter<'a, T>` only (the macro body, written out by hand, with the same text).

The result compiles on its own (`cargo check` in this folder) and is `src/lib.rs`.

## Files

- `Cargo.toml`, `src/lib.rs`: the original as a standalone crate.
- `bare/src/lib.brs`, `bare/src/lib.constraints.json`, `bare/refusals.json`: produced by `cargo run -q -- strip examples/03-peeking-take-while --out examples/03-peeking-take-while/bare` (run in `tools/bare-rust`). `tests/examples.rs` keeps them current.
- `LICENSE-*`: the licence texts from the source commit.

## What is interesting in the bare/constraint split

- The lifetime `'a` of `PeekingTakeWhile<'a, I, F>` appears in the bare file as `struct PeekingTakeWhile` and as `generics.lifetime` in the constraints; the field `iter: &'a mut I` has its reference and exclusive access in constraints (`fields.access`).
- All closure bounds (`F: FnOnce(&Self::Item) -> bool`, `F: FnMut(&I::Item) -> bool`) are in the constraints; the bodies that call `accept(r)` or `f(r) && g(r)` stay as host text.
- `impl<I> PeekingNext for &mut I` becomes `impl PeekingNext for I` in the bare file and the real Self type `&mut I` is a `host` `ref_self` entry in constraints (the one host-text type slot).
- 17 bounds are kept as host text (`bounds (host text)`), the most of any example, because closure and trait-projection bounds are not in the typed vocabulary.
