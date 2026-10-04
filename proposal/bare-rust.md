# Bare Rust: a stripper and a weaver

**Status:** Plan
**Date:** 2026-10-03
**Written for:** whoever builds the first derived language of the constraint language, and the team that owns the
language, who will read its results.
**Companion documents:** [`constraint-language.md`](constraint-language.md) defines derived languages (§ 11.2), the
round trip law, `local`, the bare Rust example this plan starts from (§ 14.9), and the open questions the trial is meant
to answer (§ 17, questions 10, 13, 14 and 15). Its sections are cited here as "language § n"; a bare "§ n" is a section
of this plan.

---

## 0. The plan on one page

The constraint language claims that a host language can be split into a bare body, which says what the program does,
and a constraint file, which says what must hold, and that `strip` and `weave` move code between the two without loss
(language § 11.2). This plan tests the claim on one host, as cheaply as possible, before any other host is built.

- **The host is Rust.** Its checker proves the most: `rustc` turns the memory clauses (`access`, `hold … borrows from`,
  `sendable`) into checked borrows and lifetimes, where TypeScript's checker would only exercise the types of language
  § 6.1. And language § 14.9 is already a bare Rust example with its expected woven output, which becomes the first
  test.
- **The bare language is the image of erasure.** It is neither a subset nor a superset of Rust: `rustc` rejects
  `fn get(self, key)`, and bare Rust rejects `&'a self`. It is defined as `strip(Rust)`, in three parts: a grammar
  delta, written as an extension of tree-sitter-rust; a lint that bans constraints disguised as code; and a list of
  constructs that are both behaviour and constraint, each split in two (§ 2).
- **The constraint file is JSON.** Agents read and write it, so the machine-readable form is the source of truth. The
  text syntax of language § 15 is printed from the JSON, last, as a view for people.
- **The weaver prints; it does not infer.** The file is authoritative. If it says `access self : write exclusive`,
  `weave` prints `&mut self`. Where the body leaves a hole that only the file can fill, and the file does not fill it,
  weaving fails (§ 2.4).
- **Libraries need constraint files, not bare bodies.** An agent writing bare code needs to know what the standard
  library and its dependencies promise, so `strip` also runs in a partial mode that produces only a library's
  constraint file. It starts from rustdoc's JSON and, where the source is available, looks into bodies to find more
  than the signatures say (§ 7).
- **It is small.** The core of the bare grammar is already written: 21 lines extending tree-sitter-rust 0.24, which
  generate a parser that parses language § 14.9's bare body and rejects its woven form (§ 3.1). The whole trial is a
  Rust crate of about 2,500 lines.
- **It ends in a decision.** The experiments (§ 10) say whether bare bodies stay bare, whether a file can always fill
  the holes, whether library constraints are good enough to rely on, and whether an agent does better with bare Rust
  and JSON than with Rust. Their results decide whether to build a second host, change the design, or stop.

---

## 1. What the trial must show

1. **Lossless.** `strip(weave(b, c)) == (b, c)` exactly, and `weave(strip(s)) == s` up to `rustfmt` and lifetime
   elision, for every file in the corpora; the woven code compiles and passes its tests.
2. **Host-neutral.** Most of what moves maps onto the constraint language's own clauses and types, not onto Rust text
   carried along unread. What does not map is a list of gaps in the language, with a count for each (§ 12).
3. **Bare in practice.** Bodies need few types of their own. The trial measures how many holes a body leaves that only
   `local`, or something stronger, can fill (language § 17, question 14).
4. **Libraries are visible.** The standard library and each dependency have a constraint file, in the same JSON, with
   a status on every clause that says how far it can be trusted.
