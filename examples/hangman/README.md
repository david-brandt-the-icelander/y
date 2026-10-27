# Hangman, from Y to Rust

The old pencil-and-paper game, written in Y, lowered to bare Rust plus its constraint files, and woven into Rust that
builds, runs and passes its tests.

No Y compiler exists yet, so one step was done by hand: the lowering from Y to bare Rust. It follows the rules in the
table below, as a compiler would. The other steps are the real tools.

```
y/src/*.y  ──lowering (by hand)──▶  bare/src/*.brs + bare/src/*.constraints.json + bare/y-facts.json
                                     ──bare-rust weave (cargo)──▶  src/*.rs  ──cargo──▶  the game
```

## Playing it

```
cargo run          # in this folder
cargo test         # the lowered Y tests, and the check of the one `compile`
```

A game shows the gallows, the word with the unknown letters as underscores, and the misses so far, then asks for a
letter. Six misses hang the man.

## Files

| Stage | Files | Lines | Written by |
|---|---|---|---|
| Y source | `y/src/hangman.y`, `y/src/hangman.test.y`, `y/src/main.y` | 226, 211, 16 | hand |
| Bare bodies | `bare/src/hangman.brs`, `bare/src/hangman/tests.brs`, `bare/src/main.brs`, `bare/src/lib.brs` | 285, 168, 43, 3 | hand, as the Y compiler would lower the source |
| Constraint files | `bare/src/*.constraints.json` (47 subjects, 19 clauses) | 386 | the same, then `bare-rust libfmt` for the canonical form |
| Y facts | `bare/y-facts.json` | 353 | the same |
| Rust | `src/hangman.rs`, `src/hangman/tests.rs`, `src/main.rs`, `src/lib.rs` | 285, 168, 43, 3 | `bare-rust weave` |
| Y runtime | `../y-runtime/src/lib.rs` | 225 | hand: a small stand-in for the runtime that lowered code calls |

The Y package has two modules. `hangman` holds the game, and `main` is the entry point. They lower to one Rust package:
a library crate with the module `hangman`, and a binary crate whose `main` calls it.

## How each Y construct is lowered

