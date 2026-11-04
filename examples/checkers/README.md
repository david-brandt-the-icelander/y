# Checkers, from Y to Rust

Checkers (English draughts), for one player against the computer, in a window drawn with [Macroquad](https://macroquad.rs).
The computer chooses its moves with a minimax search and a Monte Carlo search, and how hard it plays is how many positions
it may look at each move. It is written in Y, lowered to bare Rust plus its constraint files, and woven into Rust that
builds, runs and passes its tests.

The pipeline and the lowering rules are the ones [Hangman](../hangman/README.md) sets out. Checkers was made the way
[Snake](../snake/README.md), [Tic-tac-toe](../tic-tac-toe/README.md) and [Memory](../memory/README.md) were, and this README
states only what it adds. The lowering from Y to Rust is done by hand, as a compiler would do it; the other steps are the
real tools.

## Playing it

```
cargo run          # in this folder: opens the window
cargo test         # the lowered Y tests, which need no window
```

First click a level. Then play red, at the bottom of the board, and move first: click one of your pieces, and the squares
it may move to are marked; click one of them to move there. The computer plays white.

The rules are those of English draughts:

- Men move one square diagonally forward. Kings, which are crowned on the far row, move one square diagonally either way.
- A piece captures by jumping over a piece of the other player's onto the empty square beyond. A player who can capture
  must, and a piece that can jump again after a jump must go on: the whole chain is one move. A man crowned by a jump
  stops there.
- A player who cannot move, having no pieces or none that can move, loses. Eighty turns in a row without a capture draw
  the game.

When a game ends, Space goes back to the levels. Escape, or closing the window, ends play, and the best score is written at
the terminal.

On Linux, Macroquad needs the X11 and OpenGL libraries (`libx11-dev`, `libxi-dev`, `libgl1-mesa-dev`) to build. The crate
builds with optimisation even for `cargo run`, as the hardest level looks at 250,000 positions a move.

## The computer

Each level gives the computer a number of **checks**: the positions it may look at to choose one move.

| Level | Checks a move | A win scores |
|---|---|---|
| Beginner | 25 | ×1 |
| Casual | 250 | ×2 |
| Club | 2,500 | ×3 |
| Expert | 25,000 | ×4 |
| Master | 250,000 | ×5 |

The computer takes its moves in an order drawn at random, then spends its checks in two parts:

1. **Minimax**, with half the checks. It looks one move ahead, then two, and so on, up to ten, each time weighing each move
   by the pieces left on the board when both players play their best: a man is worth 3, a king 5, and a game won 100.
   Each position the search reaches is a check. When the checks run out partway through a depth, that depth is thrown
   away, and the weights of the last whole depth stand.
2. **Monte Carlo**, with the rest. Among the moves the minimax search weighs best, the checks are shared out, and each move
   is worth the average of games played out from it at random, twelve moves at most, then weighed the same way. A move
   that is weighed best on its own is taken without them.

In the first thirty moves of a game played by the computer against itself, the minimax search mostly looked one move
ahead at the Beginner level, two or three at Casual, three or four at Club, five or six at Expert and six or seven at
Master, and further as fewer pieces were left. A Beginner mostly sees only what each of its moves takes, and plays almost
at random among the rest. Matches of ten games each way between neighbouring levels, played while the example was made,
went to the higher level each time:

| Higher level against lower | As red (won, lost, drawn) | As white (won, lost, drawn) |
|---|---|---|
| Casual against Beginner | 10, 0, 0 | 10, 0, 0 |
| Club against Casual | 10, 0, 0 | 8, 0, 2 |
| Expert against Club | 6, 2, 2 | 8, 2, 0 |
| Master against Expert | 4, 1, 5 | 4, 1, 5 |

## Scoring

A game lost or drawn scores nothing. A game won scores 1,000 points, less 10 for each of the player's moves, but never less
than 100, and all of it times the level's weight. A win at the Club level in 30 moves scores (1,000 − 300) × 3 = 2,100.

## Files

| Stage | Files | Lines | Written by |
|---|---|---|---|
| Y source | `y/src/checkers.y`, `y/src/checkers.test.y`, `y/src/main.y` | 1300, 2274, 23 | hand |
| Bare bodies | `bare/src/checkers.brs`, `bare/src/checkers/tests.brs`, `bare/src/main.brs`, `bare/src/lib.brs` | 1146, 1703, 30, 3 | `bare-rust strip` of the Rust below, which was written as the Y compiler would lower the source |
| Constraint files | `bare/src/*.constraints.json` (311 subjects, 103 clauses) | 2199 | `strip`, plus the clauses and status the Y lowering adds, then `bare-rust libfmt` |
| Y facts | `bare/y-facts.json` | 3540 | the same, from the Y source and the bare bodies |
| Rust | `src/checkers.rs`, `src/checkers/tests.rs`, `src/main.rs`, `src/lib.rs` | 1146, 1703, 30, 3 | `bare-rust weave`, which gives back the hand-written Rust exactly |

The window, which now also draws circles, comes from [`../y-window`](../y-window/), and the randomness from
[`../y-runtime`](../y-runtime/).

## What Checkers adds to the earlier lowerings

| Y | Bare body | Constraint file or `y-facts.json` | Rust |
|---|---|---|---|
| `structure level:` with `name is string`, and `levels.all is list<element=level> = [level(name: "Beginner", …), …]` | `const ALL: [Level; 5] = [Level { name: "Beginner", … }, …];` | the field's type, `&'static str` | a constant array of structures |
| `chains(…)`, which calls itself through `chains_after` for each further jump | the same | `effects pure` | two mutually recursive `fn`s. A piece can jump at most twelve times, so the recursion is shallow |
| `searched(game, owner, depth, mutable left)`, a recursive minimax that spends a budget it is given | `left: &mut u32` | `access mutates only left` | the budget is shared by every call of the search |
| `mutable longer = path`, a copy of a list parameter to go on with | `path.to_vec()`, marked `// y: a copy` | `y-facts.json` `copies` | the same |
| `apply(mutable after, candidate)` and `options.at_number(n)`, which hand on or give back a move the list still holds | `candidate.clone()`, `list::at_number_ref(&options, n).clone()`, each marked `// y: a copy` | `y-facts.json` `copies` | the same |
| `values.largest()`, `values.smallest()`, `values.number_of(value)` | `list::largest(&values)`, `list::smallest(&values)`, `list::number_of(&values, &value)` | — | in `y_runtime::list` |
| `random.one_of(moves)` | `random.one_of(&moves)` | — | in `y_runtime::Random` |
| `chosen.path.last()` on a list that cannot be empty | `list::last(&chosen.path)` | — | faults on an empty list |
| `(square.row * board.size + square.column + 1).to_uint32()` | `number::to_uint32(…)` | — | a call into `y_runtime::number`, which faults below zero, not an `as` cast |
| `game.turns.divided_rounding_up(by: 2)` | `game.turns.div_ceil(2)` | — | the same |
| `conditional shown = standing(game):` with `condition shown.your_move = ((shown == outcome.playing) and (game.to_move == side.red)):` | a `match` arm with a guard | `decisions`, kind `conditional` | the same |
| `compare ranked = values.at_number(option) with best:` with `condition ranked.top = equal:`, `condition ranked.lower = less:` and `condition ranked.above = greater:` (Y § 3.5) | `match a.cmp(&b)`, an arm per `Ordering`, each marked `// y: #ranked.…` | `decisions`, kind `compare` | the same |
| `fail not_reachable` under `ranked.above` and `ties.none`, orders the values cannot stand in | `not_reachable("checkers::best_sampled#ranked.above")` | — | a fault |
| `for placed in own_pieces(game):` with a bare `else no_pieces`, the case in which the loop never runs | the loop, marked `// y: #placed, else #no_pieces` | `decisions`, kind `for` | the same |
| `function directions_of(piece) returns ways ensures several = (ways.count() >= 2):` | `ensure(list::count(ways) >= 2, "checkers::directions_of#several")` before the return | `ensure several : size(result) >= 2`, status `lifted` | a run-time check whose failure is a fault; the path enumeration knows from it that a loop over the directions runs more than once |
| `window.circle(x:, y:, radius:, colour:)` | the same, on `Window` | — | Macroquad's `draw_circle`, or nothing in a fake window |

## What had to be assumed

Beyond what the earlier examples assumed:

- **A variable may be named after a type,** as `square` is, when it is a parameter; where a value of that type is made
  on the same line, the variable has another name (`here = square(row: row, column: column)`).
- **`and` and `or` stop early,** so `on_board(land)` is tested before the square is read. Where the early stop matters,
  the source tests it with an `if` all the same, so as not to rest on it.
- **Narrowing in a `conditional` whose first condition is `x == optional.none` or `x != optional.none`,** as in
  Memory, for `held`, `picked`, `found` and `made`.
- **Library names:** `list.last()`, `list.smallest()`, `random.one_of(…)`, `int32.to_uint32()`,
  `uint32.divided_rounding_up(by:)`, `uint32.min(…)` and `window.circle(…)`.
- **A computation blocks the frame it is in.** The computer's search runs inside one frame; the frame before it says that
  the computer is thinking. A Y runtime could run the search as a task of its own.

## Checks

- **`bare-rust weave`** of `bare/`: 4 files woven, 0 weave errors, 159 `mut` derived, 0 holes undetermined, 0 warnings.
  The woven Rust is identical to the hand-written Rust it was stripped from.
- **`cargo test`:** the 184 lowered Y tests pass, each covering one path. They cover the rules (steps, forced captures,
  chains of jumps, crowning, kings, winning, losing, drawing), the scoring, the worth of a position, both parts of the
  computer's search, the drawing, and play through a fake window: choosing a level, selecting a man, moving him, the
  computer's reply, and Space after a game.
- **`cargo clippy --all-targets`:** no warnings, here and in `../y-window` and `../y-runtime`.
- **Coverage:** the tests cover all 180 execution paths of the module that need a test (Y § 3.14). Its 51 functions
  have 184 paths through 68 decisions; 4 end in `fail not_reachable`. Each test was also run alone with every decision
  and loop instrumented, to check that it takes the path it declares. The searches are tested with small budgets of
  checks (1 to 2,100), chosen so that each path of the search is taken.
- **One path per test:** each of the 184 tests covers one path, and its `when` is one call of that path's function
  (Y § 3.14). Where two tests cover the same path, each checks something the other does not.
- **`tools/bare-rust/tests/y_examples.rs`** runs the same checks on Checkers as on the other examples, the 10 copies
  among them.
- **The window itself:** under a virtual X display, the game showed the levels, started a game at the Club level, marked
  the squares a clicked man could move to, moved him, showed the computer's reply with its squares marked, and wrote the
  best score at the terminal when Escape was pressed.

## What the example showed

- **A budget is a parameter that changes.** The checks are a `mutable` parameter of the recursive search, which every
  call spends from; it lowers to a `&mut u32`, and the constraint file says that it is all the search changes.
- **Value semantics cost copies in Rust.** Most of the 10 copies are moves: a move taken from a list to apply, or to
  return, while the list still holds it. Y passes values; Rust must `clone` where both go on being used.
- **Path coverage asks for smaller functions.** `draw` had a path for each way its loops over the board, the pieces and
  the marks combine; it is now `draw_board`, `draw_pieces` and `draw_marks`, each with a few. `choose_level` and
  `play_game` hand the clicks of a frame to `level_clicked` and `act_on_clicks` for the same reason.
- **The facts have to come from somewhere.** A loop over a piece's directions never fails to run, but nothing in the
  code said so until `directions_of` ensured it; where a value is known only by the program's logic, as the best of a
  list of values being in the list, `compare` names every order and `fail not_reachable` closes the impossible ones.
- **Nothing needed a new kind of fact,** beyond the `paths` that all the examples now list. Recursion, copies, suspensions and claims all fit the kinds the earlier examples
  use.
