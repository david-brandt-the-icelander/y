# Memory, from Y to Rust

The game of matching pairs, for one player, in a window drawn with [Macroquad](https://macroquad.rs). Its pictures are
fetched from [OpenMoji](https://openmoji.org) each time a board is set up. It is written in Y, lowered to bare Rust plus
its constraint files, and woven into Rust that builds, runs and passes its tests.

The pipeline and the lowering rules are the ones [Hangman](../hangman/README.md) sets out, and Memory was made the way
[Nim](../nim/README.md) and [Snake](../snake/README.md) were. This README states only what Memory adds. The lowering from
Y to Rust is done by hand, as a compiler would do it; the other steps are the real tools.

## Playing it

```
cargo run          # in this folder: opens the window, and fetches the pictures from openmoji.org
cargo test         # the lowered Y tests, which need no window and no network
```

Twenty cards lie face down: ten pairs of animals' faces, chosen at random from twenty and dealt in a random order. Click
a card to turn it over, then click a second. If the two show the same animal, they stay face up. If they do not, they
stay face up for a second, then turn back, and the turn costs a life. The game is won when every pair is found, and lost
when the ten lives run out.

The score is:

- 100 points for each pair found;
- when every pair is found, 50 points for each life left;
- when every pair is found, 10 points for each whole second under the par time of two minutes.

The clock starts when the cards are dealt. When a game ends, Space deals a new one, with new pictures. Escape, or closing
the window, ends play, and the best score is written at the terminal.

On Linux, Macroquad needs the X11 and OpenGL libraries (`libx11-dev`, `libxi-dev`, `libgl1-mesa-dev`) to build.

## The pictures

Each board fetches its ten pictures as SVG drawings from `https://openmoji.org/data/color/svg/<code>.svg`. That is the
address OpenMoji's own FAQ gives for its images. It then draws each one 96 pixels square, with
[resvg](https://github.com/linebender/resvg). The codes are those of twenty animals' faces, from `1F436` (a dog's face) to
`1F984` (a unicorn's).

OpenMoji's pictures are licensed [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/). The game credits them
beneath the board, and this repository holds none of them.

## Files

| Stage | Files | Lines | Written by |
|---|---|---|---|
| Y source | `y/src/memory.y`, `y/src/memory.test.y`, `y/src/main.y` | 523, 857, 32 | hand |
| Bare bodies | `bare/src/memory.brs`, `bare/src/memory/tests.brs`, `bare/src/main.brs`, `bare/src/lib.brs` | 493, 671, 56, 3 | `bare-rust strip` of the Rust below, which was written as the Y compiler would lower the source |
| Constraint files | `bare/src/*.constraints.json` (134 subjects, 50 clauses) | 1055 | `strip`, plus the clauses, regions and status the Y lowering adds, then `bare-rust libfmt` |
| Y facts | `bare/y-facts.json` | 1198 | the same, from the Y source and the bare bodies |
| Rust | `src/memory.rs`, `src/memory/tests.rs`, `src/main.rs`, `src/lib.rs` | 493, 671, 56, 3 | `bare-rust weave`, which gives back the hand-written Rust exactly |

The network is a capability. It comes from [`../y-http`](../y-http/), a stand-in for a Y standard module
`standard.http` that gets text over HTTPS with [ureq](https://github.com/algesten/ureq). It is a crate of its own, so
that the other examples do not build an HTTP client. The window, which now also reads clicks and draws pictures, comes
from [`../y-window`](../y-window/), and the clock, the randomness and deadlines come from [`../y-runtime`](../y-runtime/).

## What Memory adds to the earlier lowerings

| Y | Bare body | Constraint file or `y-facts.json` | Rust |
|---|---|---|---|
| `service openmoji:` with `address = url"https://openmoji.org"` | `mod openmoji { const ADDRESS = "https://openmoji.org"; }` | — | the same |
| `injected site`, as `http<service=openmoji>`, and `site = system.http(openmoji)` in `main` | `site: &Http`, a shared borrow, because `get` does not change it; `system.http(openmoji::ADDRESS)` | `effects only random, clock, site` | `y_http::Http`, or a fake for tests |
| `region fetch is within game_rules.fetch_time or fail:` around the fetches (Y § 3.8: a region with an execution constraint) | `Deadline::after(game_rules::FETCH_TIME)`, passed to each `site.get` | `y-facts.json` `deadlines`, a new kind; the region `new_game#fetch`, `within game_rules.fetch_time`, in the constraint file (language 6.23) | a deadline from the runtime's monotonic time |
| `site.get(path"…") or fail`, which fails with `http_error` or, at the deadline, `expired` | `site.get(&path, &deadline).await?` | `y-facts.json` `suspensions` | `Within<HttpError>`, converted through `setup_error`'s `unreachable` and `timed_out` cases |
| `picture.from_svg(drawing, size: …) or fail` | `Picture::from_svg(&drawing, …)?` | — | drawn by resvg into pixels; the window makes a texture of them when it first draws them |
| `window.clicks()`, `window.picture(…)`, `point(x:, y:)` | the same, on `Window` and `Point` | — | Macroquad's mouse and textures, or a fake window's frames |
| `new_game` and `play`, which may suspend because `site.get()` and `window.next_frame()` may | `async fn`s, and `.await` at seven points, and one more in `main` | `y-facts.json` `suspensions` | the same |
| `random.chosen(from: faces.codes, count: board.pairs)`, `random.shuffled(dealt)` | `List::from(random.chosen(&faces::CODES.map(YString::new), board::PAIRS))`, `List::from(random.shuffled(&dealt))` | — | in `y_runtime::Random`, which takes a slice and gives a `Vec`; the result is held as a `List` |
| `game.elapsed.whole_seconds()` | `time::whole_seconds(game.elapsed)` | — | a call into `y_runtime::time`, not an `as` cast |
| `new_game(…) returns started fails setup_error ensures playing = (standing(started) == outcome.playing)`, and `ensures nothing_turned = started.turned.is_empty()` | `ensure(standing(&started) == Outcome::Playing, "memory::new_game#playing")` and the same for `nothing_turned`, before the return | `ensure playing : result is Ok(started) implies standing(started) is Playing`, and the same for `nothing_turned`; status `lifted` | run-time checks whose failure is a fault; the path enumeration knows from them that a new game has no card turned and is being played |
| `string`, and `list<…>` in `game`'s fields, `banner`'s lines, `pick`'s clicks and the locals | `YString` and `List<…>`, from [`../y-string`](../y-string/) and [`../y-collections`](../y-collections/); a list parameter is `&List<…>`. The text `site.get` gives is wrapped in `YString::from`, and `window.keys_pressed()` and `window.clicks()` in `List::from`; a `YString` goes to the window by `as_str()` | `list<…> as 'List'`, `string as 'YString'` | the facades; `faces.codes` stays an array of `&str`, made into `YString`s where it is chosen from |
| `game.pictures.at_number(n)`, for values that are not copied | `list::at_number_ref(&game.pictures, n)` | — | borrows the picture |

## What had to be assumed

Beyond what Hangman, Nim and Snake assumed:

- **Services.** The proposal names `http<service=…>` (§ 3.7) but not how a service is declared. Here a `service`
  declaration gives its address, and `system.http(service)` gives the capability for it.
- **A standard http module.** `standard.http` gives `http` and `http_error`, and `site.get(path)` gets the text at a
  path of the service. Images are fetched as SVG text, so the module needs no bytes.
- **Pictures in `standard.window`.** The module gives `picture`, `picture.from_svg(drawing, size:)`, which fails with a
  `picture_error`, and `window.picture(picture, x:, y:)`. It also gives `point` and `window.clicks()`.
- **`expired` in `standard.time`,** with the case `timed_out(deadline is expired)` carrying it. This is as the proposal's
  own example does (§ 3.8). The deadline is read from the runtime's monotonic time, not from the `clock` capability.
- **An HTTP call blocks the frame it is in.** Macroquad's executor drives no I/O, so `site.get()` completes when it is
  called, and the window shows "Fetching pictures from OpenMoji..." for that time. A Y runtime would suspend there.
- **Library names:** `random.chosen(from:, count:)`, `random.shuffled(…)`, `duration.whole_seconds()`,
  `list.extend(…)`, `list.sorted()`, `result.failure()` and `http_error.fake_status(…)`.
- **Narrowing in a `conditional`:** after `condition judged.off_cards = (number == optional.none):`, `number` is a
  `uint32` in the conditions that follow.
- **Tests may `inject` before `given`,** when the given values need a capability, as `dealt(…, at: clock.now())` does.
- **Fakes:** `fake http(text: …)`, `fake http(status: …)`, `fake http(expiring: true)`, `fake http(text: …, answers: n,
  then_status: …)`, which answers `n` times and then fails, and `fake http(texts: […])`, which gives each text in turn; and
  `fake window(frames: …)`, in which `frame(keys: …, clicks: …)` gives what the player does in each frame.

## Checks

- **`bare-rust weave`** of `bare/`: 4 files woven, 0 weave errors, 132 `mut` derived, 0 holes undetermined, 0 warnings.
  The woven Rust is identical to the hand-written Rust it was stripped from.
- **`cargo test`:** the 55 lowered Y tests pass. They cover the rules, the scoring, the banner, setting up a board
  through a fake site, and each way the setup fails. Two tests run `play` with a fake window: one wins a whole game,
  clicking each pair in turn, and starts another; in the other, a pair that does not match shows for a second and turns
  back.
- **`cargo clippy --all-targets`:** no warnings, here and in `../y-http`, `../y-window` and `../y-runtime`.
- **Coverage:** the tests cover all 52 execution paths of the module (Y § 3.14), through the 19 decisions and the
  `or fail` calls of its 17 functions. None ends in `fail not_reachable`. Each test was also run alone with every
  decision and loop instrumented, to check that it takes each path it declares.
- **One path per test:** 52 of the 55 tests each cover one path, and their `when` is one call of that path's
  function (Y § 3.14). The other 3 cover none: they check what the paths do not tell apart, such as which failure
  comes back.
- **`tools/bare-rust/tests/y_examples.rs`** runs the same checks on Memory as on the other examples, and a new one: each
  `within` in `deadlines` starts its deadline in the bare body, and passes it to each wait inside it.
- **The window itself:** under a virtual X display, the game fetched its pictures, drew the board, turned over the cards
  clicked, and wrote the best score at the terminal when Escape was pressed. openmoji.org is outside the network policy
  of the machine this was made on, so that run fetched the same SVG files from OpenMoji's repository on GitHub, by
  changing the address and the path in a copy of the code.

## What the example showed

- **`strip` and `weave` needed nothing new.** A deadline, a failure converted from a generic `Within<…>`, and a crate
  that reaches the network all lower to Rust that the tools already handle.
- **A suspension that fails is still one point.** `site.get()` both suspends and fails, and `.await?` keeps both at
  the place the Y source has them.
- **A blank `else` is avoidable here as well.** A click that turns no card is the `else ignored:` of a loop over the
  clicks, which goes on to the next. A pair that turns back is a `condition` of the frame, and leaves the other branches
  free.
- **Paths are about the code's own facts.** The first frame of `play` could only be told apart from the rest once
  `new_game` ensured what a new game is; without it, a first frame that turns a pair back is a path no test can take.
  The clicks of a frame went to `pick`, whose loop and `else no_clicks` are tested on their own, so that `play` has a
  path for each way a frame goes rather than for each number of clicks in it.
