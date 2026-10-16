# The bare format on one page

A Rust file becomes two files. `bare-rust strip` writes `x.brs`, the **bare body**: names, control flow and
expressions, and no types, lifetimes, `&`, `mut`, visibility, generics or `where`. It writes `x.constraints.json`
beside it, the **constraint file**: one *subject* per item, holding everything the bare body left out. `bare-rust
weave` puts them together again. Edit either file; the weave refuses a body and a constraint file that disagree.

Rules that cover most edits:

- A subject is named by its path: `crate::module::Type::method`; a trait impl's methods are
  `crate::Type as Trait::method`; a second item of the same name gets `#2`.
- `self` stays in the bare parameter list; whether it is `&self`, `&mut self` or `self` is in the subject's first
  parameter (`access` read or write, or none for an owned `self`).
- Types are written in the constraint type syntax (`text`, `list<T> as 'Vec'`, `map<K, V> as 'HashMap'`, `ref<T>`,
  `ptr<T>` for a raw pointer, `T?` for `Option<T>`, `u32`, a plain name for any other type); `{ "host": "..." }` is
  Rust text for a type that has no such form.
- `mut` is not written: the weave derives it through rustc, in every build the crate is checked in. Besides the
  default build, `strip` chooses a few others (another target, a feature, a `--cfg`, a release build, listed in
  `builds.json`) so that each piece of code a `#[cfg(..)]` guards is compiled in one of them, where the original crate
  compiles on stable Rust. The exception is code that no such build compiles (a feature that needs nightly Rust, a
  target that is not installed); there the bare body keeps its `mut`. A `let` has no type in
  the bare body; where rustc cannot infer it, it goes under `"holes"` (a local, a cast local; see the last example), and
  a realization of a generic goes in the crate's table, `"realizations"`, in the crate root's file.
- A field or parameter whose type is a `ref<...>` needs an `access`; a result that borrows says `"hold": {"kind":
  "borrows", "from": "self"}`. A `ptr<...>` has `"access": {"rights": "read"}` for `*const` and no `access` for `*mut`:
  a pointer has no guarantee and no lifetime.
- A union is a `type` subject with `fields`, as a struct is; the bare body keeps the keyword `union`. A generic
  parameter's default is `"default"` on it: a type (`{"name": "E", "bounds": [], "default": "Error"}`), or host text for
  a const parameter's value.
- The examples below come from `docs/bare-format-src/lib.rs`, stripped as crate `format_demo 0.1.0`; the test
  `tests/docs.rs` re-strips it and fails if a block here has drifted.

## One example per subject kind

### Free function

A `function` subject: `params` in order, each with a `type`; `result` if it returns something; `visible` for `pub`.

Rust:

```rust
pub fn double(x: u32) -> u32 {
    x * 2
}
```

Bare (`.brs`):

```brs
fn double(x) {
    x * 2
}
```

Constraint file (`.constraints.json`, under `"subjects"`):

```json
{
  "kind": "function",
  "name": "format_demo::double",
  "visible": "public",
  "params": [{"name": "x", "type": "u32"}],
  "result": {"type": "u32"}
}
```

### Methods: `&self` and `&mut self`

`self` is a parameter. `&self` is `access` read/readers; `&mut self` is write/exclusive; a result that points into `self` says what it borrows from.

Rust:

```rust
impl Counter {
    pub fn get(&self, word: &str) -> Option<&u32> {
        self.counts.get(word)
    }

    pub fn bump(&mut self, word: String) {
        *self.counts.entry(word).or_insert(0) += 1;
    }
}
```

Bare (`.brs`):

```brs
impl Counter {
    fn get(self, word) {
        self.counts.get(word)
    }

    fn bump(self, word) {
        *self.counts.entry(word).or_insert(0) += 1;
    }
}
```

Constraint file (`.constraints.json`, under `"subjects"`):

```json
{
  "kind": "function",
  "name": "format_demo::Counter::get",
  "visible": "public",
  "params": [{"name": "self", "type": "ref<Counter>", "access": {"rights": "read", "guarantee": "readers"}}, {"name": "word", "type": "ref<text>", "access": {"rights": "read", "guarantee": "readers"}}],
  "result": {"type": "ref<u32>?", "hold": {"kind": "borrows", "from": "self"}}
}
{
  "kind": "function",
  "name": "format_demo::Counter::bump",
  "visible": "public",
  "params": [{"name": "self", "type": "ref<Counter>", "access": {"rights": "write", "guarantee": "exclusive"}}, {"name": "word", "type": "text"}]
}
```

