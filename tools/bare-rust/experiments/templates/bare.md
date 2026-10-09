# Task @TASK@ (crate `@CRATE@`, trial @TRIAL@)

You are a maintainer of a Rust crate that is stored in a "bare" form. Resolve the following issue.

## Issue

@STATEMENT@

## The bare format (everything you need)

There are no `.rs` files in this directory, and you must never create or edit any under the crate. The crate is kept as pairs:

- `X.brs`: Rust *bodies*. Statements, expressions, `use` lines, doc comments, attributes and the shape of items are ordinary Rust, but the parts a type checker can work out are left out. Function signatures lose their reference marks and lifetimes (`fn finish(self)`, `fn write(self, bytes)`), and parameter types, return types, field types and `mut` are mostly absent. Where a type is not written, it lives in the constraint file.
- `X.constraints.json`: the facts about each item: for every `subjects[]` entry (functions, types, impls, aliases, consts) the generics and bounds, the `params` (name, `type`, and `access`: `rights` read/write and `guarantee` readers/exclusive, which become `&`, `&mut`, or by value), the `result` type, visibility (`visible`), and any `clauses` (promises such as `ensure`, `effects`, `never panics`). A new function needs a new subject here **and** a body in the `.brs` file; a new parameter or a type change is an edit of the JSON; a change to what a function does is usually an edit of the `.brs` only. Types are written in a small notation: `u64`, `ref<list<u8> as 'std::primitive::slice'>` (a reference to a slice), `ref<str>`, `f64`, `list<T> as 'Vec'`, `T?` for an option, and so on. Copy the notation of neighbouring entries. Names are `crate::path::item`; impl methods are `crate::Type as Trait::method` or `crate::Type::method`.
- `weave.sh`: run `./weave.sh` to turn the pair into a normal crate in `.weave/` (it prints errors from the weaver and from `cargo check`; holes the weaver cannot fill, such as an unsure type or `mut`, are reported, and you resolve them by adding an annotation in the body or a type in the JSON). Run `./weave.sh test` to also run `cargo test` in `.weave/`. The tests in `tests/` are included; to add tests of your own, put plain Rust files in `extra-tests/*.rs` (they use the public API, e.g. `use @CRATE@::*;`). Always edit the `.brs` and `.constraints.json`, never `.weave/`.
- `Cargo.toml` and other non-`.rs` files are ordinary.

Optional reading, only if you get stuck: the format is defined by `/home/user/y/proposal/constraint-language.md` and `/home/user/y/proposal/bare-rust.md`. You should not need them.

## Library files

`libraries/std-1.97.0.constraints.json` is a constraint file for the parts of the standard library this crate uses (same format as above: `subjects[]` with `params`, `result` and `clauses`). Its clauses are promises the standard library makes, for example what `HashMap::get`, `str::chars` or `Iterator::count` guarantee and when they panic. Use it instead of guessing std behaviour; it is large, so search it (`jq`, `grep`) rather than reading it all. Clauses of kind `unexpressed` are promises that the constraint language cannot yet state formally; their `text` says them in prose.

Your change will be checked by tests you cannot see: the pair is woven and the result is compiled and tested against the public API described in the issue.

## When you are done

Finish your final message with a single line of JSON, and nothing after it:

```
{"done":true,"attempts":3,"json_edits":2,"library_promises_relied_on":["std::collections::HashMap::get"],"notes":"one or two sentences"}
```

- `done`: true if you believe the task is complete and the code weaves, compiles and works.
- `attempts`: how many times you ran `./weave.sh` or `cargo` in total.
- `json_edits`: how many edits you made to `.constraints.json` files (count each added, removed or changed entry or field once).
- `library_promises_relied_on`: the ids or names (`subjects[].name`, e.g. `std::collections::HashMap::get`) of library-file entries whose promises you used to decide something; `[]` if none.
- `notes`: anything surprising, in one or two sentences.

Do not look for, ask about, or try to reconstruct hidden tests. Work only inside this directory.
