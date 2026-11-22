# Interpolate capture references in a replacement string

Features: a byte-string parser with callback parameters (`FnMut(usize, &mut Vec<u8>)`, `FnMut(&str) -> Option<usize>`), a lifetime-carrying enum and struct.

- Source: <https://github.com/BurntSushi/ripgrep/blob/3fce3b5bb0236da2df6d99672afb8a719642eca7/crates/matcher/src/interpolate.rs> (raw: <https://raw.githubusercontent.com/BurntSushi/ripgrep/3fce3b5bb0236da2df6d99672afb8a719642eca7/crates/matcher/src/interpolate.rs>), repository `BurntSushi/ripgrep` at commit `3fce3b5bb0236da2df6d99672afb8a719642eca7`.
- Licence: Unlicense OR MIT (ripgrep `Cargo.toml`); used here under the MIT licence. Copyright (c) 2015 Andrew Gallant (`LICENSE-MIT` in this folder, fetched from the same commit). The MIT notice and permission text are kept in `LICENSE-MIT`, as the licence requires; where `src/lib.rs` is a modified copy, the changes are stated below.

## What was changed from the original

- Removed `use memchr::memchr` and replaced the one call `memchr(b'$', replacement)` by `replacement.iter().position(|&b| b == b'$')`, so the crate has no dependency.
- Removed the `#[cfg(test)] mod tests` module (about 190 lines, macro-based).

The result compiles on its own (`cargo check` in this folder) and is `src/lib.rs`.

## Files

- `Cargo.toml`, `src/lib.rs`: the original as a standalone crate.
- `bare/src/lib.brs`, `bare/src/lib.constraints.json`, `bare/refusals.json`: produced by `cargo run -q -- strip examples/05-interpolate --out examples/05-interpolate/bare` (run in `tools/bare-rust`). `tests/examples.rs` keeps them current.
- `LICENSE-*`: the licence texts from the source commit.

## What is interesting in the bare/constraint split

- `impl<'a> From<&'a str> for Ref<'a>` has the named lifetime inside the trait argument. The bare header is `impl From for Ref`; the constraint file keeps `From<&'a str>` as a `host` trait entry, the one host-text type slot.

- `interpolate<A, N>` takes two callbacks; their types `A: FnMut(usize, &mut Vec<u8>)` and `N: FnMut(&str) -> Option<usize>` are `bounds` in constraints, while the `while`/`match` loop that calls `append(i, dst)` stays as host text.
- `struct CaptureRef<'a> { cap, end }` and `enum Ref<'a> { Named, Number }` keep names only; the lifetime and the `&'a str` payload are constraints.
- `find_cap_ref` has 2 hole hints (a `cast`, `i as usize` and a `targ`), which are the places where the body needs a type that the bare file does not show.
- All the index arithmetic and byte comparisons (`replacement[i] == b'{'`) are host text; none of them moved.
