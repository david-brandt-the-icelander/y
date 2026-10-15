# Bare Rust: a stripper and a weaver

**Status:** Plan
**Date:** 2026-10-03, rescoped 2026-10-05
**Scope:** A proof of concept, not a complete tool. Stages 0 to 3 (§ 11) are the proof of concept; the rest, including
tooling that generates library constraint files, is for later releases.
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
  library and its dependencies promise. That code is stable: the standard library and packages from a package manager
  are versioned and do not change under the code that uses them. So an agent writes each library's constraint file
  once per version, for the items in use, and `rustc` and a second agent check it; no stripper is needed for it. A
  partial strip that generates library files is the lowest priority of the later releases (§ 7).
- **It is small.** The core of the bare grammar is already written: 21 lines extending tree-sitter-rust 0.24, which
  generate a parser that parses language § 14.9's bare body and rejects its woven form (§ 3.1). The proof of concept
  was estimated at 1,500 lines of Rust; after stage 1 the tool is 4,600, and still cheap to build. Stages 0 to 2 are
  built, for about $3, $11 and $22 at API rates against estimates of $50, $13 and $9. With agents on Claude Sonnet 5.5,
  the proof of concept is estimated at about 0.3 billion tokens and $140, about a quarter of a Claude Max 5x week
  (§ 14).
- **It ends in a decision.** The experiments (§ 10) say whether bare bodies stay bare, whether a file can always fill
  the holes, whether library constraints help an agent, and whether an agent does better with bare Rust and JSON than
  with Rust. Their results decide whether to complete the tool and build a second host, change the design, or stop.
  The proof of concept aims for a clear signal, not for full coverage of Rust or for precise measurements.

---

## 1. What the trial must show

1. **Lossless.** `strip(weave(b, c)) == (b, c)` exactly, and `weave(strip(s)) == bind(s)` up to `rustfmt` and
   lifetime elision, for every file in the corpora; the woven code compiles and passes its tests. `bind` (§ 2.2) gives
   every value that carries a written type a name, without changing what the program does; it is the one rewrite of
   the source the round trip allows.
2. **Host-neutral.** Most of what moves maps onto the constraint language's own clauses and types, not onto Rust text
   carried along unread. What does not map is a list of gaps in the language, with a count for each (§ 12).
3. **Bare in practice.** Bodies need few types of their own. The trial measures how many holes a body leaves that only
   `local`, or something stronger, can fill (language § 17, question 14).
4. **Libraries are visible.** Every external item the corpora call, from the standard library or a dependency, has a
   constraint file entry in the same JSON, with a status on every clause that says how far it can be trusted. An agent
   writes these entries (§ 7.2).
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

**Every constraint has a binding.** A cast or a literal's suffix states a type in the middle of an expression, where
nothing is named for it to attach to. `strip` first binds each one (`bind`): the value initialises a typed `let`, before
its statement or, where the statement would compute it when the source might not, in a new block in that place; and
anything earlier in the statement that has an effect is bound first, in order. The local then carries the type (a cast
local, `{"type": "u64", "cast": true}`, weaves `let x = e;` back to `let x: u64 = e as u64;`). The names are mechanical,
and are renamed once by an agent or a person who knows what the values are. A mid-expression form is an obfuscation in
this view: the round trip is exact on the bound source, not on the original. A macro call whose tokens state a type is
kept as Rust text, `rust! { ... }`, and should be rewritten by hand to name the value first. A closure with typed
parameters is named the same way, and its parameters are keyed by that name (`by_len.a`); a closure's return type goes
onto a local in its body. A value that releases something at its end (a type with `lifetime self released at end`,
language § 7.3: a lock guard, a file) keeps its end where the source has it: `bind` names it and puts the statement in a
block that ends where the statement did. A turbofish is a realization of a template (language § 3.5), named in the body:
`s.parse_u16()` for `s.parse::<u16>()`, with `"parse_u16": {"template": "<_>::parse", "args": ["u16"]}` in the
crate's table of realizations, in the root module's file; the same template with the same arguments has one name across
the crate. A cast to a pointer is a cast local of type `ptr<T>` with its access. A constant context gets a block in
its place, and a `const` whose whole value is a cast carries it on its subject. A suffixed literal in a pattern loses
its suffix where the value it matches is a name declared with that type, and otherwise is named, `_k1_u8 @ 1`, with
the type keyed by that pattern. What cannot be bound yet (a cast to a type alias, the left of a compound assignment whose right
has an effect) keeps `_` in the body and a positional hint; `_` in casts and type arguments is valid Rust, so the bare
form keeps the target's syntax and only its types are erased. The
proof of concept leaves `Rc`, `Arc`, `Box` and trait objects in the body as written and counts them, and keeps every
`.unwrap()` and `.expect()` in the body, as decided (§ 3.2).

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

