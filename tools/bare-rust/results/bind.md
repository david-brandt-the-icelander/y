# bind: every written type has a name

A cast (`e as u64`) or a suffixed literal (`1u8`) in the middle of an expression states a type with nothing named for
it to belong to. In the bare form that type went into a positional hint. Positional hints are the one binding that E3
found fragile, and they have no equivalent in a host such as TypeScript, where `x as T` is a pure type assertion.

The bare form now gives each of these values a name. `strip` first runs `bind`, a rewrite of the Rust source:

```rust
(h as u64).wrapping_mul(P)          let h_u64: u64 = h as u64;
                                    h_u64.wrapping_mul(P)
```

The local then carries the type: `"h_u64": {"type": "u64", "cast": true}`, with `let h_u64 = h;` in the body. The
round trip is exact on the bound source: `weave(strip(s)) == bind(s)` after `rustfmt`, as proposal § 1 and § 9 now
say. A mid-expression type is treated as an obfuscation that `bind` undoes once.

## What bind does

- **Where the binding goes.**
  - A value is bound just before the statement it is in.
  - In some places the statement would run the value when the source might not, or would run it once where the
    source runs it many times: the right of `&&` or `||`, a match arm or guard, an `else if`, a `while` condition, a
    closure body. There the binding goes into a new block in that place: `c && { let h_u8: u8 = h as u8; h_u8 > 3 }`.
  - A constant context (an array's length, the value of a `const` or `static`) runs once, when the program is built.
    Every constant context allows a block with a `let`, so the binding goes into one in its place:
    `[0; { let n_usize: usize = N as usize; n_usize }]`. A `const` or `static` whose whole value is the cast is the
    name itself, as a `let` is: `const W: usize = N as usize;` is `const W = N;` with `"cast": true` on its subject.
  - A `while let` runs its value each time round, and a let chain (Rust 2024) runs each part only if the parts
    before it matched, and a part may use the names they bind. Each gets a block in its place:
    `if let Some(y) = a && { let n_u8: u8 = n as u8; y > n_u8 }`.
- **Order.** A bound value now runs before the rest of its statement. Anything earlier in the statement that could have
  an effect (a call, an index, a macro) is bound first, in order: `g(1) + g(h) as u32` becomes `let g_v = g(1);` and
  then `let g_u32: u32 = g(h) as u32;`. Rust runs a method's receiver before its arguments, and how it is bound
  depends on what it is:
  - **A value** is bound whole: `v.iter().max().copied().unwrap_or(0).max(a as u8)` becomes
    `let unwrap_or_v = v.iter().max().copied().unwrap_or(0);`, `let a_u8: u8 = a as u8;` and `unwrap_or_v.max(a_u8)`.
  - **A place** (a name, a field, an element, what a reference points to) stays where it is, because binding it would
    move out of it or borrow it for longer. What it computes on the way is bound instead:
    `self.rows[next_row()].push(n as u32)` becomes `let next_row_v = next_row();`, then the cast, then
    `self.rows[next_row_v].push(n_u32)`. A borrow keeps its place the same way: `update(&mut cells[pick()], n as u32)`
    names `pick()`.
  - **An assignment** runs its right before its left (the Rust reference, 8.2.14), so a cast in the left has the right
    bound first. `bind` used to bind it after, which changed the order of `v[f() as usize] = g()`.
  - **A compound assignment** runs its right first when both sides are primitives, and its left first otherwise.
    `bind` cannot see which. So where the left has an effect, a cast in the right is bound in a block in the right's
    place, which runs where the right did either way: `*m.entry(k).or_insert(0) += { let len_u64: u64 = b.len() as
    u64; len_u64 }`. A cast in the left, where the right has an effect, goes into a block in place of the part of
    the left it is in (an index, what is dereferenced): `v[{ let i_usize: usize = i as usize; i_usize }] += next()`.

  `tests/bind.rs` logs every call in such statements, and checks that the bound program logs the same sequence.
- **Already named.** `let n = e as f64;` gets a type, `let n: f64 = e as f64;`, rather than a second name. Brackets
  that weave would not put back are dropped: `(x) as u16` becomes `x as u16`.
- **Macros.** The standard macros' arguments are bound as a call's are: the format family (`format!`, `println!`,
  `write!`, `panic!`, ...), the assertions, `vec!`, `dbg!` and `json!`. While it binds, `bind` writes each such macro
  as a call (`__bm_println_p("{}", x as u8)`, a JSON object as a call with its keys and values in turn), and writes it
  back after. So `println!("{}", x as u8)` becomes `let x_u8: u8 = x as u8;` and `println!("{}", x_u8)`.
  - An argument with a turbofish is named before the macro (`let collect_v = v.iter().collect::<Vec<_>>();`), where
    `strip` names the realization.
  - A binding never goes inside a macro's arguments, where `strip` could not take its type out: an argument with a
    block or a closure that needs a binding is named first, and bound inside after.
  - An argument the macro runs only sometimes (an assertion's message runs only when it fails, every argument of
    `debug_assert!` only in a debug build) is left, and reported.
  - A macro whose syntax the tool does not know, and that states a type, is kept as Rust text in the bare body:
    `rust! { twice!(x as u8) }`, which weaves back to the call. It is never the preferred form.
- **Any cast target.** A cast local carries whatever type the cast writes, not only a primitive or a pointer:
  `x as Word` (a type alias) is `{"type": "Word", "cast": true}`, `&x as &dyn Display` is `{"type": "ref<any
  Display>", "cast": true}`, and `double as fn(u8) -> u8` carries the `fn` pointer as host text. `&x as T` needed one
  fix: `&` binds tighter than `as`, which the tool did not know, so it took the cast for one that needed brackets.
- **Left as hints, and reported by `bare-rust bind`:**
  - a cast or a turbofish in a macro argument that runs only sometimes;
  - a cast holding a value that releases something at its end, where the statement might not run;
  - a suffixed literal under one name with literals of another type.
- **Left as written:** a cast in an enum's discriminant (`A = N as isize`), which stays in the bare body as before.
  A discriminant is not a body that `strip` takes holes from.

`tests/bind.rs` compiles a function with every kind of position, before and after `bind`, runs both on the same
inputs, and checks that they print the same. It also checks the placements above and that a second `bind` changes
nothing.

## Suffixed literals in patterns

A pattern has no `let` to carry a literal's type, and the suffix cannot simply go, because it can decide what the
program computes. In `match s.parse().unwrap() { 200u8 => .. }` the pattern is what makes `parse` produce a `u8`.
`bind` uses the first of these that applies:

- **The value matched is a name declared with that type.** That covers a parameter `x: u8` or a typed `let`, and it
  reaches through `&`, `*` and a tuple's elements: in `match (x, y) { (7u8, 9u16) => .. }` with `x: u8` and `y: &u16`,
  the declarations already say both types, so the suffixes go: `(7, 9)`. The name must be bound once in the function,
  so that the declaration is the one in scope. A cast in an `if let` value is named before the pattern is looked at,
  so `if let -1i8 = x as i8` becomes `let x_i8: i8 = x as i8;` and `if let -1 = x_i8`.
- **A name is already over the literal.** In `n @ 4u8..=6` the name carries the type for every literal under it, and
  each takes the suffix (`n @ 4u8..=6u8`). `strip` keys the type by the name's pattern, as a `let`'s is keyed:
  `"n @ 4..=6": {"literal": "u8"}`, which weaves back to the suffixes.
- **Otherwise the literal gets a name** that does nothing else, together with the range or the alternatives it is part
  of: `Some(1u8)` becomes `Some(_k1_u8 @ 1u8)`, and `Some(2u8 | 3u8)` becomes `Some(_k2_3_u8 @ (2u8 | 3u8))`. The `_`
  says that nothing uses it, and the compiler gives no warning for it. A `let ... else` pattern is treated the same:
  `let Some(0u8) = it.next() else { .. }` becomes `let Some(_k0_u8 @ 0u8) = it.next() else { .. }`.

`bind` also found that a top-level literal pattern of an `if let` or `while let` (`if let -1i8 = x`) was read as an
expression and bound to a `let`, which made the pattern a name that always matches. Patterns there are now patterns.

## Pointers

A cast to a raw pointer (`p as *const u8`) used to be left as a hint, because the language had no type for one. It has
`ptr<T>` now (constraint language § 3.4, § 8.1): the address of a `T`, with no guarantee that it is valid or about any
other path. The slot's `access` gives only rights: `*const T` reads (`{"rights": "read"}`, the table's `read any`),
and `*mut T` has no access, since `write any` constrains nothing. So the cast is an ordinary cast local:
`let p_ptr: *const u8 = p as *const u8;` is `let p_ptr = p;` with
`{"type": "ptr<u8>", "access": {"rights": "read"}, "cast": true}`.