### Generic function with bounds

`generics` lists the type parameters; bounds written inline are `bounds`, bounds from a `where` clause are `where`.

Rust:

```rust
pub fn largest<T: PartialOrd + Copy>(items: &[T]) -> T {
    let mut best = items[0];
    for &x in items {
        if x > best {
            best = x;
        }
    }
    best
}
```

Bare (`.brs`):

```brs
fn largest(items) {
    let best = items[0];
    for &x in items {
        if x > best {
            best = x;
        }
    }
    best
}
```

Constraint file (`.constraints.json`, under `"subjects"`):

```json
{
  "kind": "function",
  "name": "format_demo::largest",
  "visible": "public",
  "generics": [{"name": "T", "bounds": ["PartialOrd", "Copy"]}],
  "params": [{"name": "items", "type": "ref<list<T> as 'std::primitive::slice'>", "access": {"rights": "read", "guarantee": "readers"}}],
  "result": {"type": "T"}
}
```

### Impl of a trait

An `impl` subject (named `Type as Trait`) carries the impl's own generics; each method is a `function` subject named under it. The type's subject lists the trait under `implements`.

Rust:

```rust
impl Describe for Counter {
    fn describe(&self) -> String {
        format!("{} words", self.counts.len())
    }
}
```

Bare (`.brs`):

```brs
impl Describe for Counter {
    fn describe(self) {
        format!("{} words", self.counts.len())
    }
}
```

Constraint file (`.constraints.json`, under `"subjects"`):

```json
{
  "kind": "impl",
  "name": "format_demo::Counter as Describe"
}
{
  "kind": "function",
  "name": "format_demo::Counter as Describe::describe",
  "params": [{"name": "self", "type": "ref<Counter>", "access": {"rights": "read", "guarantee": "readers"}}],
  "result": {"type": "text"}
}
```

### Type alias

The bare body says `type Table;` and the subject (kind `alias`) holds the right-hand side in `type`.

Rust:

```rust
pub type Table = HashMap<String, u32>;
```

Bare (`.brs`):

```brs
type Table;
```

Constraint file (`.constraints.json`, under `"subjects"`):

```json
{
  "kind": "alias",
  "name": "format_demo::Table",
  "visible": "public",
  "type": "map<text, u32> as 'HashMap'"
}
```

### Struct, named fields

A `type` subject; `fields` in order, each with `type`, `hold` (`owns` unless it borrows) and `visible` if `pub`.

Rust:

```rust
pub struct Point {
    pub x: f64,
    y: f64,
}
```

Bare (`.brs`):

```brs
struct Point {
    x,
    y,
}
```

Constraint file (`.constraints.json`, under `"subjects"`):

```json
{
  "kind": "type",
  "name": "format_demo::Point",
  "visible": "public",
  "fields": [{"name": "x", "visible": "public", "type": "f64", "hold": {"kind": "owns"}}, {"name": "y", "type": "f64", "hold": {"kind": "owns"}}]
}
```

### Struct, tuple fields

A tuple struct is `struct Meters(_, _);` in the bare body; its fields have no `name`.

Rust:

```rust
pub struct Meters(pub f64, f64);
```

Bare (`.brs`):

```brs
struct Meters(_, _);
```

Constraint file (`.constraints.json`, under `"subjects"`):

```json
{
  "kind": "type",
  "name": "format_demo::Meters",
  "visible": "public",
  "fields": [{"visible": "public", "type": "f64", "hold": {"kind": "owns"}}, {"type": "f64", "hold": {"kind": "owns"}}]
}
```

### Enum

`variants` in order; a variant with fields has `fields`, named for a struct variant and unnamed for a tuple variant.

Rust:

```rust
pub enum Shape {
    Dot,
    Circle(f64),
    Rect { w: f64, h: f64 },
}
```

Bare (`.brs`):

```brs
enum Shape {
    Dot,
    Circle(_),
    Rect { w, h },
}
```

Constraint file (`.constraints.json`, under `"subjects"`):

