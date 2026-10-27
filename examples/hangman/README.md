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
| Y source | `y/src/hangman.y`, `y/src/hangman.test.y`, `y/src/main.y` | 227, 280, 17 | hand |
| Bare bodies | `bare/src/hangman.brs`, `bare/src/hangman/tests.brs`, `bare/src/main.brs`, `bare/src/lib.brs` | 265, 168, 43, 3 | hand, as the Y compiler would lower the source |
| Constraint files | `bare/src/*.constraints.json` (46 subjects, 19 clauses) | 398 | the same, then `bare-rust libfmt` for the canonical form |
| Y facts | `bare/y-facts.json` | 345 | the same |
| Rust | `src/hangman.rs`, `src/hangman/tests.rs`, `src/main.rs`, `src/lib.rs` | 265, 168, 43, 3 | `bare-rust weave` |
| Y runtime | `../y-runtime/src/lib.rs` | 225 | hand: a small stand-in for the runtime that lowered code calls |

The Y package has two modules. `hangman` holds the game, and `main` is the entry point. They lower to one Rust package:
a library crate with the module `hangman`, and a binary crate whose `main` calls it.

## How each Y construct is lowered

| Y | Bare body | Constraint file or `y-facts.json` | Rust |
|---|---|---|---|
| `fn f(x: T) -> R` | `fn f(x)` | a `function` subject: `params`, `result` | `fn f(x: T) -> R` |
| `x: T` (moves in), `&T`, `&mut T` | the name | `type`, and `access` read/readers or write/exclusive | `T`, `&T`, `&mut T` |
| `R ! E`, `fail .Case(...)`, `?` | `return Err(E::Case { .. })`, `?` | `result.fails` | `Result<R, E>` |
| `?` converting through the one case that carries the failure | `?` | an `impl From` subject | `impl From<ConsoleError> for PlayError` |
| `type Word = Text where letters_only = ..., long_enough = ...` | `struct Word(_)` and a constructor that checks each constraint in order | two `invariant` clauses, with status `lifted` by the constructor | a newtype with `Word::new(..) -> Result<Word, ConstraintViolation>` |
| `record`, closed sum type | `struct`, `enum` with named fields | `fields`, `variants` | the same, deriving the structural equality Y always derives |
| No `with` clause: the function is pure | — | `effects` clause `pure` | — |
| `&mut` parameter and no `with` | — | `no io`, `no clock`, `no randomness`, `mutates only game.hits, game.misses` | — |
| `with console: Console, random: Random, forever: Forever` | three more parameters | their types and access; `effects only console, random`; the clause also in `y-facts.json` `capabilities` | `&mut Console, &mut Random, &Forever` |
| `with system: System` on the entry point | `System::root()` | `y-facts.json` `capabilities` | the same |
| `requires name = (p)` | `require(p, "hangman::f#name")` | a `require` clause, status `lifted` by that call | a run-time check whose failure is a fault (a panic naming the construct) |
| `invariant name = (p)` on a record | `invariant(p, "hangman::Game#name")` after the one write that can break it | an `invariant` clause on the type | the same |
| `if name = (p)`, `while name = (p)` | `if p { // y: #name` | `y-facts.json` `decisions`: the Y line and the bare line of each | the same, with the comment |
| A condition that narrows (`if not_one_rune = (letter == .None) { fail ... }`) | `let Some(letter) = letter else { ... }` | — | let-else, so rustc checks the narrowing |
| `forever because "..."` | the `&Forever` argument of the call that waits | `y-facts.json` `claims` | `console.read_line(forever)` |
| `compile parse_word_list(WORD_LIST)` | the value, as a `const` array, and a `static` built once from it | `y-facts.json` `compiled` | the same, and a generated test that makes the call at run time and compares |
| `Count<Element>`, `Count<Rune>` | — (units are erased) | `y-facts.json` `units` | `usize` |
| `Text`, `&Text`, `Rune`, `List<T>`, `Set<T>` | — | `text`, `ref<text>`, `Rune`, `list<T> as 'Vec'`, `set<T> as 'BTreeSet'` | `String`, `&str`, `y_runtime::Rune`, `Vec<T>`, `BTreeSet<T>` |
| Interpolation, `"{name}"` and `"{game.word}"` | `format!("{name}")`, `format!("{}", game.word)` | — | the same |
| `var` | `let` | — | `let mut`, which `weave` derived through rustc (18 of them) |
| `test name covers a, b { given / with / when / then }` | `#[test] fn name()`; a `?` in `given` or `when` lowers to `expect`; `fake Console(input: [...])` to `Console::fake(...)` | `y-facts.json` `tests`: what each covers; repeated in a comment in the test | the same |
| `/// ...` with roles | `///` with the roles as a list | — | the same |

## What had to be assumed

`Y-proposal.md` does not settle everything a whole program needs. Where it is silent, the example assumes the
following:

- **Standard capabilities and their methods:** a `Console` capability with `write_line`, `write` and `read_line`;
  `Random.index_below`; and `System.console()`, `System.random()` and `System.forever()`. The proposal lists `Clock`,
  `Random`, `FileSystem` and others, but not a terminal.
- **The entry point** is `pub fn main() -> Void ! E with system: System`.
- **One `with` block can bind several capabilities:** `with console = ..., random = ..., forever = ... { ... }`.
- **A refined type can have several named constraints,** separated by commas after one `where`.
- **Rune literals** are written `'z'`, as in `value.count_of('@')` in the proposal.
- **Library names:** `Set.of([...])`, `List.join(separator:)`, `Text.single_rune()`, `Text.split(...)`, `Text.trim()`.
- **Fakes for tests:** `fake Console(input: [...], read_fails: ...)`, `fake Random(seed: ...)` and `fake Forever`.
- **Placement of `forever`:** `forever because "..."` follows the waiting call, as `within 5s` does.

## Where the runtime is simpler than Y

`../y-runtime` is only as large as this example needs, and each shortcut is stated on the item that takes it:

- **`Rune`** holds one Unicode scalar value, where Y's is an extended grapheme cluster. They are the same for the
  words here, which a `Word` limits to `a` to `z`.
- **`Set`** lowers to `BTreeSet`, which iterates in sorted order where Y iterates in insertion order. The game never
  iterates a set, so the difference cannot be seen here.
- **`Random`** seeds from the clock without recording the seed, so a game cannot be replayed (Y § 4.7).

## Checks

- **`bare-rust weave`** of `bare/` (with `--crate .` and an output folder beside this one, so that the path to
  `../y-runtime` resolves): 4 files woven, 0 weave errors, 18 `mut` derived, 0 holes undetermined, 0 warnings.
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
