# Examples

Programs written in Y and taken through the whole pipeline to Rust: Y source, the bare bodies and constraint files a Y
compiler would lower it to, and the Rust that `bare-rust weave` makes from those. The lowering committed here was done
by hand, and each example's README states its rules and what it had to assume. The Y compiler,
[`../tools/y-compile`](../tools/y-compile/), now lowers every example by those rules; its output weaves into Rust that
passes the same tests. The hand-written Rust stays the reference, and `tools/paths/rebare.sh --from-y <example>` puts
the compiler's output in its place to compare the two.

The tests of every example cover every execution path of its module that needs a test (Y § 3.5, § 3.14): each way
through each function, named by the outcomes along it, such as `computer_move > winnable > number'many > winning`.
`tools/bare-rust/tests/y_examples.rs` enumerates the paths from the Y source and checks that the tests cover them.

After changing an example's Y or Rust, remake its `bare/` files with [`../tools/paths/rebare.sh`](../tools/paths/)
`<example>`, and check that each test takes the path it claims with `tools/paths/trace.py`.

| Folder | What it is |
|---|---|
| [`checkers`](checkers/) | English draughts, in a window drawn with Macroquad, against a computer whose minimax and Monte Carlo searches spend a budget of positions set by the level: recursion that spends a `mutable` budget, constant structures, copies of moves, `compare`, and scoring by level and moves |
| [`hangman`](hangman/) | The pencil-and-paper word game, at the terminal: refined types, failures, named decisions, rules and a derived value, capabilities, a claim, `compile`, narrowing, and loops whose `else` is the case in which they never run |
| [`memory`](memory/) | The game of matching pairs, in a window drawn with Macroquad, with pictures fetched from OpenMoji: an `http` capability to a declared service, a region `within` a deadline whose expiry is a typed failure, regions that wait `forever`, pictures drawn from SVG, mouse clicks, and scoring by pairs, lives and time |
| [`nim`](nim/) | Taking counters from heaps, at the terminal, against a computer that plays perfectly: arithmetic on `uint32`, a `conditional` over a failing call, preconditions, and `fail not_reachable` for a path that mathematics rules out |
| [`snake`](snake/) | The arcade game, in a window drawn with Macroquad: a `window` capability, a clock and a duration, a function that may suspend lowered to `async`, `conditional`s over closed cases, and a play loop tested with a fake window |
| [`tic-tac-toe`](tic-tac-toe/) | Noughts and crosses, at the terminal, against a computer that never loses: a recursive search, copies of a game to try moves on, constants shown as text, narrowing in a `conditional`, and a result the function `ensures` |
| [`y-http`](y-http/) | The Rust implementation of the native functions (Y § 3.18) of a standard module `standard.http`, written by hand and only as large as the examples need: the `http<service=…>` capability, over ureq, and fakes of it for tests |
| [`warehouse`](warehouse/) | One day of a warehouse, at the terminal: atomic regions over shared stock with `waits until` and `orelse`, supervised tasks, a detached region and an `uncancellable` one, decimals with derived precision and a rounding mode, a bill split exactly with rationals, pipelines, a bag, a queue and a sorted map whose constraints are enforced, an arena with typed indexes, a resource released exactly once, and a schema with a migration, read from a store |
| [`y-collections`](y-collections/) | The standard library's collections and mappings (Y § 3.13) as Rust types, one per kind, each offering only what its constraints allow: `list`, `array`, `stack`, `queue`, `deque`, `linked_list`, `sorted_list`, `set` and `map` in insertion order, `sorted_set`, `sorted_map`, `unordered_set`, `unordered_map` and `bag` with no iterator, `multimap` and `enumerable` |
| [`y-runtime`](y-runtime/) | The Rust implementation of the Y runtime that lowered code calls, and of the standard library's native functions (Y § 3.18) that the examples use, written by hand: capabilities, list operations by number, number conversions, deadlines, faults, the constrained integers, decimals, rationals and rounding modes (Y § 3.3), shared state and atomic regions kept with one lock (Y § 3.9), supervised tasks, detached work and cancellation (Y § 3.8), arenas and resources (Y § 3.6), stores of versioned records (Y § 3.12), and commands, their handlers and a journal that makes workflows durable (Y § 3.10) |
| [`y-string`](y-string/) | The Rust implementation of `standard.string`, y-string (Y § 3.3): strings of runes normalised to NFC, counted by runes, scalars or code units, and ASCII strings for `string<character_set=ascii>`, counted and indexed in constant time |
| [`y-window`](y-window/) | The Rust implementation of the native functions (Y § 3.18) of a standard module `standard.window`, written by hand: the `window` capability, drawn with Macroquad, with clicks, circles and pictures drawn from SVG, and a fake of it for tests |

The examples of stripping existing Rust, which go the other way, are in [`../tools/bare-rust/examples`](../tools/bare-rust/examples/).
