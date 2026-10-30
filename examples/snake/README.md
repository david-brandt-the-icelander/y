# Snake, from Y to Rust

The arcade game, played in a window drawn with [Macroquad](https://macroquad.rs). It is written in Y, lowered to bare
Rust plus its constraint files, and woven into Rust that builds, runs and passes its tests.

The pipeline and the lowering rules are the ones [Hangman](../hangman/README.md) sets out, and Snake was made the way
[Nim](../nim/README.md) was. This README states only what Snake adds. The lowering from Y to Rust is done by hand, as a
compiler would do it; the other steps are the real tools.

## Playing it

```
cargo run          # in this folder: opens the window
cargo test         # the lowered Y tests, which need no window
```

The snake starts three cells long and moving right. Steer it with the arrow keys or W, A, S and D. Each piece of food
it eats makes it one cell longer and scores one. The game ends when the snake runs into the wall or into itself, or
fills the board. Space then starts the next game. Escape, or closing the window, ends play, and the best score is
written at the terminal.

On Linux, Macroquad needs the X11 and OpenGL libraries (`libx11-dev`, `libxi-dev`, `libgl1-mesa-dev`) to build.

## Files

| Stage | Files | Lines | Written by |
|---|---|---|---|
| Y source | `y/src/snake.y`, `y/src/snake.test.y`, `y/src/main.y` | 397, 332, 24 | hand |
| Bare bodies | `bare/src/snake.brs`, `bare/src/snake/tests.brs`, `bare/src/main.brs`, `bare/src/lib.brs` | 360, 267, 31, 3 | `bare-rust strip` of the Rust below, which was written as the Y compiler would lower the source |
| Constraint files | `bare/src/*.constraints.json` (62 subjects, 43 clauses) | 534 | `strip`, plus the clauses and status the Y lowering adds, then `bare-rust libfmt` |
| Y facts | `bare/y-facts.json` | 865 | the same, from the Y source and the bare bodies |
| Rust | `src/snake.rs`, `src/snake/tests.rs`, `src/main.rs`, `src/lib.rs` | 360, 267, 31, 3 | `bare-rust weave`, which gives back the hand-written Rust exactly |

The window is a capability. It comes from [`../y-window`](../y-window/), a stand-in for a Y standard module
`standard.window` that draws with Macroquad. It is a crate of its own so that the console examples do not build
Macroquad. The clock comes from [`../y-runtime`](../y-runtime/), as the console and the randomness do.

## What Snake adds to the earlier lowerings

| Y | Bare body | Constraint file or `y-facts.json` | Rust |
|---|---|---|---|
| `injected window`: a capability that draws, reads keys and waits for the next frame | `window: &mut Window`, last among the parameters | `effects only window` | `y_window::Window` over Macroquad, or a fake for tests |
| `injected clock`, and `clock.now()` | `clock: &mut Clock`, `clock.now()` | `effects only window, clock, random` on `play` | `y_runtime::Clock`; Y's `instant` and `duration` are `std::time::Instant` and `Duration` |
| `step_time is duration = 150ms`: a typed literal with a physical unit (Y § 3.3) | `const STEP_TIME = Duration::from_millis(150);` | `y-facts.json` `units` | the same, typed `Duration` |
| `window.next_frame() forever because "…"`, a point where `play` may suspend | `window.next_frame(forever).await` | `y-facts.json` `suspensions` and `claims` | `play` is an `async fn` |
| `best = play()` in `main` | `play(…).await` in an `async fn entry` | `y-facts.json` `suspensions` | `y_window::run` opens the window and runs `entry` on Macroquad's executor |
| `conditional going = heading:` with a condition for each case of a closed type, and no `else` | `match heading`, an arm per case, each marked `// y: #going.…` | `decisions`, kind `conditional` | the same |
| `while due = (…):` in `catch_up`, with a bare `else not_due`, and the frame loop `while open = (…):` with a bare `else closed_at_once`: each `else` names the case in which the loop never runs | each loop marked `// y: #due, else #not_due` | `decisions`, kind `while` | the same |
| `turned(game, pressed)`'s `condition arrow.up = ((pressed == key.up) or (pressed == key.w)):` | a `match` arm for each operand of the `or`, marked `// y: #arrow.up'1` and `// y: #arrow.up'2`, the names of the two paths | `decisions`, kind `conditional` | the same |
| `function new_game(injected random) returns started ensures playing = (started.standing == outcome.playing):` | `ensure(…, "snake::new_game#playing")` before the return | `ensure playing : result.standing is Playing`, status `lifted` | a run-time check whose failure is a fault |
| `int32` cells, which may step off the board, and `on_board(place)` | `i32` | — | `i32` |
| `game.tail.without_last()`, `insert_first`, `remove_last`, `range(from: 0, below: board.rows)` | `list::without_last(&game.tail)`, `insert(0, …)`, `pop()`, `0..board::ROWS` | — | the same |

## Suspension: Y has no `async`, Rust does

In Y, a function may suspend when a capability it holds may suspend, and nothing else marks it (Y § 3.7). `window` may
suspend in `next_frame()`, so `play` may, and so may `main`, which calls it. `y explain` would list the points. Rust
colours such functions instead: the lowering makes `play` and the entry point `async fn`s and awaits each call that may
suspend. `y-facts.json` records the two points in `suspensions`, and `tools/bare-rust/tests/y_examples.rs` checks that
each one is awaited in the bare body.

A fake window never suspends. A test of `play` runs it with `y_window::block_on`, so the Y tests of `play` need nothing
the other tests do not: a `fake window(frames: …)` that presses the given keys in each frame and records the text
written, and a `fake clock(tick: 150ms)` whose every reading is one step time after the last.

## What had to be assumed

Beyond what Hangman and Nim assumed:

- **A standard window module.** `standard.window` gives the `window` capability (`clear`, `fill`, `write`,
  `keys_pressed`, `close_requested`, `next_frame`), `key` and `colour`. The proposal names no such module.
- **A window opened with constants.** Macroquad opens the window before any code runs. The window's title and size in
  `system.window(title: "Snake", width: layout.width, height: layout.height)` are constants, so the lowering moves
  them into `fn main`, and `system.window()` then gives the window that is already open.
- **`standard.time`.** It gives `clock` and `duration`, with `clock.now()`, the difference of two instants as a
  `duration`, and comparison of durations.
- **Library names:** `list.without_last()`, `insert_first`, `remove_last`, `contains`, `range(from:, below:)` and
  `uint32.max(other)`.
- **Helper functions in a test file.** `snake.test.y` declares `moving(heading)`, a game to start tests from, with the
  same documentation as any function.
- **Closed constants in a `conditional` with no `else`.** `opposite`, `ahead_of` and `banner` compare their label with
  every case of `direction` or `outcome`, which the proposal says the compiler checks for coverage.

## Checks

- **`bare-rust weave`** of `bare/`: 4 files woven, 0 weave errors, 51 `mut` derived, 0 holes undetermined, 0 warnings.
  The woven Rust is identical to the hand-written Rust it was stripped from.
- **`cargo test`:** the 18 lowered Y tests pass. Among them are a snake moving each way, turning and refusing to turn
  back, crashing, biting itself, following its own tail, eating, and filling the board. Three tests run `play` with a
  fake window: until the window closes, through a crash and a new game, and until Escape.
- **`cargo clippy --all-targets`:** no warnings, here, in `../y-window` and in `../y-runtime`.
- **Coverage:** the tests cover all 52 execution paths of the module (Y § 3.14), through the 15 decisions of its 15
  functions. No function of Snake can fail, and none of its paths is ruled out by `fail not_reachable`. Each test was
  also run alone with every decision and loop instrumented, to check that it takes each path it declares.
- **`tools/bare-rust/tests/y_examples.rs`** runs the same checks on Snake as on Hangman and Nim, and a new one: each
  point in `suspensions` is awaited in the bare body.
- **The window itself:** run under a virtual X display, it draws the board, the snake, the food and the banner. Left
  alone, the snake runs into the wall and the banner says so.

## What the example showed

- **`strip` and `weave` handle `async fn` and `.await` with no change.** They read Rust syntax, so an async body is a
  body like any other, and the return type of `async fn entry` is held in the constraint file like any other.
- **A loop's `else` is the case in which it never runs.** Each frame steps the snake for every step time that has
  passed; `catch_up`'s `else not_due` is a frame too quick for a step, and the frame loop's `else closed_at_once` a window
  closed before the first frame. Both are paths a test must take, and neither was tested before.
- **An `or` is two paths.** `turned` turns the snake up for either the up arrow or W; the tests had pressed only one of
  each pair. The lowering gives each operand its own `match` arm, so the Rust has the same two ways through.
- **What a function ensures keeps paths out.** `play`'s first frame can only start a new game if the game it began with
  is over, which it never is: `new_game` now ensures that the game it gives is being played.
- **Clause paths are not host paths.** A precondition on an enum value is written as Hangman's is,
  `game.standing is Playing`. The clause linter refuses `Outcome::Playing` as an unquoted host path.
