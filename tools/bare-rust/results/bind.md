# bind: every written type has a name

A cast (`e as u64`) or a suffixed literal (`1u8`) in the middle of an expression states a type with nothing named for
it to belong to. In the bare form that type went into a positional hint. Positional hints are the one binding that E3
found fragile, and they have no equivalent in a host such as TypeScript, where `x as T` is a pure type assertion.

The bare form now gives each of these values a name. `strip` first runs `bind`, a rewrite of the Rust source:

```rust
(h as u64).wrapping_mul(P)          let h_u64: u64 = h as u64;
                                    h_u64.wrapping_mul(P)
```

The local then carries the type: `"h_u64": {"type": "u64", "cast": true}`, with `let h_u64 = h;` in the body. The
round trip is exact on the bound source: `weave(strip(s)) == bind(s)` after `rustfmt`, as proposal § 1 and § 9 now
say. A mid-expression type is treated as an obfuscation that `bind` undoes once.

## What bind does

- **Where the binding goes.**
  - A value is bound just before the statement it is in.
  - In some places the statement would run the value when the source might not, or would run it once where the
    source runs it many times: the right of `&&` or `||`, a match arm or guard, an `else if`, a `while` condition, a
    closure body. There the binding goes into a new block in that place: `c && { let h_u8: u8 = h as u8; h_u8 > 3 }`.
- **Order.** A bound value now runs before the rest of its statement. Anything earlier in the statement that could have
  an effect (a call, an index, a macro) is bound first, in order: `g(1) + g(h) as u32` becomes `let g_v = g(1);` and
  then `let g_u32: u32 = g(h) as u32;`. Where that earlier thing is not a plain value, the site is left as it is and
  reported. That covers a method receiver, a borrowed place, and the left of an assignment.
- **Already named.** `let n = e as f64;` gets a type, `let n: f64 = e as f64;`, rather than a second name. Brackets
  that weave would not put back are dropped: `(x) as u16` becomes `x as u16`.
- **Macros.** A macro call whose tokens state a type cannot be split, and is kept as Rust text in the bare body:
  `rust! { println!("{}", x as u8) }`, which weaves back to the call. It is never the preferred form. Where an agent can
  name the value before the call, it should.
- **Left as hints, and reported by `bare-rust bind`:**
  - a cast to a type that is not a primitive (`p as *const u8`);
  - a cast in a constant context (`[0; N as usize]`, a `const` item);
  - a suffixed literal in a pattern (`1u8 => ...`);
  - a cast after a receiver, borrowed place or assignee that has an effect;
  - a cast in a `while let` condition or a let chain.

`tests/bind.rs` compiles a function with every kind of position, before and after `bind`, runs both on the same
inputs, and checks that they print the same. It also checks the placements above and that a second `bind` changes
nothing.

## On the corpora

| corpus | files | typed names added | of them, casts | literals | earlier values bound for order | sites left |
|---|---:|---:|---:|---:|---:|---:|
| strsim | 2 | 27 | 27 | 0 | 1 | 0 |
| fnv | 1 | 1 | 1 | 0 | 0 | 0 |
| the tool's own source | 31 | 42 | 35 | 7 | 11 | 0 |
| examples | 6 | 1 | 1 | 0 | 0 | 0 |

**A stronger check than the tests above:** the tool's own source, with every file bound, still compiles with no
warnings. Built from that bound source, the tool passes its unit tests and five of its test files (85 tests).

After `bind`, no `cast` or `literal` hint is left in any corpus. The hints that remain are of three kinds:

| corpus | `targ` (turbofish) | `closure_param` | `closure_return` |
|---|---:|---:|---:|
| strsim | 1 | 0 | 0 |
| examples | 2 | 0 | 0 |
| the tool's own source | 9 | 151 | 19 |

A closure parameter is already a binding, so its hint is attached to a name. Turbofishes and closure return types
are the open questions below.

**Bugs the tool's own source found.** The first full run found two bugs in `bind`, both in struct literals, and
both now have a regression test:

- **A shorthand field** (`Diag { message, .. }`) was taken for an expression with an effect, and bound. The rewrite
  renamed the field, which does not compile.
- **The earlier values in one expression were bound in reverse order.** `level`, `code` and `file` became `file`,
  `code`, `level`. That is the order change `bind` exists to prevent.

The first fails to compile, and `cargo check` on the woven tool caught it. The second compiles: the values here
have no effects, so the program's behaviour did not change. It was found by reading the bound text while chasing the
first bug. A check that compares evaluation order would have to run the program, as `tests/bind.rs` does for its
fixture.

## Renaming: one agent pass on strsim

The names `bind` makes are mechanical: `a_len_f64`, `value_f64`, `i_isize_3`. One Sonnet agent was given the stripped
strsim, the list of the names `bind` added, and the rule: rename each to say what the value is, in the `.brs` and
the `holes` key together, change nothing else, and keep `./weave.sh test` passing.

- **28 renames**, two of them a repeated name in a second function. The full list is in
  [`bind-strsim-renames.json`](bind-strsim-renames.json).
- Some examples:
  - `a_len_f64` became `a_total`;
  - `matches_f64` became `a_matches`;
  - `value_f64` became `twice_common` and `total_bigrams` in `sorensen_dice`;
  - `i_isize_3` became `row_cur`;
  - `r_item_usize` became `result`.
- **Checked afterwards, independently of the agent:**
  - the bare body's token stream is identical apart from the renamed names (5,423 tokens each);
  - `subjects`, every local's type and every hint are unchanged;
  - the woven crate passes its 19 tests.
- **Cost:** 2 weaves, 8 tool calls, about 62K tokens, a few cents.
