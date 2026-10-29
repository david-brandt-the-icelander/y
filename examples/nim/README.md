# Nim, from Y to Rust

The old game of taking counters from heaps, played at the console against the computer, written in Y, lowered to bare
Rust plus its constraint files, and woven into Rust that builds, runs and passes its tests.

The pipeline and the lowering rules are the ones [Hangman](../hangman/README.md) sets out; this README states only what
Nim adds. As there, the lowering from Y to bare Rust is done by hand, as a compiler would do it, and the other steps are
the real tools.

## Playing it

```
cargo run          # in this folder
cargo test         # the lowered Y tests
```

The game starts with three heaps of 3, 4 and 5 counters. On each turn a player takes any number of counters, at least
one, from a single heap; whoever takes the last counter wins. You move first, by typing a heap's number and how many to
take, such as `2 3`. The computer then moves, and plays perfectly: when it can, it leaves a nim-sum of zero (the heaps'
sizes combined with exclusive or), which wins every game that can be won. From the opening position the first player
can win, but only by never making a mistake.

## Files

| Stage | Files | Lines | Written by |
|---|---|---|---|
| Y source | `y/src/nim.y`, `y/src/nim.test.y`, `y/src/main.y` | 263, 145, 16 | hand |
| Bare bodies | `bare/src/nim.brs`, `bare/src/nim/tests.brs`, `bare/src/main.brs`, `bare/src/lib.brs` | 247, 118, 20, 3 | `bare-rust strip` of the Rust below, which was written as the Y compiler would lower the source |
| Constraint files | `bare/src/*.constraints.json` (36 subjects, 17 clauses) | 299 | `strip`, plus the clauses and status the Y lowering adds, then `bare-rust libfmt` |
| Y facts | `bare/y-facts.json` | 407 | the same, from the Y source and the bare bodies |
| Rust | `src/nim.rs`, `src/nim/tests.rs`, `src/main.rs`, `src/lib.rs` | 247, 118, 20, 3 | `bare-rust weave`, which gives back the hand-written Rust exactly |

Hangman's bare bodies were written first and woven into Rust. Nim went the other way: the Rust was written as the
lowering of the Y source, `strip` made the bare bodies and the constraint files from it, and `weave` of those gives the
same Rust again. The checks below hold either way.

## What Nim adds to Hangman's lowering

| Y | Bare body | Constraint file or `y-facts.json` | Rust |
|---|---|---|---|
| `starting_heaps is list<uint32> = [3, 4, 5]` in a constant group | `const STARTING_HEAPS = [3, 4, 5];` | `list<u32> where size(value) == 3 as 'std::primitive::array'` | `[u32; 3]` |
| `uint32` for heap sizes and heap numbers, where no unit is wanted (Y § 3.3) | — | `u32` | `u32` |
| The list operations by number: `game.heaps.at_number(n)`, `set_number`, `number_of`, `largest`, `count().to_uint32()` | `list::at_number(&game.heaps, n)` and so on | — | calls into `y_runtime::list`, so that the Rust has no `as` casts for `strip` to bind to mechanical names |
| `heap.xor(sum)` | `heap ^ sum` | — | the same |
| A `while` whose named `else` holds the fallback move: `while searching = …:` with `else losing:` | the loop, then the fallback, marked `// y: #losing` | `y-facts.json` `decisions` | the same |
| `conditional attempted = read_move(game, typed):` over a failing call, with a condition per failure case | `match read_move(…)`, an arm per case, each marked `// y: #attempted.…` | `decisions`, kind `conditional` | the same |
| `remove(…) where allowed = (check(game, chosen) == result.ok)` | `require(check(game, chosen).is_ok(), "nim::remove#allowed")` | `require allowed : check(game, chosen) is Ok(_)` | a run-time check whose failure is a fault |
| `play(injected console, injected forever)`, and `inject(console = …, forever = …):` in `main` | two more parameters; `System::root()` and a block | `y-facts.json` `capabilities` | `&mut Console, &Forever` |

## What had to be assumed

Beyond what Hangman assumed:

- **Library names:** `string.split_whitespace()`, `string.parse_uint32()` (an `optional<uint32>`), `string.repeated(n)`,
  `uint32.xor(other)`, and on lists `at_number`, `set_number`, `number_of` and `largest`, which number elements from 1
  as a player sees them, with `count().to_uint32()`.
- **Narrowing through `or`:** in the `else numbers:` branch of `if not_numbers = ((heap == optional.none) or (take ==
  optional.none)):`, both `heap` and `take` are `uint32`s. The proposal states narrowing for one condition; this extends
  it to the negation of an `or`.
- **A function with no result and no failure,** `remove`, has neither `returns` nor `fails`, and needs no `return`.

## Checks

- **`bare-rust weave`** of `bare/`: 4 files woven, 0 weave errors, 12 `mut` derived, 0 holes undetermined, 0 warnings.
  The woven Rust is identical to the hand-written Rust it was stripped from.
- **`cargo test`:** the 12 lowered Y tests pass, among them a whole game won by perfect play, one lost by careless play,
  and every wrong move the player can type.
- **`cargo clippy --all-targets`:** no warnings, here and in `../y-runtime`.
- **Coverage:** the tests declare all 36 outcomes of the module: the 26 named branches and conditions of its 11
  decisions, and the 10 failure cases its functions document.
- **`tools/bare-rust/tests/y_examples.rs`** runs the same checks on Nim as on Hangman: weave and strip in step, valid
  constraint files, facts that agree with the source, full coverage, and Y's layout and naming rules (blank lines,
  named decisions, no `else if`, no needless parentheses, documentation roles grouped, only `mutable` names change).

## What the example showed

- **Casts make strip's bare form worse.** `strip` binds every `as` cast to a typed local, so a lowering that indexes a
  `Vec` with a `u32` gets locals named `heap_usize` in its bare body and its woven Rust. Moving the conversions into the
  runtime's list operations, which is also how the Y source reads, removed all of them.
- **The clause linter did not know the prelude.** A precondition about a failing call, `check(game, chosen) is Ok(_)`,
  names `Ok`. A module's file is Rust source, where `Some`, `None`, `Ok` and `Err` are always in scope, so the linter now
  accepts them there.
- **Nothing in Nim needed a new kind of fact.** Hangman's seven kinds in `y-facts.json` covered it, and Nim uses only
  facets, capabilities, a claim, decisions and tests.