The lint is a detector in the proof of concept. Lifting head checks into the file, with `weave` putting them back as
generated checks, is for a later release (§ 11).

---

## 4. The files

### 4.1 Beside each module

`src/lib.rs` becomes `src/lib.brs`, the bare body, and `src/lib.constraints.json`. Dependencies' constraint files live
by crate and version, as `Cargo.lock` pins them (§ 7.4).

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
| `*const T`, `*mut T` | `ptr<T>`, with `access : read` or no access | A pointer has no guarantee and no lifetime (language § 8.1) |
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
```

A later release adds `bare-rust print <x.constraints.json>`, which prints the text form (§ 11). Library files have
their own commands: `bare-rust externals`, `libcheck`, `libfmt` and `libstats` (§ 7.2).

---

## 7. Libraries: constraint files without bare bodies

### 7.1 What an agent needs

An agent writing bare code calls the standard library and its dependencies by name, and `rustc` checks the calls after
weaving, so weaving is correct without any library constraints: if the file is wrong, the code does not compile. But
the agent cannot see what a library promises, and nothing can be optimised on it, without the library's constraints.
It needs those in the same JSON, and never the library's bare body.

Library code is stable, unlike the code being written. The standard library and the packages a package manager supplies
are versioned, and a project moves from one version to the next deliberately, not with every edit. A library's
constraint file is therefore written once per version and shared by every project that uses that version. `strip` and
`weave` exist for code that changes with every edit; a library needs neither. An agent writes the file, as stage 2 did,
and that is the method, not a stand-in for tooling.

### 7.2 Written by an agent, once per version

The corpora are chosen to call a limited set of external code: the parts of `std` they use (collections, `Option`,
`Result`, `String`, the common iterator adapters) and the items they call from one or two small dependencies. An agent
writes the constraint file entries for exactly those items, from their documentation and, where it helps, their
source, in the format of § 4.2. A second agent checks each entry against the same documentation and source. The
written files live beside the corpora, by crate and version (§ 7.4).

The procedure, as stage 2 ran it:

1. `bare-rust externals` lists, mechanically, the items each crate uses.
2. Agents write the entries for those items, in batches of about 20 per fresh agent, so that no agent's context grows
   long (§ 14.1).
3. `bare-rust libcheck` checks coverage, the schema and clause syntax, and compiles each function's signature against
   the real item.
4. A second agent reviews every entry against the item's documentation and source.

A file grows with use. When code starts to call an item that has no entry, `bare-rust check` reports it, and an agent
writes the entry. A new version of a library starts from the old version's file and its diff, and re-runs the
checks. Stage 2 cost about $22 for 105 `std` items, about half of it from writing them all in one long agent.

### 7.3 Status says what may be relied on

A library is not woven from its file, so its clauses are claims about code the team does not control:

| Source | Status |
| --- | --- |
| Written by an agent: signatures that `libcheck` compiles against the real item | `proven`: the library's compiler checked them |
| Written by an agent: every other clause | `assumed … because "hand-written"`, after a second agent's review |
| Signatures, from rustdoc or from source | `proven`: the library's compiler checked them |
| Guards found in bodies | `lifted`: the library's own code enforces them, in the locked version |
| Contracts verified by Kani | `proven`, with the verification named |
| Documentation | `assumed … because "documentation"` |
| Panics, effects and call-graph bounds found in bodies | `observed`, until an analysis proves them or a person accepts them |

Anything that changes generated code, such as dropping a generated check that a library's postcondition already
guarantees (the discharge of language § 11.1, extended across calls), relies only on `proven` and `lifted` clauses and
on assumptions a person has accepted. The proof of concept changes no generated code this way, so agent-written clauses
are enough for it. The stage 2 file still marks every clause `assumed`; raising checked signatures to `proven` is a
change to `libcheck` that writes the status it has earned.

### 7.4 Modules look like libraries from outside

A bare module's constraint file is its public interface: callers depend on the file, never on the body. So the team's
own modules and a library are the same at the boundary, and the agent's view of its dependencies and separate
compilation are one mechanism. Library files are kept by crate and version from `Cargo.lock`, and rewritten when a
version changes.

### 7.5 Lowest priority: generated from three sources

Tooling that writes library constraint files mechanically is worth building only where an agent writing them is not
enough: libraries that change faster than their users can rewrite the files, a dependency surface too large to write
by hand at a reasonable cost, or a cross-check on what agents wrote. None of these arises in the trial. If one does,
the tooling draws on three sources.

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

---

## 8. Corpora

- **Language § 14.9**, the golden test, with `Key` and `Entry` stubbed.
- **The tool's own source**, which strips and weaves itself.
- **Two small crates with test suites and some history**, chosen for few macros and procedural macros, stable Rust,
  explicit signatures, and few dependencies, so that the external code they call stays a limited set.
- **The external items the corpora call**, from `std` and the crates' dependencies, for the hand-written library files
  (§ 7.2).
- **Later, the engine's Rust procedure kit** (database-engine track): the Rust that the constraint language exists to
  constrain.

---

## 9. Checks

1. **Bare round trip.** `strip(weave(b, c)) == (b, c)` exactly.
2. **Source round trip.** `weave(strip(s)) == bind(s)` after `rustfmt` on both sides and with lifetimes elided where
   Rust allows.
3. **Compiles and behaves.** The woven crate passes `cargo check`, `cargo clippy` with the corpus's settings, and its
   tests.
4. **Holes.** Every hole is either filled or reported (§ 2.4); none is filled silently by a guess.
5. **Library files.** Every external item the corpora call has an agent-written entry; `bare-rust libcheck` passes
   (coverage, schema, clause syntax, and each signature compiled against the real item); and a second agent has
   reviewed it against the item's documentation and source (§ 7.2).

Checks 1 to 4 and the mechanical half of check 5 run in the tool's CI over every corpus. The review is repeated when an
entry is written or a library's version changes.

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
  agent does each twice: in Rust, and in bare Rust with the JSON and the hand-written library files. Compare
  whether the tests pass, whether the woven code compiles, how many attempts and tokens it took, and how many JSON
  edits the signature changes needed.
- **E5, libraries.** Repeat part of E4 in two variants: without the library files, to measure what they are worth to
  an agent; and with the files stripped of their `unexpressed` clauses, to measure what the promises the language
  cannot yet state are worth to an agent when they are given as prose. The second variant sets the priority of
  closing those gaps ([`language-gaps.md`](language-gaps.md)). Agents also report, per task, which library promises
  they relied on.
- **E6, JSON or text, later.** Repeat part of E4 with the constraints printed as text instead of JSON. This answers
  question 10 of the language, and needs the printer of a later release.

The proof of concept runs E0 to E5 at the sizes of § 14.2, which are large enough to show a clear difference and too
small to measure a small one.

---

## 11. Stages

Each stage says when it is done; none has a date. Stages 0 to 3 are the proof of concept, and each does only what the
experiments need.

- **Stage 0, language § 14.9.** The grammar delta for structs, functions, parameters, `self` and `let`, with `impl`
  blocks as they come; `strip` and `weave` for exactly the slots § 14.9 uses. Done when E0 passes. Done: built in
  `tools/bare-rust`, where E0's golden tests pass (§ 14.1).
- **Stage 1, the signature, as far as the corpora need it.** The emptied productions of § 4.3 that the corpora use,
  the type mapping of § 4.4 with its host-text fallback for the rest, `mut` derived through `rustc`, holes reported,
  the JSON Schema, and checks 1 to 4 over the corpora. A file that uses a production not yet built is refused and
  counted (§ 2.7). Done when the checks pass on the files accepted and E1 is written up. Done: checks 1 to 4 pass on
  all four corpora, 18 files with none refused, and E1 is in `tools/bare-rust/results/e1.md` (§ 14.1).
- **Stage 2, library files, by hand.** Agents write the constraint file entries for the limited set of external code
  the corpora call, with the status of § 7.3, and a second agent checks them (check 5). Done when every external item
  the corpora call has a checked entry. Done: `tools/bare-rust/libraries/std-1.97.0.constraints.json` covers the 105
  `std` items that strsim, fnv and the golden crate use, `bare-rust check` requires that coverage, and an independent
  review passed it after one repair round. The summary is `tools/bare-rust/results/libraries.md` (§ 14.1). The tool's
  own source stays a corpus for checks 1 to 4 only, so its dependencies have no library files.
- **Stage 3, edits and agents.** Errors mapped back to the bare body and the JSON, E2, E3, E4 and E5. Done when the
  results are written up with a recommendation: complete the tool and build a second host, change the design of
  language § 11.2, or stop. Done: errors map to the `.brs` position and the JSON pointer of the slot (`src/errmap.rs`); E2, E3, E4
  and E5 are in `tools/bare-rust/results/` (`e2.md`, `e3.md`, `e4-e5.md`), and `results/stage3.md` recommends
  completing the tool and building a second host, TypeScript first, after making new items cheaper for agents to add.
  The second hosts are planned in [`second-hosts.md`](second-hosts.md).

Later releases, only if stage 3 recommends going on, and in an order its results decide:

- **The whole signature.** Every emptied production of § 4.3 that stage 1 left out, so that no file is refused for a
  production the tool does not know.
- **The holes the trial left closed.** `Rc` or `Arc`, `Box`, and trait objects, chosen by `weave` from `shares`,
  `sendable` and the set of implementations (§ 2.2).
- **Disguised constraints.** The lint's head checks lifted into the file as `require … else`, and woven back as
  generated checks through a macro that `strip` recognises (`y::require!(found, …, Gone)`), so the round trip still
  holds.
- **The text form.** A printer from the JSON to the syntax of language § 15, and E6. A parser for the text form only if
  E6 shows agents or reviewers do better with it.
- **Library tooling, last and only if needed.** The rustdoc translator and the constraint-only strip with its look into
  bodies (§ 7.5), the `bare-rust lib` command, and E5's counts by source. Agent-written files are the method for stable
  code (§ 7.2); this tooling is built only for one of the cases § 7.5 names, and the agent-written files are its first
  test.

---

## 12. What the trial will feed back to the language

These gaps are visible before anything is built; the trial counts how often each occurs, and so says which are worth
closing. E1's counts for stage 1 are in `tools/bare-rust/results/e1.md`: 93% of type slots map to the type language,
and the rest are mostly slices, lifetime arguments and arrays.

Stage 2 adds the library side. Of the 303 promises written by hand for 106 `std` subjects, 104 are stated in the
language and 199 are not, each with a labelled reason (`tools/bare-rust/results/libraries.md`). The largest groups:
types, traits and impls (101: no impl or associated type 38, promises that hold unless an impl overrides a default body
22, must-use on a type 14); host state and effects (40); text, numbers and const-ness (29); sequences and collections
(22, mostly no sequence value). Two findings concern the file rather than the language: the externals lists name a
trait method but never the impl a call resolves to, and the file has no clause kinds for `budget` and `layout`, which
language § 5 has. How each gap is to be addressed is planned in [`language-gaps.md`](language-gaps.md).

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
3. **How far to look into library bodies,** once the tooling of § 7.5 is built. A syntactic call graph is cheap and
   approximate; MIR is exact and costly. Leaning: syntactic first, with everything it finds `observed`, and MIR only if
   E5's counts show the clauses are worth relying on.
4. **Where library files live.** A file is written once per version and serves every project that uses that version,
   so where it is kept decides how often the work is repeated. A shared store keyed by crate and version, or published
   beside each crate? Decided for the proof of concept: beside the corpora (§ 7.2). Leaning, after it: one shared store
   for the studio, which every project reads and any project's agent extends.
5. **After the trial.** If the results are good, TypeScript is the cheapest second host (its compiler already defines
   what erases), and Python with LibCST the one closest to the game code the language exists for.

---

## 14. Tokens and cost

Prices are API rates per million tokens, as of October 2026. Thinking is billed as output.

| Model | Input | Output | Cache reads | Cache writes, 5 minutes | Cache writes, 1 hour |
| --- | --- | --- | --- | --- | --- |
| Claude Sonnet 5.5 | $2 | $10 | $0.20 | $2.50 | $4 |
| Claude Opus 5.5 | $4 | $20 | $0.20 | $5 | $8 |

### 14.1 Stages 0 to 2, measured

#### Stage 0

Stage 0 was built by a workflow: one implementer, then an independent checker that ran the golden tests, the CLI and a
round-trip probe with odd spacing and comments. Both agents ran on Claude Sonnet 5.5 at effort `low`. That is the
cheapest model and effort that are still plainly more than the job needs: about 500 lines of well-specified Rust,
starting from a grammar already shown to work, with golden files to test against. Claude Haiku 4.5 is cheaper, but
debugging tree-sitter conflicts and Rust build errors is where it is likely to struggle. The checker passed the first
build, so no repair round ran. An Opus 5.5 session orchestrated: it checked the toolchain, wrote the workflow, verified
the result independently, and tallied the cost.

| Part | Calls | Cache writes | Cache reads | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| Implementer, Sonnet 5.5 `low` | 6 | 74K | 293K | about 8.5K visible | $0.33 to $0.40 |
| Checker, Sonnet 5.5 `low` | 5 | 57K | 204K | about 1K visible | $0.19 to $0.22 |
| Orchestration, Opus 5.5 | 22 | 43K, 1-hour | 8.25M | about 7K visible | $2.15 to $2.75 |
| **Stage 0** | **33** | **174K** | **8.75M** | **about 16K visible** | **about $3, or 9M tokens** |

The input-side figures come from each call's recorded usage. Output is not recorded per call, and the thinking text is
omitted, so output is the visible text of each call, about 3.5 characters a token, plus a bound on the unseen
thinking. The ranges in the cost column come from that bound.

Against the first estimate:

- **$3 against $50, and 9M tokens against 45M.** The workflow alone used 0.6M tokens and about $0.6, about 1% of the
  stage's estimate. The implementer wrote the whole crate in one command and passed its own tests at once. The estimate
  had assumed 3 sessions of 150 turns each.
- **Orchestration cost more than the build.** Each orchestrating call re-read about 375K tokens of this long
  conversation. A fresh, short session per stage would cut that to a fraction.
- **Stage 0 was the easiest stage.** It had a spike to start from, golden files written in advance, and nothing to
  debug against real code. Later stages have none of these, so the units below are larger than stage 0's, not scaled
  down from it.

#### Stage 1

Stage 1 was built by Claude Sonnet 5.5 agents run directly from the orchestrating session, not through a workflow, so
their effort was the default rather than `low`. Two build agents ran in turn: part A built `strip` and `weave` for the
corpora's signatures, with the schema and checks 1 and 2; part B added `cargo`, `mut` derivation, hole reports, checks 3
and 4, and E1. An independent checker found no blocking defect and five minor ones (comments lost inside erased
productions, a stale E1 table, check 3 not running the tool's own tests, stale file entries ignored silently, `pub use`
keeping its `pub`), and one repair agent fixed them all. The orchestrating Opus 5.5 session verified each part, ran the
checks itself, committed, and tallied the cost up to that point. The tool grew from 480 to 4,600 lines.

| Part | Calls | Cache writes | Cache reads | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| Build, part A | 48 | 311K | 9.81M | about 48K visible | $3.2 to $3.7 |
| Build, part B | 69 | 209K | 11.42M | about 41K visible | $3.2 to $3.6 |
| Independent check | 50 | 99K | 4.87M | about 8K visible | $1.3 to $1.4 |
| Repair | 38 | 159K | 5.31M | about 15K visible | $1.6 to $1.8 |
| Orchestration, Opus 5.5 | 25 | 45K, 1-hour | 3.49M | 21K | $1.47 |
| **Stage 1** | **230** | **0.82M** | **34.9M** | **about 0.13M** | **$11 to $12, or 36M tokens** |

Output is counted as for stage 0: visible text at about 3.5 characters a token, with the unseen thinking bounded at as
much again for the agents; the orchestrating session's output is recorded.

Against the estimate of § 14.2 (5 build runs and 5 check runs, 19M tokens and $10 for agents, $3 to orchestrate, $13 in
all):

- **About $11 against $13, and 36M tokens against 27M** (19M for agents and about 8M to orchestrate). Three build runs
  and one check run did the work of five and five.
- **Each run was larger than the unit, at the same price.** The build runs made 38 to 69 calls at an average context
  of 140K to 205K tokens, against the unit's 40 calls at 80K, so they used two to three times the unit's tokens. Almost
  all of that is cache reads, which cost a tenth of fresh input, so each build run cost about twice the unit and there
  were fewer of them.
- **Orchestration was cheaper than in stage 0:** $1.47 against $2.75 at most. The conversation had been compacted, so
  each orchestrating call re-read about 140K tokens instead of 375K.

#### Stage 2

Stage 2 was run like stage 1, with Sonnet 5.5 agents at the default effort. Part A built `bare-rust externals` (the
`std` items each corpus uses, read from rustc's MIR), the library file format and the mechanical half of check 5
(`bare-rust libcheck`: coverage, schema, and each function's signature compiled against the real item). Part B wrote
the entries for all 105 items, from the 1.97.0 source on GitHub. An independent reviewer found nothing the corpora rely
on to be false, but found overclaims from overridable default bodies, ill-formed clauses and weak citations; one repair
agent fixed them all and added a clause lint.

| Part | Calls | Cache writes | Cache reads | Output | Cost |
| --- | --- | --- | --- | --- | --- |
| Build, part A: tooling and format | 59 | 286K | 11.07M | about 42K visible | $3.4 to $3.8 |
| Build, part B: 105 entries | 130 | 435K | 38.30M | about 63K visible | $9.4 to $10.0 |
| Independent review | 34 | 149K | 4.06M | about 5K visible | $1.2 to $1.3 |
| Repair | 87 | 300K | 18.91M | about 29K visible | $4.8 to $5.1 |
| Orchestration, Opus 5.5 | 30 | 57K, 1-hour | 6.10M | 22K | $2.11 |
| **Stage 2** | **340** | **1.23M** | **78.4M** | **about 0.16M** | **$21 to $22, or 80M tokens** |

Against the estimate of § 14.2 (3 build runs and 3 check runs, 11M tokens and $6 for agents, $3 to orchestrate, $9 in
all):

- **About $22 against $9, and 80M tokens against 19M.** The estimate is the first one the trial has missed.
- **Long agents cost the most.** Part B wrote all 105 entries in one agent of 130 calls, whose context grew to an
  average of about 295K tokens, so its cache reads alone cost about $7.70. The repair agent averaged about 220K. The
  work was the size expected; what it cost was set by how long each agent ran. Writing the entries in batches of about
  20, each in a fresh agent, would have cost about half as much.
- **The repair round was larger than in stage 1,** because the review checked meaning, not only mechanics: 119 stated
  clauses against their sources. Writing claims about someone else's code is where a second agent earns its cost.

#### Stage 3

Stage 3 was run with Sonnet 5.5 agents at the default effort:

- two build agents, one for the fixes, error mapping, E2 and E3, and one for the E4 and E5 harness;
- 124 experiment runs, of which 4 were pilots.

The orchestrating Opus session checked the work and graded every run. The details are in
`tools/bare-rust/results/stage3.md`.

| Part | Calls | Cache writes | Cache reads | Cost |
| --- | --- | --- | --- | --- |
| Build, two agents | 107 | 0.60M | 13.7M | $4.3 to $5.3 |
| Experiment runs | about 700 | 1.75M | 31.6M | $11 to $13 |
| Orchestration, Opus 5.5 | about 120 | | | about $5, estimated |
| **Stage 3** | | | **about 47M** | **about $20 to $23** |

That is against the estimate of § 14.2 of about $103. The experiment runs averaged 6 calls and 270K tokens, a seventh
of the unit, because the tasks were small.

### 14.2 The rest, priced on Sonnet

Build and experiment agents run on Claude Sonnet 5.5 at effort `low`, and an Opus 5.5 session orchestrates, as in stage
0. Three units:

| Unit | Calls | Average context | Tokens | Cache writes | Output | Cost |
| --- | --- | --- | --- | --- | --- | --- |
| A build run: an implementer or repair agent on one part of a stage | 40 | 80K | 3.2M | 0.16M | 60K | $1.61 |
| A check run: an independent checker | 10 | 60K | 0.6M | 0.07M | 8K | $0.36 |
| An experiment run: an agent doing one task from a corpus's history (E4 to E6) | 20 | 65K | 1.3M | 0.10M | 25K | $0.74 |

These units are larger than stage 0's measured runs (6 and 5 calls) to allow for debugging against real crates.
Orchestration is $3 a stage, as measured, or $6 for stage 3; its tokens, about 8M a stage in a long session, are not in
the Tokens column. The Max 5x column is explained in § 14.3.

The proof of concept, stages 0 to 3 at the scope of § 11:

| Stage | Build and check runs | Experiment runs | Tokens | Cost | Max 5x week | Same units on Opus 5.5 `high` | Before rescoping |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 0, language § 14.9 (measured) | 1 and 1 | — | 9M | $3 | 0.6% | — | $3 |
| 1, the signature, as far as the corpora need it (measured) | 3 and 1 | — | 36M | $11 | 2.2% | — | $22 |
| 2, library files, by hand (measured) | 3 and 1 | — | 80M | $22 | 4.2% | — | $13, for the tooling |
| 3, edits and agents: E4 at 20 tasks × 2 arms × 2 trials, E5 at 10 × 2 in 2 variants | 4 and 4 | 120 | 171M | $103 | 20% | $200 | $236 |
| **The proof of concept** | | **120** | **about 0.3B** | **about $140** | **about 27%** | **about $235** | **about $275** |

The "same units on Opus 5.5 `high`" column prices the same runs at Opus rates, with half again as much output for the
higher effort. "Before rescoping" is this plan's previous estimate, on Sonnet, for the stage as it was then (§ 11 had
the library tooling as stage 2, and E4 at 40 tasks × 2 arms × 3 trials); the first estimate, on Opus, was about $1,200.

Later releases, only if the proof of concept recommends going on, priced with the same units:

| Release | Build and check runs | Experiment runs | Tokens | Cost | Max 5x week |
| --- | --- | --- | --- | --- | --- |
| The whole signature | 3 and 3 | — | 11M | $9 | 1.7% |
| The holes left closed | 4 and 4 | — | 15M | $11 | 2% |
| Disguised constraints | 3 and 3 | — | 11M | $9 | 1.7% |
| The text form, with E6 at 20 × 3 | 1 and 1 | 60 | 82M | $49 | 9% |
| **Later releases** | | **60** | **about 0.12B** | **about $80** | **about 15%** |
| Library tooling, only if needed (§ 7.5) | 5 and 5 | — | 19M | $13 | 2.5% |

- **The experiments are most of the cost.** E4's 80 runs cost about $60, and E5's 40 about $30. Running E4's agents on
  Opus 5.5 instead would double that, and would answer a different question: whether bare Rust helps the strongest
  agent rather than the cheaper one.
- **Hand-written library files cost more than estimated,** $22 against $9 (§ 14.1), but still about the price of the
  tooling's first estimate, and they now say what that tooling must produce.
- **Stage 3 keeps its agents short.** Its experiment runs are already one task per fresh agent. Its build work, the
  error mapping and the harnesses for E2 to E5, is split into parts small enough that no agent's context grows past
  about 150K tokens, which is what kept stage 1 on its estimate and what stage 2's part B lacked.
- **The range is still wide.** Each unit could be off by a factor of two either way, so the proof of concept lies
  between about $85 and $240, since stages 0 to 2 are measured. Each stage's measured cost corrects the next stage's
  estimate.
- **Not counted:** people's time, and the machines that run the mechanical work. `strip`, `weave`, the checks, and
  experiments E0 to E3 are programs and cost no tokens.

### 14.3 As a share of a Claude Max 5x week

Anthropic does not publish the Max 5x weekly limit as a number: the plan has a weekly limit across all models, and the
current state is visible only in each account's usage settings. As a proxy, this plan uses one third-party
measurement, which put a full Max 5x week at about $523 of usage at API rates. That figure could not be checked from
here: its date, the models it measured, and how Anthropic weighs token types against the limit are unknown. Weekly
limits were also raised in September 2026, which would make the share smaller than shown. Read the column as an order
of magnitude.

On that proxy:

- **Stage 0 used about 0.6% of a week,** and its workflow alone about 0.1%. **Stage 1 used about 2.2%, and stage 2
  about 4.2%.**
- **The proof of concept needs about a quarter of a week on Sonnet,** or about two fifths on Opus 5.5 `high`. Before
  rescoping it needed about half a week, and the first estimate more than two weeks.
- **The later releases would need about another sixth of a week,** and library tooling, if it is ever needed, about
  another 2.5%.
- **Opus is likely to draw on the limit faster than its API price suggests.** One third-party report counts an Opus
  hour as about 2.5 Sonnet hours against the shared weekly limit. That is another reason to run the agents on Sonnet
  and keep Opus to orchestration.
