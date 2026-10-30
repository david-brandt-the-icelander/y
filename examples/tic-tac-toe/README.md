# Tic-tac-toe, from Y to Rust

Noughts and crosses, played at the console against a computer that never loses. It is written in Y, lowered to bare Rust
plus its constraint files, and woven into Rust that builds, runs and passes its tests.

The pipeline and the lowering rules are the ones [Hangman](../hangman/README.md) sets out. Tic-tac-toe was made the way
[Nim](../nim/README.md) was, and this README states only what it adds. The lowering from Y to Rust is done by hand, as a
compiler would do it; the other steps are the real tools.

## Playing it

```
cargo run          # in this folder
cargo test         # the lowered Y tests
```

You play crosses and move first, by typing the number of an empty square. The board shows the number of each empty
square, from 1 at the top left to 9 at the bottom right:

```
 1 | 2 | 3
---+---+---
 4 | 5 | 6
---+---+---
 7 | 8 | 9
```

The computer plays noughts. It looks ahead through every game that can follow from the board (minimax) and takes the
first square, in the order they are numbered, with the highest value to it. A win is worth more the sooner it comes, so
it takes a win at once rather than later. Against it, the best you can do is draw.

## Files

| Stage | Files | Lines | Written by |
|---|---|---|---|
| Y source | `y/src/tic_tac_toe.y`, `y/src/tic_tac_toe.test.y`, `y/src/main.y` | 346, 415, 16 | hand |
| Bare bodies | `bare/src/tic_tac_toe.brs`, `bare/src/tic_tac_toe/tests.brs`, `bare/src/main.brs`, `bare/src/lib.brs` | 332, 320, 21, 3 | `bare-rust strip` of the Rust below, which was written as the Y compiler would lower the source |
| Constraint files | `bare/src/*.constraints.json` (64 subjects, 27 clauses) | 511 | `strip`, plus the clauses and status the Y lowering adds, then `bare-rust libfmt` |
| Y facts | `bare/y-facts.json` | 945 | the same, from the Y source and the bare bodies |
| Rust | `src/tic_tac_toe.rs`, `src/tic_tac_toe/tests.rs`, `src/main.rs`, `src/lib.rs` | 332, 320, 21, 3 | `bare-rust weave`, which gives back the hand-written Rust exactly |

## What Tic-tac-toe adds to the earlier lowerings

| Y | Bare body | Constraint file or `y-facts.json` | Rust |
|---|---|---|---|
| `value_of(game)`, which calls itself once for each empty square | the same | `effects pure` | a recursive `fn`. The board has nine squares, so the recursion is at most nine calls deep |
| `mutable after = game`, a copy to try a move on while `game` is still read | `game.clone()`, marked `// y: a copy` | `y-facts.json` `copies` | the same |
| `constants mark:` with `cross = "X"` and `nought = "O"`, and `"{held}"` | `impl fmt::Display for Mark` | — | the enum shows as the constant's value |
| `squares is list<optional<mark>>`, made with `list.repeated(optional.none, times: board.squares)` | `list::repeated(None, board::SQUARES)` | the field's type, `list<Mark?> as 'Vec'` | `Vec<Option<Mark>>` |
| `lines is list<list<uint32>> = [[1, 2, 3], …]`, a constant list of lists | `const LINES = [[1, 2, 3], …];` | `list<list<u32> where size(value) == 3 as …> where size(value) == 8 as 'std::primitive::array'` | `[[u32; 3]; 8]` |
| `conditional held = game.squares.at_number(number):` with `condition held.empty = (held == optional.none):` and `else held.marked:`, which narrows `held` to a `mark` | `match`, with `None` and `Some(held)` arms | `decisions`, kind `conditional` | the same |
| `if drawn = (found == optional.none):` with `else won:`, which narrows `found` | `match found`, with `None` and `Some(found)` arms | `decisions` | the same |
| `place(mutable game, square) where unmarked = (game.squares.at_number(square) == optional.none)` | `require(…, "tic_tac_toe::place#unmarked")` | `require unmarked : game.squares[square - 1] is None` | a run-time check whose failure is a fault |
| `function value_of(game) returns value ensures bounded = ((value >= -10) and (value <= 10)):` | `ensure(…, "tic_tac_toe::value_of#bounded")` before the return | `ensure bounded : result >= -10 and result <= 10`, status `lifted` | a run-time check whose failure is a fault; the path enumeration knows from it that a value found is never below the computer's starting best |
| `for square in open:` in `computer_move`, with `else no_open: fail not_reachable`, since the precondition `still_playing` leaves a square open | the loop, then `if !ran { // y: #no_open` and `not_reachable("tic_tac_toe::computer_move#no_open")` | `decisions`, kind `for` | the same |
| `play(started, …)`, which plays a game from a board it is given; `main` passes `new_game()` | the same | — | the same |
| `open.count().to_int32()` | `list::count_int32(&open)` | — | a call into `y_runtime::list`, not an `as` cast |

## What had to be assumed

Beyond what Hangman and Nim assumed:

- **A constant shows as its value.** In a string, `"{held}"` with `held` a `mark` gives `X` or `O`, the text the
  constant group gives each constant. The proposal gives constants values, but does not say how a constant is shown.
- **Recursion is allowed.** The proposal leaves `rule`s non-recursive in v1 (§ 3.13), and says nothing against a
  function calling itself. `value_of` does, and the board bounds how deep it goes.
- **Library names:** `list.repeated(value, times: n)`, `string.trim()`, `count().to_int32()`, `int32.max(other)` and
  `range(from:, through:)`, which includes its last number.
- **Narrowing in a `conditional`:** in `else held.marked:`, `held` is a `mark`, as `typed` is a `string` in Nim's
  `else line_read:`.

## Checks

- **`bare-rust weave`** of `bare/`: 4 files woven, 0 weave errors, 36 `mut` derived, 0 holes undetermined, 0 warnings.
  The woven Rust is identical to the hand-written Rust it was stripped from.
- **`cargo test`:** the 32 lowered Y tests pass. Among them are a whole game drawn by careful play, one lost by careless
  play, every wrong move the player can type, the computer taking a win over a block, and a line found on a diagonal.
- **`cargo clippy --all-targets`:** no warnings, here and in `../y-runtime`.
- **Coverage:** the tests cover all 58 execution paths of the module that need a test (Y § 3.14). Its 15 functions have
  59 paths through 20 decisions and their `or fail` calls; one ends in `fail not_reachable`. Each test was also run
  alone with every decision and loop instrumented, to check that it takes each path it declares.
- **`tools/bare-rust/tests/y_examples.rs`** runs the same checks on Tic-tac-toe as on the other examples, the two copies
  among them.

## What the example showed

- **A clause names what the constraint language knows.** The precondition of `place` is written in Y with the list
  operation `at_number`, which numbers squares from 1. The clause linter does not know the runtime's functions, so the
  clause indexes the list instead: `game.squares[square - 1] is None`.
- **A path a test cannot reach is a design question.** `play` always began from an empty board, so a game that is over
  before anyone moves, or ends on the computer's move after the player's, was a path no test could take. `play` now
  takes the board to start from, and its turn and its ending moved into `persons_move` and `announce`, each tested on
  its own, so that a console failing at each write is a path a test can name.
- **Nothing needed a new kind of fact.** Recursion, copies and constants shown as text all fit the kinds Hangman, Nim and
  Snake use.
