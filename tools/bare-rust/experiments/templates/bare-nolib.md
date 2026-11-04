# Task @TASK@ (crate `@CRATE@`, trial @TRIAL@)

You are a maintainer of a Rust crate that is stored in a "bare" form. Resolve the following issue.

## Issue

@STATEMENT@

## The bare format (everything you need)

There are no `.rs` files in this directory, and you must never create or edit any under the crate. The crate is kept as pairs:

- `X.brs`: Rust *bodies*. Statements, expressions, `use` lines, doc comments, attributes and the shape of items are ordinary Rust, but the parts a type checker can work out are left out. Function signatures lose their reference marks and lifetimes (`fn finish(self)`, `fn write(self, bytes)`), and parameter types, return types, field types and `mut` are mostly absent. Where a type is not written, it lives in the constraint file.
- `X.constraints.json`: the facts about each item: for every `subjects[]` entry (functions, types, impls, aliases, consts) the generics and bounds, the `params` (name, `type`, and `access`: `rights` read/write and `guarantee` readers/exclusive, which become `&`, `&mut`, or by value), the `result` type, visibility (`visible`), and any `clauses` (promises such as `ensure`, `effects`, `never panics`). A new function needs a new subject here **and** a body in the `.brs` file; a new parameter or a type change is an edit of the JSON; a change to what a function does is usually an edit of the `.brs` only. Types are written in a small notation: `u64`, `ref<target type=list<element type=u8> as 'std::primitive::slice'>` (a reference to a slice), `ref<target type=str>`, `f64`, `list<element type=T> as 'Vec'`, `T?` for an option, and so on. Copy the notation of neighbouring entries. Names are `crate::path::item`; impl methods are `crate::Type as Trait::method` or `crate::Type::method`.
- `weave.sh`: run `./weave.sh` to turn the pair into a normal crate in `.weave/` (it prints errors from the weaver and from `cargo check`; holes the weaver cannot fill, such as an unsure type or `mut`, are reported, and you resolve them by adding an annotation in the body or a type in the JSON). Run `./weave.sh test` to also run `cargo test` in `.weave/`. The tests in `tests/` are included; to add tests of your own, put plain Rust files in `extra-tests/*.rs` (they use the public API, e.g. `use @CRATE@::*;`). Always edit the `.brs` and `.constraints.json`, never `.weave/`.
- `FORMAT.md`: a one-page reference with one example (Rust, bare body, JSON entry) for each kind of item: free function, methods with `&self` and `&mut self`, generic function with bounds, trait impl, type alias, struct, enum, and local and cast hints. Read it before you add or change an item, and copy its entries rather than inventing the shape.
- New item: `./weave.sh scaffold <fn|impl|alias|struct> <path> [options] --write .` appends a bare stub to the right `.brs` and its entry to the matching `.constraints.json`, with `TODO` where a type goes (for example `./weave.sh scaffold fn util::Counter::peek --self ref --params key --write .`); replace each `TODO`, since the weave refuses a leftover one. Without `--write` it only prints them; `./weave.sh scaffold` with no arguments lists the options.
- The error messages name the fix: a `let x: T` or `type X = T;` in a `.brs` is refused with the `holes` or alias entry to add, and a weave that says `slot missing: no subject ...` ends with a ready-to-paste entry for the item.
- `Cargo.toml` and other non-`.rs` files are ordinary.

Optional reading, only if you get stuck: the format is defined by `/home/user/y/proposal/constraint-language.md` and `/home/user/y/proposal/bare-rust.md`. You should not need them.

There is no library information beyond what rustc tells you; the standard library's own behaviour is yours to know or to check by running code.

Your change will be checked by tests you cannot see: the pair is woven and the result is compiled and tested against the public API described in the issue.

## When you are done

Finish your final message with a single line of JSON, and nothing after it:

```
{"done":true,"attempts":3,"json_edits":2,"library_promises_relied_on":[],"notes":"one or two sentences"}
```

- `done`: true if you believe the task is complete and the code weaves, compiles and works.
- `attempts`: how many times you ran `./weave.sh` or `cargo` in total.
- `json_edits`: how many edits you made to `.constraints.json` files (count each added, removed or changed entry or field once).
- `library_promises_relied_on`: always `[]` for this crate (no library files are provided).
- `notes`: anything surprising, in one or two sentences.

Do not look for, ask about, or try to reconstruct hidden tests. Work only inside this directory.
