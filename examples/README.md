# Examples

Programs written in Y and taken through the whole pipeline to Rust: Y source, the bare bodies and constraint files a Y
compiler would lower it to, and the Rust that `bare-rust weave` makes from those. No Y compiler exists yet, so the
lowering is done by hand; each example's README states its rules and what it had to assume.

The tests of every example cover every execution path of its module that needs a test (Y § 3.5, § 3.14): each way
through each function, named by the outcomes along it, such as `computer_move > winnable > number'many > winning`.
`tools/bare-rust/tests/y_examples.rs` enumerates the paths from the Y source and checks that the tests cover them.

After changing an example's Y or Rust, remake its `bare/` files with [`../tools/paths/rebare.sh`](../tools/paths/)
`<example>`, and check that each test takes the path it claims with `tools/paths/trace.py`.

| Folder | What it is |
|---|---|
| [`checkers`](checkers/) | English draughts, in a window drawn with Macroquad, against a computer whose minimax and Monte Carlo searches spend a budget of positions set by the level: recursion that spends a `mutable` budget, constant structures, copies of moves, `compare`, and scoring by level and moves |
| [`hangman`](hangman/) | The pencil-and-paper word game, at the terminal: refined types, failures, named decisions, rules and a derived value, capabilities, a claim, `compile`, narrowing, and loops whose `else` is the case in which they never run |
| [`memory`](memory/) | The game of matching pairs, in a window drawn with Macroquad, with pictures fetched from OpenMoji: an `http` capability to a declared service, a `within` deadline whose expiry is a typed failure, pictures drawn from SVG, mouse clicks, and scoring by pairs, lives and time |
| [`nim`](nim/) | Taking counters from heaps, at the terminal, against a computer that plays perfectly: arithmetic on `uint32`, a `conditional` over a failing call, preconditions, and `fail not_reachable` for a path that mathematics rules out |
| [`snake`](snake/) | The arcade game, in a window drawn with Macroquad: a `window` capability, a clock and a duration, a function that may suspend lowered to `async`, `conditional`s over closed cases, and a play loop tested with a fake window |
| [`tic-tac-toe`](tic-tac-toe/) | Noughts and crosses, at the terminal, against a computer that never loses: a recursive search, copies of a game to try moves on, constants shown as text, narrowing in a `conditional`, and a result the function `ensures` |
| [`y-http`](y-http/) | The Rust implementation of the native functions (Y § 3.18) of a standard module `standard.http`, written by hand and only as large as the examples need: the `http<service=…>` capability, over ureq, and fakes of it for tests |
| [`y-runtime`](y-runtime/) | The Rust implementation of the Y runtime that lowered code calls, and of the standard library's native functions (Y § 3.18) that the examples use, written by hand: capabilities, runes, list operations by number, number conversions, deadlines, faults, and the decimals, rationals and rounding modes (Y § 3.3) |
| [`y-window`](y-window/) | The Rust implementation of the native functions (Y § 3.18) of a standard module `standard.window`, written by hand: the `window` capability, drawn with Macroquad, with clicks, circles and pictures drawn from SVG, and a fake of it for tests |

The examples of stripping existing Rust, which go the other way, are in [`../tools/bare-rust/examples`](../tools/bare-rust/examples/).