- **Parameters, results, fields and `const`s** take `ptr<T>` too. They used to be host text with the form `pointer`.
  A result keeps its access, since a pointer has no default as a shared reference does, and a pointer gives no
  lifetime to elision.
- **Where nothing carries an access** (a turbofish argument, an associated type, an impl's self type), a `*const`
  stays host text, as a `&mut` does there.
- **A slot with two accesses** (`*mut *const u8`, or a pointer and a reference) stays host text, as two references of
  different access do.

## Closures

- **Closures with typed parameters get a name.** `v.retain(|s: &String| s.len() > n)` becomes
  `let retain_fn = |s: &String| s.len() > n;` and then `v.retain(retain_fn)`. The parameter's type is keyed
  `"retain_fn.s"` in `locals`. Creating a closure has no effect, so moving it before its statement changes the order
  of nothing.
- **A closure's return type goes onto a local.** `|s| -> usize { s.len() * 2 }` becomes
  `|s| { let out: usize = { s.len() * 2 }; out }`.

## Values that release something at their end

A `let` ends the temporaries of its own value when it ends. Binding `m.lock().unwrap().len() as u32` out of
`log(...)` would therefore unlock the mutex before `log` runs, where the source unlocked it after. Whether that
matters depends on the type: when a lock guard ends, it unlocks; when a `Chars` iterator ends, nothing happens. So
the language now says it of the type, `lifetime self released at end` (constraint language § 7.3), and `bind` reads
it:

- **Where the clause comes from.**
  - For a crate's own types, `strip` writes it: a type with an `impl Drop`, or with a field of a type that has the
    clause.
  - For the standard library, the library file has it: lock guards, `RefCell` borrows, files, buffered writers,
    standard stream locks and TCP streams, with the calls that make them.
- **What bind does with it.** A value of such a type inside a bound expression is named, and the statement goes into
  a block that ends where the statement did, so the value ends at the same point:

  ```rust
  let a = log(c, acquire(c).len() as u32);
  // becomes
  let a = { let acquire_guard = acquire(c); let len_u32: u32 = acquire_guard.len() as u32; log(c, len_u32) };
  ```

  Such a value in a position that might not run (the right of `&&`, a match arm) is left unbound and reported.
- **How a call is recognised.** A method call by its name (`.lock()`), a call by path by its type and name
  (`File::open`, not `Box::new`). A type the crate defines shadows a standard one of the same name, and only the types
  of fields count, not the names of variants.
- **Tested** with a guard whose end is visible: the function it is passed to prints whether the guard is still held.
  The output is the same before and after `bind`.
- **The corpora have no such values.** The 10 bound casts that hold a temporary hold iterators, `Option`s and
  references.

## Realizations: a turbofish is named

A turbofish writes a generic's type arguments where it is used: `s.parse::<u16>()`, `Vec::<u8>::new()`,
`std::mem::size_of::<u64>()`. The constraint language now calls a generic subject a *template*, and one with its
arguments a *realization*, a subject of its own (constraint language § 3.5). The bare body names the realization in
place of the template's last segment and its arguments; a path before it stays. The crate's realizations are one
table, at the top of its root's constraint file (`src/lib.constraints.json`):

| Rust | Bare body | `realizations` |
|---|---|---|
| `s.parse::<u16>()` | `s.parse_u16()` | `"parse_u16": {"template": "<_>::parse", "args": ["u16"]}` |
| `std::mem::size_of::<u64>()` | `std::mem::size_of_u64()` | `"size_of_u64": {"template": "std::mem::size_of", "args": ["u64"]}` |
| `size_of::<u64>()` | `size_of_u64()` | the same entry |
| `Vec::<u8>::new()` | `VecU8::new()` | `"VecU8": {"template": "std::vec::Vec", "args": ["u8"]}` |
| `x.collect::<Vec<_>>()` | `x.collect_vec()` | `"collect_vec": {"template": "<_>::collect", "args": ["list<_> as 'Vec'"]}` |

- **One realization per template and arguments, across the crate.** The template is its full path, found through the
  file's `use` items, the items it defines and the standard prelude; a method's is `<_>::name`, since its type is the
  receiver's, which needs type information. `strip` of a crate strips each file, gives each realization one name for
  the whole package (kept clear of every identifier in every bare body), strips again the files whose names differ,
  and moves each crate's table into its root's file. In the tool's own source, 30 entries in functions' `holes`
  became 16 in two tables (the library's and the binary's).
- **A turbofish of only `_`** (`Ok::<_, _>(x)`) states no type and stays as it is.
- **The names are mechanical**, and renamed like the others (`bare-rust rename`, below).
- **Check 4** drops a realization's arguments to `_` across the crate and keeps its template, so `parse::<_>()` is what
  rustc must infer. Check 1 strips the woven crate as a crate.

## On the corpora

After `strip`, no positional hint and no `rust! { ... }` text is left in strsim, fnv, the examples or the tool's own
source. The four crates added for the forms below have a few, all of one of three kinds (below the table). What the
constraint files hold:

| corpus | cast locals | typed locals | closure parameters, keyed by name | realizations | `rust!` | positional hints |
|---|---:|---:|---:|---:|---:|---:|
| strsim | 30 | 13 | 0 | 2 | 0 (was 1) | 0 |
| fnv | 1 | 0 | 0 | 0 | 0 | 0 |
| examples | 1 | 0 | 0 | 2 | 0 | 0 |
| the tool's own source | 49 | 344 | 162 | 16 | 0 (was 43) | 0 |
| itoa | 30 | 0 | 0 | 2 | 0 | 0 |
| memchr | 22 | 5 | 0 | 10 | 3 | 0 |
| miniz_oxide | 266 | 43 | 4 | 4 | 24 | 1 |
| png | 320 | 162 | 13 | 15 | 4 | 2 |

Closure return types are among the typed locals, as `out`. The tool's own source grew with this round's code, which is
where its new typed locals and closure parameters come from. The four crates at the bottom are the ones added for
pattern literals, pointers and constant contexts (below). The tool's one raw pointer, `*const ()` from the grammar, is `ptr<()>` now where it was
host text, and its three casts in a compound assignment whose left has an effect are bound in place. The `rust!` markers were all standard macros: `json!` (14),
`format!` (8), `assert_eq!` (6), `writeln!` (5), `vec!` (3) and `println!` (1) in the tool's source, and most of them
for a turbofish (`collect::<Vec<_>>()`) in an argument.

**What is left in the new crates** is of three kinds, none of them new:

- **An argument a macro runs only sometimes:** `debug_assert!(n as usize <= MAX)` (6 in miniz_oxide), and an
  `assert_eq!` message with a turbofish (3 in memchr's tests). A binding before the macro would run it always.
- **A macro the tool does not know** that states a type: miniz_oxide's own `generate_state!` (19) and `approx`'s
  `assert_relative_eq!` in png's tests (4). They stay as Rust text.
- **Const generic arguments in a turbofish**, `b.extract::<{ STRIDE_BYTES - BPP }, BPP>()` (2 hints, in png's
  nightly-only SIMD module). A realization's arguments are types, and these are values.

**A stronger check than the tests:** the tool's own source, with every file bound, still compiles with no warnings.
Built from that bound source, the tool passes its unit tests and six of its test files (90 tests).

**Bugs the tool's own source found.** The first full run found two bugs in `bind`, both in struct literals, and
both now have a regression test:

- **A shorthand field** (`Diag { message, .. }`) was taken for an expression with an effect, and bound. The rewrite
  renamed the field, which does not compile.
- **The earlier values in one expression were bound in reverse order.** `level`, `code` and `file` became `file`,
  `code`, `level`. That is the order change `bind` exists to prevent.

The first fails to compile, and `cargo check` on the woven tool caught it. The second compiles: the values here
have no effects, so the program's behaviour did not change. It was found by reading the bound text while chasing the
first bug. A check that compares evaluation order would have to run the program, as `tests/bind.rs` does for its
fixture.

## Renaming: one agent pass on strsim

The names `bind` makes are mechanical: `a_len_f64`, `value_f64`, `i_isize_3`. One Sonnet agent was given the stripped
strsim, the list of the names `bind` added, and the rule: rename each to say what the value is, in the `.brs` and
the `holes` key together, change nothing else, and keep `./weave.sh test` passing.

- **28 renames**, two of them a repeated name in a second function. The full list is in
  [`bind-strsim-renames.json`](bind-strsim-renames.json).
- Some examples:
  - `a_len_f64` became `a_total`;
  - `matches_f64` became `a_matches`;
  - `value_f64` became `twice_common` and `total_bigrams` in `sorensen_dice`;
  - `i_isize_3` became `row_cur`;
  - `r_item_usize` became `result`.
- **Checked afterwards, independently of the agent:**
  - the bare body's token stream is identical apart from the renamed names (5,423 tokens each);
  - `subjects`, every local's type and every hint are unchanged;
  - the woven crate passes its 19 tests.
- **Cost:** 2 weaves, 8 tool calls, about 62K tokens, a few cents.

## `bare-rust rename`, and the tool's own source

Renaming by hand meant editing the `.brs` and the `holes` keys together and then checking that nothing else changed.
That check is now a command: `bare-rust rename <dir> <map.json>`, where the map is
`{"<function>": {"<old>": "<new>"}, "realizations": {"<old>": "<new>"}}`.

- **What it changes.** A local, in its function's own body (not in an item declared inside it) and in the keys of the
  function's `holes` that name it (`len_f64`, `retain_fn.s`, `(m, n)`, `x#2`). A shorthand field keeps its field
  (`S { x }` becomes `S { x: total }`). A realization, at every use in the crate and in the crate's table.
- **What it refuses**, writing nothing:
  - a new name that is not a name, or is already a name in that function (for a realization: anywhere in the crate);
  - a new name that is an item of the crate, or that the map gives twice in one function;
  - an old name that is not bound in that body (a parameter, an item);
  - a name a format string captures (`"{x}"`).
- **The check after.** The token streams are the same, but for the renamed names. Every file weaves as it did,
  with as many unbound entries.
- **Other modes.** `--check` checks and writes nothing. `--list` lists the names each body binds. `--lenient` skips an
  entry that does not apply, for a map made on another version of the crate. The experiments use it: each task's base
  differs a little from the snapshot the map was made on.
- Tests are in `tests/rename.rs`. They cover a rename across two modules, that the woven crate is the same program,
  and each refusal.

**The tool's own source.** `strip` of the tool binds 160 values with mechanical names, in 64 functions, and has 15
realizations. (The other 335 typed locals and 158 closure parameters are named in the source, not by `bind`.)

- **Who chose the names.** Three Sonnet agents, one per third of the functions, each with the bare files, the original
  Rust for reference, and the command's `--check`. They gave the 160 locals and 11 of the realizations new names; four
  realizations already read well (`collect_string`, `BTreeMapStringUsize`, ...).
- **The map is** [`rename-tool-own-source.json`](rename-tool-own-source.json). Some examples:
  - in `cargo::parse`, `unwrap_or_usize_3` became `diag_line` and `to_string_v_2` became `diag_file`;
  - in `check::check4`, `out_2` became `was_filled`;
  - the realization `sum_usize` became `sum_counts`.
- **Checked:** the three maps merged pass the check. Applied, the renamed tool weaves with no unbound entry,
  compiles with no errors and no warnings, and passes its 45 unit tests.
- **Cost:** about 240K tokens for the three agents, 2 minutes each.

**The experiment crates.** One more agent named strsim's 31 mechanical locals and its 2 realizations, and fnv's one
local (`experiments/renames/`). E6 runs on workspaces renamed with these maps.


## New corpora: itoa, memchr, miniz_oxide, png

None of strsim, fnv, the examples or the tool's source has a suffixed literal in a pattern, a pointer cast or a cast in
a constant context. Four crates that do are now corpora, pinned in `corpora/fetch.sh` and checked by
`tests/corpora.rs` (checks 1 to 4; no externals list is committed for them, so check 5 does not run):

| crate | files | tests | what it brings |
|---|---:|---:|---|
| itoa | 3 | 13 | pointer casts (2, cast locals of type `ptr<u8>`), an impl for a primitive type |
| memchr | 45 | 166 | pointers in signatures (366 `ptr<..>` slots), a `const` whose whole value is a cast (2), a union, impls for pointer types |
| miniz_oxide | 16 | 55 | casts in array lengths (4 blocks in place), modules behind `#[cfg(feature = ..)]` |
| png | 24 | 117 | suffixed literals in patterns that decide what a generic call reads: `match buf.read_be()? { 0u8 => .. }` (3 names) |

All four pass checks 1 to 4. Getting there needed these, each with a test:

- **Forms strip refused.**
  - An impl for a primitive type (`impl Unsigned for u128`): the bare grammar took only a type name there.
  - An impl for a pointer type (`impl Pointer for *const T`), whose self slot is host text, as `&mut T` is.
  - A union, which is a type with fields as a struct is; the bare body keeps the keyword.
  - A generic parameter with a default (`type Result<T, E = Error>`), now `"default"` on the parameter.
- **A type the file could not read back.** A call-sugar result with `as` (`FnMut(&[u8]) -> Option<usize>`) printed as
  `-> u64 as 'usize'?`, where the `as` belongs to the whole type. It prints as `-> (u64 as 'usize')?` now, and `(T)`
  without a comma groups, as a one-tuple is `(T,)`.
- **`&raw`, a borrow of a variable named `raw`,** did not parse as bare Rust. The bare parser is generated by a newer
  tree-sitter, which lexes `raw` after `&` as the start of `&raw const x`; the keyword pair is one token now.
- **`mut` that rustc does not derive.**
  - For `&mut x` of an immutable `x`, rustc may suggest removing the `&mut` rather than adding the `mut`. The weave now
    puts `mut` on the binding the message names when rustc suggests none.
  - Code a `#[cfg(..)]` leaves out of the build (other than `cfg(test)`) is never compiled, so its `mut` cannot be
    derived. The bare body keeps it there, and in a file whose `mod` has such a `cfg`. This is the case for 186 of
    miniz_oxide's 222 `mut`s, whose modules sit behind `#[cfg(feature = "with-alloc")]`, and for 53 of png's 457.
- **Brackets bind kept.** Binding the value `(a << 9)` kept its brackets, `let v = (a << 9);`, and a block put where a
  bracketed `||` operand was ended in `(..)`. Both gave `unused_parens` warnings.
- **The checks.**
  - Check 2 reads `&'_ T` as `&T`.
  - Check 3 does not count an error the original crate has too: miniz_oxide's benchmark needs `criterion`, which
    upstream left commented out.
  - A crate in a workspace gets paths from the workspace's root in cargo's messages, and its tests and clippy
    configuration read the workspace's files (`../miniz/miniz.c`, `clippy.toml`). The paths are taken from the crate,
    and the woven copy gets the workspace's other files linked beside it.
  - A test that writes under `target/` finds the directory in the woven copy.
  - Check 4 counts an inference error at a realization, `TryInto::<_>::try_into(x)` with its arguments dropped, as
    that realization's hole.
  - `rustfmt` falls back to edition 2024 for what only it parses (let chains).

## Also fixed: a lifetime inside an `impl Trait` parameter

`fn total<'a>(ws: impl Iterator<Item = &'a W>)` lost its `'a`. The lifetime appears once among the inputs, so `strip`
took it for one that elision could give back, and wove `impl Iterator<Item = &W>`, which Rust rejects (E0658:
anonymous lifetimes in `impl Trait` are unstable). A lifetime there now keeps its name, and so does one inside
`Fn(&'a W)` or `fn(&'a W)`. There elision would compile, but it would make the lifetime the function type's own
(higher-ranked) rather than the function's.