```json
{
  "kind": "type",
  "name": "format_demo::Shape",
  "visible": "public",
  "variants": [{"name": "Dot"}, {"name": "Circle", "fields": [{"type": "f64", "hold": {"kind": "owns"}}]}, {"name": "Rect", "fields": [{"name": "w", "type": "f64", "hold": {"kind": "owns"}}, {"name": "h", "type": "f64", "hold": {"kind": "owns"}}]}]
}
```

### Locals, casts and hints

What rustc cannot infer from the bare body goes in `holes`, keyed by the subject: a `locals` entry per `let` (keyed by the pattern; the second `let x` is `x#2`), and per typed parameter of a closure that a `let` names (keyed `<closure>.<parameter>`: `let by_len = |a, b| ...` with `"by_len.a": "<T>"`). A closure's return type is carried by a local in its body: `|s| { let out = { ... }; out }` with `"out": "<T>"`. A turbofish is a *realization* of a template (a generic function or type with its type arguments), and the body names it: the name takes the place of the template's last segment and its arguments, and a path before it stays. `s.parse::<u16>()` is `s.parse_u16()` in the body, `Vec::<u8>::new()` is `VecU8::new()`, and `std::mem::size_of::<u64>()` is `std::mem::size_of_u64()`. The realizations are one table for the crate, at the top of its root's constraint file (`src/lib.constraints.json`; `src/main.constraints.json` for a binary), under `realizations`: `"parse_u16": {"template": "<_>::parse", "args": ["u16"]}`. The template is its full path (`std::mem::size_of`, `std::vec::Vec`), or `<_>::name` for a method, whose type is the receiver's. One name serves every use in the crate with the same template and arguments, and an argument can be `_` for the host to infer (`"collect_vec": {"template": "<_>::collect", "args": ["list<_> as 'Vec'"]}`). A new use of a realization the table has needs nothing more; a new one needs its entry. Rename the mechanical names like the others (every use and the key together).

**Every cast and suffixed literal has a name.** A conversion between built-in types (`as u64`, `as f64`, ...) and a literal's type (`1u8`) are not written in the body. The value initialises a `let`, and the local's entry carries the type: `{"type": "<T>", "cast": true}` for a cast, so that `let n = e;` weaves to `let n: T = e as T;` (with brackets around `e` when it needs them), and `"<T>"` for a literal, so that `let one = 1;` weaves to `let one: u8 = 1;`. A cast to a pointer carries its access too: `{"type": "ptr<u8>", "access": {"rights": "read"}, "cast": true}` weaves to `let p: *const u8 = e as *const u8;`. Any other target is carried the same way: a type alias (`{"type": "Word", "cast": true}`), a reference (`{"type": "ref<any Display>", "cast": true}` for `&x as &dyn Display`), a `fn` pointer (as host text). A `const` or `static` whose whole value is a cast carries it on its subject, `"cast": true`, as a cast local does: `const W = N;` weaves to `const W: usize = N as usize;`. Write a `let` where the Rust would cast in the middle of an expression too: `1.0 - d / m`, with `d` and `m` cast locals, rather than `1.0 - (d as f64) / (m as f64)`.

