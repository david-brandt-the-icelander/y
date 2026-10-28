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
| Y source | `y/src/hangman.y`, `y/src/hangman.test.y`, `y/src/main.y` | 290, 243, 19 | hand |
| Bare bodies | `bare/src/hangman.brs`, `bare/src/hangman/tests.brs`, `bare/src/main.brs`, `bare/src/lib.brs` | 309, 181, 44, 3 | hand, as the Y compiler would lower the source |
| Constraint files | `bare/src/*.constraints.json` (53 subjects, 20 clauses) | 432 | the same, then `bare-rust libfmt` for the canonical form |
| Y facts | `bare/y-facts.json` | 518 | the same |
| Rust | `src/hangman.rs`, `src/hangman/tests.rs`, `src/main.rs`, `src/lib.rs` | 309, 181, 44, 3 | `bare-rust weave` |
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
| `return answer = value`, `fail guess_error.empty`, `?` | `return Ok(value)` or the last expression, `return Err(E::Case { .. })`, `?` | `result.fails` | the same |
| `?` converting through the one case that carries the failure | `?` | an `impl From` subject | `impl From<ConsoleError> for PlayError` |
| A facet: `public facet setup:` makes its members visible to other modules, and its name is part of their paths (`hangman.setup.parse_word_list`); `shown_rune` and `part`, outside every facet, stay private | a module `mod setup { use super::*; … }`, so the Rust path keeps the facet too; the parent imports the facets it uses, `use self::{display::*, rounds::*, setup::*};`, so the members reach one another by their own names, as in Y | a `module` subject per facet, and `visible` on each member; `y-facts.json` `facets`: each facet, its path, its visibility and its members | `pub mod setup`; members `pub`, helpers private to `hangman`, which its facet modules can call |
| An import by path, `from hangman import setup.parse_word_list, console_play.play` | `use hangman_example::hangman::setup::Word;` and so on | — | the same |
| `lower_snake_case` type and case names (`guess_error`, `guess_error.not_one_letter`) | `GuessError`, `NotOneLetter`: the lowering's one renaming, to Rust's casing | Rust names | the same |
| A constant group, `constants game_rules:` with `allowed_misses is count<element> = 6` | a module `game_rules` holding `const ALLOWED_MISSES = 6;`; uses are `game_rules::ALLOWED_MISSES` | a `module` subject and its `const` subjects | the same |
| `type word is string:` with the block `where letters_only = value.matches(pattern"^[a-z]*$") and long_enough = ...` | `struct Word(_)` and a constructor that checks each constraint in order | two `invariant` clauses, with status `lifted` by the constructor | a newtype with `Word::new(..) -> Result<Word, ConstraintViolation>` |
| `structure` with fields `word is word`, private to the module: `pub(super)` from the facet's Rust module; a constant group used as a closed type (`outcome`, `verdict` and the failures `word_list_error`, `guess_error`: named values with no order, so not enumerations); a `type` with `case`s that carry data (`play_error`, `case console(error is console_error)`) | `struct`; `enum` with unit variants; `enum` with named fields | `fields`, `variants` | the same, deriving the structural equality Y always derives |
| Blocks: a colon and indentation | braces, as Rust has | — | the same |
| `switch` with named arms, `case valid = result.ok(parsed):`, the statements beneath | `match`, each arm marked `// y: #valid` | `y-facts.json` `decisions`, with `kind` `switch`: each arm's name, Y line and bare line | the same, with the comment |
| No inline lambdas: `masked` builds its list with a `for` loop, and `outcome_of` asks `is_subset_of` | the loop; `.all(|letter| …)`, Rust's form of the subset test | — | the same |
| No `with` clause: the function is pure | — | `effects` clause `pure` | — |
| A `mutable` parameter and no `with` | — | `no io`, `no clock`, `no randomness`, `mutates only game.hits, game.misses` | — |
| `with console, random, forever`, typed in the documentation | three more parameters | their types and access; `effects only console, random`; the clause also in `y-facts.json` `capabilities` | `&mut Console, &mut Random, &Forever` |
| `with system` on the entry point | `System::root()` | `y-facts.json` `capabilities` | the same |
| a `where name = p` precondition | `require(p, "hangman::f#name")` | a `require` clause, status `lifted` by that call | a run-time check whose failure is a fault (a panic naming the construct) |
| `invariant name = p` on a structure | `invariant(p, "hangman::Game#name")` after the one write that can break it | an `invariant` clause on the type | the same |
| A decision: `if a = p:`, `else if b = q:`, `else c:`, or `while a = p:` with `else c:` after the loop, every branch on its own line and named | `if p { // y: #a`, `} else if q { // y: #b`, `} else { // y: #c`; after a `while`, a `// y: #c` comment | `y-facts.json` `decisions`: each decision's branches, with the Y line and the bare line of each | the same, with the comment |
| A value chosen by a condition, which Y's blocks cannot give: a small function with a named result, `part(misses: misses, needed: 1, mark: "O")`, whose branches `return drawn = mark` and `return drawn = " "` | the same | — | the same |
| A condition that narrows (`if not_one_rune = (letter == optional.none): fail ...`) | `let Some(letter) = letter else { ... }` | — | let-else, so rustc checks the narrowing |
| `forever because "..."` | the `&Forever` argument of the call that waits | `y-facts.json` `claims` | `console.read_line(forever)` |
| `compile parse_word_list(game_rules.word_list)` | the value, as a `const` array, and a `static` built once from it | `y-facts.json` `compiled` | the same, and a generated test that makes the call at run time and compares |
| `count<element>`, `count<rune>` | — (units are erased) | `y-facts.json` `units` | `usize` |
| `string` (and a read-only `string` parameter), `rune`, `list<t>`, `set<t>` | — | `text`, `ref<text>`, `Rune`, `list<T> as 'Vec'`, `set<T> as 'BTreeSet'` | `String`, `&str`, `y_runtime::Rune`, `Vec<T>`, `BTreeSet<T>` |
| Interpolation, `"{name}"` and `"{game.word}"` | `format!("{name}")`, `format!("{}", game.word)` | — | the same |
| A binding, `picture = gallows(…)`, including one that later changes; a change, `name := value` | `let`; `name = value` | — | `let`; `let mut`, which `weave` derived through rustc (19 of them); `name = value` |
| `test name covers a, b { given / with / when / then }` | `#[test] fn name()`; a `?` in `given` or `when` lowers to `expect`; `fake console(input: [...])` to `Console::fake(...)` | `y-facts.json` `tests`: what each covers; repeated in a comment in the test | the same |
| Documentation: the `--` line, or the block between two rules of dashes, above a declaration, with roles; the types leave it for the signature | `///` with the roles as a list | — | the same |

