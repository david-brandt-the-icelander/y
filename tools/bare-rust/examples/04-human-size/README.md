# Parse a human readable size: error enum, From and Display

Features: an error struct with a private enum kind, `Display`, `Error`, `From<ParseSizeError> for io::Error`, `?`-style `map_err`.

- Source: <https://github.com/BurntSushi/ripgrep/blob/3fce3b5bb0236da2df6d99672afb8a719642eca7/crates/cli/src/human.rs> (raw: <https://raw.githubusercontent.com/BurntSushi/ripgrep/3fce3b5bb0236da2df6d99672afb8a719642eca7/crates/cli/src/human.rs>), repository `BurntSushi/ripgrep` at commit `3fce3b5bb0236da2df6d99672afb8a719642eca7`.
- Licence: Unlicense OR MIT (ripgrep `Cargo.toml`); used here under the MIT licence. Copyright (c) 2015 Andrew Gallant (`LICENSE-MIT` in this folder, fetched from the same commit). The MIT notice and permission text are kept in `LICENSE-MIT`, as the licence requires; where `src/lib.rs` is a modified copy, the changes are stated below.

## What was changed from the original

- Not trimmed: the file is verbatim, including its unit tests.

The result compiles on its own (`cargo check` in this folder) and is `src/lib.rs`.

## Files

- `Cargo.toml`, `src/lib.rs`: the original as a standalone crate.
- `bare/src/lib.brs`, `bare/src/lib.constraints.json`, `bare/refusals.json`: produced by `cargo run -q -- strip examples/04-human-size --out examples/04-human-size/bare` (run in `tools/bare-rust`). `tests/examples.rs` keeps them current.
- `LICENSE-*`: the licence texts from the source commit.

## What is interesting in the bare/constraint split

- `ParseSizeErrorKind`'s variants are in the bare file as names only (`InvalidFormat`, `InvalidInt`, `Overflow`); the payload type `std::num::ParseIntError` is a `variants` entry in constraints.
- `impl From for std::io::Error`: the converted-from type `ParseSizeError` is a constraint (`impl.trait` slot), so the bare file keeps `fn from(size_err)` with no types at all.
- All string text (the `Display` format strings) and the logic of `parse_human_readable_size` stay as host text; 11 macro invocations (`write!`, `assert_eq!` in the tests) stay in bodies.
- `result.fails` records `Result<u64, ParseSizeError>`'s failure type, and one hole hint (`targ`) is a `parse::<u64>()` turbofish.