`strip` does this to the Rust it reads (`bare-rust bind <file.rs>` shows the result), and gives a closure with typed parameters a name the same way (`v.retain(|s: &String| ...)` becomes `let retain_fn = |s: &String| ...;` and `v.retain(retain_fn)`): `(h as u64).wrapping_mul(P)` becomes `let h_u64: u64 = h as u64;` before the statement and `h_u64.wrapping_mul(P)` in it. Where the statement would compute the value when the Rust might not (the right of `&&` or `||`, a match arm or guard, an `else if`, a `while` condition, a closure body), the `let` goes into a new block in that place: `c && { let h_u8 = h; h_u8 > 3 }`. A constant context (an array's length, the value of a `const` or `static`), a `while let` value and a part of a let chain get a block in their place the same way: `[0; { let n_usize = N; n_usize }]`, `if let Some(y) = a && { let n_u8 = n; y > n_u8 }`. What runs earlier in the statement and has an effect is named first, so the order stays the Rust's: a method receiver that is a value is named whole (`let largest = v.iter().max().copied().unwrap_or(0);` and `largest.max(a_u8)`), a place stays where it is and what it computes on the way is named (`let row = next_row();` and `self.rows[row].push(n_u32)`), and the right of an assignment is named before its left. Which side of `+=` runs first depends on the types, so where the other side has an effect the `let` goes into a block in place of the part it is in: `v[{ let i_usize = i; i_usize }] += next()`. Names such as `h_u64` and `k1_u8` are mechanical; rename them (the `.brs` and the `holes` key together) to say what the value is.

The arguments of the standard macros (`format!`, `println!`, `write!`, `writeln!`, `panic!`, `assert!`, `assert_eq!`, `vec!`, `dbg!`, `json!` and the like) are bound like a call's: `println!("{}", x as u8)` is `let x_u8 = x;` (a cast local) and `println!("{}", x_u8)`, and an argument with a turbofish is named before the macro (`let total = v.iter().sum_u64();` and `format!("{}", total)`). A binding never goes inside a macro's arguments. A macro whose syntax the tool does not know, or an argument that runs only sometimes (an assertion's message, `debug_assert!`), is kept as Rust text when it states a type: `rust! { assert!(ok, "{}", n as u8) }`, which weaves to the call. Write new code the way `strip` writes it: name the value before the macro.

Rust:

```rust
pub fn mean(values: &[f64]) -> f64 {
    let total: f64 = values.iter().sum();
    let n: f64 = values.len() as f64;
    total / n
}
```

Bare (`.brs`):

```brs
fn mean(values) {
    let total = values.iter().sum();
    let n = values.len();
    total / n
}
```

Constraint file (`.constraints.json`, under `"subjects"` and `"holes"`):

```json
{
  "kind": "function",
  "name": "format_demo::mean",
  "visible": "public",
  "params": [{"name": "values", "type": "ref<list<f64> as 'std::primitive::slice'>", "access": {"rights": "read", "guarantee": "readers"}}],
  "result": {"type": "f64"}
}
{
  "holes": {
    "format_demo::mean": {
      "locals": {
        "total": "f64",
        "n": {"type": "f64", "cast": true}
      }
    }
  }
}
```

A suffixed literal in a pattern (`1u8 => ...`) has no `let` to carry its type. Where what is matched is a name declared with that type (a parameter `x: u8`, a typed `let`; through `&` and a tuple's elements, `match (x, y) { (1, _) => ... }`), the declaration already says it, and the literal has no suffix. Otherwise the literal gets a name over it, which carries its type keyed by the name's pattern: `_k1_u8 @ 1` with `"_k1_u8 @ 1": {"literal": "u8"}` weaves to `_k1_u8 @ 1u8`. The `_` says that nothing uses the name; a name already there (`n @ 4..=6`) carries the type the same way, for every literal under it.

Two kinds of site cannot be bound and stay `e as _` with a hint `{"kind": "cast", "n": <ordinal of the cast in the body>, "type": "<T>"}`: a cast in a macro argument that runs only sometimes (an assertion's message, `debug_assert!`), and one holding a value that releases something at its end (a lock guard) where the statement might not run. Do not write new ones; use a cast local.

## When something is wrong, the message names the fix

- `let x: T = ...` in a `.brs`: `src/lib.brs:12:9: a local's type does not go in the bare body; ...` prints the
  `"holes": {"<fn path>": {"locals": {"x": "<T>"}}}` entry to add (`<T>` converted to the constraint syntax, or host
  text). (`let mut x` is accepted and `mut` is derived, so it is not
  an error.)
- `type X = ...;` in a `.brs`: the message says to write `type X;` and prints the `alias` subject to add.
- `slot missing: no subject ...` from a weave: the message ends with a ready-to-paste subject entry for the item,
  with `TODO` where a type goes. Fill each `TODO`; the weave refuses a leftover `TODO` and names where it is.

The same messages appear in the text output of `bare-rust weave` and in the `errors` of `weave --json`.

## Adding an item

`bare-rust scaffold <fn|impl|alias|struct> <path> [options]` prints a bare stub and its subject entries, or with
`--write <dir>` appends the stub to the module's `.brs` and the entries to its `.constraints.json` (canonical form).
Types default to `TODO`. See `bare-rust` with no arguments for the options.

```
bare-rust scaffold fn util::Counter::peek --self ref --params key --param-types 'ref<text>' --result 'u32?'
bare-rust scaffold impl Counter --trait Describe --methods describe,reset:mut
bare-rust scaffold alias Table --type "map<text, u32> as 'HashMap'"
bare-rust scaffold struct Point --fields x,y --field-types f64,f64 --write .
```