5. **Useful to agents.** An agent given a bare body, its constraint file and its dependencies' constraint files
   completes real changes at least as well as one given the Rust. This is the claim language § 11.2 rests on ("agents
   write what the code does"), and the only one that decides whether the rest is worth building.

---

## 2. Decisions

### 2.1 The bare language is the image of erasure

`L_bare = strip(L_rust)`, and `weave : Bare × Constraints → Rust`, with `strip(weave(b, c)) = b`. Each bare program
stands for a family of Rust programs, one for each constraint file that fills its slots and holes. The bare language is
defined in three parts:

1. **A grammar delta.** Take tree-sitter-rust's grammar and make every constraint production empty: parameter and
   return types, lifetimes, `&` and `&mut` and `mut` on `self`, visibility, generic parameters and bounds, `where`
   clauses, field types, and the types of `let`. Structure stays: a struct keeps its field names, and a tuple variant
   keeps its arity, written `A(_)`. Only the types go. Every emptied production is a kind of slot that the JSON must be
   able to fill, and a point where `weave` prints.
2. **A ban on disguised constraints.** Some constraints are not syntax: `.unwrap()`, `.expect()`, `assert!`, and a
   leading `if cond { return Err(NotYours) }`. They are ordinary calls and statements, so the grammar cannot remove
   them; a lint, which is part of the bare language's definition, finds them. The ban depends on position: a check at
   the head of a function, on its inputs, belongs in the file as a `require … else`; a check on an intermediate value
   may stay in the body, as principle 5 of the language allows.
3. **A list of mixed constructs**, each split by the rule of § 2.2.

### 2.2 A construct that is both behaviour and constraint is split in two

The behaviour stays in the bare body; the constraint moves to the file. This replaces the sentence in language § 11.2
that kept such a construct ("C#'s `ref`, Rust's trait objects") whole in the body; § 11.2 now states the split.

| Construct | Behaviour, which stays | Constraint, which moves |
| --- | --- | --- |
| `x as u32` | The conversion: `x as _` | The target type, which decides truncation |
| `iter.collect::<Vec<u8>>()` | The collection chosen: `collect::<Vec<_>>()` | The element type |
| `s.parse::<u32>()` | The parse: `s.parse::<_>()` | The result type, which selects the parser |
| `1u8`, `a + b` | The literal and the arithmetic | The width, which decides overflow, from the declared types |
| `x.unwrap()` on an input | `.unwrap()` itself, kept as written for now | That the value is present: a `require … else` |
| `&self`, `&mut self`, `let mut` | The receiver; the binding | Access and mutability: `access`, and the writes the body makes |
| `Rc` or `Arc`, `Box` | Sharing; owning | Which one, from `shares` and `sendable`; `Box` where an owned recursive type needs it |
| `Box<dyn T>` or a generic `T` | Dispatch through the interface | The set of implementations allowed |

`_` in casts and type arguments is already valid Rust, so the bare form of an expression keeps the target's syntax and
only its types are erased. The trial leaves `Rc`, `Arc`, `Box` and trait objects in the body as written and counts them,
and keeps every `.unwrap()` and `.expect()` in the body, as decided (§ 3.2).

### 2.3 The file is authoritative, and the weaver does no type inference

`weave` walks the bare tree, copies every token unchanged, and at each emptied production prints what the file says for
that slot: `&self` from `access self : read readers`, `&mut self` from `write exclusive`, `pub(crate)` from
`visible to crate`. The weaver does not work out `&` or `&mut`, lifetimes or types; the file states them. One case is
derived rather than stated, as language § 11.2 says: `let mut`. `weave` prints no `mut`, runs `cargo check`, and applies
the suggestions `rustc` gives with errors E0384 and E0596, which add `mut`, until none remain. This keeps the weaver
syntactic and lets the compiler decide.

### 2.4 Holes: a bare body means something only with its file

Where the constraint chooses the behaviour (`x as _`, `parse::<_>`, the width of an integer, `&` or `&mut`), splitting
leaves a hole in the body that only the file can fill. So a bare body says what the program does up to the choices its
file makes, and two rules hold, as intended from the start:

- **`weave` fails if a hole is undetermined.** A hole is determined when the file states it (a parameter's type, a
  `local`) or when `rustc` infers it from what the file states. `weave` leaves the hole as `_`, runs `cargo check`, and
  reports an inference error at a hole as "hole undetermined", with the hole's place in the bare body and the clause
  that would fill it.
- **The agent reads the file, not only the body,** to know what the body does.

The Rust profile lists every kind of hole (§ 2.2's table is its first version).

### 2.5 JSON is the source of truth

The constraint file is JSON, with a published JSON Schema and one canonical form (fixed key order, two-space indent) so
that diffs show meaning. JSON rather than YAML, because YAML's implicit typing (`no`, `1.10`, `on`) turns values into
other values, and because every agent and tool emits and validates JSON reliably. Types are strings in the language's
own type syntax (language § 3.4: `u64?`, `ref<Entry>?`, `map<Key, Entry> as 'std::collections::HashMap'`): compact,
readable, and parsed by a small type parser. Access, holding and visibility are JSON structure, not strings.

### 2.6 Tree-sitter on both sides

`strip` parses Rust with tree-sitter-rust; `weave` parses bare Rust with the parser generated from the extension. Both
trees keep every comment and every byte range, so the body is copied, never re-printed, and comments stay where they
were. The tools are written in Rust, with the `tree-sitter` crate and both grammars linked in. `syn` is not used: it
drops comments, and the body must come through untouched.

### 2.7 Refuse rather than guess

A file that does not parse, or whose woven form does not compile when the original does, is refused with its reason,
and counted. Types inside macro invocations are token trees the grammar does not see; they stay where they are, as
language § 11.2 says, and are counted.

---

## 3. The bare language

### 3.1 The grammar delta

The core of the delta, as written and run against tree-sitter-rust 0.24 while preparing this plan:

```js
const rust = require('tree-sitter-rust/grammar');
module.exports = grammar(rust, {
  name: 'bare_rust',
  rules: {
    struct_item: $ => seq('struct', field('name', $._type_identifier),
      choice(field('body', $.field_declaration_list), seq(field('body', $.ordered_field_declaration_list), ';'), ';')),
    field_declaration: $ => field('name', $._field_identifier),
    ordered_field_declaration_list: $ => seq('(', sepBy(',', '_'), optional(','), ')'),
    function_item: $ => seq(optional($.function_modifiers), 'fn', field('name', choice($.identifier, $.metavariable)),
      field('parameters', $.parameters), field('body', $.block)),
    parameters: $ => seq('(', sepBy(',', seq(optional($.attribute_item), choice($.parameter, $.self_parameter, '_'))),
      optional(','), ')'),
    self_parameter: $ => $.self,
    parameter: $ => field('pattern', $._pattern),
    closure_parameters: $ => seq('|', sepBy(',', $.parameter), '|'),
    closure_expression: $ => prec(-1, seq(optional('static'), optional('async'), optional('move'),
      field('parameters', $.closure_parameters), field('body', choice($._expression, '_')))),
    let_declaration: $ => seq('let', field('pattern', $._pattern),
      optional(seq('=', field('value', $._expression))), optional(seq('else', field('alternative', $.block))), ';'),
  },
});
```

`tree-sitter generate` builds a parser from it (the external scanner is copied from tree-sitter-rust, with its symbols
renamed). The parser reads language § 14.9's bare body into a full tree, `self_parameter` and untyped `parameter`
nodes included, and marks its woven form (`pub fn get<'a>(&'a self, key: &Key) -> …`) as errors. The rest of the delta
is the same kind of override: enums, unions and their variants; traits and their items; `impl` headers, which keep
their trait and type names and lose their type parameters, arguments and `where` clauses; `const` and `static` items;
type aliases, which move whole; visibility on every item.

### 3.2 The lint

The bare language's lint reports, by position:

- at the head of a function, before any effect: `if … { return Err(…) }` and `assert!` on a parameter, `self` or a
  value read from the world. These belong in the file as `require … else`;
- `.unwrap()` and `.expect()` on a parameter, `self` or a value read from the world. These stay in the body, as
  decided; the presence they assert is stated in the file as a `require`, and the lint marks the call as covered by it,
  or reports it when no `require` covers it;
- anywhere: concrete types in expression position (`as u32`, `::<u32>`, `1u8`), which bare Rust writes as `_`;
- elsewhere: `.unwrap()`, `.expect()`, `panic!` and `unreachable!` on intermediate values, which may stay (principle 5),
  and are counted.

The lint is a detector in the trial. Lifting head checks into the file, with `weave` putting them back as generated
checks, is stage 5 (§ 11).

---

## 4. The files

### 4.1 Beside each module

`src/lib.rs` becomes `src/lib.brs`, the bare body, and `src/lib.constraints.json`. Dependencies' constraint files live
in a cache keyed by crate and version, as `Cargo.lock` pins them (§ 7.4).

### 4.2 The constraint data

| Member | Holds |
| --- | --- |
| `format` | `"y-constraints/0"` |
| `host`, `crate`, `module` | `"rust"`, the crate and version, and the module's path |
| `status` | The default status of the file's clauses: `"lifted"` for what `strip` wrote (language § 10.1); a clause can carry its own |
| `subjects` | In source order: each with `kind` (`type`, `function`, `trait`, `impl`, `const`), `name`, and its slots |
| `holes` | Per function, the `local` types and body hints that fill holes no slot covers |

Subjects are named by their Rust paths, `eve_cache::Cache::get`; an `impl` of a trait is `eve_cache::Cache as
std::fmt::Debug`. A stable name given with `#[y::stable("…")]` (language § 3.2), an attribute from a small `y` crate
that expands to nothing, takes precedence; it stays in the body, where both sides can read it.

### 4.3 The slots

Every emptied production is a slot, and every slot maps to a clause:

| Emptied production | Slot | Clause |
| --- | --- | --- |
| Visibility | `visible` | `visible to public \| crate \| module \| path` |
| Generic parameters, bounds, `where` | `generics: [{ name, bounds }]` | `for T where T: Bound` |
| A parameter's type | `params[i].type` | The parameter's type |
| `&`, `&mut`, a lifetime on `self` or a parameter | `params[i].access`, `params[i].hold` | `access p : rights guarantee`; `hold p : borrows from q` |
| The return type | `result.type`, `result.hold` | The result's type; `Result<T, E>` gives `T` and `fails only E` |
| A field's type | `fields[f].type`, `fields[f].hold` | `field f : T`; `hold f : owns \| shares \| borrows` |
| A tuple field's type (`A(_)`) | `fields[i].type` | As above |
| A `let`'s type | `holes.locals[name]` | `local name : T` |
| `mut` on a binding | Not stored: derived (§ 2.3) | — |
| `#[must_use]`, `#[non_exhaustive]` | Clauses | `result must be used`; `open` |
| `const fn`, `unsafe fn` | Host text only (§ 12) | — |
| A hole in an expression | `holes.hints`, by position | Body hint (§ 2.4) |

### 4.4 The types

| Rust | Constraint language | Note |
| --- | --- | --- |
| `bool`, `u8` to `u128`, `i8` to `i128`, `f32`, `f64` | The same | |
| `usize`, `isize` | `u64 as 'usize'`, `i64 as 'isize'` | Width is the target's (§ 12) |
| `String`, `&str` | `text`, `ref<text>` with `access : read readers` | |
| `char` | `text where length(value) == 1 as 'char'` | |
| `Option<T>` | `T?` | |
| `Result<T, E>` as a result | `T`, with `fails only` the variants of `E` | |
| `Vec<T>`, `HashMap<K, V>`, `BTreeMap<K, V>`, `HashSet<T>` | `list<T>`, `map<K, V>`, `set<T>`, with `as` naming the host type as written | |
| `(A, B)` | `(A, B)` | |
| `&T`, `&mut T` | `ref<T>`, with `access : read readers` or `write exclusive` | |
| A lifetime | `hold … : borrows from p`, `outlives` | Elided lifetimes are expanded by Rust's rules before mapping |
| `Box<T>`, `Rc<T>`, `Arc<T>` | `T`, with `as` naming the wrapper, and `hold : owns` or `shares` | The choice stays named until the trial opens the hole (§ 2.2) |
| `[T; N]`, `impl Trait`, `dyn Trait`, function pointers, associated types | Host text only: `{ "host": "<text>" }` | Counted per form (§ 10, E1) |

`as` carries the host path as the module writes it, so it resolves through the module's own `use` declarations, which
stay in the body.

### 4.5 Language § 14.9, as JSON

The constraint file of language § 14.9, in the form the trial uses:

```json
{
  "format": "y-constraints/0",
  "host": "rust",
  "crate": "eve_cache 0.1.0",
  "module": "src/lib.rs",
  "status": "lifted",
  "subjects": [
    { "kind": "type", "name": "eve_cache::Cache", "visible": "crate",
      "fields": [
        { "name": "entries", "type": "map<Key, Entry> as 'std::collections::HashMap'", "hold": { "kind": "owns" } }
      ] },
    { "kind": "function", "name": "eve_cache::Cache::get", "visible": "public",
      "params": [
        { "name": "self", "type": "ref<Cache>", "access": { "rights": "read", "guarantee": "readers" } },
        { "name": "key", "type": "ref<Key>", "access": { "rights": "read", "guarantee": "readers" } }
      ],
      "result": { "type": "ref<Entry>?", "hold": { "kind": "borrows", "from": "self" } } },
    { "kind": "function", "name": "eve_cache::Cache::insert", "visible": "public",
      "params": [
        { "name": "self", "type": "ref<Cache>", "access": { "rights": "write", "guarantee": "exclusive" } },
        { "name": "key", "type": "Key" },
        { "name": "entry", "type": "Entry" }
      ] }
  ]
}
```

`weave` turns it and the bare body into the Rust of language § 14.9. It elides the lifetime of `get`, because Rust's
elision rules give the same meaning (`pub fn get(&self, key: &Key) -> Option<&Entry>`); lifetimes are written out only
where elision cannot express them, and § 14.9 now shows the elided form.

---

## 5. `strip`

1. Parse with tree-sitter-rust. Refuse the file on a parse error.
2. Walk the items. For each emptied production (§ 4.3), map its text to a slot: parse the type (§ 4.4), expand elided
   lifetimes by Rust's rules, read `&` and `&mut` as access and lifetimes as holding, and write the slot. Do not descend
   into macro invocations.
3. Cut every emptied production's bytes out of the source, with the whitespace that belonged to it, to form the bare
   body. A typed `let` becomes a `local`; a concrete type in an expression (§ 2.2) becomes `_`, and its type a body
   hint.
4. Parse the bare body with the bare parser, and refuse the file if it fails.
5. Run the lint (§ 3.2) and record what it finds.
6. Write `x.brs` and `x.constraints.json`.

## 6. `weave`

1. Parse `x.brs` with the bare parser; index its items by subject name.
2. Walk the tree, copying every byte. At each emptied production, print the slot from the file; where a slot has no
   value, print nothing if Rust allows that (no visibility means private) and otherwise fail with the slot named.
3. Fill expression holes from `holes`, and leave the rest as `_`.
4. Run `cargo check --message-format=json`. Apply the suggestions that add `mut` (§ 2.3). Map every remaining error back
   to the bare line or the JSON slot it concerns, through a map of every inserted span; an inference error at a hole is
   reported as "hole undetermined" (§ 2.4).
5. Write `x.rs` and the map.

The command line:

```text
bare-rust strip <crate>                  x.rs -> x.brs, x.constraints.json
bare-rust weave <crate> [--out <dir>]    x.brs, x.constraints.json -> x.rs, then cargo check
bare-rust check <crate>                  the checks of § 9 that apply to the stage reached
bare-rust lib   <crate>@<version> | std  the dependency's constraint file (§ 7)
bare-rust print <x.constraints.json>     the text form (§ 11, stage 6)
```

---

## 7. Libraries: constraint files without bare bodies

### 7.1 What an agent needs

An agent writing bare code calls the standard library and its dependencies by name, and `rustc` checks the calls after
weaving, so weaving is correct without any library constraints: if the file is wrong, the code does not compile. But
the agent cannot see what a library promises, and nothing can be optimised on it, without the library's constraints.
It needs those in the same JSON, and never the library's bare body. A partial strip, which writes the constraint file
and discards the body, is enough.

### 7.2 Three sources

- **The surface, from rustdoc.** rustdoc's JSON output gives every public item's signature, generics, bounds, lifetimes
  and visibility; for the standard library it ships as a rustup component (`rust-docs-json`, on nightly), and for any
  crate it is one `cargo +nightly rustdoc` away. A translator from rustdoc JSON (through the `rustdoc-types` crate, at
  a pinned format version) to constraint JSON reuses the type mapping of § 4.4. This covers `std` and every crate, with
  or without source.
- **The bodies, where the source is available.** A partial strip is not limited to the surface: crates from crates.io
  ship their source, and `std`'s comes with the `rust-src` component. `bare-rust lib` runs `strip` over that source in
  constraint-only mode, and looks into bodies for what the signatures do not say: the guards at a function's head, as
  `require … else` and `fails … when` (the lint of § 3.2, applied to someone else's code); the panics a body can reach
  (`panic!`, `unwrap`, indexing, arithmetic in debug builds), as candidate `fails` and `never panics`; and the calls it
  makes, as candidate bounds on effects (`no io`, `no clock`), by a syntactic call graph over the crate.
- **The documentation and annotations.** `# Panics`, `# Errors` and `# Safety` sections give failures and
  preconditions; `#[must_use]` and `const fn` map directly; parts of `std` carry formal contracts from the Verify Rust
  std effort (Kani's `requires` and `ensures`).

### 7.3 Status says what may be relied on

A library is not woven from its file, so its clauses are claims about code the team does not control:

| Source | Status |
| --- | --- |
| Signatures, from rustdoc or from source | `proven`: the library's compiler checked them |
| Guards found in bodies | `lifted`: the library's own code enforces them, in the locked version |
| Contracts verified by Kani | `proven`, with the verification named |
| Documentation | `assumed … because "documentation"` |
| Panics, effects and call-graph bounds found in bodies | `observed`, until an analysis proves them or a person accepts them |

Anything that changes generated code, such as dropping a generated check that a library's postcondition already
guarantees (the discharge of language § 11.1, extended across calls), relies only on `proven` and `lifted` clauses and
on assumptions a person has accepted.

### 7.4 Modules look like libraries from outside

A bare module's constraint file is its public interface: callers depend on the file, never on the body. So the team's
own modules and a library are the same at the boundary, and the agent's view of its dependencies and separate
compilation are one mechanism. Library files are cached by crate and version from `Cargo.lock`, and regenerated when a
version changes.

---

## 8. Corpora

- **Language § 14.9**, the golden test, with `Key` and `Entry` stubbed.
- **The tool's own source**, which strips and weaves itself.
- **Two or three small crates with test suites**, chosen for few macros and procedural macros, stable Rust, and
  explicit signatures.
- **The standard library and the corpora's dependencies**, for `bare-rust lib` only.
- **Later, the engine's Rust procedure kit** (database-engine track): the Rust that the constraint language exists to
  constrain.

---

## 9. Checks

1. **Bare round trip.** `strip(weave(b, c)) == (b, c)` exactly.
2. **Source round trip.** `weave(strip(s)) == s` after `rustfmt` on both sides and with lifetimes elided where Rust
   allows.
3. **Compiles and behaves.** The woven crate passes `cargo check`, `cargo clippy` with the corpus's settings, and its
   tests.
4. **Holes.** Every hole is either filled or reported (§ 2.4); none is filled silently by a guess.
5. **Library files.** For each dependency, the translator's output from rustdoc and the constraint-only strip agree on
   every public signature.

All run in the tool's CI over every corpus.

---

## 10. Experiments

- **E0, language § 14.9.** Bare body and JSON weave into § 14.9's Rust, which compiles, and
  strip back into the same two files.
- **E1, coverage.** Files accepted and refused, by reason; slots filled, by production; the share of types that map to
  the language and the share carried as host text, by form. The output is § 12 with numbers.
- **E2, bare in practice.** Weave every corpus with all `local` types and body hints dropped, keeping only the
  signatures, and count the holes `rustc` cannot infer, per function and by kind. This answers question 14 of the
  language for one host: if few functions need them, bodies stay bare; if many do, principle 5 must give, or the file
  must say more.
- **E3, edits.** Over each corpus's history, strip the older version of a changed file, take the newer bare body, and
  weave it with the older file. Count slots that no longer bind, and holes that become undetermined. Slots attach to
  named items and parameters, so most body edits should leave them untouched; this measures how true that is.
- **E4, agents.** Take tasks from the corpora's histories, some changing bodies only and some changing signatures. An
  agent does each twice: in Rust, and in bare Rust with the JSON and its dependencies' constraint files. Compare
  whether the tests pass, whether the woven code compiles, how many attempts and tokens it took, and how many JSON
  edits the signature changes needed.
- **E5, libraries.** For `std` and the dependencies, count clauses by source and status (§ 7.3), and how many of the
  corpora's lifted checks a library's `proven` or `lifted` clauses would discharge. Repeat part of E4 without the
  library files, to measure what they are worth to an agent.
- **E6, JSON or text, optional.** Repeat part of E4 with the constraints printed as text instead of JSON. This answers
  question 10 of the language.

---

## 11. Stages

Each stage says when it is done; none has a date.

- **Stage 0, language § 14.9.** The grammar delta for structs, functions, parameters, `self` and `let`, with `impl`
  blocks as they come; `strip` and `weave` for exactly the slots § 14.9 uses. Done when E0 passes.
- **Stage 1, the whole signature.** Every emptied production of § 4.3, the type mapping of § 4.4 with its host-text
  fallback, `mut` derived through `rustc`, holes reported, the JSON Schema, and checks 1 to 4 over the corpora. Done
  when the checks pass and E1 is written up.
- **Stage 2, libraries.** The rustdoc translator for `std` and the dependencies, then the constraint-only strip with
  its look into bodies, with statuses as in § 7.3, and check 5. Done when every dependency of the corpora has a file
  and E5's counts are written up.
- **Stage 3, edits and agents.** Errors mapped back to the bare body and the JSON, E2, E3 and E4. Done when the results
  are written up with a recommendation: build a second host, change the design of language § 11.2, or stop.
- **Stage 4, the holes the trial left closed.** `Rc` or `Arc`, `Box`, and trait objects, chosen by `weave` from
  `shares`, `sendable` and the set of implementations (§ 2.2). Only if stage 3 recommends going on.
- **Stage 5, disguised constraints.** The lint's head checks lifted into the file as `require … else`, and woven back as
  generated checks through a macro that `strip` recognises (`y::require!(found, …, Gone)`), so the round trip still
  holds.
- **Stage 6, the text form.** A printer from the JSON to the syntax of language § 15. A parser for the text form only
  if E6 shows agents or reviewers do better with it.

---

## 12. What the trial will feed back to the language

These gaps are visible before anything is built; the trial counts how often each occurs, and so says which are worth
closing.

- **Language § 11.2 now has the split rule, the two rules on holes, and libraries.** The trial tests whether they hold
  as written there.
- **Generic host subjects.** `for T where …` exists (language § 9), but the grammar's `subject_head` has no place for
  it on a host type, function or `impl`.
- **Associated types, `impl Trait` and trait objects** have no counterpart in the type language.
- **`usize` and `isize`** have a width that depends on the target.
- **`unsafe fn`** states an obligation on callers that is not a precondition on values: a precondition the language
  cannot check, which is `assume` seen from the other side.
- **`const fn`** restricts what a body may do, which is a bound on effects (language § 6.3) the bound list does not
  yet name.
- **Body hints**, if E2 shows they are needed, contradict principle 5 as written.

---

## 13. Open questions

1. **A bare form of extraction.** Decided for now: `.unwrap()` and `.expect()` stay in the bare body as written, and
   the presence they assert on an input is stated in the file as a `require` (§ 3.2). Whether bare Rust later needs an
   extraction that asserts nothing, so that the presence is said only once, waits on E4.
2. **Body hints.** Keep them as positional records outside the language, part of the Rust profile, or require bodies to
   be rewritten so that every needed type belongs to a named local? Leaning: records, until E2 says how many there are.
3. **How far to look into library bodies.** A syntactic call graph is cheap and approximate; MIR is exact and costly.
   Leaning: syntactic for the trial, with everything it finds `observed`, and MIR only if E5 shows the clauses are
   worth relying on.
4. **Where library files live.** A shared cache keyed by crate and version, or published beside each crate? Leaning: a
   local cache for the trial; publishing is a question for after it.
5. **After the trial.** If the results are good, TypeScript is the cheapest second host (its compiler already defines
   what erases), and Python with LibCST the one closest to the game code the language exists for.
