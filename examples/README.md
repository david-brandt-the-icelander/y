# Examples

Programs written in Y and taken through the whole pipeline to Rust: Y source, the bare bodies and constraint files a Y
compiler would lower it to, and the Rust that `bare-rust weave` makes from those. No Y compiler exists yet, so the
lowering is done by hand; each example's README states its rules and what it had to assume.

| Folder | What it is |
|---|---|
| [`hangman`](hangman/) | The pencil-and-paper word game, at the terminal: refined types, failures, named decisions, capabilities, a claim, `compile`, narrowing and tests with full coverage |
| [`nim`](nim/) | Taking counters from heaps, at the terminal, against a computer that plays perfectly: arithmetic on `uint32`, a loop with a named `else`, a `conditional` over a failing call, preconditions, and tests with full coverage |
| [`y-runtime`](y-runtime/) | A small hand-written stand-in for the Y runtime that lowered code calls: capabilities, runes, list operations by number, faults |

The examples of stripping existing Rust, which go the other way, are in [`../tools/bare-rust/examples`](../tools/bare-rust/examples/).
