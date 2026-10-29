# Examples

Programs written in Y and taken through the whole pipeline to Rust: Y source, the bare bodies and constraint files a Y
compiler would lower it to, and the Rust that `bare-rust weave` makes from those. No Y compiler exists yet, so the
lowering is done by hand; each example's README states its rules and what it had to assume.

| Folder | What it is |
|---|---|
| [`hangman`](hangman/) | The pencil-and-paper word game, at the terminal: refined types, failures, named decisions, capabilities, a claim, `compile`, narrowing and tests with full coverage |
| [`memory`](memory/) | The game of matching pairs, in a window drawn with Macroquad, with pictures fetched from OpenMoji: an `http` capability to a declared service, a `within` deadline whose expiry is a typed failure, pictures drawn from SVG, mouse clicks, and scoring by pairs, lives and time |
| [`nim`](nim/) | Taking counters from heaps, at the terminal, against a computer that plays perfectly: arithmetic on `uint32`, a loop with a named `else`, a `conditional` over a failing call, preconditions, and tests with full coverage |
| [`snake`](snake/) | The arcade game, in a window drawn with Macroquad: a `window` capability, a clock and a duration, a function that may suspend lowered to `async`, `conditional`s over closed cases, and a play loop tested with a fake window |
| [`tic-tac-toe`](tic-tac-toe/) | Noughts and crosses, at the terminal, against a computer that never loses: a recursive search, copies of a game to try moves on, constants shown as text, narrowing in a `conditional`, and tests with full coverage |
| [`y-http`](y-http/) | A small hand-written stand-in for a Y standard module `standard.http`: the `http<service>` capability, over ureq, and fakes of it for tests |
| [`y-runtime`](y-runtime/) | A small hand-written stand-in for the Y runtime that lowered code calls: capabilities, runes, list operations by number, deadlines, faults |
| [`y-window`](y-window/) | A small hand-written stand-in for a Y standard module `standard.window`: the `window` capability, drawn with Macroquad, with clicks and pictures drawn from SVG, and a fake of it for tests |

The examples of stripping existing Rust, which go the other way, are in [`../tools/bare-rust/examples`](../tools/bare-rust/examples/).