| Y | Bare body | Constraint file or `y-facts.json` | Rust |
|---|---|---|---|
| `function f(x, mutable y) returns r fails e`, with `x: …, as t.` and `r: …, as u.` in the documentation above | `fn f(x, y)` | a `function` subject: `params` with the types from the documentation, `result` with `fails` | `fn f(x: &T, y: &mut Y) -> Result<U, E>` |
| `x`, read only (Y's default) | the name | `type` `ref<t>`, `access` read/readers | `&T`, a shared borrow |
| `x` that the function keeps (`new_game(word)`) | the name | `type` `t` | `T` by value; the caller's last use moves it, and any other use is a copy |
| `mutable x` in the signature, and `mutable x` at the call | the name; `&mut x` at the call | `type` `ref<t>`, `access` write/exclusive | `&mut T` |
| A copy the compiler had to make (`new_game(words[chosen])`, where `words` is still read) | `.clone()`, marked `// y: a copy` | `y-facts.json` `copies` | the same |
| `return result = value`, `fail .case(...)`, `?` | `return Ok(value)` or the last expression, `return Err(E::Case { .. })`, `?` | `result.fails` | the same |
| `?` converting through the one case that carries the failure | `?` | an `impl From` subject | `impl From<ConsoleError> for PlayError` |
| `lower_snake_case` type and case names (`guess_error`, `.not_one_letter`) | `GuessError`, `NotOneLetter`: the lowering's one renaming, to Rust's casing | Rust names | the same |
| A constant group, `public constants game_rules:` with `allowed_misses is count<element> = 6` | a module `game_rules` holding `const ALLOWED_MISSES = 6;`; uses are `game_rules::ALLOWED_MISSES` | a `module` subject and its `const` subjects | the same |
| `type word is string where letters_only = ... and long_enough = ...` | `struct Word(_)` and a constructor that checks each constraint in order | two `invariant` clauses, with status `lifted` by the constructor | a newtype with `Word::new(..) -> Result<Word, ConstraintViolation>` |
| `record`; `enumeration` (`outcome`, `verdict` and the failures `word_list_error`, `guess_error`); a `type` with `case`s that carry data (`play_error`) | `struct`; `enum` with unit variants; `enum` with named fields | `fields`, `variants` | the same, deriving the structural equality Y always derives |
| Blocks: a colon and indentation | braces, as Rust has | — | the same |
| No `with` clause: the function is pure | — | `effects` clause `pure` | — |
| A `mutable` parameter and no `with` | — | `no io`, `no clock`, `no randomness`, `mutates only game.hits, game.misses` | — |
| `with console, random, forever`, typed in the documentation | three more parameters | their types and access; `effects only console, random`; the clause also in `y-facts.json` `capabilities` | `&mut Console, &mut Random, &Forever` |
| `with system` on the entry point | `System::root()` | `y-facts.json` `capabilities` | the same |
| `requires name = (p)` | `require(p, "hangman::f#name")` | a `require` clause, status `lifted` by that call | a run-time check whose failure is a fault (a panic naming the construct) |
| `invariant name = (p)` on a record | `invariant(p, "hangman::Game#name")` after the one write that can break it | an `invariant` clause on the type | the same |
| `if name = (p)`, `while name = (p)` | `if p { // y: #name` | `y-facts.json` `decisions`: the Y line and the bare line of each | the same, with the comment |
| A value chosen by a condition: `variable head = " "`, then `if has_head = (misses >= 1) { head = "O" }` (Y's blocks give no value) | the same shape | — | the same |
| A condition that narrows (`if not_one_rune = (letter == .none) { fail ... }`) | `let Some(letter) = letter else { ... }` | — | let-else, so rustc checks the narrowing |
| `forever because "..."` | the `&Forever` argument of the call that waits | `y-facts.json` `claims` | `console.read_line(forever)` |
| `compile parse_word_list(game_rules.word_list)` | the value, as a `const` array, and a `static` built once from it | `y-facts.json` `compiled` | the same, and a generated test that makes the call at run time and compares |
| `count<element>`, `count<rune>` | — (units are erased) | `y-facts.json` `units` | `usize` |
| `string` (and a read-only `string` parameter), `rune`, `list<t>`, `set<t>` | — | `text`, `ref<text>`, `Rune`, `list<T> as 'Vec'`, `set<T> as 'BTreeSet'` | `String`, `&str`, `y_runtime::Rune`, `Vec<T>`, `BTreeSet<T>` |
| Interpolation, `"{name}"` and `"{game.word}"` | `format!("{name}")`, `format!("{}", game.word)` | — | the same |
| `variable` | `let` | — | `let mut`, which `weave` derived through rustc (24 of them) |
| `test name covers a, b { given / with / when / then }` | `#[test] fn name()`; a `?` in `given` or `when` lowers to `expect`; `fake console(input: [...])` to `Console::fake(...)` | `y-facts.json` `tests`: what each covers; repeated in a comment in the test | the same |
| Documentation: the `--` line, or the block between two rules of dashes, above a declaration, with roles; the types leave it for the signature | `///` with the roles as a list | — | the same |

## What had to be assumed

`Y-proposal.md` does not settle everything a whole program needs. Where it is silent, the example assumes the
following:

- **Standard capabilities and their methods:** a `console` capability with `write_line`, `write` and `read_line`;
  `random.index_below`; and `system.console()`, `system.random()` and `system.forever()`. The proposal lists `clock`,
  `random`, `file_system` and others, but not a terminal.
- **The entry point** is `public function main() fails e with system`.
- **One `with` block can bind several capabilities:** `with console = ..., random = ..., forever = ... { ... }`.
- **Rune literals** are written `'z'`, as in `value.count_of('@')` in the proposal.
- **Library names:** `set.of([...])`, `list.join(separator:)`, `string.single_rune()`, `string.split(...)`, `string.trim()`.
- **Fakes for tests:** `fake console(input: [...], read_fails: ...)`, `fake random(seed: ...)` and `fake forever`.
- **Placement of `forever`:** `forever because "..."` follows the waiting call, as `within 5s` does.

## Where the runtime is simpler than Y

`../y-runtime` is only as large as this example needs, and each shortcut is stated on the item that takes it:

- **`rune`** (Rust `Rune`) holds one Unicode scalar value, where Y's is an extended grapheme cluster. They are the same for the
  words here, which a `word` limits to `a` to `z`.
- **`set`** lowers to `BTreeSet`, which iterates in sorted order where Y iterates in insertion order. The game never
  iterates a set, so the difference cannot be seen here.
- **`random`** seeds from the clock without recording the seed, so a game cannot be replayed (Y § 4.7).

## Checks

- **`bare-rust weave`** of `bare/` (with `--crate .` and an output folder beside this one, so that the path to
  `../y-runtime` resolves): 4 files woven, 0 weave errors, 24 `mut` derived, 0 holes undetermined, 0 warnings.
- **`cargo test`:** the 18 lowered Y tests and the generated check of the `compile` pass.
- **`cargo clippy --all-targets`:** no warnings, here and in `../y-runtime`.
- **Coverage:** the tests declare all 45 outcomes of the module. These are both sides of each of the 19 named decisions,
  plus the 7 failure cases, which is what a Y release build requires (Y § 3.14).
- **`tools/bare-rust/tests/y_examples.rs`** keeps the stages in step without cargo:
  - Weaving the committed bare files gives the committed Rust, apart from the derived `mut`.
  - Stripping the committed Rust gives the committed bare bodies byte for byte, and the same subjects and holes. What
    the lowering adds beyond `strip` is only the clauses and the file status.
  - The constraint files are valid, and their clauses are well formed. Clause texts are checked by the same linter as
    library files.
  - `y-facts.json` agrees with the Y source and the bare bodies.
- **`bare-rust check` does not run on this crate.** It copies the crate into a work folder, where the relative path
  to `../y-runtime` no longer resolves, so its woven copies cannot build.

## What the example showed

- **The bare form is a workable target for a Y compiler.** The hand lowering was written to the format's rules, and
  stripping the woven Rust gave it back byte for byte. The compiler would write bare bodies and constraint files, and
  `weave` would do the rest, including deriving `mut`, which Y does not have.
- **The constraint file holds the types and contracts, but not everything Y states.** Six kinds of fact had no slot,
  so they went into `y-facts.json`:
  1. units;
  2. named decisions;
  3. capabilities as Y declares them;
  4. claims;
  5. `compile` sites;
  6. what each test covers.

  The constraint language already has units (`quantity`, `unit`) and could hold the others as clauses. The JSON format
  has no place for them yet.
- **The clause linter needed three additions.**
  - It now knows `pure`, `no device` and `devices only`, which § 6.3 added.
  - In a module's file, a clause may name the module's constants, and call its functions that are bound `pure` or
    `effects none`. That is how `require still_playing : outcome(game) is Playing` names `outcome`.
- **Narrowing costs nothing to lower.** A Y condition that narrows becomes a Rust let-else, so rustc checks the fact
  the Y checker relied on.
- **A named decision's identity survives only as a comment.** A real backend would also carry it into traces and fault
  reports.