## What had to be assumed

`Y-proposal.md` does not settle everything a whole program needs. Where it is silent, the example assumes the
following:

- **Standard capabilities and their methods:** a `console` capability with `write_line`, `write` and `read_line`;
  `random.index_below`; and `system.console()`, `system.random()` and `system.forever()`. The proposal lists `clock`,
  `random`, `file_system` and others, but not a terminal.
- **The entry point** is `function main() fails e with system` in the module `main`. It is in no facet, so it is
  private: the runtime calls it, and no module imports it.
- **One `with` block can bind several capabilities:** `with console = ..., random = ..., forever = ...:`.
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
  `../y-runtime` resolves): 4 files woven, 0 weave errors, 21 `mut` derived, 0 holes undetermined, 0 warnings.
- **`cargo test`:** the 19 lowered Y tests and the generated check of the `compile` pass.
- **`cargo clippy --all-targets`:** no warnings, here and in `../y-runtime`.
- **Coverage:** the tests declare all 37 outcomes of the module. These are the 30 named branches and arms of its 11
  decisions, plus the 7 failure cases, which is what a Y release build requires (Y § 3.14).
- **`tools/bare-rust/tests/y_examples.rs`** keeps the stages in step without cargo:
  - Weaving the committed bare files gives the committed Rust, apart from the derived `mut`.
  - Stripping the committed Rust gives the committed bare bodies byte for byte, and the same subjects and holes. What
    the lowering adds beyond `strip` is only the clauses and the file status.
  - The constraint files are valid, and their clauses are well formed. Clause texts are checked by the same linter as
    library files.
  - `y-facts.json` agrees with the Y source and the bare bodies, and the tests cover every outcome.
  - The Y source keeps the layout rules: two blank lines between declarations, at module level and in a facet, and a
    blank line before each flow-control statement.
- **`bare-rust check` does not run on this crate.** It copies the crate into a work folder, where the relative path
  to `../y-runtime` no longer resolves, so its woven copies cannot build.

## What the example showed

- **The bare form is a workable target for a Y compiler.** The hand lowering was written to the format's rules, and
  stripping the woven Rust gave it back byte for byte. The compiler would write bare bodies and constraint files, and
  `weave` would do the rest, including deriving `mut`, which Y does not have.
- **The constraint file holds the types and contracts, but not everything Y states.** Seven kinds of fact had no
  slot, so they went into `y-facts.json`:
  1. units;
  2. named decisions;
  3. capabilities as Y declares them;
  4. claims;
  5. `compile` sites;
  6. what each test covers;
  7. facets, of which Rust keeps only each member's `pub`.

  The constraint language already has units (`quantity`, `unit`) and could hold the others as clauses. The JSON format
  has no place for them yet.
- **The clause linter needed three additions.**
  - It now knows `pure`, `no device` and `devices only`, which § 6.3 added.
  - In a module's file, a clause may name the module's constants, and call its functions that are bound `pure` or
    `effects none`. That is how `require still_playing : outcome_of(game) is Playing` names `outcome_of`.
- **Narrowing costs nothing to lower.** A Y condition that narrows becomes a Rust let-else, so rustc checks the fact
  the Y checker relied on.
- **A named decision's identity survives only as a comment.** A real backend would also carry it into traces and fault
  reports.
